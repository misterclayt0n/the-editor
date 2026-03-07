import AppKit
import SwiftUI

enum SurfaceRailItemKind: String, Equatable {
    case buffer
    case terminal
}

struct SurfaceRailItemSnapshot: Identifiable, Equatable {
    let id: String
    let kind: SurfaceRailItemKind
    let title: String
    let subtitle: String?
    let isActive: Bool
    let isModified: Bool
    let statusText: String?
    let bufferId: UInt64?
    let bufferIndex: Int?
    let terminalId: UInt64?
}

struct SurfaceRailSectionSnapshot: Identifiable, Equatable {
    let kind: SurfaceRailItemKind
    let title: String
    let items: [SurfaceRailItemSnapshot]

    var id: String { kind.rawValue }
}

struct SurfaceRailSnapshot: Equatable {
    let sections: [SurfaceRailSectionSnapshot]

    var totalItemCount: Int {
        sections.reduce(into: 0) { partialResult, section in
            partialResult += section.items.count
        }
    }

    var isEmpty: Bool {
        totalItemCount == 0
    }
}

struct SurfaceRailView: View {
    let snapshot: SurfaceRailSnapshot
    let onSelectBuffer: (Int) -> Void
    let onSelectTerminal: (UInt64) -> Void
    let onCloseBuffer: (UInt64) -> Void
    let onCloseTerminal: (UInt64) -> Void

    var body: some View {
        SurfaceRailNativeView(
            snapshot: snapshot,
            onSelectBuffer: onSelectBuffer,
            onSelectTerminal: onSelectTerminal,
            onCloseBuffer: onCloseBuffer,
            onCloseTerminal: onCloseTerminal
        )
    }
}

private final class SurfaceRailSectionNode: NSObject {
    let snapshot: SurfaceRailSectionSnapshot

    init(snapshot: SurfaceRailSectionSnapshot) {
        self.snapshot = snapshot
    }
}

private final class SurfaceRailItemNode: NSObject {
    let snapshot: SurfaceRailItemSnapshot

    init(snapshot: SurfaceRailItemSnapshot) {
        self.snapshot = snapshot
    }
}

private final class SurfaceRailRowView: NSTableRowView {
    override func drawSelection(in dirtyRect: NSRect) {
        guard selectionHighlightStyle != .none else { return }
        let rect = bounds.insetBy(dx: 5, dy: 1.5)
        let path = NSBezierPath(roundedRect: rect, xRadius: 6, yRadius: 6)
        (isEmphasized
            ? NSColor.selectedContentBackgroundColor
            : NSColor.unemphasizedSelectedContentBackgroundColor
        ).setFill()
        path.fill()
    }
}

