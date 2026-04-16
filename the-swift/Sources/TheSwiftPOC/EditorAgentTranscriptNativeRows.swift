import AppKit
import Foundation

private enum AgentNativeLayout {
    static let userBubbleMaxWidthFraction: CGFloat = 0.78
    static let userBubbleCornerRadius: CGFloat = 16
    static let toolRowCornerRadius: CGFloat = 10
}

private enum AgentNativeMarkdownSegment {
    case text([EditorDocsRun])
    case code(language: String?, runs: [EditorDocsRun])
}

@MainActor
private protocol AgentTranscriptNativeRenderable where Self: NSView {
    func configure(
        item: EditorAgentTranscriptItem,
        selectionColor: NSColor,
        width: CGFloat,
        isToolExpanded: Bool,
        onToggleToolExpansion: @escaping () -> Void
    )
}

@MainActor
private protocol AgentTranscriptMeasuredSubview where Self: NSView {
    var preferredHeight: CGFloat { get }
}

@MainActor
final class AgentTranscriptNativeRowView: NSView {
    private var contentView: (NSView & AgentTranscriptNativeRenderable)?
    private var currentKind: EditorAgentTranscriptItem.Kind?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func configure(
        item: EditorAgentTranscriptItem,
        selectionColor: NSColor,
        width: CGFloat,
        isToolExpanded: Bool,
        onToggleToolExpansion: @escaping () -> Void
    ) {
        let nextKind = item.kind
        if currentKind != nextKind || contentView == nil {
            contentView?.removeFromSuperview()
            let nextView: (NSView & AgentTranscriptNativeRenderable)
            switch nextKind {
            case .user:
                nextView = AgentUserNativeRowView()
            case .assistant:
                nextView = AgentAssistantNativeRowView()
            case .thinking:
                nextView = AgentNoteNativeRowView()
            case .note:
                nextView = AgentNoteNativeRowView()
            case .tool:
                nextView = AgentToolNativeRowView()
            }
            contentView = nextView
            currentKind = nextKind
            addSubview(nextView)
        }

        contentView?.frame = bounds
        contentView?.configure(
            item: item,
            selectionColor: selectionColor,
            width: width,
            isToolExpanded: isToolExpanded,
            onToggleToolExpansion: onToggleToolExpansion
        )
        needsLayout = true
    }

    override func layout() {
        super.layout()
        contentView?.frame = bounds
    }
}

@MainActor
private final class AgentUserNativeRowView: NSView, AgentTranscriptNativeRenderable {
    private let contextPillView = NSView()
    private let contextIconView = NSImageView()
    private let contextLabel = NSTextField(labelWithString: "")
    private let bubbleView = NSView()
    private let textView = AgentInteractiveTextView()

    private var currentItem: EditorAgentTranscriptItem?
    private var currentSelectionColor: NSColor = .selectedContentBackgroundColor
    private var currentWidth: CGFloat = 0
    private var bubbleTextWidth: CGFloat = 160
    private var bubbleTextHeight: CGFloat = 0
    private var showsContext = false

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true

        contextPillView.wantsLayer = true
        contextPillView.layer?.cornerRadius = 12
        contextPillView.layer?.masksToBounds = true
        addSubview(contextPillView)

        contextIconView.imageScaling = .scaleProportionallyDown
        contextPillView.addSubview(contextIconView)

        contextLabel.font = .systemFont(ofSize: 10, weight: .medium)
        contextLabel.textColor = .secondaryLabelColor
        contextLabel.lineBreakMode = .byTruncatingTail
        contextPillView.addSubview(contextLabel)

