import AppKit
import CoreImage
import CoreText
import MetalKit

@MainActor
final class MetalEditorRenderer: NSObject, MTKViewDelegate {
    private let device: MTLDevice
    private let queue: MTLCommandQueue
    private let ciContext: CIContext
    private let fontMetrics: EditorFontMetrics
    private let font: NSFont
    private let rowRenderPadding: CGFloat
    private let scaleProvider: () -> CGFloat

    private var scene: EditorRenderScene?
    private var lineCache: [EditorLineCacheKey: CGImage] = [:]

    let view: MTKView

    init?(fontMetrics: EditorFontMetrics, scaleProvider: @escaping () -> CGFloat) {
        guard let device = MTLCreateSystemDefaultDevice(),
              let queue = device.makeCommandQueue() else {
            return nil
        }
        self.device = device
        self.queue = queue
        self.ciContext = CIContext(mtlDevice: device)
        self.fontMetrics = fontMetrics
        self.font = fontMetrics.font
        self.rowRenderPadding = max(ceil((font.boundingRectForFont.height - fontMetrics.cellSize.height) / 2), 1)
        self.scaleProvider = scaleProvider

        let view = MTKView(frame: .zero, device: device)
        view.enableSetNeedsDisplay = true
        view.isPaused = true
        view.framebufferOnly = false
        view.colorPixelFormat = .bgra8Unorm
        view.clearColor = MTLClearColorMake(0.12, 0.12, 0.12, 1)
        self.view = view
        super.init()
        view.delegate = self
    }

    func update(scene: EditorRenderScene) {
        self.scene = scene
        pruneCache(for: scene)
        view.setNeedsDisplay(view.bounds)
    }

    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {}

    func draw(in view: MTKView) {
        guard let scene,
              let drawable = view.currentDrawable,
              let commandBuffer = queue.makeCommandBuffer() else {
            return
        }

        let scale = max(scaleProvider(), 1)
        let cellSize = scene.info.surfaceMetrics.cellSizePoints
        let baselineFromBottom = scene.info.surfaceMetrics.baselineFromBottomPoints
        let cursorThickness = max(scene.info.surfaceMetrics.cursorThicknessPoints, 1)
        let pixelWidth = max(Int(view.drawableSize.width), 1)
        let pixelHeight = max(Int(view.drawableSize.height), 1)
        guard let context = makeBitmapContext(width: pixelWidth, height: pixelHeight) else {
            return
        }

        context.scaleBy(x: scale, y: scale)
        context.setFillColor(scene.backgroundColor.cgColor)
        context.fill(CGRect(x: 0, y: 0, width: view.bounds.width, height: view.bounds.height))

        let gutterWidth = CGFloat(scene.info.contentOffsetX) * cellSize.width
        if gutterWidth > 0 {
            context.setFillColor(scene.gutterBackgroundColor.cgColor)
            context.fill(CGRect(x: 0, y: 0, width: gutterWidth, height: view.bounds.height))
        }

        for selection in scene.selections {
            let color = selection.style.backgroundColor ?? NSColor.selectedTextBackgroundColor.withAlphaComponent(0.35)
            context.setFillColor(color.cgColor)
            context.fill(selectionRect(selection, cellSize: cellSize, viewportHeight: view.bounds.height))
        }

        for overlay in scene.overlays where overlay.kind == .rect {
            let color = overlay.style.backgroundColor ?? overlay.style.foregroundColor.withAlphaComponent(0.3)
            context.setFillColor(color.cgColor)
            context.fill(CGRect(
                x: CGFloat(overlay.x) * cellSize.width,
                y: topY(forRow: overlay.y, rowSpan: max(overlay.height, 1), viewportHeight: view.bounds.height, cellHeight: cellSize.height),
                width: CGFloat(overlay.width) * cellSize.width,
                height: CGFloat(max(overlay.height, 1)) * cellSize.height
            ))
        }

        for line in scene.lines {
            let key = EditorLineCacheKey(
                row: line.row,
                layoutGeneration: scene.info.layoutGeneration,
                textGeneration: scene.info.textGeneration,
                scrollGeneration: scene.info.scrollGeneration,
                themeGeneration: scene.info.themeGeneration,
                cellWidthPx: scene.info.surfaceMetrics.cellWidthPx,
                cellHeightPx: scene.info.surfaceMetrics.cellHeightPx,
                cellBaselinePx: scene.info.surfaceMetrics.cellBaselinePx,
                signature: line.cacheSignature
            )
            if let image = lineImage(
                for: line,
                key: key,
                viewportWidth: view.bounds.width,
                scale: scale,
                cellSize: cellSize,
                baselineFromBottom: baselineFromBottom
            ) {
                let rect = CGRect(
                    x: 0,
                    y: topY(forRow: line.row, rowSpan: 1, viewportHeight: view.bounds.height, cellHeight: cellSize.height) - rowRenderPadding,
                    width: CGFloat(image.width) / scale,
                    height: CGFloat(image.height) / scale
                )
                context.draw(image, in: rect)
            }
        }

        if let markedText = scene.markedText {
            drawMarkedText(
                markedText,
                in: context,
                cellSize: cellSize,
                baselineFromBottom: baselineFromBottom,
                viewportHeight: view.bounds.height
            )
        }

        for overlay in scene.overlays where overlay.kind == .text {
            drawOverlayText(
                overlay,
                in: context,
                cellSize: cellSize,
                baselineFromBottom: baselineFromBottom,
                viewportHeight: view.bounds.height
            )
        }

        for cursor in scene.cursors {
            drawCursor(
                cursor,
                in: context,
                cellSize: cellSize,
                cursorThickness: cursorThickness,
                viewportHeight: view.bounds.height
            )
        }

        guard let frameImage = context.makeImage() else { return }
        let ciImage = CIImage(cgImage: frameImage)
        ciContext.render(
            ciImage,
            to: drawable.texture,
            commandBuffer: commandBuffer,
            bounds: CGRect(x: 0, y: 0, width: pixelWidth, height: pixelHeight),
            colorSpace: CGColorSpaceCreateDeviceRGB()
        )
        commandBuffer.present(drawable)
        commandBuffer.commit()
    }

