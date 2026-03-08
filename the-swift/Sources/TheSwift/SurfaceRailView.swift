import AppKit
import SwiftUI

enum SurfaceRailItemKind: String, Equatable {
    case editorSurface
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
    let paneId: UInt64?
    let bufferId: UInt64?
    let bufferIndex: Int?
    let terminalId: UInt64?
    let canClose: Bool
}

struct SurfaceRailSectionSnapshot: Identifiable, Equatable {
    let id: String
    let title: String
    let items: [SurfaceRailItemSnapshot]
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
    let onFocusEditorSurface: (UInt64) -> Void
    let onSelectOpenBuffer: (Int) -> Void
    let onSelectTerminal: (UInt64) -> Void
    let onCloseBuffer: (UInt64) -> Void
    let onCloseTerminal: (UInt64) -> Void

    var body: some View {
        SurfaceRailNativeView(
            snapshot: snapshot,
            onFocusEditorSurface: onFocusEditorSurface,
            onSelectOpenBuffer: onSelectOpenBuffer,
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
    var diagnosticSummary: String?

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

    override func mouseDown(with event: NSEvent) {
        let parentTableView = enclosingScrollView?.documentView as? NSTableView
        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "surface_rail.row.mouse_down row=\(parentTableView?.row(for: self) ?? -1) selectedRow=\(parentTableView?.selectedRow ?? -1) item=\(diagnosticSummary ?? "none")"
            )
        }
        super.mouseDown(with: event)
    }

    override func mouseUp(with event: NSEvent) {
        let parentTableView = enclosingScrollView?.documentView as? NSTableView
        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "surface_rail.row.mouse_up row=\(parentTableView?.row(for: self) ?? -1) selectedRow=\(parentTableView?.selectedRow ?? -1) item=\(diagnosticSummary ?? "none")"
            )
        }
        super.mouseUp(with: event)
    }
}

private final class SurfaceRailOutlineView: NSOutlineView {
    var diagnosticsItemSummary: ((Any?) -> String)?

    override func mouseDown(with event: NSEvent) {
        logMouseEvent(event, phase: "down_before")
        super.mouseDown(with: event)
        logMouseEvent(event, phase: "down_after")
    }

    override func mouseUp(with event: NSEvent) {
        logMouseEvent(event, phase: "up_before")
        super.mouseUp(with: event)
        logMouseEvent(event, phase: "up_after")
    }

    private func logMouseEvent(_ event: NSEvent, phase: String) {
        guard DiagnosticsDebugLog.enabled else { return }
        let point = convert(event.locationInWindow, from: nil)
        let row = row(at: point)
        let column = self.column(at: point)
        let hitView = hitTest(point)
        let item = row >= 0 ? self.item(atRow: row) : nil
        let hitDescription = hitView.map { String(describing: type(of: $0)) } ?? "none"
        let itemSummary = diagnosticsItemSummary?(item) ?? "none"
        DiagnosticsDebugLog.log(
            "surface_rail.outline.mouse_\(phase) point=\(Int(point.x)),\(Int(point.y)) row=\(row) column=\(column) clickedRow=\(clickedRow) selectedRow=\(selectedRow) hit=\(hitDescription) item=\(itemSummary)"
        )
    }
}