        bubbleView.wantsLayer = true
        bubbleView.layer?.cornerRadius = AgentNativeLayout.userBubbleCornerRadius
        bubbleView.layer?.masksToBounds = true
        addSubview(bubbleView)
        bubbleView.addSubview(textView)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func configure(
        item: EditorAgentTranscriptItem,
        selectionColor: NSColor,
        width: CGFloat,
        isToolExpanded: Bool,
        onToggleToolExpansion: @escaping () -> Void
    ) {
        _ = isToolExpanded
        _ = onToggleToolExpansion
        let needsUpdate = currentItem != item
            || !currentSelectionColor.isEqual(selectionColor)
            || abs(currentWidth - width) > 0.5
        guard needsUpdate else { return }

        currentItem = item
        currentSelectionColor = selectionColor
        currentWidth = width
        bubbleTextWidth = max(min(width * AgentNativeLayout.userBubbleMaxWidthFraction, 520) - 28, 160)
        bubbleTextHeight = agentMeasurePlainTextHeight(
            item.text,
            width: bubbleTextWidth,
            font: .systemFont(ofSize: 13),
            lineSpacing: 1
        )
        textView.setAttributedString(
            agentPlainAttributedString(
                item.text,
                font: .systemFont(ofSize: 13),
                color: .labelColor,
                alignment: .left,
                lineSpacing: 1
            )
        )

        if let context = item.contextSummary?.trimmingCharacters(in: .whitespacesAndNewlines), !context.isEmpty {
            showsContext = true
            contextLabel.stringValue = context
            contextIconView.image = agentSymbolImage(name: "doc.text", pointSize: 9, weight: .semibold)
            contextIconView.contentTintColor = .secondaryLabelColor
            contextPillView.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.05).cgColor
            contextPillView.layer?.borderWidth = 0.5
            contextPillView.layer?.borderColor = NSColor.labelColor.withAlphaComponent(0.08).cgColor
            contextPillView.isHidden = false
        } else {
            showsContext = false
            contextPillView.isHidden = true
        }

        bubbleView.layer?.backgroundColor = selectionColor.withAlphaComponent(0.22).cgColor
        bubbleView.layer?.borderWidth = 0.5
        bubbleView.layer?.borderColor = NSColor.labelColor.withAlphaComponent(0.08).cgColor
        needsLayout = true
    }

    override func layout() {
        super.layout()
        let bubbleWidth = bubbleTextWidth + 28
        let bubbleHeight = bubbleTextHeight + 16
        var bubbleOriginY: CGFloat = 0

        if showsContext {
            let pillHeight: CGFloat = 24
            let pillWidth = min(max(contextLabel.intrinsicContentSize.width + 28, 44), max(bubbleWidth, 44))
            contextPillView.frame = CGRect(
                x: max(bounds.width - pillWidth, 0),
                y: 0,
                width: pillWidth,
                height: pillHeight
            )
            contextIconView.frame = CGRect(x: 8, y: floor((pillHeight - 10) * 0.5), width: 10, height: 10)
            contextLabel.frame = CGRect(x: 20, y: 4, width: pillWidth - 28, height: pillHeight - 8)
            bubbleOriginY = 28
        }

        bubbleView.frame = CGRect(
            x: max(bounds.width - bubbleWidth, 0),
            y: bubbleOriginY,
            width: bubbleWidth,
            height: bubbleHeight
        )
        textView.frame = CGRect(x: 14, y: 8, width: bubbleTextWidth, height: bubbleTextHeight)
    }
}

@MainActor
private final class AgentAssistantNativeRowView: NSView, AgentTranscriptNativeRenderable {
    private var currentItem: EditorAgentTranscriptItem?
    private var currentWidth: CGFloat = 0
    private var blockViews: [(NSView & AgentTranscriptMeasuredSubview)] = []

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func configure(
        item: EditorAgentTranscriptItem,
        selectionColor: NSColor,
        width: CGFloat,
        isToolExpanded: Bool,
        onToggleToolExpansion: @escaping () -> Void
    ) {
        _ = selectionColor
        _ = isToolExpanded
        _ = onToggleToolExpansion
        let needsUpdate = currentItem != item || abs(currentWidth - width) > 0.5
        guard needsUpdate else { return }

        currentItem = item
        currentWidth = width

        for view in blockViews {
            view.removeFromSuperview()
        }
        blockViews.removeAll()

        let contentWidth = max(width - 10, 180)

        if let rendered = item.renderedMarkdown, !rendered.blocks.isEmpty {
            for segment in agentNativeRenderedMarkdownSegments(for: rendered) {
                let view: (NSView & AgentTranscriptMeasuredSubview)
                switch segment {
                case .text(let runs):
                    view = AgentRunsTextBlockView(runs: runs, width: contentWidth)
                case .code(let language, let runs):
                    view = AgentCodeBlockNativeView(language: language, runs: runs, width: contentWidth)
                }
                blockViews.append(view)
                addSubview(view)
            }
        } else {
            let attributed = agentPlainAttributedString(
                item.text,
                font: .systemFont(ofSize: 13),
                color: .labelColor,
                alignment: .left,
                lineSpacing: 1
            )
            let view = AgentAttributedTextBlockView(
                attributedString: attributed,
                width: contentWidth,
                textInsets: NSEdgeInsets(top: 0, left: 0, bottom: 0, right: 0)
            )
            blockViews.append(view)
            addSubview(view)
        }

        if item.isStreaming && item.text.isEmpty {
            let spinner = AgentSpinnerBlockView()
            blockViews.append(spinner)
            addSubview(spinner)
        }

        needsLayout = true
    }

