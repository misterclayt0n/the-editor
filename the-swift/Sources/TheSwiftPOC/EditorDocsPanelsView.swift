import AppKit
import SwiftUI

private let docsStyleBold: UInt16 = 1 << 0
private let docsStyleItalic: UInt16 = 1 << 2
private let docsPanelHorizontalInset: CGFloat = 20
private let docsPanelVerticalInset: CGFloat = 16
private let docsPanelEdgePadding: CGFloat = 8
private let signaturePanelCursorGap: CGFloat = 6

private struct EditorDocsContentKey: Hashable {
    let kind: EditorDocsPanelKind
    let runs: [EditorDocsRun]
}

private struct EditorDocsBoundsKey: Hashable {
    let content: EditorDocsContentKey
    let width: Int
}

@MainActor
private final class EditorDocsRenderCache {
    static let shared = EditorDocsRenderCache()

    private var attributedTexts: [EditorDocsContentKey: NSAttributedString] = [:]
    private var bounds: [EditorDocsBoundsKey: CGRect] = [:]

    func attributedText(for key: EditorDocsContentKey, build: () -> NSAttributedString) -> NSAttributedString {
        if let cached = attributedTexts[key] {
            return cached
        }
        let value = measureCompletionPerf("docs.attributedText.build runs=\(key.runs.count) kind=\(String(describing: key.kind))") {
            build()
        }
        attributedTexts[key] = value
        return value
    }

    func bounds(for key: EditorDocsContentKey, width: CGFloat, build: () -> CGRect) -> CGRect {
        guard width.isFinite, width < CGFloat(Int.max) else {
            return measureCompletionPerf("docs.bounds.probe runs=\(key.runs.count) kind=\(String(describing: key.kind))") {
                build()
            }
        }
        let widthKey = EditorDocsBoundsKey(content: key, width: Int(width.rounded(.toNearestOrEven)))
        if let cached = bounds[widthKey] {
            return cached
        }
        let value = measureCompletionPerf("docs.bounds.build width=\(widthKey.width) runs=\(key.runs.count) kind=\(String(describing: key.kind))") {
            build()
        }
        bounds[widthKey] = value
        return value
    }
}

enum EditorDocsPanelKind {
    case hover
    case completionDocs
    case signatureHelp

    var accessibilityLabel: String {
        switch self {
        case .hover:
            return "Hover"
        case .completionDocs:
            return "Completion Documentation"
        case .signatureHelp:
            return "Signature Help"
        }
    }

    var minimumSize: CGSize {
        switch self {
        case .hover, .completionDocs:
            return CGSize(width: 240, height: 84)
        case .signatureHelp:
            return CGSize(width: 220, height: 54)
        }
    }

    var maximumSize: CGSize {
        switch self {
        case .hover, .completionDocs:
            return CGSize(width: 460, height: 280)
        case .signatureHelp:
            return CGSize(width: 400, height: 180)
        }
    }
}

struct EditorDocsPanelsView: View {
    @ObservedObject var controller: EditorSurfaceController

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .topLeading) {
                if let scene = controller.scene, !controller.completionMenu.isOpen {
                    if controller.signatureHelp.isOpen {
                        EditorDocsPanelOverlay(
                            kind: .signatureHelp,
                            panel: controller.signatureHelp,
                            scene: scene,
                            backgroundColor: controller.chrome.backgroundColor,
                            anchorFrame: nil,
                            onEscape: controller.closeDocsPanels
                        )
                        .zIndex(1)
                    }

                    if controller.hoverDocs.isOpen {
                        EditorDocsPanelOverlay(
                            kind: .hover,
                            panel: controller.hoverDocs,
                            scene: scene,
                            backgroundColor: controller.chrome.backgroundColor,
                            anchorFrame: nil,
                            onEscape: controller.closeDocsPanels
                        )
                        .zIndex(2)
                    }
                }
            }
            .frame(width: geometry.size.width, height: geometry.size.height, alignment: .topLeading)
        }
        .allowsHitTesting(true)
    }
}

