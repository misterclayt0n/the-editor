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

    var rootDisplayName: String {
        guard !root.isEmpty else {
            return "workspace"
        }
        let value = URL(fileURLWithPath: root).lastPathComponent
        return value.isEmpty ? root : value
    }
}

private final class FileTreeNode: NSObject {
    let id: String
    let path: String
    let name: String
    let isDirectory: Bool
    let hasUnloadedChildren: Bool
    var children: [FileTreeNode]

    init(snapshot: FileTreeNodeSnapshot) {
        self.id = snapshot.id
        self.path = snapshot.path
        self.name = snapshot.name
        self.isDirectory = snapshot.isDirectory
        self.hasUnloadedChildren = snapshot.hasUnloadedChildren
        self.children = []
    }
}

struct FileTreeSidebarView: View {
    let snapshot: FileTreeSnapshot
    let onSetVisible: (Bool) -> Void
    let onSetExpanded: (String, Bool) -> Void
    let onSelectPath: (String) -> Void
    let onOpenSelected: () -> Void

    var body: some View {
        let rootNodes = buildOutlineNodes(from: snapshot.nodes)

        VStack(spacing: 0) {
            HStack(spacing: 8) {
                Text(snapshot.mode == 1 ? "Explorer (Current Folder)" : "Explorer")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(.secondary)
                Spacer()
                Button {
                    onSetVisible(false)
                } label: {
                    Image(systemName: "sidebar.left")
                        .font(.system(size: 11, weight: .semibold))
                }
                .buttonStyle(.plain)
                .foregroundStyle(.secondary)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(Color(nsColor: .windowBackgroundColor).opacity(0.65))

            Divider()

            NativeOutlineFileTreeView(
                rootNodes: rootNodes,
                selectedNodeID: snapshot.selectedNodeID,
                expandedNodeIDs: snapshot.expandedNodeIDs,
                onSetExpanded: onSetExpanded,
                onSelectPath: onSelectPath,
                onOpenSelected: onOpenSelected
            )
        }
        .background(Color(nsColor: .controlBackgroundColor))
    }

    private func buildOutlineNodes(from snapshots: [FileTreeNodeSnapshot]) -> [FileTreeNode] {
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

private struct NativeOutlineFileTreeView: NSViewRepresentable {
    let rootNodes: [FileTreeNode]
    let selectedNodeID: String?
    let expandedNodeIDs: Set<String>
    let onSetExpanded: (String, Bool) -> Void
    let onSelectPath: (String) -> Void
    let onOpenSelected: () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()
        scrollView.drawsBackground = false
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.borderType = .noBorder

        let outlineView = FileTreeOutlineView()
        outlineView.headerView = nil
        outlineView.delegate = context.coordinator
        outlineView.dataSource = context.coordinator
        outlineView.rowHeight = 22
        outlineView.indentationPerLevel = 14
        outlineView.selectionHighlightStyle = .regular
        outlineView.focusRingType = .none
        outlineView.usesAlternatingRowBackgroundColors = false
        outlineView.target = context.coordinator
        outlineView.doubleAction = #selector(Coordinator.handleDoubleAction(_:))
        outlineView.onConfirmSelection = { [weak coordinator = context.coordinator] in
            coordinator?.openSelectedIfPossible()
        }
        if #available(macOS 11.0, *) {
            outlineView.style = .sourceList
        }

        let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("name"))
        column.title = "Name"
        outlineView.addTableColumn(column)
        outlineView.outlineTableColumn = column
        outlineView.columnAutoresizingStyle = .lastColumnOnlyAutoresizingStyle

        scrollView.documentView = outlineView

        context.coordinator.parent = self
        context.coordinator.bind(outlineView: outlineView)
        context.coordinator.rootNodes = rootNodes

        outlineView.reloadData()
        context.coordinator.restoreExpansionState(expandedNodeIDs: expandedNodeIDs)
        context.coordinator.restoreSelection(selectedNodeID: selectedNodeID)

        return scrollView
    }

    func updateNSView(_ nsView: NSScrollView, context: Context) {
        guard let outlineView = nsView.documentView as? FileTreeOutlineView else {
            return
        }

        context.coordinator.parent = self
        context.coordinator.rootNodes = rootNodes

        outlineView.reloadData()
        context.coordinator.restoreExpansionState(expandedNodeIDs: expandedNodeIDs)
        context.coordinator.restoreSelection(selectedNodeID: selectedNodeID)
    }

    final class Coordinator: NSObject, NSOutlineViewDataSource, NSOutlineViewDelegate {
        var parent: NativeOutlineFileTreeView
        var rootNodes: [FileTreeNode] = []
        private weak var outlineView: FileTreeOutlineView?
        private var suppressSelectionEvents = false
        private var suppressExpansionEvents = false

        init(_ parent: NativeOutlineFileTreeView) {
            self.parent = parent
        }

        func bind(outlineView: FileTreeOutlineView) {
            self.outlineView = outlineView
        }

        func restoreExpansionState(expandedNodeIDs: Set<String>) {
            guard let outlineView else {
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
            guard let outlineView else {
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
            guard let outlineView, outlineView.selectedRow >= 0 else {
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
                let outlineView,
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
            cell.textField?.font = NSFont.systemFont(
                ofSize: 12,
                weight: node.isDirectory ? .semibold : .regular
            )
            cell.textField?.textColor = .labelColor
            cell.imageView?.image = Self.icon(for: node)
            cell.imageView?.contentTintColor = node.isDirectory ? .secondaryLabelColor : .tertiaryLabelColor
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
            let symbolName = node.isDirectory ? "folder.fill" : "doc.text"
            guard let image = NSImage(systemSymbolName: symbolName, accessibilityDescription: nil) else {
                return nil
            }
            image.isTemplate = true
            return image
        }
    }
}