    override func layout() {
        super.layout()
        var y: CGFloat = 0
        for view in blockViews {
            view.frame = CGRect(x: 0, y: y, width: bounds.width, height: view.preferredHeight)
            y += view.preferredHeight
        }
    }
}

@MainActor
private final class AgentNoteNativeRowView: NSView, AgentTranscriptNativeRenderable {
    private let label = NSTextField(wrappingLabelWithString: "")
    private var currentItem: EditorAgentTranscriptItem?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        label.font = .systemFont(ofSize: 11, weight: .medium)
        label.textColor = .secondaryLabelColor
        label.alignment = .center
        label.lineBreakMode = .byWordWrapping
        addSubview(label)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func configure(
        item: EditorAgentTranscriptItem,
        selectionColor: NSColor,
        width: CGFloat,
        isToolExpanded: Bool,
        onToggleToolExpansion: @escaping () -> Void
    ) {
        _ = selectionColor
        _ = width
        _ = isToolExpanded
        _ = onToggleToolExpansion
        guard currentItem != item else { return }
        currentItem = item
        label.stringValue = item.text
        needsLayout = true
    }

    override func layout() {
        super.layout()
        label.frame = bounds.insetBy(dx: 12, dy: 2)
    }
}

@MainActor
private final class AgentToolNativeRowView: NSView, AgentTranscriptNativeRenderable {
    private let chevronView = NSImageView()
    private let iconBadgeView = NSView()
    private let toolIconView = NSImageView()
    private let toolBadgeLabel = NSTextField(labelWithString: "")
    private let titleLabel = NSTextField(labelWithString: "")
    private let summaryLabel = NSTextField(labelWithString: "")
    private let statusDotView = NSView()
    private let spinner = NSProgressIndicator()
    private let headerButton = AgentActionButton()
    private let bodyContainerView = NSView()
    private let bodyHeaderLabel = NSTextField(labelWithString: "Output")
    private let copyButton = AgentActionButton()
    private let bodyTextView = AgentInteractiveTextView(textContainerInset: NSSize(width: 10, height: 8))

    private var currentItem: EditorAgentTranscriptItem?
    private var currentWidth: CGFloat = 0
    private var isExpanded = false
    private var bodyTextHeight: CGFloat = 0
    private var bodyScrollHeight: CGFloat = 0
    private var copyResetTask: Task<Void, Never>?

    private static let headerHeight: CGFloat = 32
    private static let maxBodyContentHeight: CGFloat = 200

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true

        chevronView.imageScaling = .scaleProportionallyDown
        chevronView.contentTintColor = .tertiaryLabelColor
        addSubview(chevronView)

        iconBadgeView.wantsLayer = true
        iconBadgeView.layer?.cornerRadius = 8
        iconBadgeView.layer?.masksToBounds = true
        addSubview(iconBadgeView)

        toolIconView.imageScaling = .scaleProportionallyDown
        iconBadgeView.addSubview(toolIconView)

        toolBadgeLabel.font = .monospacedSystemFont(ofSize: 9, weight: .semibold)
        toolBadgeLabel.textColor = .tertiaryLabelColor
        toolBadgeLabel.lineBreakMode = .byClipping
        addSubview(toolBadgeLabel)

        titleLabel.font = .systemFont(ofSize: 12, weight: .medium)
        titleLabel.textColor = .labelColor
        titleLabel.lineBreakMode = .byTruncatingTail
        addSubview(titleLabel)

        summaryLabel.font = .systemFont(ofSize: 12, weight: .regular)
        summaryLabel.textColor = .secondaryLabelColor
        summaryLabel.lineBreakMode = .byTruncatingMiddle
        addSubview(summaryLabel)

        statusDotView.wantsLayer = true
        statusDotView.layer?.cornerRadius = 3.5
        statusDotView.layer?.masksToBounds = true
        addSubview(statusDotView)

