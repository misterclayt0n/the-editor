import AppKit
import SwiftUI

final class FileTreeNode: NSObject {
    enum Kind {
        case file
        case directory
    }

    let id: String
    let name: String
    let kind: Kind
    let children: [FileTreeNode]

    var isDirectory: Bool {
        kind == .directory
    }

    init(
        id: String,
        name: String,
        kind: Kind,
        children: [FileTreeNode] = []
    ) {
        self.id = id
        self.name = name
        self.kind = kind
        self.children = children
    }
}

final class FileTreeViewModel: ObservableObject {
    @Published var rootNodes: [FileTreeNode]
    @Published var selectedNodeID: String?

    init(rootNodes: [FileTreeNode], selectedNodeID: String? = nil) {
        self.rootNodes = rootNodes
        self.selectedNodeID = selectedNodeID
    }

    static func sample() -> FileTreeViewModel {
        let docs = FileTreeNode(
            id: "workspace/docs",
            name: "docs",
            kind: .directory,
            children: [
                FileTreeNode(
                    id: "workspace/docs/SWIFT_FILE_TREE_PLAN.md",
                    name: "SWIFT_FILE_TREE_PLAN.md",
                    kind: .file
                ),
                FileTreeNode(
                    id: "workspace/docs/TODO.md",
                    name: "TODO.md",
                    kind: .file
                ),
                FileTreeNode(
                    id: "workspace/docs/REWRITE.md",
                    name: "REWRITE.md",
                    kind: .file
                )
            ]
        )

        let swift = FileTreeNode(
            id: "workspace/the-swift",
            name: "the-swift",
            kind: .directory,
            children: [
                FileTreeNode(
                    id: "workspace/the-swift/Sources",
                    name: "Sources",
                    kind: .directory,
                    children: [
                        FileTreeNode(
                            id: "workspace/the-swift/Sources/TheSwift",
                            name: "TheSwift",
                            kind: .directory,
                            children: [
                                FileTreeNode(
                                    id: "workspace/the-swift/Sources/TheSwift/EditorView.swift",
                                    name: "EditorView.swift",
                                    kind: .file
                                ),
                                FileTreeNode(
                                    id: "workspace/the-swift/Sources/TheSwift/EditorModel.swift",
                                    name: "EditorModel.swift",
                                    kind: .file
                                ),
                                FileTreeNode(
                                    id: "workspace/the-swift/Sources/TheSwift/FileTreeSidebarView.swift",
                                    name: "FileTreeSidebarView.swift",
                                    kind: .file
                                )
                            ]
                        )
                    ]
                )
            ]
        )

        let root = FileTreeNode(
            id: "workspace",
            name: "the-editor",
            kind: .directory,
            children: [
                docs,
                swift,
                FileTreeNode(
                    id: "workspace/the-default",
                    name: "the-default",
                    kind: .directory,
                    children: [
                        FileTreeNode(
                            id: "workspace/the-default/command.rs",
                            name: "command.rs",
                            kind: .file
                        ),
                        FileTreeNode(
                            id: "workspace/the-default/keymap.rs",
                            name: "keymap.rs",
                            kind: .file
                        )
                    ]
                ),
                FileTreeNode(
                    id: "workspace/Cargo.toml",
                    name: "Cargo.toml",
                    kind: .file
                ),
                FileTreeNode(
                    id: "workspace/README.md",
                    name: "README.md",
                    kind: .file
                )
            ]
        )

        return FileTreeViewModel(
            rootNodes: [root],
            selectedNodeID: "workspace/docs/SWIFT_FILE_TREE_PLAN.md"
        )
    }
}

struct FileTreeSidebarView: View {
    @StateObject private var viewModel: FileTreeViewModel = .sample()

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Explorer")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(.secondary)
                Spacer()
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(Color(nsColor: .windowBackgroundColor).opacity(0.65))

            Divider()

            NativeOutlineFileTreeView(
                rootNodes: viewModel.rootNodes,
                selectedNodeID: $viewModel.selectedNodeID
            )
        }
        .background(Color(nsColor: .controlBackgroundColor))
    }
}

private struct NativeOutlineFileTreeView: NSViewRepresentable {
    let rootNodes: [FileTreeNode]
    @Binding var selectedNodeID: String?

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

        let outlineView = NSOutlineView()
        outlineView.headerView = nil
        outlineView.delegate = context.coordinator
        outlineView.dataSource = context.coordinator
        outlineView.rowHeight = 22
        outlineView.indentationPerLevel = 14
        outlineView.selectionHighlightStyle = .regular
        outlineView.focusRingType = .none
        outlineView.usesAlternatingRowBackgroundColors = false
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
        context.coordinator.seedInitialExpansionIfNeeded()

        outlineView.reloadData()
        context.coordinator.restoreExpansionState()
        context.coordinator.restoreSelection(selectedNodeID: selectedNodeID)

        return scrollView
    }

    func updateNSView(_ nsView: NSScrollView, context: Context) {
        guard let outlineView = nsView.documentView as? NSOutlineView else {
            return
        }

        context.coordinator.parent = self
        context.coordinator.rootNodes = rootNodes
        context.coordinator.seedInitialExpansionIfNeeded()

        outlineView.reloadData()
        context.coordinator.restoreExpansionState()
        context.coordinator.restoreSelection(selectedNodeID: selectedNodeID)
    }

    final class Coordinator: NSObject, NSOutlineViewDataSource, NSOutlineViewDelegate {
        var parent: NativeOutlineFileTreeView
        var rootNodes: [FileTreeNode] = []
        private weak var outlineView: NSOutlineView?
        private var expandedNodeIDs: Set<String> = []
        private var seededInitialExpansion = false
        private var suppressSelectionEvents = false

        init(_ parent: NativeOutlineFileTreeView) {
            self.parent = parent
        }

        func bind(outlineView: NSOutlineView) {
            self.outlineView = outlineView
        }

        func seedInitialExpansionIfNeeded() {
            guard !seededInitialExpansion else {
                return
            }
            seededInitialExpansion = true
            for node in rootNodes where node.isDirectory {
                expandedNodeIDs.insert(node.id)
            }
        }

        func restoreExpansionState() {
            guard let outlineView else {
                return
            }
            applyExpansion(to: rootNodes, parentExpanded: true, outlineView: outlineView)
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

            for ancestor in nodePath.dropLast() where ancestor.isDirectory {
                expandedNodeIDs.insert(ancestor.id)
                outlineView.expandItem(ancestor)
            }

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
                }
                applyExpansion(
                    to: node.children,
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
            return node.isDirectory && !node.children.isEmpty
        }

        func outlineView(
            _ outlineView: NSOutlineView,
            shouldExpandItem item: Any
        ) -> Bool {
            if let node = item as? FileTreeNode {
                expandedNodeIDs.insert(node.id)
            }
            return true
        }

        func outlineView(
            _ outlineView: NSOutlineView,
            shouldCollapseItem item: Any
        ) -> Bool {
            if let node = item as? FileTreeNode {
                expandedNodeIDs.remove(node.id)
            }
            return true
        }

        func outlineViewSelectionDidChange(_ notification: Notification) {
            guard
                !suppressSelectionEvents,
                let outlineView,
                outlineView.selectedRow >= 0,
                let node = outlineView.item(atRow: outlineView.selectedRow) as? FileTreeNode
            else {
                parent.selectedNodeID = nil
                return
            }
            parent.selectedNodeID = node.id
        }

        func outlineView(
            _ outlineView: NSOutlineView,
            viewFor tableColumn: NSTableColumn?,
            item: Any
        ) -> NSView? {
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
