import AppKit
import Foundation
import SwiftUI

struct FileTreeNodeSnapshot: Identifiable, Equatable {
    let id: String
    let path: String
    let name: String
    let depth: Int
    let isDirectory: Bool
    let expanded: Bool
    let selected: Bool
    let hasUnloadedChildren: Bool
}

struct FileTreeSnapshot: Equatable {
    let visible: Bool
    let mode: UInt8
    let root: String
    let selectedPath: String?
    let refreshGeneration: UInt64
    let nodes: [FileTreeNodeSnapshot]

    static let hidden = FileTreeSnapshot(
        visible: false,
        mode: 0,
        root: "",
        selectedPath: nil,
        refreshGeneration: 0,
        nodes: []
    )

    var selectedNodeID: String? {
        nodes.first(where: { $0.selected })?.id
    }

    var expandedNodeIDs: Set<String> {
        Set(nodes.filter { $0.isDirectory && $0.expanded }.map(\.id))
    }
}

private final class FileTreeNode: NSObject {
    let id: String
    let path: String
    let name: String
    let isDirectory: Bool
    let hasUnloadedChildren: Bool
    var children: [FileTreeNode]

    init(
        id: String,
        path: String,
        name: String,
        isDirectory: Bool,
        hasUnloadedChildren: Bool,
        children: [FileTreeNode] = []
    ) {
        self.id = id
        self.path = path
        self.name = name
        self.isDirectory = isDirectory
        self.hasUnloadedChildren = hasUnloadedChildren
        self.children = children
    }

    convenience init(snapshot: FileTreeNodeSnapshot) {
        self.init(
            id: snapshot.id,
            path: snapshot.path,
            name: snapshot.name,
            isDirectory: snapshot.isDirectory,
            hasUnloadedChildren: snapshot.hasUnloadedChildren,
            children: []
        )
    }

    func clone(with children: [FileTreeNode]) -> FileTreeNode {
        FileTreeNode(
            id: id,
            path: path,
            name: name,
            isDirectory: isDirectory,
            hasUnloadedChildren: hasUnloadedChildren,
            children: children
        )
    }
}

struct FileTreeSidebarView: View {
    let snapshot: FileTreeSnapshot
    let onSetExpanded: (String, Bool) -> Void
    let onSelectPath: (String) -> Void
    let onOpenSelected: () -> Void

    var body: some View {
        NavigatorSidebarView(
            snapshot: snapshot,
            onSetExpanded: onSetExpanded,
            onSelectPath: onSelectPath,
            onOpenSelected: onOpenSelected
        )
    }
}

private final class FileTreeOutlineView: NSOutlineView {
    var onConfirmSelection: (() -> Void)?

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 36 || event.keyCode == 76 {
            onConfirmSelection?()
            return
        }
        super.keyDown(with: event)
    }
}

private final class NavigatorSidebarContainerView: NSView {
    let topBar = NSVisualEffectView()
    let segmentedControl: NSSegmentedControl
    let scrollView = NSScrollView()
    let outlineView = FileTreeOutlineView()
    let bottomBar = NSVisualEffectView()
    let searchField = NSSearchField()
    let optionsButton = NSPopUpButton(frame: .zero, pullsDown: true)

    override init(frame frameRect: NSRect) {
        self.segmentedControl = NSSegmentedControl(images: Self.navigatorImages(), trackingMode: .selectOne, target: nil, action: nil)
        super.init(frame: frameRect)
        wantsLayer = true
        configureViews()
        buildLayout()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func configureViews() {
        topBar.translatesAutoresizingMaskIntoConstraints = false
        topBar.material = .sidebar
        topBar.blendingMode = .withinWindow
        topBar.state = .active

        segmentedControl.translatesAutoresizingMaskIntoConstraints = false
        segmentedControl.selectedSegment = 0
        segmentedControl.segmentStyle = .texturedRounded
        segmentedControl.controlSize = .small

        scrollView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.drawsBackground = false
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.borderType = .noBorder

        outlineView.translatesAutoresizingMaskIntoConstraints = false
        outlineView.headerView = nil
        outlineView.floatsGroupRows = false
        outlineView.indentationMarkerFollowsCell = true
        outlineView.backgroundColor = .clear
        if #available(macOS 11.0, *) {
            outlineView.style = .sourceList
        }

        scrollView.documentView = outlineView

        bottomBar.translatesAutoresizingMaskIntoConstraints = false
        bottomBar.material = .sidebar
        bottomBar.blendingMode = .withinWindow
        bottomBar.state = .active

        searchField.translatesAutoresizingMaskIntoConstraints = false
        searchField.placeholderString = "Filter"
        searchField.controlSize = .small
        searchField.sendsSearchStringImmediately = true

        optionsButton.translatesAutoresizingMaskIntoConstraints = false
        optionsButton.bezelStyle = .texturedRounded
        optionsButton.controlSize = .small
        optionsButton.imagePosition = .imageOnly
        configureOptionsMenu()
    }