        spinner.style = .spinning
        spinner.controlSize = .mini
        spinner.isDisplayedWhenStopped = false
        addSubview(spinner)

        headerButton.onAction = { [weak self] in
            guard let self, !(self.currentItem?.text.isEmpty ?? true) else { return }
            self.headerButton.onActionBody?()
        }
        addSubview(headerButton)

        bodyContainerView.wantsLayer = true
        bodyContainerView.layer?.cornerRadius = 8
        bodyContainerView.layer?.masksToBounds = true
        bodyContainerView.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.025).cgColor
        bodyContainerView.layer?.borderWidth = 0.5
        bodyContainerView.layer?.borderColor = NSColor.labelColor.withAlphaComponent(0.06).cgColor
        addSubview(bodyContainerView)

        bodyHeaderLabel.font = .systemFont(ofSize: 10, weight: .semibold)
        bodyHeaderLabel.textColor = .tertiaryLabelColor
        bodyContainerView.addSubview(bodyHeaderLabel)

        copyButton.title = "Copy"
        copyButton.image = agentSymbolImage(name: "doc.on.doc", pointSize: 9, weight: .semibold)
        copyButton.onAction = { [weak self] in
            guard let self, let currentItem = self.currentItem, !currentItem.text.isEmpty else { return }
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(currentItem.text, forType: .string)
            self.showCopiedState()
        }
        bodyContainerView.addSubview(copyButton)

        bodyContainerView.addSubview(bodyTextView)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        copyResetTask?.cancel()
    }

    func configure(
        item: EditorAgentTranscriptItem,
        selectionColor: NSColor,
        width: CGFloat,
        isToolExpanded: Bool,
        onToggleToolExpansion: @escaping () -> Void
    ) {
        _ = selectionColor
        let needsUpdate = currentItem != item || abs(currentWidth - width) > 0.5 || self.isExpanded != isToolExpanded
        guard needsUpdate else { return }

        currentItem = item
        currentWidth = width
        self.isExpanded = isToolExpanded
        headerButton.onActionBody = onToggleToolExpansion

        let presentation = agentToolPresentation(for: item)

        iconBadgeView.layer?.backgroundColor = presentation.iconColor.withAlphaComponent(0.12).cgColor
        toolIconView.image = agentSymbolImage(name: presentation.iconName, pointSize: 9, weight: .semibold)
        toolIconView.contentTintColor = presentation.iconColor

        toolBadgeLabel.stringValue = presentation.badgeText
        titleLabel.stringValue = presentation.titleText
        summaryLabel.stringValue = presentation.previewText ?? ""
        summaryLabel.isHidden = presentation.previewText == nil

        chevronView.isHidden = item.text.isEmpty
        if !item.text.isEmpty {
            chevronView.image = agentSymbolImage(
                name: isToolExpanded ? "chevron.down" : "chevron.right",
                pointSize: 8,
                weight: .bold
            )
        }

        if item.isStreaming {
            statusDotView.isHidden = true
            spinner.isHidden = false
            spinner.startAnimation(nil)
        } else {
            spinner.isHidden = true
            spinner.stopAnimation(nil)
            statusDotView.isHidden = false
            statusDotView.layer?.backgroundColor = presentation.statusColor.cgColor
        }

        bodyTextHeight = agentMeasurePlainTextHeight(
            item.text,
            width: max(width - 70, 80),
            font: .monospacedSystemFont(ofSize: 11, weight: .regular),
            lineSpacing: 1
        )
        bodyScrollHeight = min(max(bodyTextHeight + 16, 40), Self.maxBodyContentHeight)
        bodyTextView.setAttributedString(
            agentPlainAttributedString(
                item.text,
                font: .monospacedSystemFont(ofSize: 11, weight: .regular),
                color: NSColor.labelColor.withAlphaComponent(0.7),
                alignment: .left,
                lineSpacing: 1
            )
        )
        bodyContainerView.isHidden = !isToolExpanded || item.text.isEmpty
        needsLayout = true
    }

    override func layout() {
        super.layout()

        let h = Self.headerHeight
        let leftPad: CGFloat = 8
        let rightPad: CGFloat = 10
        let chevronSize: CGFloat = 10
        let hasChevron = !chevronView.isHidden

        chevronView.frame = hasChevron
            ? CGRect(x: leftPad, y: floor((h - chevronSize) / 2), width: chevronSize, height: chevronSize)
            : .zero

        let iconX = leftPad + (hasChevron ? chevronSize + 6 : 0)
        let iconSize: CGFloat = 16
        iconBadgeView.frame = CGRect(x: iconX, y: floor((h - iconSize) / 2), width: iconSize, height: iconSize)
        let iconInset: CGFloat = 3
        toolIconView.frame = CGRect(x: iconInset, y: iconInset, width: iconSize - iconInset * 2, height: iconSize - iconInset * 2)

        var trailingX = bounds.width - rightPad

        if !spinner.isHidden {
            let spinnerSize: CGFloat = 12
            trailingX -= spinnerSize
            spinner.frame = CGRect(x: trailingX, y: floor((h - spinnerSize) / 2), width: spinnerSize, height: spinnerSize)
            trailingX -= 6
        } else {
            spinner.frame = .zero
        }

        if !statusDotView.isHidden {
            let dotSize: CGFloat = 7
            trailingX -= dotSize
            statusDotView.frame = CGRect(x: trailingX, y: floor((h - dotSize) / 2), width: dotSize, height: dotSize)
            trailingX -= 6
        } else {
            statusDotView.frame = .zero
        }

        let labelsX = iconX + iconSize + 8
        let badgeWidth = min(toolBadgeLabel.intrinsicContentSize.width + 2, 60)
        toolBadgeLabel.frame = CGRect(x: labelsX, y: floor((h - 12) / 2), width: badgeWidth, height: 12)

        let titleX = labelsX + badgeWidth + 4
        if summaryLabel.isHidden {
            let titleWidth = max(trailingX - titleX - 4, 40)
            titleLabel.frame = CGRect(x: titleX, y: floor((h - 16) / 2), width: titleWidth, height: 16)
            summaryLabel.frame = .zero
        } else {
            let maxTitleWidth = max(trailingX - titleX - 4, 40)
            let titleWidth = min(titleLabel.intrinsicContentSize.width + 2, maxTitleWidth * 0.4)
            titleLabel.frame = CGRect(x: titleX, y: floor((h - 16) / 2), width: titleWidth, height: 16)
            let summaryX = titleX + titleWidth + 6
            let summaryWidth = max(trailingX - summaryX - 4, 20)
            summaryLabel.frame = CGRect(x: summaryX, y: floor((h - 16) / 2), width: summaryWidth, height: 16)
        }

        headerButton.frame = CGRect(x: 0, y: 0, width: bounds.width, height: h)

        guard !bodyContainerView.isHidden else {
            bodyContainerView.frame = .zero
            return
        }

        let bodyX = iconX
        let bodyY = h + 4
        let bodyWidth = max(bounds.width - bodyX - rightPad, 80)
        let bodyContainerHeight = 28 + bodyScrollHeight
        bodyContainerView.frame = CGRect(x: bodyX, y: bodyY, width: bodyWidth, height: bodyContainerHeight)
        bodyHeaderLabel.frame = CGRect(x: 10, y: 6, width: max(bodyWidth - 80, 20), height: 14)
        copyButton.frame = CGRect(x: bodyWidth - 52, y: 3, width: 44, height: 20)

        let textY: CGFloat = 26
        let textWidth = max(bodyWidth - 16, 40)
        bodyTextView.frame = CGRect(x: 4, y: textY, width: textWidth, height: bodyTextHeight + 16)
    }

    private func showCopiedState() {
        copyResetTask?.cancel()
        copyButton.title = "Copied"
        copyButton.image = agentSymbolImage(name: "checkmark", pointSize: 9, weight: .semibold)
        copyButton.contentTintColor = .systemGreen
        copyResetTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .seconds(1.2))
            self?.copyButton.title = "Copy"
            self?.copyButton.image = agentSymbolImage(name: "doc.on.doc", pointSize: 9, weight: .semibold)
            self?.copyButton.contentTintColor = .secondaryLabelColor
        }
    }
}