struct EditorDocsPanelOverlay: View {
    let kind: EditorDocsPanelKind
    let panel: EditorDocsPanelState
    let scene: EditorRenderScene
    let backgroundColor: NSColor
    let anchorFrame: CGRect?
    let onEscape: () -> Void

    var body: some View {
        if panel.isOpen, panel.width > 0, panel.height > 0 {
            EditorPopoverPanel(frame: fittedFrame, backgroundColor: backgroundColor) {
                EditorSelectableDocsTextView(
                    attributedText: attributedText,
                    contentSignature: contentSignature,
                    backgroundColor: backgroundColor,
                    onEscape: onEscape
                )
            }
            .transition(.opacity)
            .accessibilityLabel(kind.accessibilityLabel)
        }
    }

    private var contentKey: EditorDocsContentKey {
        EditorDocsContentKey(kind: kind, runs: panel.runs)
    }

    private var contentSignature: Int {
        var hasher = Hasher()
        hasher.combine(contentKey)
        return hasher.finalize()
    }

    private var fittedFrame: CGRect {
        let metrics = scene.info.surfaceMetrics
        let viewportSize = CGSize(
            width: CGFloat(scene.info.viewportWidth) * metrics.cellSizePoints.width,
            height: CGFloat(scene.info.viewportHeight) * metrics.cellSizePoints.height
        )
        let baseOrigin = CGPoint(
            x: CGFloat(panel.col) * metrics.cellSizePoints.width,
            y: CGFloat(panel.row) * metrics.cellSizePoints.height
        )
        let exportedSize = CGSize(
            width: CGFloat(panel.width) * metrics.cellSizePoints.width,
            height: CGFloat(panel.height) * metrics.cellSizePoints.height
        )

        let maxWidth = max(
            min(exportedSize.width, kind.maximumSize.width, max(viewportSize.width - docsPanelEdgePadding * 2, 0)),
            min(kind.minimumSize.width, max(viewportSize.width - docsPanelEdgePadding * 2, 0))
        )
        let maxHeight = max(
            min(exportedSize.height, kind.maximumSize.height, max(viewportSize.height - docsPanelEdgePadding * 2, 0)),
            min(kind.minimumSize.height, max(viewportSize.height - docsPanelEdgePadding * 2, 0))
        )

        let unconstrainedWidth = ceil(textBounds(forWidth: CGFloat.greatestFiniteMagnitude).width) + docsPanelHorizontalInset
        let width = min(maxWidth, max(kind.minimumSize.width, unconstrainedWidth))
        let contentWidth = max(width - docsPanelHorizontalInset, 1)
        let contentHeight = ceil(textBounds(forWidth: contentWidth).height) + docsPanelVerticalInset
        let height = min(maxHeight, max(kind.minimumSize.height, contentHeight))

        if kind == .completionDocs, let anchorFrame {
            let gap: CGFloat = 8
            let rightAvailable = viewportSize.width - docsPanelEdgePadding - (anchorFrame.maxX + gap)
            let leftAvailable = anchorFrame.minX - gap - docsPanelEdgePadding
            let placeRight = rightAvailable >= leftAvailable
            let availableWidth = max(placeRight ? rightAvailable : leftAvailable, 0)
            if availableWidth > 0 {
                let anchoredWidth = max(min(width, availableWidth), min(kind.minimumSize.width, availableWidth))
                let x = placeRight
                    ? min(anchorFrame.maxX + gap, viewportSize.width - anchoredWidth - docsPanelEdgePadding)
                    : max(anchorFrame.minX - gap - anchoredWidth, docsPanelEdgePadding)
                let y = clamp(anchorFrame.minY, lower: docsPanelEdgePadding, upper: max(viewportSize.height - height - docsPanelEdgePadding, docsPanelEdgePadding))
                return CGRect(x: x, y: y, width: anchoredWidth, height: height)
            }
        }

        let clampedX = clamp(baseOrigin.x, lower: docsPanelEdgePadding, upper: max(viewportSize.width - width - docsPanelEdgePadding, docsPanelEdgePadding))
        let anchoredY = anchoredOriginY(baseOriginY: baseOrigin.y, exportedHeight: exportedSize.height, fittedHeight: height)
        let lowerBoundY: CGFloat = kind == .signatureHelp ? 0 : docsPanelEdgePadding
        let clampedY = clamp(anchoredY, lower: lowerBoundY, upper: max(viewportSize.height - height - docsPanelEdgePadding, lowerBoundY))
        return CGRect(x: clampedX, y: clampedY, width: width, height: height)
    }

