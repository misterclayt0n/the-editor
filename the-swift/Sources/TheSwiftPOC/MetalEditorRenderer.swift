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
    private var lastThemeGeneration: UInt64?
    private var activeCursorBlinkOpacity: CGFloat = 1

    private struct DrawPerfStats {
        var cacheHits = 0
        var cacheMisses = 0
        var rasterizedLines = 0
        var rasterizedCells = 0
        var rasterMs: Double = 0
    }

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
        view.autoResizeDrawable = false
        view.colorPixelFormat = .bgra8Unorm
        view.clearColor = MTLClearColorMake(0.12, 0.12, 0.12, 1)
        view.layerContentsRedrawPolicy = .duringViewResize
        self.view = view
        super.init()
        view.delegate = self
    }

    func update(scene: EditorRenderScene) {
        let started = CFAbsoluteTimeGetCurrent()
        let previousThemeGeneration = lastThemeGeneration
        let cacheCountBefore = lineCache.count
        self.scene = scene
        pruneCache(for: scene)
        let cacheCountAfter = lineCache.count
        let themeChanged = previousThemeGeneration != nil && previousThemeGeneration != scene.info.themeGeneration
        if previousThemeGeneration != scene.info.themeGeneration {
            themePerfLog(
                "renderer.update themeGen=\(scene.info.themeGeneration) previousThemeGen=\(previousThemeGeneration.map(String.init) ?? "nil") visibleLines=\(scene.lines.count) cacheBefore=\(cacheCountBefore) cacheAfter=\(cacheCountAfter)"
            )
        }
        lastThemeGeneration = scene.info.themeGeneration
        if themeChanged {
            view.draw()
        } else {
            view.setNeedsDisplay(view.bounds)
        }
        let totalMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
        scrollPerfLog(
            "renderer.update damage=\(scene.info.damageReason) full=\(scene.info.damageIsFull) visibleLines=\(scene.lines.count) cacheBefore=\(cacheCountBefore) cacheAfter=\(cacheCountAfter) themeChanged=\(themeChanged) totalMs=\(String(format: "%.2f", totalMs))"
        )
    }

    func drawImmediately() {
        view.draw()
    }

    func setActiveCursorBlinkOpacity(_ opacity: CGFloat) {
        activeCursorBlinkOpacity = min(max(opacity, 0), 1)
    }

    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {}

    func draw(in view: MTKView) {
        let drawStarted = CFAbsoluteTimeGetCurrent()
        guard let scene,
              let drawable = view.currentDrawable,
              let commandBuffer = queue.makeCommandBuffer() else {
            return
        }

        var perf = DrawPerfStats()

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

        for pane in scene.panes where pane.kind == .editorBuffer {
            let gutterWidth = CGFloat(pane.contentOffsetX) * cellSize.width
            let contentRect = scene.paneContentRect(for: pane)
            guard gutterWidth > 0, contentRect.height > 0 else { continue }
            context.setFillColor(scene.gutterBackgroundColor.cgColor)
            context.fill(contextRect(
                fromTopLeftRect: CGRect(
                    x: contentRect.minX,
                    y: contentRect.minY,
                    width: gutterWidth,
                    height: contentRect.height
                ),
                viewportHeight: view.bounds.height
            ))
        }

        for selection in scene.selections {
            let color = selection.style.backgroundColor ?? NSColor.selectedTextBackgroundColor.withAlphaComponent(0.35)
            let pane = scene.paneContainingCell(col: selection.x, row: selection.y)
            withPaneClip(for: pane, in: scene, context: context, viewportHeight: view.bounds.height) {
                context.setFillColor(color.cgColor)
                context.fill(selectionRect(selection, scene: scene, cellSize: cellSize, viewportHeight: view.bounds.height))
            }
        }

        for overlay in scene.overlays where overlay.kind == .rect {
            let color = overlay.style.backgroundColor ?? overlay.style.foregroundColor.withAlphaComponent(0.3)
            let pane = scene.paneContainingCell(col: overlay.x, row: overlay.y)
            let rect = overlayRect(overlay, scene: scene, cellSize: cellSize, viewportHeight: view.bounds.height)
            withPaneClip(for: pane, in: scene, context: context, viewportHeight: view.bounds.height) {
                context.setFillColor(color.cgColor)
                let radius = min(CGFloat(overlay.radius), min(rect.width, rect.height) * 0.5)
                if radius > 0 {
                    context.addPath(CGPath(roundedRect: rect, cornerWidth: radius, cornerHeight: radius, transform: nil))
                    context.fillPath()
                } else {
                    context.fill(rect)
                }
            }
        }

        for line in scene.lines {
            let key = EditorLineCacheKey(
                paneID: line.paneID,
                x: line.x,
                width: line.width,
                themeGeneration: scene.info.themeGeneration,
                cellWidthPx: scene.info.surfaceMetrics.cellWidthPx,
                cellHeightPx: scene.info.surfaceMetrics.cellHeightPx,
                cellBaselinePx: scene.info.surfaceMetrics.cellBaselinePx,
                signature: line.cacheSignature
            )
            let pane = scene.pane(id: line.paneID)
            if let pane, !scene.isContentRowVisible(line.row, in: pane) {
                continue
            }
            if let image = lineImage(
                for: line,
                key: key,
                viewportWidth: CGFloat(line.width) * cellSize.width,
                gutterColumnCount: pane?.contentOffsetX ?? scene.info.contentOffsetX,
                scale: scale,
                cellSize: cellSize,
                baselineFromBottom: baselineFromBottom,
                perf: &perf
            ) {
                let rect = CGRect(
                    x: CGFloat(line.x) * cellSize.width,
                    y: displayTopY(
                        forRow: line.row,
                        rowSpan: 1,
                        paneID: line.paneID,
                        in: scene,
                        viewportHeight: view.bounds.height,
                        cellHeight: cellSize.height
                    ) - rowRenderPadding,
                    width: CGFloat(image.width) / scale,
                    height: CGFloat(image.height) / scale
                )
                withPaneClip(for: pane, in: scene, context: context, viewportHeight: view.bounds.height) {
                    context.draw(image, in: rect)
                }
            }
        }

        drawDiagnosticUnderlines(scene, in: context, cellSize: cellSize, viewportHeight: view.bounds.height)
        drawInactivePaneOverlays(scene, in: context, cellSize: cellSize, viewportHeight: view.bounds.height)
        drawPaneSeparators(scene, in: context, cellSize: cellSize, viewportHeight: view.bounds.height)
        drawPaneScrollbars(scene, in: context, cellSize: cellSize, viewportHeight: view.bounds.height)

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

        for (index, cursor) in scene.cursors.enumerated() {
            drawCursor(
                cursor,
                at: index,
                in: context,
                scene: scene,
                cellSize: cellSize,
                baselineFromBottom: baselineFromBottom,
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
        let totalMs = (CFAbsoluteTimeGetCurrent() - drawStarted) * 1000
        themePerfLog(
            "renderer.draw themeGen=\(scene.info.themeGeneration) totalMs=\(String(format: "%.2f", totalMs)) cacheHits=\(perf.cacheHits) cacheMisses=\(perf.cacheMisses) rasterizedLines=\(perf.rasterizedLines) rasterizedCells=\(perf.rasterizedCells) rasterMs=\(String(format: "%.2f", perf.rasterMs))"
        )
        scrollPerfLog(
            "renderer.draw damage=\(scene.info.damageReason) full=\(scene.info.damageIsFull) totalMs=\(String(format: "%.2f", totalMs)) cacheHits=\(perf.cacheHits) cacheMisses=\(perf.cacheMisses) rasterizedLines=\(perf.rasterizedLines) rasterizedCells=\(perf.rasterizedCells) rasterMs=\(String(format: "%.2f", perf.rasterMs))"
        )
    }

    private func pruneCache(for scene: EditorRenderScene) {
        let validKeys = scene.visibleLineKeys
        lineCache = lineCache.filter { validKeys.contains($0.key) }
    }

    private func lineImage(
        for line: EditorSceneLine,
        key: EditorLineCacheKey,
        viewportWidth: CGFloat,
        gutterColumnCount: Int,
        scale: CGFloat,
        cellSize: CGSize,
        baselineFromBottom: CGFloat,
        perf: inout DrawPerfStats
    ) -> CGImage? {
        if let cached = lineCache[key] {
            perf.cacheHits += 1
            return cached
        }

        let rasterStarted = CFAbsoluteTimeGetCurrent()
        perf.cacheMisses += 1
        perf.rasterizedLines += 1
        perf.rasterizedCells += line.textCells.count

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
            let localCol = max(textCell.col - line.x, 0)
            if isGutterDiffBarCell(textCell, lineX: line.x, gutterColumnCount: gutterColumnCount) {
                drawGutterDiffBar(
                    style: textCell.style,
                    atCol: localCol,
                    in: cgContext,
                    cellSize: cellSize,
                    rowHeight: cellSize.height + rowRenderPadding * 2
                )
                continue
            }

            drawText(
                textCell.text,
                style: textCell.style,
                atCol: localCol,
                row: 0,
                in: cgContext,
                cellSize: cellSize,
                baselineFromBottom: baselineFromBottom + rowRenderPadding,
                viewportHeight: nil
            )
        }

        NSGraphicsContext.restoreGraphicsState()
        perf.rasterMs += (CFAbsoluteTimeGetCurrent() - rasterStarted) * 1000
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

    private func selectionRect(
        _ selection: EditorSnapshotSelection,
        scene: EditorRenderScene,
        cellSize: CGSize,
        viewportHeight: CGFloat
    ) -> CGRect {
        CGRect(
            x: CGFloat(selection.x) * cellSize.width,
            y: displayTopY(
                forRow: selection.y,
                rowSpan: max(selection.height, 1),
                paneID: scene.paneContainingCell(col: selection.x, row: selection.y)?.paneID,
                in: scene,
                viewportHeight: viewportHeight,
                cellHeight: cellSize.height
            ),
            width: max(CGFloat(selection.width) * cellSize.width, 2),
            height: max(CGFloat(selection.height) * cellSize.height, cellSize.height)
        )
    }

    private func drawDiagnosticUnderlines(
        _ scene: EditorRenderScene,
        in context: CGContext,
        cellSize: CGSize,
        viewportHeight: CGFloat
    ) {
        guard !scene.diagnosticUnderlines.isEmpty else { return }
        let lineWidth = max(scene.info.surfaceMetrics.underlineThicknessPoints, 1)
        context.saveGState()
        context.setLineWidth(lineWidth)
        context.setLineCap(.round)
        context.setLineJoin(.round)
        for underline in scene.diagnosticUnderlines {
            let pane = scene.paneContainingCell(col: underline.startCol, row: underline.row)
            if let pane, !scene.isContentRowVisible(underline.row, in: pane) {
                continue
            }
            let color = diagnosticColor(for: underline.severity)
            let rowBottom = displayTopY(
                forRow: underline.row,
                rowSpan: 1,
                paneID: pane?.paneID,
                in: scene,
                viewportHeight: viewportHeight,
                cellHeight: cellSize.height
            )
            let baselineY = rowBottom + max(lineWidth * 2, 3)
            let startX = CGFloat(underline.startCol) * cellSize.width
            let endX = CGFloat(underline.endCol) * cellSize.width
            withPaneClip(for: pane, in: scene, context: context, viewportHeight: viewportHeight) {
                drawDiagnosticSquiggle(
                    in: context,
                    color: color,
                    fromX: startX,
                    toX: endX,
                    baselineY: baselineY,
                    amplitude: max(min(cellSize.height * 0.08, 3), 1.5),
                    step: max(cellSize.width * 0.22, 3)
                )
            }
        }
        context.restoreGState()
    }

    private func drawInactivePaneOverlays(
        _ scene: EditorRenderScene,
        in context: CGContext,
        cellSize: CGSize,
        viewportHeight: CGFloat
    ) {
        context.saveGState()
        for pane in scene.panes where !pane.isActive {
            let rect = contextRect(fromTopLeftRect: scene.paneRect(for: pane), viewportHeight: viewportHeight)
            context.setFillColor(NSColor.black.withAlphaComponent(0.06).cgColor)
            context.fill(rect)
        }
        context.restoreGState()
    }

    private func drawPaneSeparators(
        _ scene: EditorRenderScene,
        in context: CGContext,
        cellSize: CGSize,
        viewportHeight: CGFloat
    ) {
        guard !scene.separators.isEmpty else { return }
        context.saveGState()
        context.setStrokeColor(NSColor.separatorColor.withAlphaComponent(0.9).cgColor)
        context.setLineWidth(1)
        for separator in scene.separators {
            switch separator.axis {
            case .vertical:
                let x = CGFloat(separator.line) * cellSize.width + 0.5
                let startY = viewportHeight - CGFloat(separator.spanEnd) * cellSize.height
                let endY = viewportHeight - CGFloat(separator.spanStart) * cellSize.height
                context.move(to: CGPoint(x: x, y: startY))
                context.addLine(to: CGPoint(x: x, y: endY))
            case .horizontal:
                let y = viewportHeight - CGFloat(separator.line) * cellSize.height + 0.5
                let startX = CGFloat(separator.spanStart) * cellSize.width
                let endX = CGFloat(separator.spanEnd) * cellSize.width
                context.move(to: CGPoint(x: startX, y: y))
                context.addLine(to: CGPoint(x: endX, y: y))
            }
            context.strokePath()
        }
        context.restoreGState()
    }

    private func drawPaneScrollbars(
        _ scene: EditorRenderScene,
        in context: CGContext,
        cellSize: CGSize,
        viewportHeight: CGFloat
    ) {
        context.saveGState()
        for pane in scene.panes {
            guard let geometry = paneScrollbarGeometry(for: pane, cellSize: cellSize) else { continue }
            let thumbRect = contextRect(fromTopLeftRect: geometry.thumbRect, viewportHeight: viewportHeight)
            let thumbPath = CGPath(
                roundedRect: thumbRect,
                cornerWidth: thumbRect.width * 0.5,
                cornerHeight: thumbRect.width * 0.5,
                transform: nil
            )
            context.addPath(thumbPath)
            context.setFillColor(NSColor.labelColor.withAlphaComponent(pane.isActive ? 0.42 : 0.26).cgColor)
            context.fillPath()
        }
        context.restoreGState()
    }

    private func paneScrollbarGeometry(for pane: EditorSnapshotPane, cellSize: CGSize) -> (trackRect: CGRect, thumbRect: CGRect)? {
        guard let scene, pane.kind == .editorBuffer else { return nil }
        let contentRect = scene.paneContentRect(for: pane)
        let visibleRows = max(Int(floor(contentRect.height / max(cellSize.height, 1))), 1)
        let totalRows = max(pane.documentLineCount, visibleRows)
        let maxScrollRow = max(totalRows - visibleRows, 0)
        guard maxScrollRow > 0, contentRect.height > 0 else { return nil }

        let trackWidth = min(max(floor(cellSize.width * 0.55), 6), 8)
        let inset = max(2, floor(cellSize.width * 0.18))
        let trackRect = CGRect(
            x: contentRect.maxX - inset - trackWidth,
            y: contentRect.minY + inset,
            width: trackWidth,
            height: max(contentRect.height - inset * 2, trackWidth)
        )
        let thumbHeight = max(trackWidth * 2, floor(trackRect.height * (CGFloat(visibleRows) / CGFloat(totalRows))))
        let travel = max(trackRect.height - thumbHeight, 0)
        let progress = CGFloat(min(max(pane.scrollRow, 0), maxScrollRow)) / CGFloat(maxScrollRow)
        let thumbRect = CGRect(
            x: trackRect.minX,
            y: trackRect.minY + progress * travel,
            width: trackRect.width,
            height: thumbHeight
        )
        return (trackRect, thumbRect)
    }

    private func contextRect(fromTopLeftRect rect: CGRect, viewportHeight: CGFloat) -> CGRect {
        CGRect(x: rect.minX, y: viewportHeight - rect.maxY, width: rect.width, height: rect.height)
    }

    private func drawCursor(
        _ cursor: EditorSnapshotCursor,
        at index: Int,
        in context: CGContext,
        scene: EditorRenderScene,
        cellSize: CGSize,
        baselineFromBottom: CGFloat,
        cursorThickness: CGFloat,
        viewportHeight: CGFloat
    ) {
        let pane = paneContaining(cursor: cursor, in: scene)
        if let pane, !scene.isContentRowVisible(cursor.row, in: pane) {
            return
        }
        let x = CGFloat(cursor.col) * cellSize.width
        let y = displayTopY(
            forRow: cursor.row,
            rowSpan: 1,
            paneID: pane?.paneID,
            in: scene,
            viewportHeight: viewportHeight,
            cellHeight: cellSize.height
        )
        let baseRect = CGRect(x: x, y: y, width: max(cellSize.width, cursorThickness), height: cellSize.height)
        let isFocusedCursor = pane?.isActive ?? (cursor.kind == .block)
        let blinkOpacity = shouldBlinkCursor(cursor, at: index, in: scene, isFocusedCursor: isFocusedCursor)
            ? activeCursorBlinkOpacity
            : 1
        if blinkOpacity <= 0.001 {
            return
        }
        let fillColor = (cursor.style.backgroundColor ?? cursor.style.foregroundColor)
            .withAlphaComponent((cursor.style.backgroundColor ?? cursor.style.foregroundColor).alphaComponent * blinkOpacity)
        let strokeColor = fillColor.withAlphaComponent((isFocusedCursor ? 1 : 0.9) * blinkOpacity)

        context.saveGState()
        if let pane {
            context.clip(to: contextRect(fromTopLeftRect: scene.paneRect(for: pane), viewportHeight: viewportHeight))
        }
        context.setFillColor(fillColor.cgColor)
        context.setStrokeColor(strokeColor.cgColor)

        switch cursor.kind {
        case .hidden:
            context.restoreGState()
            return
        case .hollow:
            drawHollowCursor(in: context, rect: baseRect, color: strokeColor)
            context.restoreGState()
            return
        case .bar where !isFocusedCursor:
            drawHollowCursor(in: context, rect: baseRect, color: strokeColor)
            context.restoreGState()
            return
        case .bar:
            let rect = CGRect(x: x, y: y, width: max(cursorThickness, 2), height: cellSize.height)
            let path = CGPath(roundedRect: rect.insetBy(dx: 0.5, dy: 0.5), cornerWidth: 1.5, cornerHeight: 1.5, transform: nil)
            context.addPath(path)
            context.fillPath()
            context.restoreGState()
            return
        case .underline:
            let rect = CGRect(x: x + 1, y: y, width: max(cellSize.width - 2, 2), height: max(cursorThickness, 2))
            let path = CGPath(roundedRect: rect, cornerWidth: rect.height * 0.5, cornerHeight: rect.height * 0.5, transform: nil)
            context.addPath(path)
            context.fillPath()
            context.restoreGState()
            return
        case .block:
            let blockRect = baseRect.insetBy(dx: 0.5, dy: 0.5)
            context.fill(blockRect)

            if let textCell = textCell(under: cursor, in: scene) {
                let cursorTextStyle = EditorResolvedStyle(
                    fg: EditorRGBA(color: readableCursorTextColor(fillColor: fillColor, preferred: cursor.style.foregroundColor).withAlphaComponent(blinkOpacity)),
                    bg: nil,
                    underlineColor: textCell.style.underlineColor,
                    addModifiers: textCell.style.addModifiers,
                    removeModifiers: textCell.style.removeModifiers,
                    underlineStyle: textCell.style.underlineStyle
                )
                context.saveGState()
                context.clip(to: blockRect)
                drawText(
                    textCell.text,
                    style: cursorTextStyle,
                    atCol: textCell.col,
                    row: textCell.row,
                    paneID: pane?.paneID,
                    scene: scene,
                    in: context,
                    cellSize: cellSize,
                    baselineFromBottom: baselineFromBottom,
                    viewportHeight: viewportHeight
                )
                context.restoreGState()
            }
            context.restoreGState()
            return
        }
    }

    private func shouldBlinkCursor(
        _ cursor: EditorSnapshotCursor,
        at index: Int,
        in scene: EditorRenderScene,
        isFocusedCursor: Bool
    ) -> Bool {
        scene.info.cursorBlinkEnabled && index == 0 && isFocusedCursor && cursor.kind != .hidden
    }

    private func drawHollowCursor(in context: CGContext, rect: CGRect, color: NSColor) {
        let insetRect = rect.insetBy(dx: 1, dy: 1)
        context.setLineWidth(1.5)
        context.stroke(insetRect)
    }

    private func paneContaining(cursor: EditorSnapshotCursor, in scene: EditorRenderScene) -> EditorSnapshotPane? {
        scene.paneContainingCell(col: cursor.col, row: cursor.row)
    }

    private func textCell(under cursor: EditorSnapshotCursor, in scene: EditorRenderScene) -> EditorSnapshotTextCell? {
        guard let line = scene.lines.first(where: { line in
            line.row == cursor.row && cursor.col >= line.x && cursor.col < (line.x + line.width)
        }) else {
            return nil
        }
        return line.textCells.first(where: { cell in
            cursor.col >= cell.col && cursor.col < (cell.col + max(cell.cols, 1)) && !cell.text.isEmpty
        })
    }

    private func readableCursorTextColor(fillColor: NSColor, preferred: NSColor) -> NSColor {
        let fill = fillColor.usingColorSpace(.deviceRGB) ?? fillColor
        let candidate = preferred.usingColorSpace(.deviceRGB) ?? preferred
        let fillLuminance = (0.299 * fill.redComponent) + (0.587 * fill.greenComponent) + (0.114 * fill.blueComponent)
        let candidateLuminance = (0.299 * candidate.redComponent) + (0.587 * candidate.greenComponent) + (0.114 * candidate.blueComponent)
        if abs(fillLuminance - candidateLuminance) >= 0.35 {
            return candidate
        }
        return fillLuminance > 0.6 ? .black : .white
    }

    private func drawOverlayText(
        _ overlay: EditorSnapshotOverlay,
        in context: CGContext,
        cellSize: CGSize,
        baselineFromBottom: CGFloat,
        viewportHeight: CGFloat
    ) {
        guard let text = overlay.text else { return }
        let pane = scene?.paneContainingCell(col: overlay.col, row: overlay.row)
        if let scene, let pane, !scene.isContentRowVisible(overlay.row, in: pane) {
            return
        }
        withPaneClip(for: pane, in: scene, context: context, viewportHeight: viewportHeight) {
            drawText(
                text,
                style: overlay.style,
                atCol: overlay.col,
                row: overlay.row,
                paneID: pane?.paneID,
                scene: scene,
                in: context,
                cellSize: cellSize,
                baselineFromBottom: baselineFromBottom,
                viewportHeight: viewportHeight
            )
        }
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
        let pane = scene?.paneContainingCell(col: markedText.col, row: markedText.row)
        if let scene, let pane, !scene.isContentRowVisible(markedText.row, in: pane) {
            return
        }
        withPaneClip(for: pane, in: scene, context: context, viewportHeight: viewportHeight) {
            drawText(
                markedText.text,
                style: style,
                atCol: markedText.col,
                row: markedText.row,
                paneID: pane?.paneID,
                scene: scene,
                in: context,
                cellSize: cellSize,
                baselineFromBottom: baselineFromBottom,
                viewportHeight: viewportHeight
            )
        }
    }

    private func diagnosticColor(for severity: EditorDiagnosticSeverity) -> NSColor {
        switch severity {
        case .error:
            return .systemRed
        case .warning:
            return .systemOrange
        case .information:
            return .systemBlue
        case .hint:
            return .systemTeal
        }
    }

    private func drawDiagnosticSquiggle(
        in context: CGContext,
        color: NSColor,
        fromX: CGFloat,
        toX: CGFloat,
        baselineY: CGFloat,
        amplitude: CGFloat,
        step: CGFloat
    ) {
        guard toX > fromX else { return }
        let path = CGMutablePath()
        path.move(to: CGPoint(x: fromX, y: baselineY))
        var x = fromX
        var direction: CGFloat = 1
        while x < toX {
            let nextX = min(x + step, toX)
            let midX = (x + nextX) * 0.5
            path.addQuadCurve(
                to: CGPoint(x: nextX, y: baselineY),
                control: CGPoint(x: midX, y: baselineY + amplitude * direction)
            )
            x = nextX
            direction *= -1
        }
        context.setStrokeColor(color.withAlphaComponent(0.9).cgColor)
        context.addPath(path)
        context.strokePath()
    }

    private func drawGutterDiffBar(
        style: EditorResolvedStyle,
        atCol col: Int,
        in context: CGContext,
        cellSize: CGSize,
        rowHeight: CGFloat
    ) {
        let barWidth = max(2, floor(cellSize.width * 0.18))
        let insetX = max(1, floor((cellSize.width - barWidth) * 0.5))
        let rect = CGRect(
            x: CGFloat(col) * cellSize.width + insetX,
            y: 0,
            width: barWidth,
            height: rowHeight
        )
        context.setFillColor(style.foregroundColor.cgColor)
        context.fill(rect)
    }

    private func isGutterDiffBarCell(_ textCell: EditorSnapshotTextCell, lineX: Int, gutterColumnCount: Int) -> Bool {
        (textCell.col - lineX) < gutterColumnCount && textCell.text == "▎"
    }

    private func drawText(
        _ text: String,
        style: EditorResolvedStyle,
        atCol col: Int,
        row: Int,
        paneID: UInt? = nil,
        scene: EditorRenderScene? = nil,
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
            if let scene {
                y = displayTopY(
                    forRow: row,
                    rowSpan: 1,
                    paneID: paneID,
                    in: scene,
                    viewportHeight: viewportHeight,
                    cellHeight: cellSize.height
                ) + baselineFromBottom
            } else {
                y = topY(forRow: row, rowSpan: 1, viewportHeight: viewportHeight, cellHeight: cellSize.height) + baselineFromBottom
            }
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

    private func overlayRect(
        _ overlay: EditorSnapshotOverlay,
        scene: EditorRenderScene,
        cellSize: CGSize,
        viewportHeight: CGFloat
    ) -> CGRect {
        let topLeftRect = scene.displayRect(
            x: overlay.x,
            y: overlay.y,
            width: overlay.width,
            height: max(overlay.height, 1),
            paneID: scene.paneContainingCell(col: overlay.x, row: overlay.y)?.paneID
        )
        return contextRect(fromTopLeftRect: topLeftRect, viewportHeight: viewportHeight)
    }

    private func withPaneClip(
        for pane: EditorSnapshotPane?,
        in scene: EditorRenderScene?,
        context: CGContext,
        viewportHeight: CGFloat,
        _ body: () -> Void
    ) {
        guard let scene, let pane else {
            body()
            return
        }
        context.saveGState()
        context.clip(to: contextRect(fromTopLeftRect: scene.paneRect(for: pane), viewportHeight: viewportHeight))
        body()
        context.restoreGState()
    }

    private func displayTopY(
        forRow row: Int,
        rowSpan: Int,
        paneID: UInt?,
        in scene: EditorRenderScene,
        viewportHeight: CGFloat,
        cellHeight: CGFloat
    ) -> CGFloat {
        let base = topY(forRow: row, rowSpan: rowSpan, viewportHeight: viewportHeight, cellHeight: cellHeight)
        guard let paneID, let pane = scene.pane(id: paneID) else {
            return base
        }
        return base - scene.paneHeaderHeight(for: pane)
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

private extension EditorRGBA {
    init(color: NSColor) {
        let resolved = color.usingColorSpace(.deviceRGB) ?? color
        self.init(
            r: UInt8((resolved.redComponent * 255).rounded()),
            g: UInt8((resolved.greenComponent * 255).rounded()),
            b: UInt8((resolved.blueComponent * 255).rounded()),
            a: UInt8((resolved.alphaComponent * 255).rounded())
        )
    }
}