@MainActor
private class AgentAttributedTextBlockView: NSView, AgentTranscriptMeasuredSubview {
    private let textView = AgentInteractiveTextView()
    let preferredHeight: CGFloat
    private let textInsets: NSEdgeInsets

    init(attributedString: NSAttributedString, width: CGFloat, textInsets: NSEdgeInsets) {
        self.textInsets = textInsets
        let contentWidth = max(width - textInsets.left - textInsets.right, 1)
        let textHeight = ceil(
            attributedString.boundingRect(
                with: CGSize(width: contentWidth, height: .greatestFiniteMagnitude),
                options: [.usesLineFragmentOrigin, .usesFontLeading]
            ).height
        )
        preferredHeight = textHeight + textInsets.top + textInsets.bottom
        super.init(frame: .zero)
        addSubview(textView)
        textView.setAttributedString(attributedString)
    }

    convenience init(
        runs: [EditorDocsRun],
        width: CGFloat,
        textInsets: NSEdgeInsets = NSEdgeInsets(top: 0, left: 0, bottom: 0, right: 0)
    ) {
        self.init(
            attributedString: editorDocsAttributedText(for: runs),
            width: width,
            textInsets: textInsets
        )
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func layout() {
        super.layout()
        textView.frame = bounds.insetBy(dx: textInsets.left, dy: textInsets.top)
    }
}

@MainActor
private final class AgentRunsTextBlockView: AgentAttributedTextBlockView {}

@MainActor
private final class AgentCodeBlockNativeView: NSView, AgentTranscriptMeasuredSubview {
    private let languageLabel = NSTextField(labelWithString: "")
    private let copyButton = AgentActionButton()
    private let textView = AgentInteractiveTextView(textContainerInset: NSSize(width: 10, height: 10))
    private let contentHeight: CGFloat
    let preferredHeight: CGFloat
    private var copyResetTask: Task<Void, Never>?