private final class SurfaceRailContainerView: NSView {
    let scrollView = NSScrollView()
    let outlineView = NSOutlineView()
    let nameColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("surface-rail-name"))

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = .clear
        configureViews()
        buildLayout()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func configureViews() {
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.drawsBackground = false
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.borderType = .noBorder
        scrollView.scrollerStyle = .overlay
        scrollView.automaticallyAdjustsContentInsets = false
        scrollView.contentInsets = NSEdgeInsets(top: 4, left: 0, bottom: 4, right: 0)

        outlineView.translatesAutoresizingMaskIntoConstraints = false
        outlineView.headerView = nil
        outlineView.floatsGroupRows = false
        outlineView.backgroundColor = .clear
        outlineView.intercellSpacing = NSSize(width: 0, height: 0)
        outlineView.indentationPerLevel = 0
        outlineView.focusRingType = .none
        outlineView.selectionHighlightStyle = .regular

        nameColumn.title = "Surface"
        outlineView.addTableColumn(nameColumn)
        outlineView.outlineTableColumn = nameColumn
        outlineView.columnAutoresizingStyle = .lastColumnOnlyAutoresizingStyle

        if #available(macOS 11.0, *) {
            outlineView.style = .sourceList
        }

        scrollView.documentView = outlineView
    }

    private func buildLayout() {
        addSubview(scrollView)
        NSLayoutConstraint.activate([
            scrollView.leadingAnchor.constraint(equalTo: leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: trailingAnchor),
            scrollView.topAnchor.constraint(equalTo: topAnchor),
            scrollView.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }
}

private struct SurfaceRailNativeView: NSViewRepresentable {
    let snapshot: SurfaceRailSnapshot
    let onSelectBuffer: (Int) -> Void
    let onSelectTerminal: (UInt64) -> Void
    let onCloseBuffer: (UInt64) -> Void
    let onCloseTerminal: (UInt64) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    func makeNSView(context: Context) -> SurfaceRailContainerView {
        let container = SurfaceRailContainerView(frame: .zero)
        container.outlineView.delegate = context.coordinator
        container.outlineView.dataSource = context.coordinator
        context.coordinator.bind(container: container)
        context.coordinator.parent = self
        context.coordinator.updateSnapshot(snapshot)
        return container
    }

    func updateNSView(_ nsView: SurfaceRailContainerView, context: Context) {
        context.coordinator.bind(container: nsView)
        context.coordinator.parent = self
        context.coordinator.updateSnapshot(snapshot)
    }

    final class Coordinator: NSObject, NSOutlineViewDataSource, NSOutlineViewDelegate {
        var parent: SurfaceRailNativeView

        private weak var container: SurfaceRailContainerView?
        private var sectionNodes: [SurfaceRailSectionNode] = []
        private var childNodesBySection: [String: [SurfaceRailItemNode]] = [:]
        private var suppressSelectionEvents = false
        private var lastSnapshot: SurfaceRailSnapshot?

        init(_ parent: SurfaceRailNativeView) {
            self.parent = parent
        }

        func bind(container: SurfaceRailContainerView) {
            self.container = container
        }

        func updateSnapshot(_ snapshot: SurfaceRailSnapshot) {
            guard lastSnapshot != snapshot else {
                restoreSelection()
                return
            }
            lastSnapshot = snapshot
            sectionNodes = snapshot.sections.map(SurfaceRailSectionNode.init(snapshot:))
            childNodesBySection = Dictionary(uniqueKeysWithValues: snapshot.sections.map { section in
                (section.id, section.items.map(SurfaceRailItemNode.init(snapshot:)))
            })

            guard let outlineView = container?.outlineView else {
                return
            }

            outlineView.reloadData()
            for sectionNode in sectionNodes {
                outlineView.expandItem(sectionNode, expandChildren: false)
            }
            restoreSelection()
        }

        private func restoreSelection() {
            guard let outlineView = container?.outlineView else {
                return
            }

            guard let selectedNode = currentActiveItemNode() else {
                if outlineView.selectedRow != -1 {
                    suppressSelectionEvents = true
                    outlineView.deselectAll(nil)
                    suppressSelectionEvents = false
                }
                return
            }

            let row = outlineView.row(forItem: selectedNode)
            guard row >= 0, row != outlineView.selectedRow else {
                return
            }

            suppressSelectionEvents = true
            outlineView.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false)
            suppressSelectionEvents = false
        }

        private func currentActiveItemNode() -> SurfaceRailItemNode? {
            for section in sectionNodes {
                if let node = childNodesBySection[section.snapshot.id]?.first(where: { $0.snapshot.isActive }) {
                    return node
                }
            }
            return nil
        }

        func outlineView(_ outlineView: NSOutlineView, numberOfChildrenOfItem item: Any?) -> Int {
            if let section = item as? SurfaceRailSectionNode {
                return childNodesBySection[section.snapshot.id]?.count ?? 0
            }
            return sectionNodes.count
        }

        func outlineView(_ outlineView: NSOutlineView, child index: Int, ofItem item: Any?) -> Any {
            if let section = item as? SurfaceRailSectionNode {
                return childNodesBySection[section.snapshot.id]![index]
            }
            return sectionNodes[index]
        }

        func outlineView(_ outlineView: NSOutlineView, isItemExpandable item: Any) -> Bool {
            item is SurfaceRailSectionNode
        }

        func outlineView(_ outlineView: NSOutlineView, isGroupItem item: Any) -> Bool {
            item is SurfaceRailSectionNode
        }

        func outlineView(_ outlineView: NSOutlineView, shouldShowOutlineCellForItem item: Any) -> Bool {
            false
        }

        func outlineView(_ outlineView: NSOutlineView, shouldSelectItem item: Any) -> Bool {
            item is SurfaceRailItemNode
        }

        func outlineView(_ outlineView: NSOutlineView, heightOfRowByItem item: Any) -> CGFloat {
            if item is SurfaceRailSectionNode {
                return 24
            }
            return 36
        }

        func outlineViewSelectionDidChange(_ notification: Notification) {
            _ = notification
            guard !suppressSelectionEvents,
                  let outlineView = container?.outlineView,
                  outlineView.selectedRow >= 0,
                  let item = outlineView.item(atRow: outlineView.selectedRow) as? SurfaceRailItemNode else {
                return
            }
            activate(item.snapshot)
        }

        private func activate(_ snapshot: SurfaceRailItemSnapshot) {
            switch snapshot.kind {
            case .buffer:
                if let bufferIndex = snapshot.bufferIndex {
                    parent.onSelectBuffer(bufferIndex)
                }
            case .terminal:
                if let terminalId = snapshot.terminalId {
                    parent.onSelectTerminal(terminalId)
                }
            }
        }

        private func close(_ snapshot: SurfaceRailItemSnapshot) {
            switch snapshot.kind {
            case .buffer:
                if let bufferId = snapshot.bufferId {
                    parent.onCloseBuffer(bufferId)
                }
            case .terminal:
                if let terminalId = snapshot.terminalId {
                    parent.onCloseTerminal(terminalId)
                }
            }
        }

        func outlineView(_ outlineView: NSOutlineView, rowViewForItem item: Any) -> NSTableRowView? {
            guard item is SurfaceRailItemNode else {
                return nil
            }
            return SurfaceRailRowView()
        }

        func outlineView(
            _ outlineView: NSOutlineView,
            viewFor tableColumn: NSTableColumn?,
            item: Any
        ) -> NSView? {
            _ = tableColumn

            if let section = item as? SurfaceRailSectionNode {
                let identifier = NSUserInterfaceItemIdentifier("surface-rail-section")
                let view = (outlineView.makeView(withIdentifier: identifier, owner: nil) as? SurfaceRailSectionCellView)
                    ?? SurfaceRailSectionCellView(identifier: identifier)
                view.configure(section: section.snapshot)
                return view
            }

            guard let itemNode = item as? SurfaceRailItemNode else {
                return nil
            }

            let identifier = NSUserInterfaceItemIdentifier("surface-rail-item")
            let view = (outlineView.makeView(withIdentifier: identifier, owner: nil) as? SurfaceRailItemCellView)
                ?? SurfaceRailItemCellView(identifier: identifier)
            view.configure(item: itemNode.snapshot)
            view.onClose = { [weak self] item in
                self?.close(item)
            }
            return view
        }
    }
}