    private func configureOptionsMenu() {
        let menu = NSMenu(title: "Navigator")
        menu.addItem(withTitle: "Navigator Options", action: nil, keyEquivalent: "")
        menu.addItem(.separator())
        menu.addItem(withTitle: "Collapse Folders", action: nil, keyEquivalent: "")
        menu.addItem(withTitle: "Sort by Name", action: nil, keyEquivalent: "")
        menu.addItem(withTitle: "Sort by Type", action: nil, keyEquivalent: "")
        optionsButton.menu = menu

        optionsButton.removeAllItems()
        optionsButton.addItem(withTitle: "")
        if let image = NSImage(systemSymbolName: "slider.horizontal.3", accessibilityDescription: "Navigator options") {
            image.isTemplate = true
            optionsButton.itemArray.first?.image = image
        }
    }

    private func buildLayout() {
        addSubview(topBar)
        addSubview(scrollView)
        addSubview(bottomBar)

        topBar.addSubview(segmentedControl)
        bottomBar.addSubview(searchField)
        bottomBar.addSubview(optionsButton)

        NSLayoutConstraint.activate([
            topBar.leadingAnchor.constraint(equalTo: leadingAnchor),
            topBar.trailingAnchor.constraint(equalTo: trailingAnchor),
            topBar.topAnchor.constraint(equalTo: topAnchor),
            topBar.heightAnchor.constraint(equalToConstant: 34),

            segmentedControl.leadingAnchor.constraint(equalTo: topBar.leadingAnchor, constant: 8),
            segmentedControl.centerYAnchor.constraint(equalTo: topBar.centerYAnchor),
            segmentedControl.trailingAnchor.constraint(lessThanOrEqualTo: topBar.trailingAnchor, constant: -8),

            scrollView.leadingAnchor.constraint(equalTo: leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: trailingAnchor),
            scrollView.topAnchor.constraint(equalTo: topBar.bottomAnchor),

            bottomBar.leadingAnchor.constraint(equalTo: leadingAnchor),
            bottomBar.trailingAnchor.constraint(equalTo: trailingAnchor),
            bottomBar.topAnchor.constraint(equalTo: scrollView.bottomAnchor),
            bottomBar.bottomAnchor.constraint(equalTo: bottomAnchor),
            bottomBar.heightAnchor.constraint(equalToConstant: 32),

            searchField.leadingAnchor.constraint(equalTo: bottomBar.leadingAnchor, constant: 8),
            searchField.centerYAnchor.constraint(equalTo: bottomBar.centerYAnchor),

            optionsButton.leadingAnchor.constraint(equalTo: searchField.trailingAnchor, constant: 6),
            optionsButton.trailingAnchor.constraint(equalTo: bottomBar.trailingAnchor, constant: -8),
            optionsButton.centerYAnchor.constraint(equalTo: bottomBar.centerYAnchor),
            optionsButton.widthAnchor.constraint(equalToConstant: 24),

            searchField.heightAnchor.constraint(equalToConstant: 22)
        ])
    }

    private static func navigatorImages() -> [NSImage] {
        let symbols = [
            "folder",
            "doc.on.doc",
            "bookmark",
            "magnifyingglass",
            "exclamationmark.triangle",
            "scissors",
            "tag",
            "wrench.and.screwdriver"
        ]

        return symbols.map { symbol in
            let image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil) ?? NSImage()
            image.isTemplate = true
            return image
        }
    }
}

private struct NavigatorSidebarView: NSViewRepresentable {
    let snapshot: FileTreeSnapshot
    let onSetExpanded: (String, Bool) -> Void
    let onSelectPath: (String) -> Void
    let onOpenSelected: () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    func makeNSView(context: Context) -> NavigatorSidebarContainerView {
        let container = NavigatorSidebarContainerView(frame: .zero)
        let outlineView = container.outlineView

        outlineView.delegate = context.coordinator
        outlineView.dataSource = context.coordinator
        outlineView.target = context.coordinator
        outlineView.doubleAction = #selector(Coordinator.handleDoubleAction(_:))
        outlineView.onConfirmSelection = { [weak coordinator = context.coordinator] in
            coordinator?.openSelectedIfPossible()
        }

        container.searchField.target = context.coordinator
        container.searchField.action = #selector(Coordinator.searchQueryChanged(_:))

        context.coordinator.parent = self
        context.coordinator.bind(container: container)
        context.coordinator.updateSnapshot(snapshot)

        return container
    }