    init(language: String?, runs: [EditorDocsRun], width: CGFloat) {
        contentHeight = ceil(editorDocsAttributedBounds(for: runs, width: max(width - 20, 160)).height)
        preferredHeight = contentHeight + 62
        super.init(frame: .zero)
        wantsLayer = true
        layer?.cornerRadius = 12
        layer?.masksToBounds = true
        layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.035).cgColor
        layer?.borderWidth = 0.5
        layer?.borderColor = NSColor.labelColor.withAlphaComponent(0.08).cgColor

        languageLabel.font = .monospacedSystemFont(ofSize: 10, weight: .semibold)
        languageLabel.textColor = .secondaryLabelColor
        languageLabel.stringValue = (language?.isEmpty == false ? language! : "code").uppercased()
        addSubview(languageLabel)

        copyButton.title = "Copy"
        copyButton.image = agentSymbolImage(name: "doc.on.doc", pointSize: 10, weight: .semibold)
        copyButton.onAction = { [weak self] in
            let text = runs.map(\.text).joined()
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
            self?.showCopiedState()
        }
        addSubview(copyButton)

        addSubview(textView)
        textView.setAttributedString(editorDocsAttributedText(for: runs))
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        copyResetTask?.cancel()
    }

    override func layout() {
        super.layout()
        languageLabel.frame = CGRect(x: 10, y: 8, width: max(bounds.width - 90, 0), height: 24)
        copyButton.frame = CGRect(x: bounds.width - 64, y: 8, width: 54, height: 24)
        textView.frame = CGRect(x: 10, y: 40, width: max(bounds.width - 20, 0), height: contentHeight + 20)
    }

    private func showCopiedState() {
        copyResetTask?.cancel()
        copyButton.title = "Copied"
        copyButton.image = agentSymbolImage(name: "checkmark", pointSize: 10, weight: .semibold)
        copyButton.contentTintColor = .systemGreen
        copyResetTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .seconds(1.2))
            self?.copyButton.title = "Copy"
            self?.copyButton.image = agentSymbolImage(name: "doc.on.doc", pointSize: 10, weight: .semibold)
            self?.copyButton.contentTintColor = .secondaryLabelColor
        }
    }
}

@MainActor
private final class AgentSpinnerBlockView: NSView, AgentTranscriptMeasuredSubview {
    private let spinner = NSProgressIndicator()
    let preferredHeight: CGFloat = 18

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        spinner.style = .spinning
        spinner.controlSize = .small
        spinner.isDisplayedWhenStopped = false
        spinner.startAnimation(nil)
        addSubview(spinner)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func layout() {
        super.layout()
        spinner.frame = CGRect(x: 0, y: max((bounds.height - 14) * 0.5, 0), width: 14, height: 14)
    }
}

@MainActor
private final class AgentInteractiveTextView: NSTextView, NSTextViewDelegate {
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
        delegate = self
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
    }

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

