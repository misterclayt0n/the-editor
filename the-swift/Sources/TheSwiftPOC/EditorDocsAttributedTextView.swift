import AppKit
import SwiftUI

private let sharedDocsStyleBold: UInt16 = 1 << 0
private let sharedDocsStyleItalic: UInt16 = 1 << 2

private struct EditorDocsAttributedContentKey: Hashable {
    let runs: [EditorDocsRun]
}

private struct EditorDocsAttributedBoundsKey: Hashable {
    let content: EditorDocsAttributedContentKey
    let width: Int
}

@MainActor
private final class EditorDocsAttributedRenderCache {
    static let shared = EditorDocsAttributedRenderCache()

    private var attributedTexts: [EditorDocsAttributedContentKey: NSAttributedString] = [:]
    private var bounds: [EditorDocsAttributedBoundsKey: CGRect] = [:]

    func attributedText(for runs: [EditorDocsRun]) -> NSAttributedString {
        let key = EditorDocsAttributedContentKey(runs: runs)
        if let cached = attributedTexts[key] {
            return cached
        }
        let value = buildAttributedText(for: runs)
        attributedTexts[key] = value
        return value
    }

    func bounds(for runs: [EditorDocsRun], width: CGFloat) -> CGRect {
        let content = EditorDocsAttributedContentKey(runs: runs)
        guard width.isFinite, width < CGFloat(Int.max) else {
            return attributedText(for: runs).boundingRect(
                with: CGSize(width: width, height: .greatestFiniteMagnitude),
                options: [.usesLineFragmentOrigin, .usesFontLeading]
            )
        }
        let key = EditorDocsAttributedBoundsKey(content: content, width: Int(width.rounded(.toNearestOrEven)))
        if let cached = bounds[key] {
            return cached
        }
        let value = attributedText(for: runs).boundingRect(
            with: CGSize(width: width, height: .greatestFiniteMagnitude),
            options: [.usesLineFragmentOrigin, .usesFontLeading]
        )
        bounds[key] = value
        return value
    }

    private func buildAttributedText(for runs: [EditorDocsRun]) -> NSAttributedString {
        let storage = NSMutableAttributedString()
        for run in runs {
            storage.append(NSAttributedString(string: run.text, attributes: attributes(for: run)))
        }
        return storage
    }

    private func attributes(for run: EditorDocsRun) -> [NSAttributedString.Key: Any] {
        let font = font(for: run)
        var foregroundColor = run.kind == .activeParameter ? NSColor.controlAccentColor : run.style.foregroundColor
        var underlineStyle = run.style.underlineStyle != 0 ? NSUnderlineStyle.single.rawValue : 0
        let underlineColor = run.kind == .activeParameter
            ? NSColor.controlAccentColor.withAlphaComponent(0.8)
            : (run.style.underlineColor?.color ?? run.style.foregroundColor)

        var attributes: [NSAttributedString.Key: Any] = [
            .font: font,
            .foregroundColor: foregroundColor,
        ]

        if run.kind != .activeParameter, let backgroundColor = run.style.backgroundColor {
            attributes[.backgroundColor] = backgroundColor
        }

        if run.kind == .activeParameter {
            underlineStyle = NSUnderlineStyle.single.rawValue
        }

        if underlineStyle != 0 {
            attributes[.underlineStyle] = underlineStyle
            attributes[.underlineColor] = underlineColor
        }

        if let destination = run.linkDestination, !destination.isEmpty {
            attributes[.link] = URL(string: destination) ?? destination
            attributes[.cursor] = NSCursor.pointingHand
            foregroundColor = NSColor.linkColor
            attributes[.foregroundColor] = foregroundColor
            attributes[.underlineStyle] = NSUnderlineStyle.single.rawValue
            attributes[.underlineColor] = NSColor.linkColor
        }

        let paragraphStyle = NSMutableParagraphStyle()
        paragraphStyle.lineBreakMode = .byWordWrapping
        paragraphStyle.lineSpacing = 1
        attributes[.paragraphStyle] = paragraphStyle
        return attributes
    }

    private func font(for run: EditorDocsRun) -> NSFont {
        let isBold = run.style.addModifiers & sharedDocsStyleBold != 0 || run.kind == .activeParameter
        let isItalic = run.style.addModifiers & sharedDocsStyleItalic != 0

        let base: NSFont
        switch run.kind {
        case .heading1:
            base = NSFont.systemFont(ofSize: 15, weight: .semibold)
        case .heading2:
            base = NSFont.systemFont(ofSize: 14, weight: .semibold)
        case .heading3:
            base = NSFont.systemFont(ofSize: 13, weight: .semibold)
        case .heading4, .heading5, .heading6:
            base = NSFont.systemFont(ofSize: 12, weight: .semibold)
        case .inlineCode, .code, .activeParameter:
            base = NSFont.monospacedSystemFont(ofSize: 12, weight: isBold ? .semibold : .regular)
        default:
            base = NSFont.systemFont(ofSize: 13, weight: isBold ? .semibold : .regular)
        }

        guard isItalic else { return base }
        return NSFontManager.shared.convert(base, toHaveTrait: .italicFontMask)
    }
}