    private var attributedText: NSAttributedString {
        EditorDocsRenderCache.shared.attributedText(for: contentKey) {
            let storage = NSMutableAttributedString()
            for run in panel.runs {
                storage.append(NSAttributedString(string: run.text, attributes: attributes(for: run)))
            }
            return storage
        }
    }

    private func textBounds(forWidth width: CGFloat) -> CGRect {
        EditorDocsRenderCache.shared.bounds(for: contentKey, width: width) {
            attributedText.boundingRect(
                with: CGSize(width: width, height: CGFloat.greatestFiniteMagnitude),
                options: [.usesLineFragmentOrigin, .usesFontLeading]
            )
        }
    }

    private func attributes(for run: EditorDocsRun) -> [NSAttributedString.Key: Any] {
        let font = font(for: run)
        var foregroundColor = run.style.foregroundColor
        var underlineStyle = run.style.underlineStyle != 0 ? NSUnderlineStyle.single.rawValue : 0
        var underlineColor = run.style.underlineColor?.color ?? run.style.foregroundColor
        var attributes: [NSAttributedString.Key: Any] = [
            .font: font,
            .foregroundColor: foregroundColor
        ]

        if run.kind != .activeParameter, let backgroundColor = run.style.backgroundColor {
            attributes[.backgroundColor] = backgroundColor
        }

        if run.kind == .activeParameter {
            foregroundColor = .controlAccentColor
            underlineStyle = NSUnderlineStyle.single.rawValue
            underlineColor = NSColor.controlAccentColor.withAlphaComponent(0.8)
            attributes[.foregroundColor] = foregroundColor
        }

        if underlineStyle != 0 {
            attributes[.underlineStyle] = underlineStyle
            attributes[.underlineColor] = underlineColor
        }

        if case .link = run.kind {
            attributes[.cursor] = NSCursor.pointingHand
        }

        let paragraphStyle = NSMutableParagraphStyle()
        paragraphStyle.lineBreakMode = .byWordWrapping
        paragraphStyle.lineSpacing = 1
        attributes[.paragraphStyle] = paragraphStyle
        return attributes
    }

    private func font(for run: EditorDocsRun) -> NSFont {
        let isBold = run.style.addModifiers & docsStyleBold != 0 || run.kind == .activeParameter
        let isItalic = run.style.addModifiers & docsStyleItalic != 0

        switch run.kind {
        case .heading1:
            return NSFont.systemFont(ofSize: 15, weight: .semibold)
        case .heading2:
            return NSFont.systemFont(ofSize: 14, weight: .semibold)
        case .heading3:
            return NSFont.systemFont(ofSize: 13, weight: .semibold)
        case .heading4, .heading5, .heading6:
            return NSFont.systemFont(ofSize: 12, weight: .semibold)
        case .inlineCode, .code, .activeParameter:
            return monospacedFont(size: 12, weight: isBold ? .semibold : .regular, italic: isItalic)
        default:
            return systemFont(size: 12, weight: isBold ? .semibold : .regular, italic: isItalic)
        }
    }

    private func systemFont(size: CGFloat, weight: NSFont.Weight, italic: Bool) -> NSFont {
        let base = NSFont.systemFont(ofSize: size, weight: weight)
        guard italic else { return base }
        return NSFontManager.shared.convert(base, toHaveTrait: .italicFontMask)
    }