@MainActor
private final class AgentActionButton: NSButton {
    var onAction: (() -> Void)?
    var onActionBody: (() -> Void)?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        isBordered = false
        bezelStyle = .regularSquare
        focusRingType = .none
        setButtonType(.momentaryChange)
        imagePosition = .imageLeading
        font = .systemFont(ofSize: 10, weight: .semibold)
        contentTintColor = .secondaryLabelColor
        target = self
        action = #selector(handleAction)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc private func handleAction() {
        onAction?()
    }
}

struct AgentToolPresentation {
    let badgeText: String
    let titleText: String
    let previewText: String?
    let iconName: String
    let iconColor: NSColor
    let statusText: String
    let statusColor: NSColor
}

func agentToolPresentation(for item: EditorAgentTranscriptItem) -> AgentToolPresentation {
    let trimmedToolName = item.contextSummary?
        .trimmingCharacters(in: .whitespacesAndNewlines)
        .lowercased() ?? ""
    let toolName = trimmedToolName.isEmpty ? "tool" : trimmedToolName
    let trimmedSummary = item.title?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    let summary = trimmedSummary.isEmpty ? toolName.capitalized : trimmedSummary
    let outputPreview = agentToolOutputPreview(item.text)
    let summaryParts = agentToolSummaryParts(summary)
    let preview = summaryParts.preview ?? outputPreview
    let icon = agentToolIcon(for: toolName)
    let status = agentToolStatusPresentation(for: item.status, isStreaming: item.isStreaming)

    return AgentToolPresentation(
        badgeText: toolName.uppercased(),
        titleText: summaryParts.title,
        previewText: preview,
        iconName: icon.name,
        iconColor: icon.color,
        statusText: status.text,
        statusColor: status.color
    )
}

func agentToolRowMeasuredHeight(item: EditorAgentTranscriptItem, width: CGFloat, isExpanded: Bool) -> CGFloat {
    let headerHeight: CGFloat = 32
    guard isExpanded, !item.text.isEmpty else { return headerHeight }

    let bodyTextHeight = agentMeasurePlainTextHeight(
        item.text,
        width: max(width - 70, 80),
        font: .monospacedSystemFont(ofSize: 11, weight: .regular),
        lineSpacing: 1
    )
    let bodyScrollHeight = min(max(bodyTextHeight + 16, 40), 200)
    return headerHeight + 4 + 28 + bodyScrollHeight + 6
}

private func agentToolSummaryParts(_ summary: String) -> (title: String, preview: String?) {
    let trimmed = summary.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else { return ("Tool", nil) }

    let firstSpace = trimmed.firstIndex(of: " ")
    let leadingWord = firstSpace.map { String(trimmed[..<$0]) } ?? trimmed
    let trailing = firstSpace.map { String(trimmed[trimmed.index(after: $0)...]).trimmingCharacters(in: .whitespacesAndNewlines) }

    let compactVerbs: Set<String> = ["Read", "Edit", "Write", "Run", "Search", "Open", "List", "Create"]
    if compactVerbs.contains(leadingWord), let trailing, !trailing.isEmpty {
        return (leadingWord, trailing)
    }

    return (trimmed, nil)
}

private func agentToolOutputPreview(_ text: String) -> String? {
    let lines = text
        .components(separatedBy: .newlines)
        .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
        .filter { !$0.isEmpty }

    guard !lines.isEmpty else { return nil }
    if let changedLine = lines.first(where: { $0.localizedCaseInsensitiveContains("changed lines:") }) {
        return changedLine
    }
    if let meaningful = lines.first(where: { !$0.hasPrefix("---") && !$0.hasPrefix("+++") }) {
        return meaningful.count > 96 ? String(meaningful.prefix(96)) + "…" : meaningful
    }
    let first = lines[0]
    return first.count > 96 ? String(first.prefix(96)) + "…" : first
}

private func agentToolIcon(for toolName: String) -> (name: String, color: NSColor) {
    switch toolName {
    case "read":
        return ("eye", .systemBlue)
    case "edit":
        return ("square.and.pencil", .systemPurple)
    case "write":
        return ("square.and.arrow.down", .systemGreen)
    case "bash":
        return ("terminal", .systemOrange)
    case "grep", "search":
        return ("magnifyingglass", .systemTeal)
    default:
        return ("wrench.and.screwdriver", .secondaryLabelColor)
    }
}