    private func pruneCache(for scene: EditorRenderScene) {
        let validKeys = scene.visibleLineKeys
        lineCache = lineCache.filter { validKeys.contains($0.key) }
    }

    private func lineImage(
        for line: EditorSceneLine,
        key: EditorLineCacheKey,
        viewportWidth: CGFloat,
        scale: CGFloat,
        cellSize: CGSize,
        baselineFromBottom: CGFloat
    ) -> CGImage? {
        if let cached = lineCache[key] {
            return cached
        }

        let pixelWidth = max(Int(ceil(viewportWidth * scale)), 1)
        let pixelHeight = max(Int(ceil((cellSize.height + rowRenderPadding * 2) * scale)), 1)
        guard let rep = NSBitmapImageRep(
            bitmapDataPlanes: nil,
            pixelsWide: pixelWidth,
            pixelsHigh: pixelHeight,
            bitsPerSample: 8,
            samplesPerPixel: 4,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: .deviceRGB,
            bytesPerRow: 0,
            bitsPerPixel: 0
        ) else {
            return nil
        }

        rep.size = CGSize(width: viewportWidth, height: cellSize.height + rowRenderPadding * 2)
        NSGraphicsContext.saveGraphicsState()
        let graphicsContext = NSGraphicsContext(bitmapImageRep: rep)
        NSGraphicsContext.current = graphicsContext

        guard let cgContext = graphicsContext?.cgContext else {
            NSGraphicsContext.restoreGraphicsState()
            return nil
        }

        cgContext.clear(CGRect(x: 0, y: 0, width: viewportWidth, height: cellSize.height + rowRenderPadding * 2))

        for textCell in line.textCells {
            drawText(
                textCell.text,
                style: textCell.style,
                atCol: textCell.col,
                row: 0,
                in: cgContext,
                cellSize: cellSize,
                baselineFromBottom: baselineFromBottom + rowRenderPadding,
                viewportHeight: nil
            )
        }

        NSGraphicsContext.restoreGraphicsState()
        let image = rep.cgImage
        if let image {
            lineCache[key] = image
        }
        return image
    }

    private func attributes(for style: EditorResolvedStyle) -> [NSAttributedString.Key: Any] {
        var attrs: [NSAttributedString.Key: Any] = [
            .font: font,
            .foregroundColor: style.foregroundColor,
            .ligature: 0,
            .kern: 0,
        ]
        if (style.addModifiers & UInt16(1 << 0)) != 0 {
            attrs[.font] = NSFont.monospacedSystemFont(ofSize: font.pointSize, weight: .bold)
        }
        if style.underlineStyle != 0 {
            attrs[.underlineStyle] = Int(style.underlineStyle)
            if let underlineColor = style.underlineColor?.color {
                attrs[.underlineColor] = underlineColor
            }
        }
        return attrs
    }