    private func monospacedFont(size: CGFloat, weight: NSFont.Weight, italic: Bool) -> NSFont {
        let base = NSFont.monospacedSystemFont(ofSize: size, weight: weight)
        guard italic else { return base }
        return NSFontManager.shared.convert(base, toHaveTrait: .italicFontMask)
    }

    private func anchoredOriginY(baseOriginY: CGFloat, exportedHeight: CGFloat, fittedHeight: CGFloat) -> CGFloat {
        guard kind == .signatureHelp,
              let cursor = scene.primaryCursor
        else {
            return baseOriginY
        }

        let cellHeight = scene.info.surfaceMetrics.cellSizePoints.height
        let cursorTopY = CGFloat(cursor.row) * cellHeight
        let cursorBottomY = cursorTopY + cellHeight
        let exportedBottom = baseOriginY + exportedHeight
        let isAboveCursor = exportedBottom <= cursorBottomY
        if isAboveCursor {
            return max(0, baseOriginY + exportedHeight - fittedHeight - signaturePanelCursorGap)
        }
        return baseOriginY + signaturePanelCursorGap
    }

    private func clamp(_ value: CGFloat, lower: CGFloat, upper: CGFloat) -> CGFloat {
        min(max(value, lower), upper)
    }
}

private struct EditorSelectableDocsTextView: NSViewRepresentable {
    let attributedText: NSAttributedString
    let contentSignature: Int
    let backgroundColor: NSColor
    let onEscape: () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(onEscape: onEscape)
    }

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView(frame: .zero)
        scrollView.drawsBackground = false
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.scrollerStyle = .overlay
        scrollView.borderType = .noBorder
        scrollView.automaticallyAdjustsContentInsets = false
        scrollView.contentInsets = NSEdgeInsets(top: 0, left: 0, bottom: 0, right: 0)

        let textView = EditorDocsTextView(frame: .zero)
        textView.onEscape = context.coordinator.onEscape
        textView.isEditable = false
        textView.isSelectable = true
        textView.isRichText = true
        textView.importsGraphics = false
        textView.drawsBackground = false
        textView.textContainerInset = NSSize(width: 10, height: 8)
        textView.textContainer?.widthTracksTextView = true
        textView.textContainer?.heightTracksTextView = false
        textView.textContainer?.lineFragmentPadding = 0
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.minSize = .zero
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = false
        textView.allowsUndo = false
        textView.linkTextAttributes = [
            .foregroundColor: NSColor.linkColor,
            .underlineStyle: NSUnderlineStyle.single.rawValue
        ]
        textView.textStorage?.setAttributedString(attributedText)

        scrollView.documentView = textView
        context.coordinator.textView = textView
        context.coordinator.lastContentSignature = contentSignature
        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        guard let textView = context.coordinator.textView else { return }
        textView.onEscape = context.coordinator.onEscape
        if context.coordinator.lastContentSignature != contentSignature {
            measureCompletionPerf("docs.textView.setAttributedText signature=\(contentSignature)") {
                textView.textStorage?.setAttributedString(attributedText)
            }
            context.coordinator.lastContentSignature = contentSignature
        }
    }

    final class Coordinator {
        let onEscape: () -> Void
        weak var textView: EditorDocsTextView?
        var lastContentSignature: Int?

        init(onEscape: @escaping () -> Void) {
            self.onEscape = onEscape
        }
    }
}

private final class EditorDocsTextView: NSTextView {
    var onEscape: (() -> Void)?

    override func cancelOperation(_ sender: Any?) {
        onEscape?()
    }

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 53 || event.charactersIgnoringModifiers == "\u{1b}" {
            onEscape?()
            return
        }
        super.keyDown(with: event)
    }

    override func doCommand(by selector: Selector) {
        if selector == #selector(NSResponder.cancelOperation(_:)) {
            onEscape?()
            return
        }
        super.doCommand(by: selector)
    }
}