private final class SurfaceRailContainerView: NSView {
    let scrollView = NSScrollView()
    let outlineView = SurfaceRailOutlineView()
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
    let onFocusEditorSurface: (UInt64) -> Void
    let onSelectOpenBuffer: (Int) -> Void
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
        container.outlineView.target = context.coordinator
        container.outlineView.action = #selector(Coordinator.handlePrimaryAction(_:))
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
            container.outlineView.diagnosticsItemSummary = { [weak self] item in
                self?.debugSummary(for: item) ?? "none"
            }
        }

        func updateSnapshot(_ snapshot: SurfaceRailSnapshot) {
            guard lastSnapshot != snapshot else {
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
            let shouldSelect = item is SurfaceRailItemNode
            if DiagnosticsDebugLog.enabled {
                DiagnosticsDebugLog.log(
                    "surface_rail.should_select row=\(outlineView.row(forItem: item)) allow=\(shouldSelect ? 1 : 0) item=\(debugSummary(for: item))"
                )
            }
            return shouldSelect
        }

        func outlineViewSelectionIsChanging(_ notification: Notification) {
            guard let outlineView = notification.object as? NSOutlineView else {
                return
            }
            if DiagnosticsDebugLog.enabled {
                DiagnosticsDebugLog.log(
                    "surface_rail.selection_changing selectedRows=\(outlineView.selectedRowIndexes.map(String.init).joined(separator: ",")) clickedRow=\(outlineView.clickedRow)"
                )
            }
        }

        func outlineView(
            _ outlineView: NSOutlineView,
            selectionIndexesForProposedSelection proposedSelectionIndexes: IndexSet
        ) -> IndexSet {
            if DiagnosticsDebugLog.enabled {
                DiagnosticsDebugLog.log(
                    "surface_rail.selection_proposed rows=\(proposedSelectionIndexes.map(String.init).joined(separator: ",")) currentSelected=\(outlineView.selectedRowIndexes.map(String.init).joined(separator: ",")) clickedRow=\(outlineView.clickedRow)"
                )
            }
            return proposedSelectionIndexes
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
            if DiagnosticsDebugLog.enabled {
                DiagnosticsDebugLog.log(
                    "surface_rail.selection row=\(outlineView.selectedRow) item=\(debugSummary(for: item.snapshot))"
                )
            }
            activate(item.snapshot)
        }

        @objc
        func handlePrimaryAction(_ sender: Any?) {
            let outlineView = (sender as? NSOutlineView) ?? container?.outlineView
            guard let outlineView,
                  outlineView.clickedRow >= 0,
                  outlineView.clickedRow == outlineView.selectedRow,
                  let item = outlineView.item(atRow: outlineView.clickedRow) as? SurfaceRailItemNode,
                  item.snapshot.isActive else {
                return
            }
            if DiagnosticsDebugLog.enabled {
                DiagnosticsDebugLog.log(
                    "surface_rail.primary_action row=\(outlineView.clickedRow) item=\(debugSummary(for: item.snapshot))"
                )
            }
            activate(item.snapshot)
        }

        private func activate(_ snapshot: SurfaceRailItemSnapshot) {
            if DiagnosticsDebugLog.enabled {
                DiagnosticsDebugLog.log(
                    "surface_rail.activate item=\(debugSummary(for: snapshot))"
                )
            }
            switch snapshot.kind {
            case .editorSurface:
                if let paneId = snapshot.paneId {
                    parent.onFocusEditorSurface(paneId)
                }
            case .buffer:
                if let bufferIndex = snapshot.bufferIndex {
                    parent.onSelectOpenBuffer(bufferIndex)
                }
            case .terminal:
                if let terminalId = snapshot.terminalId {
                    parent.onSelectTerminal(terminalId)
                }
            }
        }

        private func close(_ snapshot: SurfaceRailItemSnapshot) {
            switch snapshot.kind {
            case .editorSurface:
                break
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

        private func debugSummary(for snapshot: SurfaceRailItemSnapshot) -> String {
            [
                "id=\(snapshot.id)",
                "kind=\(snapshot.kind.rawValue)",
                "active=\(snapshot.isActive ? 1 : 0)",
                "pane=\(snapshot.paneId ?? 0)",
                "bufferIndex=\(snapshot.bufferIndex ?? -1)",
                "bufferId=\(snapshot.bufferId ?? 0)",
                "terminal=\(snapshot.terminalId ?? 0)",
                "title=\(snapshot.title)"
            ].joined(separator: " ")
        }

        private func debugSummary(for item: Any?) -> String {
            if let snapshot = (item as? SurfaceRailItemNode)?.snapshot {
                return debugSummary(for: snapshot)
            }
            if let section = item as? SurfaceRailSectionNode {
                return "section id=\(section.snapshot.id) title=\(section.snapshot.title)"
            }
            return "none"
        }

        func outlineView(_ outlineView: NSOutlineView, rowViewForItem item: Any) -> NSTableRowView? {
            guard item is SurfaceRailItemNode else {
                return nil
            }
            let rowView = SurfaceRailRowView()
            rowView.diagnosticSummary = debugSummary(for: item)
            return rowView
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
        _ = event
        guard currentItem?.canClose == true else { return }
        closeButton.isHidden = false
    }

    override func mouseExited(with event: NSEvent) {
        _ = event
        closeButton.isHidden = true
    }

    override var backgroundStyle: NSView.BackgroundStyle {
        didSet { updateColors() }
    }

    override func mouseDown(with event: NSEvent) {
        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "surface_rail.cell.mouse_down hit=\(debugHitDescription(for: event)) item=\(debugCurrentItemSummary())"
            )
        }
        super.mouseDown(with: event)
    }

    override func mouseUp(with event: NSEvent) {
        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "surface_rail.cell.mouse_up hit=\(debugHitDescription(for: event)) item=\(debugCurrentItemSummary())"
            )
        }
        super.mouseUp(with: event)
    }

    func configure(item: SurfaceRailItemSnapshot) {
        currentItem = item
        titleLabel.stringValue = item.title
        subtitleLabel.stringValue = item.subtitle ?? ""
        subtitleLabel.isHidden = (item.subtitle ?? "").isEmpty
        modifiedDot.isHidden = !item.isModified
        closeButton.isHidden = !item.canClose
        closeButton.isEnabled = item.canClose
        iconView.image = NSImage(
            systemSymbolName: iconName(for: item.kind),
            accessibilityDescription: nil
        )
        updateColors()
    }

    @objc
    private func handleCloseButton() {
        guard let currentItem else { return }
        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "surface_rail.close_button item=\(debugCurrentItemSummary())"
            )
        }
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

    private func iconName(for kind: SurfaceRailItemKind) -> String {
        switch kind {
        case .editorSurface:
            return "rectangle.split.2x1"
        case .buffer:
            return "doc.text"
        case .terminal:
            return "terminal.fill"
        }
    }

    private func debugCurrentItemSummary() -> String {
        guard let currentItem else { return "none" }
        return [
            "id=\(currentItem.id)",
            "kind=\(currentItem.kind.rawValue)",
            "active=\(currentItem.isActive ? 1 : 0)",
            "pane=\(currentItem.paneId ?? 0)",
            "bufferIndex=\(currentItem.bufferIndex ?? -1)",
            "bufferId=\(currentItem.bufferId ?? 0)",
            "terminal=\(currentItem.terminalId ?? 0)",
            "title=\(currentItem.title)"
        ].joined(separator: " ")
    }

    private func debugHitDescription(for event: NSEvent) -> String {
        let point = convert(event.locationInWindow, from: nil)
        let hitView = hitTest(point)
        return hitView.map { String(describing: type(of: $0)) } ?? "none"
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