    private func selectionRect(_ selection: EditorSnapshotSelection, cellSize: CGSize, viewportHeight: CGFloat) -> CGRect {
        CGRect(
            x: CGFloat(selection.x) * cellSize.width,
            y: topY(forRow: selection.y, rowSpan: max(selection.height, 1), viewportHeight: viewportHeight, cellHeight: cellSize.height),
            width: max(CGFloat(selection.width) * cellSize.width, 2),
            height: max(CGFloat(selection.height) * cellSize.height, cellSize.height)
        )
    }

    private func drawCursor(
        _ cursor: EditorSnapshotCursor,
        in context: CGContext,
        cellSize: CGSize,
        cursorThickness: CGFloat,
        viewportHeight: CGFloat
    ) {
        let x = CGFloat(cursor.col) * cellSize.width
        let y = topY(forRow: cursor.row, rowSpan: 1, viewportHeight: viewportHeight, cellHeight: cellSize.height)
        let color = cursor.style.backgroundColor ?? cursor.style.foregroundColor
        context.setFillColor(color.cgColor)
        context.setStrokeColor(color.cgColor)
        let rect: CGRect
        switch cursor.kind {
        case .bar:
            rect = CGRect(x: x, y: y, width: cursorThickness, height: cellSize.height)
        case .underline:
            rect = CGRect(x: x, y: y + cellSize.height - cursorThickness, width: cellSize.width, height: cursorThickness)
        case .hidden:
            return
        case .hollow:
            rect = CGRect(x: x, y: y, width: max(cellSize.width, cursorThickness), height: cellSize.height)
            context.stroke(rect)
            return
        case .block:
            rect = CGRect(x: x, y: y, width: max(cellSize.width, cursorThickness), height: cellSize.height)
        }
        context.fill(rect)
    }

    private func drawOverlayText(
        _ overlay: EditorSnapshotOverlay,
        in context: CGContext,
        cellSize: CGSize,
        baselineFromBottom: CGFloat,
        viewportHeight: CGFloat
    ) {
        guard let text = overlay.text else { return }
        drawText(
            text,
            style: overlay.style,
            atCol: overlay.col,
            row: overlay.row,
            in: context,
            cellSize: cellSize,
            baselineFromBottom: baselineFromBottom,
            viewportHeight: viewportHeight
        )
    }

    private func drawMarkedText(
        _ markedText: EditorMarkedText,
        in context: CGContext,
        cellSize: CGSize,
        baselineFromBottom: CGFloat,
        viewportHeight: CGFloat
    ) {
        let style = EditorResolvedStyle(
            fg: nil,
            bg: nil,
            underlineColor: nil,
            addModifiers: 0,
            removeModifiers: 0,
            underlineStyle: UInt8(NSUnderlineStyle.single.rawValue)
        )
        drawText(
            markedText.text,
            style: style,
            atCol: markedText.col,
            row: markedText.row,
            in: context,
            cellSize: cellSize,
            baselineFromBottom: baselineFromBottom,
            viewportHeight: viewportHeight
        )
    }

    private func drawText(
        _ text: String,
        style: EditorResolvedStyle,
        atCol col: Int,
        row: Int,
        in context: CGContext,
        cellSize: CGSize,
        baselineFromBottom: CGFloat,
        viewportHeight: CGFloat?
    ) {
        guard !text.isEmpty else { return }
        let attributed = NSAttributedString(string: text, attributes: attributes(for: style))
        let line = CTLineCreateWithAttributedString(attributed)
        context.saveGState()
        context.textMatrix = .identity
        let y: CGFloat
        if let viewportHeight {
            y = topY(forRow: row, rowSpan: 1, viewportHeight: viewportHeight, cellHeight: cellSize.height) + baselineFromBottom
        } else {
            y = baselineFromBottom
        }
        context.textPosition = CGPoint(
            x: CGFloat(col) * cellSize.width,
            y: y
        )
        CTLineDraw(line, context)
        context.restoreGState()
    }

    private func topY(forRow row: Int, rowSpan: Int, viewportHeight: CGFloat, cellHeight: CGFloat) -> CGFloat {
        viewportHeight - CGFloat(row + rowSpan) * cellHeight
    }

    private func makeBitmapContext(width: Int, height: Int) -> CGContext? {
        CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )
    }
}
