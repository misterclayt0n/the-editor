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
    let panel = NSVisualEffectView()
    let topSpacer = NSView()
    let scrollView = NSScrollView()
    let outlineView = FileTreeOutlineView()
    let nameColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("name"))

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        configureViews()
        buildLayout()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func configureViews() {
        panel.translatesAutoresizingMaskIntoConstraints = false
        panel.material = .sidebar
        panel.blendingMode = .withinWindow
        panel.state = .active
        panel.wantsLayer = true
        panel.layer?.cornerRadius = 10
        panel.layer?.masksToBounds = true
        panel.layer?.borderWidth = 1
        panel.layer?.borderColor = NSColor.separatorColor.withAlphaComponent(0.30).cgColor

        topSpacer.translatesAutoresizingMaskIntoConstraints = false

        scrollView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.drawsBackground = false
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.borderType = .noBorder
        scrollView.scrollerStyle = .overlay

        outlineView.translatesAutoresizingMaskIntoConstraints = false
        outlineView.headerView = nil
        outlineView.floatsGroupRows = false
        outlineView.indentationMarkerFollowsCell = true
        outlineView.backgroundColor = .clear
        nameColumn.title = "Name"
        outlineView.addTableColumn(nameColumn)
        outlineView.outlineTableColumn = nameColumn
        outlineView.columnAutoresizingStyle = .lastColumnOnlyAutoresizingStyle
        if #available(macOS 11.0, *) {
            outlineView.style = .sourceList
        }

        scrollView.documentView = outlineView
    }

    private func buildLayout() {
        addSubview(panel)
        panel.addSubview(topSpacer)
        panel.addSubview(scrollView)

        NSLayoutConstraint.activate([
            panel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 6),
            panel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -6),
            panel.topAnchor.constraint(equalTo: topAnchor, constant: 6),
            panel.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -6),

            topSpacer.leadingAnchor.constraint(equalTo: panel.leadingAnchor),
            topSpacer.trailingAnchor.constraint(equalTo: panel.trailingAnchor),
            topSpacer.topAnchor.constraint(equalTo: panel.topAnchor),
            topSpacer.heightAnchor.constraint(equalToConstant: 28),

            scrollView.leadingAnchor.constraint(equalTo: panel.leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: panel.trailingAnchor),
            scrollView.topAnchor.constraint(equalTo: topSpacer.bottomAnchor),
            scrollView.bottomAnchor.constraint(equalTo: panel.bottomAnchor)
        ])
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
        private var rootNodes: [FileTreeNode] = []
        private var suppressSelectionEvents = false
        private var suppressExpansionEvents = false
        private var latestExpandedNodeIDs: Set<String> = []
        private var latestSelectedNodeID: String?

        init(_ parent: NavigatorSidebarView) {
            self.parent = parent
        }

        func bind(container: NavigatorSidebarContainerView) {
            self.container = container
        }

        func updateSnapshot(_ snapshot: FileTreeSnapshot) {
            rootNodes = Self.buildOutlineNodes(from: snapshot.nodes)
            latestExpandedNodeIDs = snapshot.expandedNodeIDs
            latestSelectedNodeID = snapshot.selectedNodeID

            guard let outlineView = container?.outlineView else {
                return
            }

            outlineView.reloadData()
            restoreExpansionState(expandedNodeIDs: latestExpandedNodeIDs)
            restoreSelection(selectedNodeID: latestSelectedNodeID)
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
                to: rootNodes,
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
                let nodePath = Self.path(to: selectedNodeID, in: rootNodes),
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
            let nodes = (item as? FileTreeNode)?.children ?? rootNodes
            return nodes.count
        }

        func outlineView(_ outlineView: NSOutlineView, child index: Int, ofItem item: Any?) -> Any {
            let nodes = (item as? FileTreeNode)?.children ?? rootNodes
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
