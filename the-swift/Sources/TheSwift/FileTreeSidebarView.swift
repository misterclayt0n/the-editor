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

// MARK: - Public SwiftUI entry point

struct FileTreeSidebarView: View {
    let snapshot: FileTreeSnapshot
    let onSetExpanded: (String, Bool) -> Void
    let onSelectPath: (String) -> Void
    let onOpenSelected: () -> Void
    @State private var baselineTopInset: CGFloat?
    @State private var nativeTabBarVisible: Bool = false

    var body: some View {
        GeometryReader { proxy in
            let topInset = proxy.safeAreaInsets.top
            let baseline = baselineTopInset ?? topInset
            let compensation = max(0, topInset - baseline)
            NavigatorSidebarView(
                snapshot: snapshot,
                onSetExpanded: onSetExpanded,
                onSelectPath: onSelectPath,
                onOpenSelected: onOpenSelected
            )
            .padding(.top, -compensation)
            .background(
                WindowTabBarVisibilityProbe { visible in
                    nativeTabBarVisible = visible
                    if !visible {
                        learnBaseline(from: topInset)
                    }
                }
                .frame(width: 0, height: 0)
                .allowsHitTesting(false)
            )
            .onAppear {
                DispatchQueue.main.async {
                    learnBaseline(from: topInset)
                }
            }
            .onChange(of: topInset) { newValue in
                learnBaseline(from: newValue)
            }
        }
    }

    private func learnBaseline(from topInset: CGFloat) {
        guard !nativeTabBarVisible else { return }
        guard topInset > 1 else { return }
        if let baselineTopInset {
            if topInset > baselineTopInset {
                // Ignore sudden large jumps; those are the native window-tab bar
                // appearing and should be compensated, not learned as baseline.
                if topInset - baselineTopInset > 28 {
                    return
                }
                self.baselineTopInset = topInset
            }
        } else {
            baselineTopInset = topInset
        }
    }
}

private struct WindowTabBarVisibilityProbe: NSViewRepresentable {
    let onVisibilityChanged: (Bool) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(onVisibilityChanged: onVisibilityChanged)
    }

    func makeNSView(context: Context) -> SidebarWindowProbeView {
        let view = SidebarWindowProbeView(frame: .zero)
        view.onWindowChanged = { [weak coordinator = context.coordinator] window in
            coordinator?.attach(window: window)
        }
        return view
    }

    func updateNSView(_ nsView: SidebarWindowProbeView, context: Context) {
        context.coordinator.onVisibilityChanged = onVisibilityChanged
        context.coordinator.attach(window: nsView.window)
    }

    static func dismantleNSView(_ nsView: SidebarWindowProbeView, coordinator: Coordinator) {
        coordinator.invalidate()
        nsView.onWindowChanged = nil
    }

    final class Coordinator: NSObject {
        var onVisibilityChanged: (Bool) -> Void
        private weak var window: NSWindow?
        private weak var observedTabGroup: NSWindowTabGroup?
        private var windowTabGroupObservation: NSKeyValueObservation?
        private var tabGroupVisibleObservation: NSKeyValueObservation?

        init(onVisibilityChanged: @escaping (Bool) -> Void) {
            self.onVisibilityChanged = onVisibilityChanged
        }

        func attach(window: NSWindow?) {
            if self.window !== window {
                self.window = window
                rebindWindowObservers()
            }
            emitVisibility()
        }

        func invalidate() {
            windowTabGroupObservation?.invalidate()
            tabGroupVisibleObservation?.invalidate()
            windowTabGroupObservation = nil
            tabGroupVisibleObservation = nil
            observedTabGroup = nil
            window = nil
        }

        private func rebindWindowObservers() {
            windowTabGroupObservation?.invalidate()
            tabGroupVisibleObservation?.invalidate()
            windowTabGroupObservation = nil
            tabGroupVisibleObservation = nil
            observedTabGroup = nil

            guard let window else { return }
            windowTabGroupObservation = window.observe(\.tabGroup, options: [.initial, .new]) { [weak self] window, _ in
                self?.observeTabGroup(window.tabGroup)
                self?.emitVisibility()
            }
        }

        private func observeTabGroup(_ tabGroup: NSWindowTabGroup?) {
            if observedTabGroup === tabGroup {
                return
            }
            tabGroupVisibleObservation?.invalidate()
            tabGroupVisibleObservation = nil
            observedTabGroup = tabGroup
            guard let tabGroup else { return }
            tabGroupVisibleObservation = tabGroup.observe(\.isTabBarVisible, options: [.initial, .new]) { [weak self] tabGroup, _ in
                self?.onVisibilityChanged(tabGroup.isTabBarVisible)
            }
        }

        private func emitVisibility() {
            onVisibilityChanged(window?.tabGroup?.isTabBarVisible ?? false)
        }
    }
}