private final class SurfaceRailSectionCellView: NSTableCellView {
    private let titleLabel = NSTextField(labelWithString: "")
    private let countLabel = NSTextField(labelWithString: "")

    convenience init(identifier: NSUserInterfaceItemIdentifier) {
        self.init(frame: .zero)
        self.identifier = identifier

        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.font = NSFont.systemFont(ofSize: 11, weight: .semibold)
        titleLabel.textColor = .secondaryLabelColor
        titleLabel.alignment = .left
        titleLabel.lineBreakMode = .byTruncatingTail

        countLabel.translatesAutoresizingMaskIntoConstraints = false
        countLabel.font = NSFont.systemFont(ofSize: 10, weight: .medium)
        countLabel.textColor = .tertiaryLabelColor
        countLabel.alignment = .left
        countLabel.lineBreakMode = .byClipping

        addSubview(titleLabel)
        addSubview(countLabel)

        NSLayoutConstraint.activate([
            titleLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 10),
            titleLabel.centerYAnchor.constraint(equalTo: centerYAnchor),

            countLabel.leadingAnchor.constraint(equalTo: titleLabel.trailingAnchor, constant: 6),
            countLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            countLabel.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -8),
        ])
    }

    func configure(section: SurfaceRailSectionSnapshot) {
        titleLabel.stringValue = section.title.uppercased()
        countLabel.stringValue = String(section.items.count)
    }
}

private final class SurfaceRailItemCellView: NSTableCellView {
    private let iconView = NSImageView(frame: .zero)
    private let titleLabel = NSTextField(labelWithString: "")
    private let subtitleLabel = NSTextField(labelWithString: "")
    private let modifiedDot = DotView()
    private let closeButton = NSButton()
    private var currentItem: SurfaceRailItemSnapshot?
    private var trackingArea: NSTrackingArea?
    var onClose: ((SurfaceRailItemSnapshot) -> Void)?

    convenience init(identifier: NSUserInterfaceItemIdentifier) {
        self.init(frame: .zero)
        self.identifier = identifier

        iconView.translatesAutoresizingMaskIntoConstraints = false
        iconView.imageScaling = .scaleProportionallyDown
        iconView.imageAlignment = .alignCenter

        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.font = .systemFont(ofSize: 12)
        titleLabel.lineBreakMode = .byTruncatingTail
        titleLabel.usesSingleLineMode = true
        titleLabel.textColor = .labelColor

        subtitleLabel.translatesAutoresizingMaskIntoConstraints = false
        subtitleLabel.font = .systemFont(ofSize: 11)
        subtitleLabel.lineBreakMode = .byTruncatingMiddle
        subtitleLabel.usesSingleLineMode = true
        subtitleLabel.textColor = .secondaryLabelColor

        modifiedDot.translatesAutoresizingMaskIntoConstraints = false
        modifiedDot.isHidden = true

        closeButton.translatesAutoresizingMaskIntoConstraints = false
        closeButton.bezelStyle = .inline
        closeButton.isBordered = false
        if #available(macOS 11.0, *) {
            closeButton.image = NSImage(systemSymbolName: "xmark", accessibilityDescription: "Close")
            closeButton.symbolConfiguration = NSImage.SymbolConfiguration(pointSize: 9, weight: .medium)
        }
        closeButton.imageScaling = .scaleProportionallyDown
        closeButton.isHidden = true
        closeButton.target = self
        closeButton.action = #selector(handleCloseButton)