    func updateNSView(_ nsView: NavigatorSidebarContainerView, context: Context) {
        context.coordinator.parent = self
        context.coordinator.bind(container: nsView)
        context.coordinator.updateSnapshot(snapshot)
    }

    final class Coordinator: NSObject, NSOutlineViewDataSource, NSOutlineViewDelegate {
        var parent: NavigatorSidebarView

        private weak var container: NavigatorSidebarContainerView?
        private var allRootNodes: [FileTreeNode] = []
        private var visibleRootNodes: [FileTreeNode] = []
        private var suppressSelectionEvents = false
        private var suppressExpansionEvents = false
        private var filterQuery: String = ""
        private var latestExpandedNodeIDs: Set<String> = []
        private var latestSelectedNodeID: String?

        init(_ parent: NavigatorSidebarView) {
            self.parent = parent
        }

        func bind(container: NavigatorSidebarContainerView) {
            self.container = container
        }

        func updateSnapshot(_ snapshot: FileTreeSnapshot) {
            let roots = Self.buildOutlineNodes(from: snapshot.nodes)
            allRootNodes = roots
            latestExpandedNodeIDs = snapshot.expandedNodeIDs
            latestSelectedNodeID = snapshot.selectedNodeID

            applyFilterAndReload()
        }

        @objc
        func searchQueryChanged(_ sender: NSSearchField) {
            filterQuery = sender.stringValue
            applyFilterAndReload()
        }

        private func applyFilterAndReload() {
            visibleRootNodes = filterTree(allRootNodes, query: filterQuery)

            guard let outlineView = container?.outlineView else {
                return
            }

            outlineView.reloadData()
            restoreExpansionState(expandedNodeIDs: latestExpandedNodeIDs)
            restoreSelection(selectedNodeID: latestSelectedNodeID)
        }

        private func filterTree(_ roots: [FileTreeNode], query: String) -> [FileTreeNode] {
            let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else {
                return roots
            }

            let needle = trimmed.lowercased()
            return roots.compactMap { filterNode($0, needle: needle) }
        }

        private func filterNode(_ node: FileTreeNode, needle: String) -> FileTreeNode? {
            let filteredChildren = node.children.compactMap { filterNode($0, needle: needle) }
            let matchesSelf = node.name.lowercased().contains(needle)
            if matchesSelf || !filteredChildren.isEmpty {
                return node.clone(with: filteredChildren)
            }
            return nil
        }

        private static func buildOutlineNodes(from snapshots: [FileTreeNodeSnapshot]) -> [FileTreeNode] {
            var roots: [FileTreeNode] = []
            var stack: [(depth: Int, node: FileTreeNode)] = []

            for snapshot in snapshots {
                let node = FileTreeNode(snapshot: snapshot)
                while let last = stack.last, last.depth >= snapshot.depth {
                    _ = stack.popLast()
                }

                if let parent = stack.last?.node {
                    parent.children.append(node)
                } else {
                    roots.append(node)
                }
                stack.append((snapshot.depth, node))
            }

            return roots
        }

        func restoreExpansionState(expandedNodeIDs: Set<String>) {
            guard let outlineView = container?.outlineView else {
                return
            }

            suppressExpansionEvents = true
            applyExpansion(
                to: visibleRootNodes,
                expandedNodeIDs: expandedNodeIDs,
                parentExpanded: true,
                outlineView: outlineView
            )
            suppressExpansionEvents = false
        }

        func restoreSelection(selectedNodeID: String?) {
            guard let outlineView = container?.outlineView else {
                return
            }

            guard
                let selectedNodeID,
                let nodePath = Self.path(to: selectedNodeID, in: visibleRootNodes),
                let target = nodePath.last
            else {
                if outlineView.selectedRow != -1 {
                    suppressSelectionEvents = true
                    outlineView.deselectAll(nil)
                    suppressSelectionEvents = false
                }
                return
            }

            suppressExpansionEvents = true
            for ancestor in nodePath.dropLast() where ancestor.isDirectory {
                outlineView.expandItem(ancestor)
            }
            suppressExpansionEvents = false

            let row = outlineView.row(forItem: target)
            guard row >= 0, row != outlineView.selectedRow else {
                return
            }

            suppressSelectionEvents = true
            outlineView.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false)
            suppressSelectionEvents = false
        }

        private func applyExpansion(
            to nodes: [FileTreeNode],
            expandedNodeIDs: Set<String>,
            parentExpanded: Bool,
            outlineView: NSOutlineView
        ) {
            guard parentExpanded else {
                return
            }

            for node in nodes where node.isDirectory {
                let shouldExpand = expandedNodeIDs.contains(node.id)
                if shouldExpand {
                    outlineView.expandItem(node)
                } else {
                    outlineView.collapseItem(node, collapseChildren: true)
                }

                applyExpansion(
                    to: node.children,
                    expandedNodeIDs: expandedNodeIDs,
                    parentExpanded: shouldExpand,
                    outlineView: outlineView
                )
            }
        }