private final class SidebarWindowProbeView: NSView {
    var onWindowChanged: ((NSWindow?) -> Void)?

    override var intrinsicContentSize: NSSize {
        .zero
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        onWindowChanged?(window)
    }

    override func viewDidMoveToSuperview() {
        super.viewDidMoveToSuperview()
        onWindowChanged?(window)
    }
}

// MARK: - NSOutlineView subclass

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

// MARK: - Custom row view (Xcode-style inset rounded selection)

// macOS 11+ sourceList style already renders an inset rounded selection.
// This subclass makes that explicit so the rendering is consistent across
// any future style changes and gives us control over the exact geometry.
private final class FileTreeRowView: NSTableRowView {
    override func drawSelection(in dirtyRect: NSRect) {
        guard selectionHighlightStyle != .none else { return }
        // 5pt horizontal inset, 1.5pt vertical — matches Xcode's navigator rows.
        let rect = bounds.insetBy(dx: 5, dy: 1.5)
        let path = NSBezierPath(roundedRect: rect, xRadius: 6, yRadius: 6)
        (isEmphasized
            ? NSColor.selectedContentBackgroundColor
            : NSColor.unemphasizedSelectedContentBackgroundColor
        ).setFill()
        path.fill()
    }
}

// MARK: - Container (transparent — NavigationSplitView owns the sidebar material)

// No NSVisualEffectView, no custom header. NavigationSplitView applies the
// correct macOS .sidebar material to its column automatically. The tree starts
// flush with the window chrome — zero extra ceremony.
private final class NavigatorSidebarContainerView: NSView {
    let scrollView = NSScrollView()
    let outlineView = FileTreeOutlineView()
    let nameColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("name"))

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
        // Keep sidebar row origin stable across native window-tab/titlebar changes.
        scrollView.automaticallyAdjustsContentInsets = false
        // Minimal top inset — just enough so the first row breathes a little.
        scrollView.contentInsets = NSEdgeInsets(top: 4, left: 0, bottom: 4, right: 0)

        outlineView.translatesAutoresizingMaskIntoConstraints = false
        outlineView.headerView = nil
        outlineView.floatsGroupRows = false
        outlineView.indentationMarkerFollowsCell = true
        outlineView.backgroundColor = .clear
        // Xcode-matching row metrics.
        outlineView.rowHeight = 22
        outlineView.intercellSpacing = NSSize(width: 0, height: 0)
        outlineView.indentationPerLevel = 13

        nameColumn.title = "Name"
        outlineView.addTableColumn(nameColumn)
        outlineView.outlineTableColumn = nameColumn
        outlineView.columnAutoresizingStyle = .lastColumnOnlyAutoresizingStyle

        if #available(macOS 11.0, *) {
            outlineView.style = .sourceList
        }

        scrollView.documentView = outlineView
        scrollView.verticalScroller?.controlSize = .small
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

// MARK: - NSViewRepresentable

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
            if let window = outlineView.window {
                DispatchQueue.main.async {
                    KeyCaptureFocusBridge.shared.reclaim(in: window)
                }
            }
        }

        // MARK: - Data source

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

        // MARK: - Delegate

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

        // Provide our custom row view for the inset rounded selection.
        func outlineView(_ outlineView: NSOutlineView, rowViewForItem item: Any) -> NSTableRowView? {
            FileTreeRowView()
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
            let cell = (outlineView.makeView(withIdentifier: identifier, owner: nil) as? FileTreeCellView)
                ?? FileTreeCellView(identifier: identifier)

            cell.configure(node: node)
            return cell
        }
    }
}

// MARK: - Cell view

private final class FileTreeCellView: NSTableCellView {
    private let iconView = NSImageView(frame: .zero)
    private let label = NSTextField(labelWithString: "")