        addSubview(iconView)
        addSubview(titleLabel)
        addSubview(subtitleLabel)
        addSubview(modifiedDot)
        addSubview(closeButton)

        self.imageView = iconView
        self.textField = titleLabel

        NSLayoutConstraint.activate([
            iconView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 2),
            iconView.topAnchor.constraint(equalTo: topAnchor, constant: 8),
            iconView.widthAnchor.constraint(equalToConstant: 14),
            iconView.heightAnchor.constraint(equalToConstant: 14),

            titleLabel.leadingAnchor.constraint(equalTo: iconView.trailingAnchor, constant: 4),
            titleLabel.topAnchor.constraint(equalTo: topAnchor, constant: 5),
            titleLabel.trailingAnchor.constraint(lessThanOrEqualTo: closeButton.leadingAnchor, constant: -4),

            modifiedDot.leadingAnchor.constraint(equalTo: titleLabel.trailingAnchor, constant: 4),
            modifiedDot.centerYAnchor.constraint(equalTo: titleLabel.centerYAnchor),
            modifiedDot.widthAnchor.constraint(equalToConstant: 6),
            modifiedDot.heightAnchor.constraint(equalToConstant: 6),

            subtitleLabel.leadingAnchor.constraint(equalTo: titleLabel.leadingAnchor),
            subtitleLabel.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: 1),
            subtitleLabel.trailingAnchor.constraint(lessThanOrEqualTo: closeButton.leadingAnchor, constant: -4),
            subtitleLabel.bottomAnchor.constraint(lessThanOrEqualTo: bottomAnchor, constant: -5),

            closeButton.centerYAnchor.constraint(equalTo: centerYAnchor),
            closeButton.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -6),
            closeButton.widthAnchor.constraint(equalToConstant: 14),
            closeButton.heightAnchor.constraint(equalToConstant: 14),
        ])

        updateColors()
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let trackingArea { removeTrackingArea(trackingArea) }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .activeInKeyWindow],
            owner: self,
            userInfo: nil
        )
        trackingArea = area
        addTrackingArea(area)
    }

    override func mouseEntered(with event: NSEvent) {
        closeButton.isHidden = false
    }

    override func mouseExited(with event: NSEvent) {
        closeButton.isHidden = true
    }

    override var backgroundStyle: NSView.BackgroundStyle {
        didSet { updateColors() }
    }

    func configure(item: SurfaceRailItemSnapshot) {
        currentItem = item
        titleLabel.stringValue = item.title
        subtitleLabel.stringValue = item.subtitle ?? ""
        subtitleLabel.isHidden = (item.subtitle ?? "").isEmpty
        modifiedDot.isHidden = !item.isModified
        iconView.image = NSImage(
            systemSymbolName: item.kind == .buffer ? "doc.text" : "terminal.fill",
            accessibilityDescription: nil
        )
        updateColors()
    }

    @objc
    private func handleCloseButton() {
        guard let currentItem else { return }
        onClose?(currentItem)
    }

    private func updateColors() {
        let emphasized = backgroundStyle == .emphasized
        titleLabel.textColor = emphasized ? .alternateSelectedControlTextColor : .labelColor
        subtitleLabel.textColor = emphasized
            ? NSColor.alternateSelectedControlTextColor.withAlphaComponent(0.72)
            : .secondaryLabelColor
        modifiedDot.fillColor = emphasized ? .alternateSelectedControlTextColor : .systemOrange
        closeButton.contentTintColor = emphasized
            ? NSColor.alternateSelectedControlTextColor.withAlphaComponent(0.8)
            : .tertiaryLabelColor
        if #available(macOS 11.0, *) {
            iconView.contentTintColor = emphasized ? .alternateSelectedControlTextColor : .secondaryLabelColor
        }
    }
}

private final class DotView: NSView {
    var fillColor: NSColor = .systemOrange {
        didSet {
            needsDisplay = true
        }
    }

    override var intrinsicContentSize: NSSize {
        NSSize(width: 6, height: 6)
    }

    override func draw(_ dirtyRect: NSRect) {
        fillColor.setFill()
        NSBezierPath(ovalIn: bounds).fill()
    }
}