        private static func path(to targetID: String, in nodes: [FileTreeNode]) -> [FileTreeNode]? {
            for node in nodes {
                if node.id == targetID {
                    return [node]
                }
                if let childPath = path(to: targetID, in: node.children) {
                    return [node] + childPath
                }
            }
            return nil
        }

        @objc
        func handleDoubleAction(_ sender: Any?) {
            _ = sender
            openSelectedIfPossible()
        }

        func openSelectedIfPossible() {
            guard let outlineView = container?.outlineView, outlineView.selectedRow >= 0 else {
                return
            }
            parent.onOpenSelected()
        }

        func outlineView(_ outlineView: NSOutlineView, numberOfChildrenOfItem item: Any?) -> Int {
            let nodes = (item as? FileTreeNode)?.children ?? visibleRootNodes
            return nodes.count
        }

        func outlineView(_ outlineView: NSOutlineView, child index: Int, ofItem item: Any?) -> Any {
            let nodes = (item as? FileTreeNode)?.children ?? visibleRootNodes
            return nodes[index]
        }

        func outlineView(_ outlineView: NSOutlineView, isItemExpandable item: Any) -> Bool {
            guard let node = item as? FileTreeNode else {
                return false
            }
            return node.isDirectory
        }

        func outlineView(
            _ outlineView: NSOutlineView,
            shouldExpandItem item: Any
        ) -> Bool {
            guard let node = item as? FileTreeNode else {
                return true
            }
            if !suppressExpansionEvents {
                parent.onSetExpanded(node.path, true)
            }
            return true
        }

        func outlineView(
            _ outlineView: NSOutlineView,
            shouldCollapseItem item: Any
        ) -> Bool {
            guard let node = item as? FileTreeNode else {
                return true
            }
            if !suppressExpansionEvents {
                parent.onSetExpanded(node.path, false)
            }
            return true
        }

        func outlineViewSelectionDidChange(_ notification: Notification) {
            _ = notification
            guard
                !suppressSelectionEvents,
                let outlineView = container?.outlineView,
                outlineView.selectedRow >= 0,
                let node = outlineView.item(atRow: outlineView.selectedRow) as? FileTreeNode
            else {
                return
            }
            parent.onSelectPath(node.path)
        }

        func outlineView(
            _ outlineView: NSOutlineView,
            viewFor tableColumn: NSTableColumn?,
            item: Any
        ) -> NSView? {
            _ = tableColumn
            guard let node = item as? FileTreeNode else {
                return nil
            }

            let identifier = NSUserInterfaceItemIdentifier("file-tree-cell")
            let cell = (outlineView.makeView(withIdentifier: identifier, owner: nil) as? NSTableCellView)
                ?? makeCellView(identifier: identifier)

            cell.textField?.stringValue = node.name
            cell.imageView?.image = Self.icon(for: node)
            return cell
        }

        private func makeCellView(identifier: NSUserInterfaceItemIdentifier) -> NSTableCellView {
            let cell = NSTableCellView(frame: .zero)
            cell.identifier = identifier

            let imageView = NSImageView(frame: .zero)
            imageView.translatesAutoresizingMaskIntoConstraints = false
            imageView.imageScaling = .scaleProportionallyDown

            let textField = NSTextField(labelWithString: "")
            textField.translatesAutoresizingMaskIntoConstraints = false
            textField.lineBreakMode = .byTruncatingTail
            textField.usesSingleLineMode = true

            cell.addSubview(imageView)
            cell.addSubview(textField)
            cell.imageView = imageView
            cell.textField = textField

            NSLayoutConstraint.activate([
                imageView.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 4),
                imageView.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
                imageView.widthAnchor.constraint(equalToConstant: 16),
                imageView.heightAnchor.constraint(equalToConstant: 16),

                textField.leadingAnchor.constraint(equalTo: imageView.trailingAnchor, constant: 6),
                textField.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
                textField.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -6)
            ])

            return cell
        }

        private static func icon(for node: FileTreeNode) -> NSImage? {
            let image: NSImage?
            if node.isDirectory {
                image = NSImage(named: NSImage.folderName) ?? NSWorkspace.shared.icon(forFile: node.path)
            } else {
                image = NSWorkspace.shared.icon(forFile: node.path)
            }
            guard let image else {
                return nil
            }
            image.size = NSSize(width: 16, height: 16)
            return image
        }
    }
}