private func agentToolStatusPresentation(for status: String?, isStreaming: Bool) -> (text: String, color: NSColor) {
    if isStreaming {
        return ("Running", .controlAccentColor)
    }
    switch status {
    case "failed":
        return ("Failed", .systemRed)
    case "done":
        return ("Done", .systemGreen)
    default:
        return ("Pending", .secondaryLabelColor)
    }
}

private func agentSymbolImage(name: String, pointSize: CGFloat, weight: NSFont.Weight) -> NSImage? {
    let configuration = NSImage.SymbolConfiguration(pointSize: pointSize, weight: weight)
    return NSImage(systemSymbolName: name, accessibilityDescription: nil)?.withSymbolConfiguration(configuration)
}

private func agentPlainAttributedString(
    _ text: String,
    font: NSFont,
    color: NSColor,
    alignment: NSTextAlignment,
    lineSpacing: CGFloat
) -> NSAttributedString {
    let paragraphStyle = NSMutableParagraphStyle()
    paragraphStyle.lineBreakMode = .byWordWrapping
    paragraphStyle.alignment = alignment
    paragraphStyle.lineSpacing = lineSpacing
    return NSAttributedString(
        string: text,
        attributes: [
            .font: font,
            .foregroundColor: color,
            .paragraphStyle: paragraphStyle,
        ]
    )
}

private func agentMeasurePlainTextHeight(_ text: String, width: CGFloat, font: NSFont, lineSpacing: CGFloat) -> CGFloat {
    let paragraphStyle = NSMutableParagraphStyle()
    paragraphStyle.lineBreakMode = .byWordWrapping
    paragraphStyle.lineSpacing = lineSpacing
    let attributed = NSAttributedString(
        string: text,
        attributes: [
            .font: font,
            .paragraphStyle: paragraphStyle,
        ]
    )
    return ceil(
        attributed.boundingRect(
            with: CGSize(width: max(width, 1), height: .greatestFiniteMagnitude),
            options: [.usesLineFragmentOrigin, .usesFontLeading]
        ).height
    )
}

private func agentNativeRenderedMarkdownSegments(for rendered: EditorRenderedMarkdown) -> [AgentNativeMarkdownSegment] {
    var result: [AgentNativeMarkdownSegment] = []
    var pendingTextBlocks: [EditorMarkdownBlock] = []

    func flushTextBlocks() {
        let runs = agentNativeMergedRuns(for: pendingTextBlocks, rendered: rendered)
        if !runs.isEmpty {
            result.append(.text(runs))
        }
        pendingTextBlocks.removeAll()
    }

    for block in rendered.blocks {
        if block.kind == .codeFence {
            flushTextBlocks()
            result.append(.code(language: block.language, runs: agentNativeRuns(for: block, rendered: rendered)))
        } else {
            pendingTextBlocks.append(block)
        }
    }

    flushTextBlocks()
    return result
}

private func agentNativeMergedRuns(for blocks: [EditorMarkdownBlock], rendered: EditorRenderedMarkdown) -> [EditorDocsRun] {
    guard !blocks.isEmpty else { return [] }
    var merged: [EditorDocsRun] = []

    for (index, block) in blocks.enumerated() {
        if block.kind != .blankLine {
            merged.append(contentsOf: agentNativeRuns(for: block, rendered: rendered))
        }

        guard index < blocks.count - 1 else { continue }
        let nextBlock = blocks[index + 1]
        let referenceRun = merged.last ?? agentNativeRuns(for: nextBlock, rendered: rendered).first
        merged.append(
            EditorDocsRun(
                text: "\n",
                style: referenceRun?.style ?? EditorResolvedStyle(
                    fg: nil,
                    bg: nil,
                    underlineColor: nil,
                    addModifiers: 0,
                    removeModifiers: 0,
                    underlineStyle: 0
                ),
                kind: .body,
                linkDestination: nil
            )
        )
    }

    return merged
}

private func agentNativeRuns(for block: EditorMarkdownBlock, rendered: EditorRenderedMarkdown) -> [EditorDocsRun] {
    guard block.runCount > 0,
          block.runStart >= 0,
          block.runStart + block.runCount <= rendered.runs.count
    else {
        return []
    }
    return Array(rendered.runs[block.runStart..<(block.runStart + block.runCount)])
}
