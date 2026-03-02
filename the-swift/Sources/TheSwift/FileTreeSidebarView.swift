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

struct FileTreeSidebarView: View {
    let snapshot: FileTreeSnapshot
    let onSetExpanded: (String, Bool) -> Void
    let onSelectPath: (String) -> Void
    let onOpenSelected: () -> Void

    var body: some View {
        FileTreeListView(
            snapshot: snapshot,
            onSetExpanded: onSetExpanded,
            onSelectPath: onSelectPath,
            onOpenSelected: onOpenSelected
        )
    }
}

// MARK: - Data model

private struct FileTreeItem: Identifiable {
    let id: String
    let path: String
    let name: String
    let isDirectory: Bool
    let hasUnloadedChildren: Bool
    var children: [FileTreeItem] = []
}

private func buildTree(from snapshots: [FileTreeNodeSnapshot]) -> [FileTreeItem] {
    guard !snapshots.isEmpty else { return [] }

    var parentIndex: [Int: Int] = [:]
    var childrenIndices: [Int: [Int]] = [:]
    var rootIndices: [Int] = []
    var stack: [(depth: Int, index: Int)] = []

    for i in 0..<snapshots.count {
        let depth = snapshots[i].depth
        while let last = stack.last, last.depth >= depth { stack.removeLast() }
        if let p = stack.last {
            parentIndex[i] = p.index
        } else {
            rootIndices.append(i)
        }
        stack.append((depth, i))
    }
    for (child, parent) in parentIndex {
        childrenIndices[parent, default: []].append(child)
    }

    func makeItem(_ i: Int) -> FileTreeItem {
        let s = snapshots[i]
        var item = FileTreeItem(
            id: s.id,
            path: s.path,
            name: s.name,
            isDirectory: s.isDirectory,
            hasUnloadedChildren: s.hasUnloadedChildren
        )
        item.children = (childrenIndices[i] ?? []).map { makeItem($0) }
        return item
    }
    return rootIndices.map { makeItem($0) }
}

// MARK: - Icon system

private func iconInfo(for item: FileTreeItem) -> (symbolName: String, color: Color) {
    if item.isDirectory {
        return ("folder.fill", .blue)
    }
    let ext = (item.name as NSString).pathExtension.lowercased()
    switch ext {
    case "swift":
        return ("swift", .orange)
    case "rs":
        return ("doc.text", Color(red: 0.76, green: 0.35, blue: 0.14))
    case "py":
        return ("doc.text", .yellow)
    case "js", "ts", "jsx", "tsx", "mjs", "cjs":
        return ("doc.text", .yellow)
    case "c", "h":
        return ("doc.text", .blue)
    case "cpp", "cc", "cxx", "hpp", "hxx":
        return ("doc.text", .blue)
    case "go":
        return ("doc.text", .teal)
    case "java", "kt", "kts":
        return ("doc.text", .orange)
    case "rb":
        return ("doc.text", .red)
    case "cs":
        return ("doc.text", .purple)
    case "toml":
        return ("gearshape", .secondary)
    case "yaml", "yml":
        return ("list.bullet.indent", .secondary)
    case "json":
        return ("curlybraces", .secondary)
    case "xml", "plist":
        return ("chevron.left.forwardslash.chevron.right", .secondary)
    case "md", "markdown":
        return ("doc.richtext", .secondary)
    case "txt":
        return ("doc.text", .secondary)
    case "pdf":
        return ("doc.fill", .red)
    case "png", "jpg", "jpeg", "gif", "svg", "webp", "ico", "tiff", "heic":
        return ("photo", .secondary)
    case "sh", "bash", "zsh", "fish":
        return ("terminal", .secondary)
    case "lock":
        return ("lock", .secondary)
    case "env":
        return ("lock.fill", .secondary)
    default:
        break
    }

    let name = item.name.lowercased()
    if name == ".gitignore" || name == ".gitattributes" {
        return ("eye.slash", .secondary)
    }
    if name.hasPrefix(".env") {
        return ("lock.fill", .secondary)
    }
    if name == "makefile" || name == "dockerfile" || name == "containerfile" {
        return ("shippingbox", .secondary)
    }
    if name == "license" || name == "licence" || name == "license.md" || name == "license.txt" {
        return ("doc.badge.checkmark", .secondary)
    }
    return ("doc", .secondary)
}

// MARK: - Views

private struct FileTreeItemRow: View {
    let item: FileTreeItem

    var body: some View {
        let info = iconInfo(for: item)
        Label {
            Text(item.name)
                .font(.system(size: 11))
                .lineLimit(1)
                .truncationMode(.middle)
        } icon: {
            Image(systemName: info.symbolName)
                .foregroundStyle(info.color)
                .imageScale(.small)
        }
    }
}

private struct FileTreeNodeView: View {
    let item: FileTreeItem
    @Binding var expandedIDs: Set<String>
    let onExpand: (String, Bool) -> Void
    let onOpen: () -> Void

    var body: some View {
        if item.isDirectory {
            DisclosureGroup(
                isExpanded: Binding(
                    get: { expandedIDs.contains(item.id) },
                    set: { open in
                        if open {
                            expandedIDs.insert(item.id)
                        } else {
                            expandedIDs.remove(item.id)
                        }
                        onExpand(item.path, open)
                    }
                )
            ) {
                ForEach(item.children) { child in
                    FileTreeNodeView(
                        item: child,
                        expandedIDs: $expandedIDs,
                        onExpand: onExpand,
                        onOpen: onOpen
                    )
                }
            } label: {
                FileTreeItemRow(item: item)
            }
            .tag(item.path)
        } else {
            FileTreeItemRow(item: item)
                .tag(item.path)
                .onTapGesture(count: 2) { onOpen() }
        }
    }
}

private struct FileTreeListView: View {
    let snapshot: FileTreeSnapshot
    let onSetExpanded: (String, Bool) -> Void
    let onSelectPath: (String) -> Void
    let onOpenSelected: () -> Void

    @State private var rootItems: [FileTreeItem] = []
    @State private var expandedIDs: Set<String> = []
    @State private var selectedPath: String? = nil
    @State private var suppressSelectionCallback = false
    @State private var suppressExpansionCallback = false

    var body: some View {
        List(selection: $selectedPath) {
            ForEach(rootItems) { item in
                FileTreeNodeView(
                    item: item,
                    expandedIDs: $expandedIDs,
                    onExpand: { path, expanded in
                        guard !suppressExpansionCallback else { return }
                        DispatchQueue.main.async { onSetExpanded(path, expanded) }
                    },
                    onOpen: onOpenSelected
                )
            }
        }
        .listStyle(.sidebar)
        .onChange(of: selectedPath) { newPath in
            guard !suppressSelectionCallback, let newPath else { return }
            DispatchQueue.main.async { onSelectPath(newPath) }
        }
        .onChange(of: snapshot.selectedPath) { newPath in
            guard selectedPath != newPath else { return }
            suppressSelectionCallback = true
            selectedPath = newPath
            suppressSelectionCallback = false
        }
        .onChange(of: snapshot.expandedNodeIDs) { newIDs in
            guard expandedIDs != newIDs else { return }
            suppressExpansionCallback = true
            expandedIDs = newIDs
            suppressExpansionCallback = false
        }
        .onChange(of: snapshot.refreshGeneration) { _ in
            rootItems = buildTree(from: snapshot.nodes)
        }
        .onAppear {
            rootItems = buildTree(from: snapshot.nodes)
            expandedIDs = snapshot.expandedNodeIDs
            selectedPath = snapshot.selectedPath
        }
    }
}