    convenience init(identifier: NSUserInterfaceItemIdentifier) {
        self.init(frame: .zero)
        self.identifier = identifier

        iconView.translatesAutoresizingMaskIntoConstraints = false
        // Never upscale symbols — render at natural size, centered.
        iconView.imageScaling = .scaleProportionallyDown
        iconView.imageAlignment = .alignCenter

        label.translatesAutoresizingMaskIntoConstraints = false
        label.lineBreakMode = .byTruncatingTail
        label.usesSingleLineMode = true
        label.font = .systemFont(ofSize: 12)
        label.textColor = .labelColor

        addSubview(iconView)
        addSubview(label)
        // Wire NSTableCellView outlets — AppKit auto-manages label color on selection.
        self.imageView = iconView
        self.textField = label

        NSLayoutConstraint.activate([
            // 14×14 pt icon, 2px from the cell's leading edge (after indent).
            iconView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 2),
            iconView.centerYAnchor.constraint(equalTo: centerYAnchor),
            iconView.widthAnchor.constraint(equalToConstant: 14),
            iconView.heightAnchor.constraint(equalToConstant: 14),

            // 4px gap after icon, 4px trailing margin.
            label.leadingAnchor.constraint(equalTo: iconView.trailingAnchor, constant: 4),
            label.centerYAnchor.constraint(equalTo: centerYAnchor),
            label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -4),
        ])
    }

    func configure(node: FileTreeNode) {
        label.stringValue = node.name
        let (image, tint) = Self.iconAndTint(for: node)
        iconView.image = image
        if #available(macOS 11.0, *) {
            iconView.contentTintColor = tint
        }
    }

    // MARK: - Icons

    private static func iconAndTint(for node: FileTreeNode) -> (NSImage?, NSColor?) {
        if node.isDirectory {
            return (folderIcon(), nil)
        }
        let ext = (node.path as NSString).pathExtension.lowercased()
        return fileIconAndTint(ext: ext, path: node.path)
    }

    private static func folderIcon() -> NSImage? {
        guard #available(macOS 11.0, *) else {
            let img = NSImage(named: NSImage.folderName)
            img?.size = NSSize(width: 14, height: 14)
            return img
        }
        let sizeConfig = NSImage.SymbolConfiguration(pointSize: 12, weight: .regular)
        if #available(macOS 12.0, *) {
            // Two-tone blue matching Xcode's folder style: deeper tab, lighter body.
            let colorConfig = NSImage.SymbolConfiguration(paletteColors: [
                NSColor(calibratedRed: 0.12, green: 0.40, blue: 0.82, alpha: 1.0),
                NSColor(calibratedRed: 0.32, green: 0.60, blue: 0.94, alpha: 1.0),
            ])
            return NSImage(systemSymbolName: "folder.fill", accessibilityDescription: nil)?
                .withSymbolConfiguration(sizeConfig.applying(colorConfig))
        }
        return NSImage(systemSymbolName: "folder.fill", accessibilityDescription: nil)?
            .withSymbolConfiguration(sizeConfig)
    }

    private static func fileIconAndTint(ext: String, path: String) -> (NSImage?, NSColor?) {
        guard #available(macOS 11.0, *) else {
            let img = NSWorkspace.shared.icon(forFile: path)
            img.size = NSSize(width: 14, height: 14)
            return (img, nil)
        }
        let (symbolName, tint) = symbolAndTint(forExt: ext)
        let config = NSImage.SymbolConfiguration(pointSize: 11, weight: .regular)
        let image = NSImage(systemSymbolName: symbolName, accessibilityDescription: nil)?
            .withSymbolConfiguration(config)
        return (image, tint)
    }

    // Each extension maps to an SF Symbol + tint. Colors are calibrated for the
    // dark sidebar and follow the conventions established by Xcode and VS Code.
    @available(macOS 11.0, *)
    private static func symbolAndTint(forExt ext: String) -> (String, NSColor) {
        switch ext {

        // ── Swift ──────────────────────────────────────────────────────────
        case "swift":
            return ("swift",
                    NSColor(calibratedRed: 0.94, green: 0.32, blue: 0.22, alpha: 0.92))

        // ── Rust ───────────────────────────────────────────────────────────
        case "rs":
            return ("curlybraces",
                    NSColor(calibratedRed: 0.80, green: 0.28, blue: 0.12, alpha: 0.92))

        // ── C / Objective-C ────────────────────────────────────────────────
        case "c", "m":
            return ("c.square",
                    NSColor(calibratedRed: 0.42, green: 0.68, blue: 0.98, alpha: 0.92))
        case "h", "hh":
            return ("c.square",
                    NSColor(calibratedRed: 0.42, green: 0.68, blue: 0.98, alpha: 0.82))
        case "cpp", "cc", "cxx", "mm":
            return ("plus.square",
                    NSColor(calibratedRed: 0.38, green: 0.64, blue: 0.96, alpha: 0.92))
        case "hpp", "hxx":
            return ("plus.square",
                    NSColor(calibratedRed: 0.38, green: 0.64, blue: 0.96, alpha: 0.82))

        // ── JavaScript / TypeScript ────────────────────────────────────────
        case "js", "jsx", "mjs", "cjs":
            return ("curlybraces.square",
                    NSColor(calibratedRed: 0.96, green: 0.82, blue: 0.20, alpha: 0.92))
        case "ts", "tsx", "mts", "cts":
            return ("curlybraces.square",
                    NSColor(calibratedRed: 0.20, green: 0.50, blue: 0.86, alpha: 0.92))

        // ── Python ─────────────────────────────────────────────────────────
        case "py", "pyx", "pyi":
            return ("terminal",
                    NSColor(calibratedRed: 0.24, green: 0.60, blue: 0.82, alpha: 0.92))

        // ── Go ─────────────────────────────────────────────────────────────
        case "go":
            return ("g.square",
                    NSColor(calibratedRed: 0.00, green: 0.68, blue: 0.84, alpha: 0.92))

        // ── Ruby ───────────────────────────────────────────────────────────
        case "rb", "rake", "gemspec":
            return ("r.square",
                    NSColor(calibratedRed: 0.88, green: 0.20, blue: 0.18, alpha: 0.92))

        // ── Java / Kotlin ──────────────────────────────────────────────────
        case "java":
            return ("j.square",
                    NSColor(calibratedRed: 0.92, green: 0.44, blue: 0.18, alpha: 0.92))
        case "kt", "kts":
            return ("curlybraces",
                    NSColor(calibratedRed: 0.62, green: 0.40, blue: 0.90, alpha: 0.92))

        // ── Markdown ───────────────────────────────────────────────────────
        case "md", "markdown", "mdx":
            return ("doc.richtext",
                    NSColor(calibratedRed: 0.58, green: 0.74, blue: 0.96, alpha: 0.92))

        // ── Web ────────────────────────────────────────────────────────────
        case "html", "htm", "xhtml":
            return ("globe",
                    NSColor(calibratedRed: 0.90, green: 0.38, blue: 0.16, alpha: 0.92))
        case "css", "scss", "sass", "less":
            return ("paintbrush",
                    NSColor(calibratedRed: 0.18, green: 0.48, blue: 0.94, alpha: 0.92))
        case "xml", "xsl":
            return ("chevron.left.slash.chevron.right",
                    NSColor(calibratedRed: 0.60, green: 0.62, blue: 0.65, alpha: 0.92))
        case "svg":
            return ("photo",
                    NSColor(calibratedRed: 0.78, green: 0.52, blue: 0.92, alpha: 0.92))

        // ── Data / Config ──────────────────────────────────────────────────
        case "json", "jsonc":
            return ("curlybraces",
                    NSColor(calibratedRed: 0.82, green: 0.78, blue: 0.48, alpha: 0.92))
        case "toml":
            return ("gearshape",
                    NSColor(calibratedRed: 0.65, green: 0.65, blue: 0.68, alpha: 0.92))
        case "yaml", "yml":
            return ("gearshape",
                    NSColor(calibratedRed: 0.58, green: 0.80, blue: 0.64, alpha: 0.92))
        case "ini", "cfg", "conf", "config":
            return ("gearshape",
                    NSColor(calibratedRed: 0.62, green: 0.62, blue: 0.65, alpha: 0.92))

        // ── Nix ────────────────────────────────────────────────────────────
        case "nix":
            return ("snowflake",
                    NSColor(calibratedRed: 0.44, green: 0.72, blue: 0.94, alpha: 0.92))

        // ── Shell ──────────────────────────────────────────────────────────
        case "sh", "bash", "zsh", "fish", "command":
            return ("terminal",
                    NSColor(calibratedRed: 0.30, green: 0.82, blue: 0.42, alpha: 0.92))

        // ── Images ─────────────────────────────────────────────────────────
        case "png", "jpg", "jpeg", "gif", "webp", "ico", "bmp", "tiff", "heic":
            return ("photo",
                    NSColor(calibratedRed: 0.72, green: 0.50, blue: 0.90, alpha: 0.92))

        // ── Locks / manifests ──────────────────────────────────────────────
        case "lock":
            return ("lock.doc",
                    NSColor(calibratedRed: 0.60, green: 0.60, blue: 0.63, alpha: 0.92))

        // ── Compiled / binary ──────────────────────────────────────────────
        case "dylib", "so", "a", "o":
            return ("cpu",
                    NSColor(calibratedRed: 0.58, green: 0.58, blue: 0.62, alpha: 0.92))

        // ── Default ────────────────────────────────────────────────────────
        default:
            return ("doc",
                    NSColor(calibratedRed: 0.62, green: 0.62, blue: 0.65, alpha: 0.92))
        }
    }
}