struct EditorDocsAttributedTextView: NSViewRepresentable {
    let runs: [EditorDocsRun]
    var textContainerInset: NSSize = .zero

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> EditorDocsInlineTextView {
        let textView = EditorDocsInlineTextView(textContainerInset: textContainerInset)
        textView.delegate = context.coordinator
        textView.setAttributedString(EditorDocsAttributedRenderCache.shared.attributedText(for: runs))
        context.coordinator.lastKey = EditorDocsAttributedContentKey(runs: runs)
        return textView
    }

    func updateNSView(_ textView: EditorDocsInlineTextView, context: Context) {
        textView.textContainerInset = textContainerInset
        let key = EditorDocsAttributedContentKey(runs: runs)
        if context.coordinator.lastKey != key {
            textView.setAttributedString(EditorDocsAttributedRenderCache.shared.attributedText(for: runs))
            context.coordinator.lastKey = key
            agentDebugLog("docsText.update contentChanged runs=\(runs.count) chars=\(runs.reduce(0) { $0 + $1.text.count })")
        }
    }

    func sizeThatFits(_ proposal: ProposedViewSize, nsView: EditorDocsInlineTextView, context: Context) -> CGSize? {
        let proposedWidth = proposal.width ?? 640
        let height = nsView.measuredHeight(forWidth: proposedWidth)
        agentDebugLog("docsText.sizeThatFits width=\(Int(proposedWidth.rounded())) height=\(Int(height.rounded())) runs=\(runs.count) chars=\(runs.reduce(0) { $0 + $1.text.count })")
        return CGSize(width: proposedWidth, height: height)
    }

    final class Coordinator: NSObject, NSTextViewDelegate {
        fileprivate var lastKey: EditorDocsAttributedContentKey?

        func textView(_ textView: NSTextView, clickedOnLink link: Any, at charIndex: Int) -> Bool {
            if let url = link as? URL {
                NSWorkspace.shared.open(url)
                return true
            }
            if let raw = link as? String, let url = URL(string: raw) {
                NSWorkspace.shared.open(url)
                return true
            }
            return false
        }
    }
}

@MainActor
func editorDocsAttributedText(for runs: [EditorDocsRun]) -> NSAttributedString {
    EditorDocsAttributedRenderCache.shared.attributedText(for: runs)
}

@MainActor
func editorDocsAttributedBounds(for runs: [EditorDocsRun], width: CGFloat) -> CGRect {
    EditorDocsAttributedRenderCache.shared.bounds(for: runs, width: width)
}

final class EditorDocsInlineTextView: NSTextView {
    init(textContainerInset: NSSize = .zero) {
        let textStorage = NSTextStorage()
        let layoutManager = NSLayoutManager()
        textStorage.addLayoutManager(layoutManager)
        let textContainer = NSTextContainer(size: CGSize(width: 0, height: CGFloat.greatestFiniteMagnitude))
        textContainer.widthTracksTextView = true
        textContainer.heightTracksTextView = false
        textContainer.lineFragmentPadding = 0
        layoutManager.addTextContainer(textContainer)
        super.init(frame: .zero, textContainer: textContainer)
        isEditable = false
        isSelectable = true
        isRichText = true
        importsGraphics = false
        drawsBackground = false
        backgroundColor = .clear
        self.textContainerInset = textContainerInset
        isVerticallyResizable = false
        isHorizontallyResizable = false
        allowsUndo = false
        allowsDocumentBackgroundColorChange = false
        usesFindBar = false
        linkTextAttributes = [
            .foregroundColor: NSColor.linkColor,
            .underlineStyle: NSUnderlineStyle.single.rawValue,
        ]
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func setAttributedString(_ attributedString: NSAttributedString) {
        textStorage?.setAttributedString(attributedString)
        invalidateIntrinsicContentSize()
        agentDebugLog("docsText.setAttributedString chars=\(attributedString.length)")
    }

    func measuredHeight(forWidth width: CGFloat) -> CGFloat {
        guard let textContainer, let layoutManager else { return 0 }
        let insetWidth = textContainerInset.width * 2
        let insetHeight = textContainerInset.height * 2
        let contentWidth = max(width - insetWidth, 1)
        textContainer.containerSize = CGSize(width: contentWidth, height: CGFloat.greatestFiniteMagnitude)
        layoutManager.ensureLayout(for: textContainer)
        return ceil(layoutManager.usedRect(for: textContainer).height + insetHeight)
    }

    override var intrinsicContentSize: NSSize {
        let width = bounds.width > 0 ? bounds.width : 640
        return NSSize(width: width, height: measuredHeight(forWidth: width))
    }
}
