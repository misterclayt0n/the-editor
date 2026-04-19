import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct EditorChromeModel {
    let document: EditorDocumentChrome
    let statusBar: EditorStatusBarState
    let backgroundColor: NSColor

    static let empty = EditorChromeModel(
        document: .empty,
        statusBar: .empty,
        backgroundColor: .windowBackgroundColor
    )

    func matches(_ other: EditorChromeModel) -> Bool {
        document == other.document
            && statusBar == other.statusBar
            && backgroundColor.isEqual(other.backgroundColor)
    }
}

private enum EditorSidebarMode: String {
    case files
    case buffers
}

struct EditorChromeView: View {
    @ObservedObject var controller: EditorSurfaceController

    @AppStorage("swift.fileTreeSidebarWidth") private var storedFileTreeWidth: Double = 280
    @AppStorage("swift.sidebar.mode") private var storedSidebarModeRaw: String = EditorSidebarMode.files.rawValue
    @GestureState private var fileTreeDragTranslation: CGFloat = 0
    @State private var isFileTreeResizeActive = false
    @State private var titlebarPadding: CGFloat = 32
    @State private var titlebarLeadingInset: CGFloat = 72


    private let minimumFileTreeWidth: CGFloat = 180
    private let maximumFileTreeWidth: CGFloat = 460

    private var sidebarMode: EditorSidebarMode {
        EditorSidebarMode(rawValue: storedSidebarModeRaw) ?? .files
    }

    init(controller: EditorSurfaceController) {
        self.controller = controller
    }

    var body: some View {
        GeometryReader { geometry in
            let effectiveTitlebarPadding = alignedTitlebarPadding(totalHeight: geometry.size.height)

            HStack(spacing: 0) {
                if controller.fileTree.isVisible {
                    sidebarColumn(topScrimHeight: effectiveTitlebarPadding)
                        .ignoresSafeArea(.all, edges: .top)
                    EditorSidebarResizeHandle(
                        color: sidebarTheme.separatorColor,
                        edge: .trailing,
                        gesture: fileTreeResizeGesture
                    )
                        .ignoresSafeArea(.all, edges: .top)
                }

                mainColumn(titlebarInset: effectiveTitlebarPadding)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .ignoresSafeArea()
            // Sidebar visibility must not animate layout: each frame would reconfigure the Rust
            // viewport and run a full snapshot for every column boundary crossed.
            .animation(nil, value: controller.fileTree.isVisible)
            .background(
                EditorWindowChromeAccessor(
                    chrome: controller.chrome,
                    fileTreeVisible: controller.fileTree.isVisible,
                    sidebarMode: sidebarMode,
                    sidebarTitle: sidebarTitle,
                    titlebarPadding: $titlebarPadding,
                    onToggleFileTree: controller.toggleFileTree,
                    onSelectSidebarMode: selectSidebarMode,
                    onOpenTerminal: controller.openTerminalInActivePane,
                    onOpenAgentPane: controller.openAgentInActivePane
                )
            )
            .background {
                let viewportText: String = {
                    guard let scene = controller.scene else { return "nil" }
                    return "\(scene.info.viewportWidth)x\(scene.info.viewportHeight)"
                }()
                let signature = [
                    "chrome",
                    String(format: "size=%.1fx%.1f", geometry.size.width, geometry.size.height),
                    String(format: "safeArea=top:%.1f bottom:%.1f", geometry.safeAreaInsets.top, geometry.safeAreaInsets.bottom),
                    String(format: "titlebarPadding=%.1f", titlebarPadding),
                    String(format: "alignedTitlebarPadding=%.1f", effectiveTitlebarPadding),
                    String(format: "fileTreeWidth=%.1f", fileTreeWidth),
                    "fileTreeVisible=\(controller.fileTree.isVisible)",
                    "sidebarMode=\(sidebarMode.rawValue)",
                    "viewport=\(viewportText)"
                ].joined(separator: " ")

                Color.clear
                    .onAppear {
                        layoutDebugLog(signature)
                    }
                    .onChange(of: signature) { _, newValue in
                        layoutDebugLog(newValue)
                    }
            }
            .overlay(alignment: .bottom) {
                if let pendingKeys = controller.pendingKeys {
                    EditorPendingKeyIndicatorView(pendingKeys: pendingKeys)
                        .padding(.bottom, 38)
                        .transition(.move(edge: .bottom).combined(with: .opacity))
                }
            }
            .animation(.spring(response: 0.24, dampingFraction: 0.88), value: controller.pendingKeys?.pendingDisplay)
        }
    }

    private func sidebarColumn(topScrimHeight: CGFloat) -> some View {
        Group {
            switch sidebarMode {
            case .files:
                EditorFileTreeSidebarView(
                    tree: controller.fileTree,
                    theme: sidebarTheme,
                    topScrimHeight: topScrimHeight,
                    onSelectIndex: controller.clickFileTreeIndex,
                    onActivateIndex: controller.activateFileTreeIndex,
                    onVisibleRowsChanged: controller.setFileTreeVisibleRows,
                    onScrollOffsetChanged: controller.syncFileTreeScrollOffset,
                    onFocusSidebar: { controller.setFileTreeActive(true) }
                )
                .onAppear {
                    controller.setFileTreeActive(true)
                    let widthText = String(format: "%.1f", fileTreeWidth)
                    scrollPerfLog("fileTree.sidebar appear rows=\(controller.fileTree.rows.count) width=\(widthText)")
                }
                .onDisappear {
                    scrollPerfLog("fileTree.sidebar disappear")
                }
            case .buffers:
                EditorOpenItemsSidebarView(
                    openItems: controller.openItems,
                    scene: controller.scene,
                    uniqueBufferCount: controller.bufferTabs.tabs.count,
                    theme: sidebarTheme,
                    topScrimHeight: topScrimHeight,
                    onActivateItem: controller.activateOpenItem,
                    onCloseItem: controller.closeOpenItem,
                    onFocusSidebar: { controller.setFileTreeActive(false) }
                )
                .onAppear {
                    controller.setFileTreeActive(false)
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(nsColor: sidebarTheme.backgroundColor))
        .frame(width: fileTreeWidth)
        .background(Color(nsColor: sidebarTheme.backgroundColor))
    }

    private func mainColumn(titlebarInset: CGFloat) -> some View {
        VStack(spacing: 0) {
            ZStack(alignment: .topLeading) {
                EditorSurfaceRepresentable(
                    controller: controller,
                    agentBackgroundColor: sidebarTheme.backgroundColor,
                    agentSelectionColor: sidebarTheme.selectionColor,
                    isRenderingSuspended: false
                )
                .frame(maxWidth: .infinity, maxHeight: .infinity)

                EditorDiagnosticsOverlayView(controller: controller)
                EditorDocsPanelsView(controller: controller)
                EditorCompletionMenuView(controller: controller)
                EditorPaneItemStripsOverlayView(controller: controller)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)

            EditorStatusAccessoryView(chrome: controller.chrome, mode: controller.currentMode, controller: controller)
        }
        .padding(.top, titlebarInset)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .overlay(alignment: .top) {
            customTitlebar(height: titlebarInset)
        }
        .environment(\.colorScheme, controller.chrome.backgroundColor.isLightColor ? .light : .dark)
    }

    private var sidebarTheme: EditorFileTreeSidebarTheme {
        EditorFileTreeSidebarTheme.resolve(scene: controller.scene, chrome: controller.chrome)
    }

    private var titlebarForegroundColor: Color {
        controller.chrome.backgroundColor.isLightColor
            ? Color.black.opacity(0.78)
            : Color.white.opacity(0.82)
    }

    private func alignedTitlebarPadding(totalHeight: CGFloat) -> CGFloat {
        _ = totalHeight
        return titlebarPadding
    }

    private var titlebarDirectoryPath: String? {
        if let absolutePath = controller.chrome.document.absolutePath, !absolutePath.isEmpty {
            let url = URL(fileURLWithPath: absolutePath)
            let directory = url.deletingLastPathComponent().path
            return directory.isEmpty ? nil : directory
        }
        if let root = controller.fileTree.root, !root.isEmpty {
            return root
        }
        return nil
    }

    private var titlebarDirectoryDisplayText: String {
        guard let directory = titlebarDirectoryPath else {
            return controller.chrome.document.name
        }
        return (directory as NSString).abbreviatingWithTildeInPath
    }

    /// Content-level titlebar overlay that fills the safe area at the top of the main column.
    /// Provides a draggable strip with folder icon and current working directory text.
    @ViewBuilder
    private func customTitlebar(height: CGFloat) -> some View {
        GeometryReader { geometry in
            let topInset = geometry.safeAreaInsets.top
            let titleText = titlebarDirectoryDisplayText
            ZStack {
                // Window drag handle - allows dragging from the titlebar area
                EditorWindowDragHandleView()

                EditorTitlebarLeadingInsetReader(inset: $titlebarLeadingInset)
                    .allowsHitTesting(false)

                HStack(spacing: 8) {
                    if let directory = titlebarDirectoryPath {
                        EditorSidebarTitleIconView(directory: directory)
                            .padding(.leading, -6)
                    }

                    Text(titleText)
                        .font(.system(size: 13, weight: .bold))
                        .foregroundStyle(titlebarForegroundColor)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .allowsHitTesting(false)

                    Spacer()
                }
                .frame(height: 28)
                .padding(.top, 2)
                .padding(.leading, controller.fileTree.isVisible ? 12 : titlebarLeadingInset)
                .padding(.trailing, 8)
            }
            .frame(height: max(height, topInset, 28), alignment: .bottom)
            .frame(maxWidth: .infinity)
            .contentShape(Rectangle())
            .background(EditorTitlebarDoubleClickMonitorView())
            .background(EditorTitlebarLayerBackground(backgroundColor: controller.chrome.backgroundColor))
            .overlay(alignment: .bottom) {
                Rectangle()
                    .fill(Color(nsColor: .separatorColor))
                    .frame(height: 1)
            }
        }
    }

    private var fileTreeWidth: CGFloat {
        clampFileTreeWidth(CGFloat(storedFileTreeWidth) + fileTreeDragTranslation)
    }

    private var sidebarTitle: String {
        switch sidebarMode {
        case .files:
            guard let root = controller.fileTree.root, !root.isEmpty else { return "Workspace" }
            let url = URL(fileURLWithPath: root)
            let name = url.lastPathComponent
            return name.isEmpty ? root : name
        case .buffers:
            return "Open Items"
        }
    }

    private var fileTreeResizeGesture: some Gesture {
        DragGesture(minimumDistance: 0, coordinateSpace: .global)
            .updating($fileTreeDragTranslation) { value, state, _ in
                let translation = value.location.x - value.startLocation.x
                state = translation
                let startWidth = CGFloat(storedFileTreeWidth)
                let proposedWidth = startWidth + translation
                let startText = String(format: "%.1f", startWidth)
                let translationText = String(format: "%.1f", translation)
                let proposedText = String(format: "%.1f", proposedWidth)
                let clampedText = String(format: "%.1f", clampFileTreeWidth(proposedWidth))
                let startXText = String(format: "%.1f", value.startLocation.x)
                let locationXText = String(format: "%.1f", value.location.x)
                scrollPerfLog(
                    "fileTree.resize updating start=\(startText) translation=\(translationText) proposed=\(proposedText) clamped=\(clampedText) startX=\(startXText) currentX=\(locationXText)"
                )
            }
            .onChanged { _ in
                guard !isFileTreeResizeActive else { return }
                isFileTreeResizeActive = true
                controller.beginInteractiveResize(reason: "sidebar")
            }
            .onEnded { value in
                let translation = value.location.x - value.startLocation.x
                let committedWidth = clampFileTreeWidth(CGFloat(storedFileTreeWidth) + translation)
                let storedText = String(format: "%.1f", storedFileTreeWidth)
                let translationText = String(format: "%.1f", translation)
                let committedText = String(format: "%.1f", committedWidth)
                let startXText = String(format: "%.1f", value.startLocation.x)
                let locationXText = String(format: "%.1f", value.location.x)
                scrollPerfLog(
                    "fileTree.resize ended stored=\(storedText) translation=\(translationText) committed=\(committedText) startX=\(startXText) currentX=\(locationXText)"
                )
                storedFileTreeWidth = committedWidth
                if isFileTreeResizeActive {
                    isFileTreeResizeActive = false
                    controller.endInteractiveResize(reason: "sidebar")
                }
            }
    }

    private func clampFileTreeWidth(_ width: CGFloat) -> CGFloat {
        min(max(width, minimumFileTreeWidth), maximumFileTreeWidth)
    }

    private func selectSidebarMode(_ mode: EditorSidebarMode) {
        guard sidebarMode != mode else {
            controller.setFileTreeActive(mode == .files)
            return
        }
        withAnimation(.easeInOut(duration: 0.14)) {
            storedSidebarModeRaw = mode.rawValue
        }
        controller.setFileTreeActive(mode == .files)
    }
}

private struct EditorFileTreeSidebarTheme {
    let backgroundColor: NSColor
    let headerColor: NSColor
    let separatorColor: NSColor
    let selectionColor: NSColor
    let hoverColor: NSColor

    static func resolve(scene: EditorRenderScene?, chrome: EditorChromeModel) -> Self {
        let editorBackground = chromeBackgroundColor(base: scene?.backgroundColor ?? chrome.backgroundColor)
        let sidebarBackground = chromeBackgroundColor(base: scene?.gutterBackgroundColor ?? editorBackground)
        let selectionColor = chromeBackgroundColor(
            base: scene?.info.selectionColor?.color
                ?? sidebarBackground.blended(withFraction: 0.28, of: .systemBlue)
                ?? .systemBlue
        )
        let headerColor = sidebarBackground.adjustedBrightness(by: sidebarBackground.isLightColor ? -0.035 : 0.05)
        let separatorColor = sidebarBackground.adjustedBrightness(by: sidebarBackground.isLightColor ? -0.18 : 0.22)
            .withAlphaComponent(0.95)
        let hoverColor = selectionColor.withAlphaComponent(max(selectionColor.alphaComponent * 0.32, 0.08))
        return Self(
            backgroundColor: sidebarBackground,
            headerColor: headerColor,
            separatorColor: separatorColor,
            selectionColor: selectionColor,
            hoverColor: hoverColor
        )
    }
}

private struct ChromeForegroundColors {
    let primary: Color
    let secondary: Color
    let tertiary: Color
    let primaryNS: NSColor
    let secondaryNS: NSColor
    let tertiaryNS: NSColor

    static func forBackground(_ base: NSColor) -> ChromeForegroundColors {
        let bg = chromeBackgroundColor(base: base)
        let light = bg.isLightColor
        if light {
            let p = NSColor.black.withAlphaComponent(0.88)
            let s = NSColor.black.withAlphaComponent(0.55)
            let t = NSColor.black.withAlphaComponent(0.38)
            return ChromeForegroundColors(
                primary: Color(nsColor: p),
                secondary: Color(nsColor: s),
                tertiary: Color(nsColor: t),
                primaryNS: p,
                secondaryNS: s,
                tertiaryNS: t
            )
        } else {
            let p = NSColor.white.withAlphaComponent(0.92)
            let s = NSColor.white.withAlphaComponent(0.62)
            let t = NSColor.white.withAlphaComponent(0.45)
            return ChromeForegroundColors(
                primary: Color(nsColor: p),
                secondary: Color(nsColor: s),
                tertiary: Color(nsColor: t),
                primaryNS: p,
                secondaryNS: s,
                tertiaryNS: t
            )
        }
    }
}

private struct EditorPendingKeyIndicatorView: View {
    let pendingKeys: EditorPendingKeyState

    @State private var isShowingPopover = false

    var body: some View {
        Button(action: togglePopover) {
            HStack(alignment: .center, spacing: 4) {
                EditorKeySequenceCapsulesView(sequence: pendingKeys.pendingDisplay)
                PendingIndicator()
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background {
                Capsule()
                    .fill(.regularMaterial)
                    .overlay {
                        Capsule()
                            .strokeBorder(Color.primary.opacity(0.15), lineWidth: 1)
                    }
                    .shadow(color: .black.opacity(0.2), radius: 8, y: 2)
            }
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .popover(isPresented: $isShowingPopover, arrowEdge: .bottom) {
            EditorPendingKeyPopoverView(pendingKeys: pendingKeys)
        }
        .accessibilityLabel("Pending keys")
        .accessibilityValue(pendingKeys.pendingDisplay)
        .accessibilityHint("Show possible key combinations")
    }

    private func togglePopover() {
        isShowingPopover.toggle()
    }
}

private struct EditorPendingKeyPopoverView: View {
    let pendingKeys: EditorPendingKeyState

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(pendingKeys.scope ?? "Pending keys")
                .font(.headline)

            HStack(spacing: 8) {
                EditorKeySequenceCapsulesView(sequence: pendingKeys.pendingDisplay)
                Text(summaryText)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }

            Divider()

            ScrollView(.vertical, showsIndicators: true) {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(pendingKeys.outcomes) { outcome in
                        EditorPendingKeyOutcomeRowView(outcome: outcome)
                    }
                }
            }
            .frame(minWidth: 320, idealWidth: 360, maxWidth: 420, minHeight: 140, idealHeight: 260, maxHeight: 360)
        }
        .padding()
        .frame(maxWidth: 420, alignment: .leading)
    }

    private var summaryText: String {
        let immediateCount = pendingKeys.immediateCount
        let outcomeCount = pendingKeys.outcomeCount
        let nextKeyNoun = immediateCount == 1 ? "next key" : "next keys"
        let outcomeNoun = outcomeCount == 1 ? "outcome" : "outcomes"
        return "\(immediateCount) \(nextKeyNoun) • \(outcomeCount) \(outcomeNoun)"
    }
}

private struct EditorPendingKeyOutcomeRowView: View {
    let outcome: EditorPendingKeyOutcome

    var body: some View {
        HStack(spacing: 12) {
            EditorKeySequenceCapsulesView(sequence: outcome.pathDisplay)
                .frame(minWidth: 84, alignment: .leading)

            Text(outcome.label)
                .font(.system(size: 12))
                .foregroundStyle(.primary)
                .lineLimit(1)

            Spacer(minLength: 0)
        }
        .padding(.horizontal, 2)
        .padding(.vertical, 7)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(outcome.pathDisplay), \(outcome.label)")
    }
}

private struct EditorKeySequenceCapsulesView: View {
    let sequence: String

    private var tokens: [String] {
        sequence
            .split(separator: " ")
            .map(String.init)
            .filter { !$0.isEmpty }
    }

    var body: some View {
        HStack(spacing: 4) {
            ForEach(Array(tokens.enumerated()), id: \.offset) { _, token in
                KeyCap(prettyKeyToken(token))
            }
        }
        .accessibilityElement(children: .combine)
    }
}

private struct KeyCap: View {
    let text: String

    init(_ text: String) {
        self.text = text
    }

    var body: some View {
        Text(verbatim: text)
            .font(.system(size: 12, weight: .medium, design: .rounded))
            .padding(.horizontal, 5)
            .padding(.vertical, 2)
            .background(
                RoundedRectangle(cornerRadius: 4)
                    .fill(Color(NSColor.controlBackgroundColor))
                    .shadow(color: .black.opacity(0.12), radius: 0.5, y: 0.5)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 4)
                    .strokeBorder(Color.primary.opacity(0.15), lineWidth: 0.5)
            )
    }
}

private struct PendingIndicator: View {
    @State private var animationPhase: Double = 0

    var body: some View {
        TimelineView(.animation) { context in
            HStack(spacing: 2) {
                ForEach(0..<3, id: \.self) { index in
                    Circle()
                        .fill(Color.secondary)
                        .frame(width: 4, height: 4)
                        .opacity(dotOpacity(for: index))
                }
            }
            .onChange(of: context.date.timeIntervalSinceReferenceDate, initial: true) { _, newValue in
                animationPhase = newValue
            }
        }
    }

    private func dotOpacity(for index: Int) -> Double {
        let phase = animationPhase
        let offset = Double(index) / 3.0
        let wave = sin((phase + offset) * .pi * 2)
        return 0.3 + 0.7 * ((wave + 1) / 2)
    }
}

private struct EditorFileTreeSidebarView: View {
    let tree: EditorFileTreeState
    let theme: EditorFileTreeSidebarTheme
    let topScrimHeight: CGFloat
    let onSelectIndex: (Int) -> Void
    let onActivateIndex: (Int) -> Void
    let onVisibleRowsChanged: (Int) -> Void
    let onScrollOffsetChanged: (Int) -> Void
    let onFocusSidebar: () -> Void

    var body: some View {
        EditorFileTreeListRepresentable(
            tree: tree,
            theme: theme,
            onSelectIndex: onSelectIndex,
            onActivateIndex: onActivateIndex,
            onVisibleRowsChanged: onVisibleRowsChanged,
            onScrollOffsetChanged: onScrollOffsetChanged,
            onFocusSidebar: onFocusSidebar
        )
        .padding(.top, 8)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(nsColor: theme.backgroundColor))
        .overlay(alignment: .top) {
            // Top scrim to visually clear the traffic light area when sidebar extends into titlebar
            EditorSidebarTopScrim(height: topScrimHeight, backgroundColor: theme.backgroundColor)
        }
        .environment(\.colorScheme, theme.backgroundColor.isLightColor ? .light : .dark)
    }
}

private struct EditorOpenItemsSidebarView: View {
    let openItems: EditorPaneOpenItemsState
    let scene: EditorRenderScene?
    let uniqueBufferCount: Int
    let theme: EditorFileTreeSidebarTheme
    let topScrimHeight: CGFloat
    let onActivateItem: (EditorPaneOpenItemRow) -> Void
    let onCloseItem: (EditorPaneOpenItemRow) -> Void
    let onFocusSidebar: () -> Void

    private var chromeForeground: ChromeForegroundColors {
        ChromeForegroundColors.forBackground(theme.backgroundColor)
    }

    var body: some View {
        ScrollView(.vertical, showsIndicators: true) {
            LazyVStack(alignment: .leading, spacing: 10) {
                    if openItems.groups.isEmpty {
                        VStack(alignment: .leading, spacing: 6) {
                            Text("No Open Items")
                                .font(.system(size: 12, weight: .semibold))
                                .foregroundStyle(chromeForeground.primary)
                            Text("Open buffers and pane-local items will appear here.")
                                .font(.system(size: 11))
                                .foregroundStyle(chromeForeground.secondary)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, 14)
                        .padding(.vertical, 16)
                    } else {
                        ForEach(Array(openItems.groups.enumerated()), id: \.element.id) { groupIndex, group in
                            VStack(alignment: .leading, spacing: 4) {
                                EditorOpenItemsGroupHeaderView(
                                    title: paneLocationLabel(for: group.paneID, groupIndex: groupIndex, scene: scene),
                                    count: group.items.count,
                                    isActivePane: group.isActivePane,
                                    theme: theme
                                )

                                ForEach(group.items) { item in
                                    EditorOpenItemSidebarRowView(
                                        item: item,
                                        theme: theme,
                                        canClose: canClose(item),
                                        onActivate: {
                                            onFocusSidebar()
                                            onActivateItem(item)
                                        },
                                        onClose: {
                                            onCloseItem(item)
                                        }
                                    )
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 1)
                                }
                            }
                        }
                    }
            }
            .padding(.top, 8)
            .padding(.bottom, 6)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Color(nsColor: theme.backgroundColor))
        .overlay(alignment: .top) {
            // Top scrim to visually clear the traffic light area when sidebar extends into titlebar
            EditorSidebarTopScrim(height: topScrimHeight, backgroundColor: theme.backgroundColor)
        }
        .environment(\.colorScheme, theme.backgroundColor.isLightColor ? .light : .dark)
    }

    private func canClose(_ item: EditorPaneOpenItemRow) -> Bool {
        switch item.kind {
        case .buffer:
            return uniqueBufferCount > 1
        case .terminal, .agent:
            return true
        }
    }
}

private struct EditorOpenItemsGroupHeaderView: View {
    let title: String
    let count: Int
    let isActivePane: Bool
    let theme: EditorFileTreeSidebarTheme

    private var chromeForeground: ChromeForegroundColors {
        ChromeForegroundColors.forBackground(theme.backgroundColor)
    }

    var body: some View {
        HStack(spacing: 6) {
            Circle()
                .fill(Color(nsColor: isActivePane ? theme.selectionColor : theme.separatorColor))
                .frame(width: 5, height: 5)
            Text(title)
                .font(.system(size: 10, weight: .bold))
                .foregroundStyle(isActivePane ? chromeForeground.primary : chromeForeground.secondary)
                .tracking(0.4)
            Text("\(count)")
                .font(.system(size: 10, weight: .medium))
                .foregroundStyle(chromeForeground.secondary.opacity(0.85))
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 12)
        .padding(.top, 4)
        .padding(.bottom, 4)
    }
}

private struct EditorOpenItemSidebarRowView: View {
    let item: EditorPaneOpenItemRow
    let theme: EditorFileTreeSidebarTheme
    let canClose: Bool
    let onActivate: () -> Void
    let onClose: () -> Void

    @State private var isHovered = false

    private var chromeForeground: ChromeForegroundColors {
        ChromeForegroundColors.forBackground(theme.backgroundColor)
    }

    var body: some View {
        HStack(spacing: 0) {
            Rectangle()
                .fill(Color(nsColor: leadingRailColor))
                .frame(width: 2)
                .opacity(item.isActive ? 1 : 0)

            Button(action: onActivate) {
                HStack(spacing: 8) {
                    openItemIcon(size: 11)
                        .foregroundStyle(Color(nsColor: iconColor))
                        .frame(width: 12)

                    VStack(alignment: .leading, spacing: 1) {
                        Text(item.title)
                            .font(.system(size: 12, weight: item.isActive ? .semibold : .regular))
                            .foregroundStyle(rowTextColor)
                            .lineLimit(1)
                        if let subtitle = item.subtitle, !subtitle.isEmpty {
                            Text(subtitle)
                                .font(.system(size: 10, weight: .medium))
                                .foregroundStyle(chromeForeground.secondary)
                                .lineLimit(1)
                        }
                    }

                    Spacer(minLength: 8)

                    EditorSidebarRowDecorationsView(
                        vcsKind: item.vcsKind,
                        diagnosticSeverity: item.diagnosticSeverity
                    )

                    if item.isModified {
                        Circle()
                            .fill(Color(nsColor: item.isActive ? theme.selectionColor : chromeForeground.secondaryNS).opacity(item.isActive ? 0.95 : 0.82))
                            .frame(width: 5, height: 5)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.leading, 8)
                .padding(.trailing, 6)
                .padding(.vertical, 5)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            Button(action: onClose) {
                Image(systemName: "xmark")
                    .font(.system(size: 9, weight: .bold))
                    .foregroundStyle(Color(nsColor: closeButtonForegroundColor))
                    .frame(width: 18, height: 18)
                    .background(
                        Circle()
                            .fill(Color(nsColor: closeButtonBackgroundColor))
                    )
            }
            .buttonStyle(.plain)
            .disabled(!canClose)
            .opacity(closeButtonOpacity)
            .padding(.trailing, 6)
            .help(canClose ? "Close \(item.title)" : "Cannot close the last buffer")
            .accessibilityLabel(canClose ? "Close \(item.title)" : "Cannot close the last buffer")
        }
        .background(selectionBackground)
        .clipShape(.rect(cornerRadius: 6))
        .contentShape(Rectangle())
        .onHover { hovering in
            isHovered = hovering
        }
        .help(item.filePath ?? item.title)
        .accessibilityElement(children: .contain)
    }

    @ViewBuilder
    private var selectionBackground: some View {
        RoundedRectangle(cornerRadius: 6, style: .continuous)
            .fill(backgroundColor)
    }

    private var backgroundColor: Color {
        if item.isActive {
            return Color(nsColor: theme.selectionColor).opacity(0.24)
        }
        if isHovered {
            return Color(nsColor: theme.hoverColor)
        }
        return .clear
    }

    private var leadingRailColor: NSColor {
        theme.selectionColor
    }

    @ViewBuilder
    private func openItemIcon(size: CGFloat) -> some View {
        if item.kind == .agent {
            Image(systemName: "brain.head.profile")
                .font(.system(size: size, weight: .semibold))
        } else {
            EditorSemanticIconView(iconName: item.iconName, size: size)
        }
    }

    private var iconColor: NSColor {
        if item.isActive {
            return chromeForeground.primaryNS
        }
        return chromeForeground.tertiaryNS
    }

    private var closeButtonForegroundColor: NSColor {
        if !canClose {
            return chromeForeground.tertiaryNS
        }
        if item.isActive || isHovered {
            return chromeForeground.primaryNS
        }
        return chromeForeground.secondaryNS
    }

    private var closeButtonBackgroundColor: NSColor {
        if item.isActive {
            return theme.selectionColor.withAlphaComponent(0.18)
        }
        if isHovered {
            return theme.hoverColor.withAlphaComponent(0.95)
        }
        return NSColor.tertiaryLabelColor.withAlphaComponent(0.08)
    }

    private var closeButtonOpacity: Double {
        if !canClose {
            return 0.3
        }
        if item.isActive || isHovered {
            return 0.95
        }
        return 0.55
    }

    private var rowTextColor: Color {
        item.isActive ? chromeForeground.primary : chromeForeground.primary.opacity(0.88)
    }
}

private struct EditorPaneItemTransferData: Codable, Hashable {
    let sourcePaneID: UInt
    let kindRaw: UInt8
    let itemID: UInt

    var kind: EditorOpenItemKind? {
        EditorOpenItemKind(rawValue: kindRaw)
    }
}

private struct EditorPaneItemTabDropTarget: Equatable {
    let paneID: UInt
    let index: Int
}

private enum EditorPaneItemDropZone: Equatable {
    case center
    case left
    case right
    case top
    case bottom

    var direction: EditorPaneDropDirection? {
        switch self {
        case .center:
            return nil
        case .left:
            return .left
        case .right:
            return .right
        case .top:
            return .up
        case .bottom:
            return .down
        }
    }
}

private func paneLocationLabel(for paneID: UInt, groupIndex: Int, scene: EditorRenderScene?) -> String {
    guard let scene,
          let pane = scene.panes.first(where: { $0.paneID == paneID })
    else {
        return "pane \(groupIndex + 1)"
    }

    let maxX = scene.panes.map { $0.x + $0.width }.max() ?? max(pane.x + pane.width, 1)
    let maxY = scene.panes.map { $0.y + $0.height }.max() ?? max(pane.y + pane.height, 1)
    let centerX = Double(pane.x) + Double(pane.width) / 2
    let centerY = Double(pane.y) + Double(pane.height) / 2
    let horizontal: String
    if centerX <= Double(maxX) * 0.35 {
        horizontal = "left"
    } else if centerX >= Double(maxX) * 0.65 {
        horizontal = "right"
    } else {
        horizontal = "center"
    }
    let vertical: String
    if centerY <= Double(maxY) * 0.45 {
        vertical = "top"
    } else {
        vertical = "bottom"
    }

    let arrow: String = switch (vertical, horizontal) {
    case ("top", "left"):
        "↖"
    case ("top", "right"):
        "↗"
    case ("bottom", "left"):
        "↙"
    case ("bottom", "right"):
        "↘"
    case ("top", _):
        "↑"
    case (_, "left"):
        "←"
    case (_, "right"):
        "→"
    default:
        "↓"
    }
    return "pane \(groupIndex + 1) \(arrow)"
}

private struct EditorPaneItemStripsOverlayView: View {
    private struct LayoutEntry: Identifiable {
        let groupIndex: Int
        let group: EditorPaneOpenItemGroup
        let frame: CGRect
        let contentFrame: CGRect
        let headerHeight: CGFloat
        let controlHeight: CGFloat
        let controlOriginY: CGFloat

        var id: UInt { group.id }
    }

    @ObservedObject var controller: EditorSurfaceController
    @State private var dragSourcePaneID: UInt?
    @State private var tabDropTarget: EditorPaneItemTabDropTarget?
    @State private var contentDropZones: [UInt: EditorPaneItemDropZone] = [:]

    // Tabs butt against the pane edge, cmux-style (no outer padding on the strip).
    private let horizontalInset: CGFloat = 0

    var body: some View {
        if let scene = controller.scene {
            let entries = layoutEntries(for: scene)
            if !entries.isEmpty {
                let theme = EditorFileTreeSidebarTheme.resolve(scene: scene, chrome: controller.chrome)
                GeometryReader { geometry in
                    ZStack(alignment: .topLeading) {
                        ForEach(entries) { entry in
                            let availableWidth = max(
                                min(entry.frame.width - horizontalInset * 2, geometry.size.width - entry.frame.minX - horizontalInset),
                                80
                            )
                            Rectangle()
                                .fill(Color(nsColor: theme.headerColor))
                                .frame(width: entry.frame.width, height: entry.headerHeight)
                                .overlay(alignment: .bottom) {
                                    Rectangle()
                                        .fill(Color(nsColor: theme.separatorColor).opacity(0.9))
                                        .frame(height: 1)
                                }
                                .offset(x: entry.frame.minX, y: entry.frame.minY)
                                .zIndex(1)

                            if dragSourcePaneID != nil {
                                EditorPaneContentDropLayer(
                                    paneID: entry.group.paneID,
                                    frame: entry.contentFrame,
                                    activeDropZone: Binding(
                                        get: { contentDropZones[entry.group.paneID] },
                                        set: { newValue in
                                            if let newValue {
                                                contentDropZones[entry.group.paneID] = newValue
                                            } else {
                                                contentDropZones.removeValue(forKey: entry.group.paneID)
                                            }
                                        }
                                    ),
                                    onHandleDrop: { transfer, zone in
                                        handleContentDrop(transfer: transfer, targetPaneID: entry.group.paneID, zone: zone)
                                    }
                                )
                                .zIndex(1.5)
                            }

                            EditorPaneItemTabStripView(
                                group: entry.group,
                                paneLabel: paneLocationLabel(for: entry.group.paneID, groupIndex: entry.groupIndex, scene: scene),
                                theme: theme,
                                controlHeight: entry.controlHeight,
                                dragSourcePaneID: dragSourcePaneID,
                                dropTarget: Binding(
                                    get: { tabDropTarget },
                                    set: { tabDropTarget = $0 }
                                ),
                                onCreateItemProvider: { item in
                                    createItemProvider(for: item)
                                },
                                onHandleDrop: { transfer, targetIndex in
                                    handleTabDrop(transfer: transfer, targetPaneID: entry.group.paneID, targetIndex: targetIndex)
                                },
                                canCloseItem: canClose,
                                onActivateItem: { item in
                                    if item.isActive {
                                        if item.kind == .buffer {
                                            controller.focusEditor()
                                        }
                                        return
                                    }
                                    controller.activateOpenItem(item)
                                    if item.kind == .buffer {
                                        controller.focusEditor()
                                    }
                                },
                                onCloseItem: { item in
                                    controller.closeOpenItem(item)
                                    if item.kind == .buffer {
                                        controller.focusEditor()
                                    }
                                }
                            )
                            .frame(width: availableWidth, height: entry.controlHeight, alignment: .leading)
                            .offset(
                                x: entry.frame.minX + horizontalInset,
                                y: entry.controlOriginY
                            )
                            .zIndex(entry.group.isActivePane ? 3 : 2)
                        }
                    }
                    .frame(width: geometry.size.width, height: geometry.size.height, alignment: .topLeading)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .allowsHitTesting(true)
            }
        }
    }

    private func layoutEntries(for scene: EditorRenderScene) -> [LayoutEntry] {
        Array(controller.openItems.groups.enumerated()).compactMap { groupIndex, group in
            guard let pane = scene.pane(id: group.paneID),
                  shouldShowTabStrip(for: group)
            else {
                return nil
            }
            let headerHeight = paneHeaderHeight(for: pane, group: group, in: scene)
            let controlHeight = paneTabControlHeight(headerHeight: headerHeight)
            return LayoutEntry(
                groupIndex: groupIndex,
                group: group,
                frame: paneFrame(for: pane, in: scene),
                contentFrame: scene.paneContentRect(for: pane),
                headerHeight: headerHeight,
                controlHeight: controlHeight,
                controlOriginY: paneTabControlOriginY(for: pane, headerHeight: headerHeight, controlHeight: controlHeight, in: scene)
            )
        }
    }

    private func shouldShowTabStrip(for group: EditorPaneOpenItemGroup) -> Bool {
        group.items.count > 1 || group.items.contains(where: { $0.kind != .buffer })
    }

    private func canClose(_ item: EditorPaneOpenItemRow) -> Bool {
        switch item.kind {
        case .buffer:
            return controller.bufferTabs.tabs.count > 1
        case .terminal, .agent:
            return true
        }
    }

    private func paneFrame(for pane: EditorSnapshotPane, in scene: EditorRenderScene) -> CGRect {
        scene.paneRect(for: pane)
    }

    private func paneHeaderHeight(for pane: EditorSnapshotPane, group: EditorPaneOpenItemGroup, in scene: EditorRenderScene) -> CGFloat {
        guard shouldShowTabStrip(for: group) else { return 0 }
        return min(EditorPaneTabBarMetrics.barHeight, paneFrame(for: pane, in: scene).height)
    }

    private func paneTabControlHeight(headerHeight: CGFloat) -> CGFloat {
        guard headerHeight > 0 else { return 0 }
        return min(EditorPaneTabBarMetrics.tabHeight, headerHeight)
    }

    private func paneTabControlOriginY(for pane: EditorSnapshotPane, headerHeight: CGFloat, controlHeight: CGFloat, in scene: EditorRenderScene) -> CGFloat {
        let rect = paneFrame(for: pane, in: scene)
        return rect.minY + max((headerHeight - controlHeight) * 0.5, 0)
    }

    private func createItemProvider(for item: EditorPaneOpenItemRow) -> NSItemProvider {
        dragSourcePaneID = item.paneID
        let transfer = EditorPaneItemTransferData(sourcePaneID: item.paneID, kindRaw: item.kind.rawValue, itemID: item.itemID)
        guard let data = try? JSONEncoder().encode(transfer),
              let string = String(data: data, encoding: .utf8)
        else {
            return NSItemProvider()
        }
        return NSItemProvider(object: string as NSString)
    }

    private func clearDragState() {
        dragSourcePaneID = nil
        tabDropTarget = nil
        contentDropZones.removeAll()
    }

    private func resolveOpenItem(from transfer: EditorPaneItemTransferData) -> EditorPaneOpenItemRow? {
        controller.openItems.groups
            .flatMap(\.items)
            .first(where: { $0.paneID == transfer.sourcePaneID && $0.itemID == transfer.itemID && $0.kind.rawValue == transfer.kindRaw })
    }

    private func handleTabDrop(transfer: EditorPaneItemTransferData, targetPaneID: UInt, targetIndex: Int) -> Bool {
        defer { clearDragState() }
        guard let item = resolveOpenItem(from: transfer) else { return false }
        controller.moveOpenItem(item, toPaneID: targetPaneID, atIndex: targetIndex)
        return true
    }

    private func handleContentDrop(transfer: EditorPaneItemTransferData, targetPaneID: UInt, zone: EditorPaneItemDropZone) -> Bool {
        defer { clearDragState() }
        guard let item = resolveOpenItem(from: transfer) else { return false }
        if zone == .center {
            let targetIndex = controller.openItems.groups.first(where: { $0.paneID == targetPaneID })?.items.count ?? 0
            controller.moveOpenItem(item, toPaneID: targetPaneID, atIndex: targetIndex)
            return true
        }
        guard let direction = zone.direction else { return false }
        controller.splitOpenItem(item, ontoPaneID: targetPaneID, direction: direction)
        return true
    }
}

private enum EditorPaneTabBarMetrics {
    static let barHeight: CGFloat = 33
    static let tabHeight: CGFloat = 32
    static let tabMinWidth: CGFloat = 140
    static let tabMaxWidth: CGFloat = 220
    static let tabHorizontalPadding: CGFloat = 12
    static let tabSpacing: CGFloat = 0
    static let activeIndicatorHeight: CGFloat = 2
    static let iconSize: CGFloat = 14
    static let titleFontSize: CGFloat = 12
    static let closeButtonSize: CGFloat = 16
    static let closeIconSize: CGFloat = 9
    static let dirtyIndicatorSize: CGFloat = 8
    static let contentSpacing: CGFloat = 6
    static let hoverDuration: Double = 0.1
}

private struct EditorPaneItemTabStripView: View {
    let group: EditorPaneOpenItemGroup
    let paneLabel: String
    let theme: EditorFileTreeSidebarTheme
    let controlHeight: CGFloat
    let dragSourcePaneID: UInt?
    @Binding var dropTarget: EditorPaneItemTabDropTarget?
    let onCreateItemProvider: (EditorPaneOpenItemRow) -> NSItemProvider
    let onHandleDrop: (EditorPaneItemTransferData, Int) -> Bool
    let canCloseItem: (EditorPaneOpenItemRow) -> Bool
    let onActivateItem: (EditorPaneOpenItemRow) -> Void
    let onCloseItem: (EditorPaneOpenItemRow) -> Void

    private var chromeForeground: ChromeForegroundColors {
        ChromeForegroundColors.forBackground(theme.backgroundColor)
    }

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: EditorPaneTabBarMetrics.tabSpacing) {
                ForEach(Array(group.items.enumerated()), id: \.element.id) { itemIndex, item in
                    EditorPaneItemTabView(
                        item: item,
                        targetIndex: itemIndex,
                        theme: theme,
                        chromeForeground: chromeForeground,
                        isActivePane: group.isActivePane,
                        controlHeight: controlHeight,
                        isDragSourcePane: dragSourcePaneID == group.paneID,
                        dropTarget: $dropTarget,
                        onCreateItemProvider: { onCreateItemProvider(item) },
                        onHandleDrop: { transfer, dropIndex in onHandleDrop(transfer, dropIndex) },
                        canClose: canCloseItem(item),
                        onActivate: { onActivateItem(item) },
                        onClose: { onCloseItem(item) }
                    )
                }
                EditorPaneTabDropZoneAtEnd(
                    paneID: group.paneID,
                    targetIndex: group.items.count,
                    dropTarget: $dropTarget,
                    onHandleDrop: onHandleDrop
                )
                Spacer(minLength: 0)
            }
        }
        .clipped()
        .help(helpText)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(helpText)
    }

    private var helpText: String {
        let titles = group.items.map(\.title).joined(separator: ", ")
        return "\(paneLabel): \(titles)"
    }
}

private struct EditorPaneItemTabView: View {
    let item: EditorPaneOpenItemRow
    let targetIndex: Int
    let theme: EditorFileTreeSidebarTheme
    let chromeForeground: ChromeForegroundColors
    let isActivePane: Bool
    let controlHeight: CGFloat
    let isDragSourcePane: Bool
    @Binding var dropTarget: EditorPaneItemTabDropTarget?
    let onCreateItemProvider: () -> NSItemProvider
    let onHandleDrop: (EditorPaneItemTransferData, Int) -> Bool
    let canClose: Bool
    let onActivate: () -> Void
    let onClose: () -> Void

    @State private var isHovered = false
    @State private var isCloseHovered = false

    var body: some View {
        HStack(spacing: EditorPaneTabBarMetrics.contentSpacing) {
            openItemIcon(size: EditorPaneTabBarMetrics.iconSize)
                .foregroundStyle(item.isActive ? chromeForeground.primary : chromeForeground.secondary)

            Text(item.title)
                .font(.system(size: EditorPaneTabBarMetrics.titleFontSize))
                .lineLimit(1)
                .truncationMode(.middle)
                .foregroundStyle(item.isActive ? chromeForeground.primary : chromeForeground.secondary)

            Spacer(minLength: 4)

            closeOrDirtyIndicator
        }
        .padding(.horizontal, EditorPaneTabBarMetrics.tabHorizontalPadding)
        .offset(y: item.isActive ? 0.5 : 0)
        .frame(
            minWidth: EditorPaneTabBarMetrics.tabMinWidth,
            maxWidth: EditorPaneTabBarMetrics.tabMaxWidth,
            minHeight: controlHeight,
            maxHeight: controlHeight
        )
        .padding(.bottom, item.isActive ? 1 : 0)
        .background(tabBackground)
        .contentShape(Rectangle())
        .onTapGesture(perform: onActivate)
        .onDrag {
            onCreateItemProvider()
        }
        .onDrop(
            of: [UTType.text],
            delegate: EditorPaneTabItemDropDelegate(
                paneID: item.paneID,
                targetIndex: targetIndex,
                dropTarget: $dropTarget,
                onHandleDrop: onHandleDrop
            )
        )
        .overlay(alignment: .leading) {
            if dropTarget == EditorPaneItemTabDropTarget(paneID: item.paneID, index: targetIndex) {
                EditorPaneTabDropIndicator()
            }
        }
        .onHover { hovering in
            withAnimation(.easeInOut(duration: EditorPaneTabBarMetrics.hoverDuration)) {
                isHovered = hovering
            }
        }
        .help(item.filePath ?? item.title)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(item.title)
    }

    private var showsCloseButton: Bool {
        guard canClose else { return false }
        return isHovered || isCloseHovered
    }

    @ViewBuilder
    private func openItemIcon(size: CGFloat) -> some View {
        if item.kind == .agent {
            Image(systemName: "brain.head.profile")
                .font(.system(size: size, weight: .semibold))
        } else {
            EditorSemanticIconView(iconName: item.iconName, size: size)
        }
    }

    @ViewBuilder
    private var tabBackground: some View {
        ZStack(alignment: .top) {
            if item.isActive {
                Rectangle()
                    .fill(Color(nsColor: theme.backgroundColor))
            } else if isHovered {
                Rectangle()
                    .fill(Color(nsColor: theme.hoverColor).opacity(isActivePane ? 0.8 : 0.6))
            } else {
                Color.clear
            }

            if item.isActive {
                Rectangle()
                    .fill(Color(nsColor: theme.selectionColor))
                    .frame(height: EditorPaneTabBarMetrics.activeIndicatorHeight)
            }

            HStack {
                Spacer()
                Rectangle()
                    .fill(Color(nsColor: theme.separatorColor).opacity(0.9))
                    .frame(width: 1)
            }
        }
    }

    @ViewBuilder
    private var closeOrDirtyIndicator: some View {
        ZStack {
            if item.isModified && !isHovered && !isCloseHovered {
                Circle()
                    .fill(chromeForeground.secondary.opacity(0.7))
                    .frame(
                        width: EditorPaneTabBarMetrics.dirtyIndicatorSize,
                        height: EditorPaneTabBarMetrics.dirtyIndicatorSize
                    )
            }

            if showsCloseButton {
                Button(action: onClose) {
                    Image(systemName: "xmark")
                        .font(.system(size: EditorPaneTabBarMetrics.closeIconSize, weight: .semibold))
                        .foregroundStyle(isCloseHovered ? chromeForeground.primary : chromeForeground.secondary)
                        .frame(
                            width: EditorPaneTabBarMetrics.closeButtonSize,
                            height: EditorPaneTabBarMetrics.closeButtonSize
                        )
                        .background(
                            Circle()
                                .fill(isCloseHovered ? Color(nsColor: theme.hoverColor).opacity(0.9) : .clear)
                        )
                }
                .buttonStyle(.plain)
                .onHover { hovering in
                    isCloseHovered = hovering
                }
                .help("Close \(item.title)")
                .accessibilityLabel("Close \(item.title)")
            }
        }
        .frame(
            width: EditorPaneTabBarMetrics.closeButtonSize,
            height: EditorPaneTabBarMetrics.closeButtonSize
        )
        .animation(.easeInOut(duration: EditorPaneTabBarMetrics.hoverDuration), value: isHovered)
        .animation(.easeInOut(duration: EditorPaneTabBarMetrics.hoverDuration), value: isCloseHovered)
    }
}

private struct EditorPaneTabDropIndicator: View {
    var body: some View {
        Capsule()
            .fill(Color.accentColor)
            .frame(width: 2, height: 20)
            .offset(x: -1)
    }
}

private struct EditorPaneTabDropZoneAtEnd: View {
    let paneID: UInt
    let targetIndex: Int
    @Binding var dropTarget: EditorPaneItemTabDropTarget?
    let onHandleDrop: (EditorPaneItemTransferData, Int) -> Bool

    var body: some View {
        Rectangle()
            .fill(Color.clear)
            .frame(width: 24, height: 32)
            .contentShape(Rectangle())
            .onDrop(
                of: [UTType.text],
                delegate: EditorPaneTabItemDropDelegate(
                    paneID: paneID,
                    targetIndex: targetIndex,
                    dropTarget: $dropTarget,
                    onHandleDrop: onHandleDrop
                )
            )
            .overlay(alignment: .leading) {
                if dropTarget == EditorPaneItemTabDropTarget(paneID: paneID, index: targetIndex) {
                    EditorPaneTabDropIndicator()
                }
            }
    }
}

private struct EditorPaneContentDropLayer: View {
    let paneID: UInt
    let frame: CGRect
    @Binding var activeDropZone: EditorPaneItemDropZone?
    let onHandleDrop: (EditorPaneItemTransferData, EditorPaneItemDropZone) -> Bool

    var body: some View {
        ZStack {
            Rectangle()
                .fill(Color.clear)
                .frame(width: frame.width, height: frame.height)
                .position(x: frame.midX, y: frame.midY)
                .onDrop(
                    of: [UTType.text],
                    delegate: EditorPaneContentDropDelegate(
                        paneID: paneID,
                        size: frame.size,
                        activeDropZone: $activeDropZone,
                        onHandleDrop: onHandleDrop
                    )
                )

            if let activeDropZone {
                EditorPaneContentDropPlaceholder(zone: activeDropZone, frame: frame)
            }
        }
    }
}

private struct EditorPaneContentDropPlaceholder: View {
    let zone: EditorPaneItemDropZone
    let frame: CGRect

    var body: some View {
        let padding: CGFloat = 4
        let rect: CGRect = switch zone {
        case .center:
            CGRect(x: frame.minX + padding, y: frame.minY + padding, width: frame.width - padding * 2, height: frame.height - padding * 2)
        case .left:
            CGRect(x: frame.minX + padding, y: frame.minY + padding, width: frame.width / 2 - padding, height: frame.height - padding * 2)
        case .right:
            CGRect(x: frame.midX, y: frame.minY + padding, width: frame.width / 2 - padding, height: frame.height - padding * 2)
        case .top:
            CGRect(x: frame.minX + padding, y: frame.minY + padding, width: frame.width - padding * 2, height: frame.height / 2 - padding)
        case .bottom:
            CGRect(x: frame.minX + padding, y: frame.midY, width: frame.width - padding * 2, height: frame.height / 2 - padding)
        }

        return RoundedRectangle(cornerRadius: 8, style: .continuous)
            .fill(Color.accentColor.opacity(0.22))
            .overlay {
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .stroke(Color.accentColor, lineWidth: 2)
            }
            .frame(width: rect.width, height: rect.height)
            .position(x: rect.midX, y: rect.midY)
            .allowsHitTesting(false)
    }
}

private struct EditorPaneTabItemDropDelegate: DropDelegate {
    let paneID: UInt
    let targetIndex: Int
    @Binding var dropTarget: EditorPaneItemTabDropTarget?
    let onHandleDrop: (EditorPaneItemTransferData, Int) -> Bool

    func performDrop(info: DropInfo) -> Bool {
        dropTarget = nil
        guard let provider = info.itemProviders(for: [.text]).first else { return false }
        provider.loadItem(forTypeIdentifier: UTType.text.identifier, options: nil) { item, _ in
            let transfer = decodeTransfer(item)
            DispatchQueue.main.async {
                guard let transfer else { return }
                _ = onHandleDrop(transfer, targetIndex)
            }
        }
        return true
    }

    func dropEntered(info: DropInfo) {
        dropTarget = EditorPaneItemTabDropTarget(paneID: paneID, index: targetIndex)
    }

    func dropExited(info: DropInfo) {
        if dropTarget == EditorPaneItemTabDropTarget(paneID: paneID, index: targetIndex) {
            dropTarget = nil
        }
    }

    func dropUpdated(info: DropInfo) -> DropProposal? {
        DropProposal(operation: .move)
    }

    func validateDrop(info: DropInfo) -> Bool {
        info.hasItemsConforming(to: [.text])
    }
}

private struct EditorPaneContentDropDelegate: DropDelegate {
    let paneID: UInt
    let size: CGSize
    @Binding var activeDropZone: EditorPaneItemDropZone?
    let onHandleDrop: (EditorPaneItemTransferData, EditorPaneItemDropZone) -> Bool

    private func zone(for location: CGPoint) -> EditorPaneItemDropZone {
        let edgeRatio: CGFloat = 0.25
        let horizontalEdge = max(80, size.width * edgeRatio)
        let verticalEdge = max(80, size.height * edgeRatio)
        if location.x < horizontalEdge {
            return .left
        } else if location.x > size.width - horizontalEdge {
            return .right
        } else if location.y < verticalEdge {
            return .top
        } else if location.y > size.height - verticalEdge {
            return .bottom
        } else {
            return .center
        }
    }

    func performDrop(info: DropInfo) -> Bool {
        let zone = zone(for: info.location)
        activeDropZone = nil
        guard let provider = info.itemProviders(for: [.text]).first else { return false }
        provider.loadItem(forTypeIdentifier: UTType.text.identifier, options: nil) { item, _ in
            let transfer = decodeTransfer(item)
            DispatchQueue.main.async {
                guard let transfer else { return }
                _ = onHandleDrop(transfer, zone)
            }
        }
        return true
    }

    func dropEntered(info: DropInfo) {
        activeDropZone = zone(for: info.location)
    }

    func dropExited(info: DropInfo) {
        activeDropZone = nil
    }

    func dropUpdated(info: DropInfo) -> DropProposal? {
        activeDropZone = zone(for: info.location)
        return DropProposal(operation: .move)
    }

    func validateDrop(info: DropInfo) -> Bool {
        info.hasItemsConforming(to: [.text])
    }
}

private func decodeTransfer(_ item: NSSecureCoding?) -> EditorPaneItemTransferData? {
    let string: String?
    if let data = item as? Data {
        string = String(data: data, encoding: .utf8)
    } else if let nsString = item as? NSString {
        string = nsString as String
    } else if let str = item as? String {
        string = str
    } else {
        string = nil
    }
    guard let string,
          let data = string.data(using: .utf8)
    else {
        return nil
    }
    return try? JSONDecoder().decode(EditorPaneItemTransferData.self, from: data)
}

private struct EditorFileTreeListRepresentable: NSViewRepresentable {
    let tree: EditorFileTreeState
    let theme: EditorFileTreeSidebarTheme
    let onSelectIndex: (Int) -> Void
    let onActivateIndex: (Int) -> Void
    let onVisibleRowsChanged: (Int) -> Void
    let onScrollOffsetChanged: (Int) -> Void
    let onFocusSidebar: () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }

    func makeNSView(context: Context) -> NSScrollView {
        context.coordinator.makeScrollView()
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        context.coordinator.update(parent: self)
    }

    @MainActor
    final class Coordinator: NSObject, NSTableViewDataSource, NSTableViewDelegate {
        private struct RenderState: Equatable {
            let isVisible: Bool
            let root: String?
            let rows: [EditorFileTreeRow]

            init(tree: EditorFileTreeState) {
                isVisible = tree.isVisible
                root = tree.root
                rows = tree.rows
            }
        }

        private struct ThemeSignature: Equatable {
            let backgroundColor: UInt32
            let headerColor: UInt32
            let separatorColor: UInt32
            let selectionColor: UInt32
            let hoverColor: UInt32

            init(theme: EditorFileTreeSidebarTheme) {
                backgroundColor = Self.signature(theme.backgroundColor)
                headerColor = Self.signature(theme.headerColor)
                separatorColor = Self.signature(theme.separatorColor)
                selectionColor = Self.signature(theme.selectionColor)
                hoverColor = Self.signature(theme.hoverColor)
            }

            private static func signature(_ color: NSColor) -> UInt32 {
                let resolved = color.usingColorSpace(.deviceRGB) ?? color
                let red = UInt32((resolved.redComponent * 255).rounded())
                let green = UInt32((resolved.greenComponent * 255).rounded())
                let blue = UInt32((resolved.blueComponent * 255).rounded())
                let alpha = UInt32((resolved.alphaComponent * 255).rounded())
                return (red << 24) | (green << 16) | (blue << 8) | alpha
            }
        }

        var parent: EditorFileTreeListRepresentable

        private weak var scrollView: NSScrollView?
        private weak var tableView: NSTableView?
        private var isApplyingSnapshot = false
        private var suppressScrollSync = false
        private var lastReportedVisibleRows: Int = 1
        private var lastReportedTopRow: Int = 0
        private var lastRenderState: RenderState?
        private var lastThemeSignature: ThemeSignature?
        private var lastRequestedScrollOffset: Int?
        private var pendingUserScrollOffset: Int?
        private var recentUserScrollDeadline: CFAbsoluteTime = 0
        private var hoveredRowIndex: Int?

        init(parent: EditorFileTreeListRepresentable) {
            self.parent = parent
        }

        func makeScrollView() -> NSScrollView {
            let scrollView = NSScrollView()
            scrollView.drawsBackground = false
            scrollView.borderType = .noBorder
            scrollView.hasVerticalScroller = true
            scrollView.hasHorizontalScroller = false
            scrollView.autohidesScrollers = true

            let tableView = NSTableView()
            tableView.headerView = nil
            tableView.backgroundColor = .clear
            tableView.focusRingType = .none
            tableView.selectionHighlightStyle = .none
            tableView.intercellSpacing = .zero
            tableView.allowsEmptySelection = true
            tableView.rowHeight = 24
            tableView.delegate = self
            tableView.dataSource = self

            let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("fileTree"))
            column.resizingMask = .autoresizingMask
            tableView.addTableColumn(column)

            scrollView.documentView = tableView
            scrollView.contentView.postsBoundsChangedNotifications = true
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(boundsDidChange),
                name: NSView.boundsDidChangeNotification,
                object: scrollView.contentView
            )

            self.scrollView = scrollView
            self.tableView = tableView
            update(parent: parent)
            return scrollView
        }

        func update(parent: EditorFileTreeListRepresentable) {
            self.parent = parent
            applySnapshotIfNeeded()
        }

        func numberOfRows(in tableView: NSTableView) -> Int {
            max(parent.tree.rows.count, parent.tree.rows.isEmpty ? 1 : 0)
        }

        func tableView(_ tableView: NSTableView, heightOfRow row: Int) -> CGFloat {
            parent.tree.rows.isEmpty ? 56 : 24
        }

        func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
            let identifier = NSUserInterfaceItemIdentifier("EditorFileTreeCell")
            let cell = (tableView.makeView(withIdentifier: identifier, owner: nil) as? EditorFileTreeTableCellView)
                ?? EditorFileTreeTableCellView(identifier: identifier)
            if parent.tree.rows.isEmpty {
                cell.configureEmpty(theme: parent.theme)
                return cell
            }
            let rowValue = parent.tree.rows[row]
            cell.configure(
                row: rowValue,
                index: row,
                isHovered: hoveredRowIndex == row,
                theme: parent.theme,
                onFocusSidebar: parent.onFocusSidebar,
                onSelectIndex: parent.onSelectIndex,
                onActivateIndex: parent.onActivateIndex,
                onHoverRowChanged: { [weak self] hoveredRow in
                    self?.setHoveredRow(hoveredRow)
                }
            )
            return cell
        }

        private func applySnapshotIfNeeded() {
            guard let tableView, let scrollView else { return }
            let renderState = RenderState(tree: parent.tree)
            let themeSignature = ThemeSignature(theme: parent.theme)
            let width = max(scrollView.contentSize.width, 1)
            if let column = tableView.tableColumns.first, abs(column.width - width) > 0.5 {
                column.width = width
            }
            let needsReload = renderState != lastRenderState || themeSignature != lastThemeSignature
            if needsReload {
                isApplyingSnapshot = true
                lastRenderState = renderState
                lastThemeSignature = themeSignature
                tableView.reloadData()
                DispatchQueue.main.async { [weak self] in
                    guard let self else { return }
                    self.isApplyingSnapshot = false
                    self.syncVisibleGeometry(reason: "snapshot")
                    self.reconcileHoveredRowWithPointer()
                    self.applyProgrammaticScrollIfNeeded(reason: "snapshot")
                }
                return
            }
            syncVisibleGeometry(reason: "update")
            reconcileHoveredRowWithPointer()
            applyProgrammaticScrollIfNeeded(reason: "update")
        }

        @objc
        private func boundsDidChange() {
            syncVisibleGeometry(reason: suppressScrollSync ? "programmatic-scroll" : "user-scroll")
            reconcileHoveredRowWithPointer()
        }

        private func applyProgrammaticScrollIfNeeded(reason: String) {
            guard let tableView, let scrollView, !parent.tree.rows.isEmpty else { return }
            let targetIndex = resolvedProgrammaticTargetIndex()
            let currentTopRow = visibleTopRow(in: tableView, scrollView: scrollView)
            if currentTopRow == targetIndex {
                lastRequestedScrollOffset = targetIndex
                if pendingUserScrollOffset == targetIndex {
                    pendingUserScrollOffset = nil
                }
                return
            }
            guard lastRequestedScrollOffset != targetIndex else { return }
            suppressScrollSync = true
            lastRequestedScrollOffset = targetIndex
            let targetRect = tableView.rect(ofRow: targetIndex)
            scrollView.contentView.scroll(to: NSPoint(x: 0, y: targetRect.minY))
            scrollView.reflectScrolledClipView(scrollView.contentView)
            scrollPerfLog(
                "fileTree.appkitScroll applied reason=\(reason) target=\(targetIndex) previousTop=\(currentTopRow) rows=\(parent.tree.rows.count)"
            )
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                self.suppressScrollSync = false
                self.syncVisibleGeometry(reason: "programmatic-scroll-verify")
            }
        }

        private func syncVisibleGeometry(reason: String) {
            guard let tableView, let scrollView else { return }
            let visibleRect = scrollView.contentView.documentVisibleRect
            let visibleRange = tableView.rows(in: visibleRect)
            let topRow = max(visibleRange.location, 0)
            let visibleRows = stableVisibleRowCount(in: tableView, visibleRect: visibleRect)
            scrollPerfLog(
                "fileTree.appkitVisible reason=\(reason) top=\(topRow) visibleRows=\(visibleRows) rustScroll=\(parent.tree.scrollOffset) rows=\(parent.tree.rows.count)"
            )
            if lastReportedVisibleRows != visibleRows {
                lastReportedVisibleRows = visibleRows
                parent.onVisibleRowsChanged(visibleRows)
            }
            guard !isApplyingSnapshot else { return }
            if !suppressScrollSync, lastReportedTopRow != topRow {
                lastReportedTopRow = topRow
                pendingUserScrollOffset = topRow
                recentUserScrollDeadline = CFAbsoluteTimeGetCurrent() + 0.20
                parent.onScrollOffsetChanged(topRow)
            }
            if topRow == parent.tree.scrollOffset {
                lastRequestedScrollOffset = topRow
                pendingUserScrollOffset = nil
            }
        }

        private func visibleTopRow(in tableView: NSTableView, scrollView: NSScrollView) -> Int {
            let range = tableView.rows(in: scrollView.contentView.documentVisibleRect)
            return max(range.location, 0)
        }

        private func stableVisibleRowCount(in tableView: NSTableView, visibleRect: NSRect) -> Int {
            guard !parent.tree.rows.isEmpty else { return 1 }
            let rowHeight = max(tableView.rowHeight, 1)
            return max(Int(floor((visibleRect.height + 0.5) / rowHeight)), 1)
        }

        private func resolvedProgrammaticTargetIndex() -> Int {
            let rustTarget = min(parent.tree.scrollOffset, max(parent.tree.rows.count - 1, 0))
            guard let pendingUserScrollOffset,
                  CFAbsoluteTimeGetCurrent() <= recentUserScrollDeadline
            else {
                return rustTarget
            }
            return min(pendingUserScrollOffset, max(parent.tree.rows.count - 1, 0))
        }

        private func reconcileHoveredRowWithPointer() {
            guard let tableView, let scrollView, let window = scrollView.window else {
                setHoveredRow(nil)
                return
            }
            let pointInClipView = scrollView.contentView.convert(window.mouseLocationOutsideOfEventStream, from: nil)
            guard scrollView.contentView.bounds.contains(pointInClipView) else {
                setHoveredRow(nil)
                return
            }
            let pointInTable = tableView.convert(window.mouseLocationOutsideOfEventStream, from: nil)
            let row = tableView.row(at: pointInTable)
            setHoveredRow(row >= 0 ? row : nil)
        }

        private func setHoveredRow(_ row: Int?) {
            let normalizedRow = row.flatMap { index in
                parent.tree.rows.indices.contains(index) ? index : nil
            }
            guard normalizedRow != hoveredRowIndex else { return }
            let previousRow = hoveredRowIndex
            hoveredRowIndex = normalizedRow
            updateHoverAppearance(for: previousRow)
            updateHoverAppearance(for: normalizedRow)
        }

        private func updateHoverAppearance(for row: Int?) {
            guard let row, let tableView, row < tableView.numberOfRows else { return }
            guard let cell = tableView.view(atColumn: 0, row: row, makeIfNecessary: false) as? EditorFileTreeTableCellView else {
                return
            }
            cell.setHovered(hoveredRowIndex == row)
        }
    }
}

private final class EditorFileTreeNonInteractiveHostingView<Content: View>: NSHostingView<Content> {
    override func hitTest(_ point: NSPoint) -> NSView? {
        nil
    }
}

private final class EditorFileTreeTableCellView: NSTableCellView {
    private var hostingView: EditorFileTreeNonInteractiveHostingView<EditorFileTreeCellContentView>?
    private var trackingArea: NSTrackingArea?
    private var row: EditorFileTreeRow?
    private var rowIndex: Int?
    private var theme: EditorFileTreeSidebarTheme?
    private var isHovered = false
    private var onFocusSidebar: (() -> Void)?
    private var onSelectIndex: ((Int) -> Void)?
    private var onActivateIndex: ((Int) -> Void)?
    private var onHoverRowChanged: ((Int?) -> Void)?

    init(identifier: NSUserInterfaceItemIdentifier) {
        super.init(frame: .zero)
        self.identifier = identifier
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let trackingArea {
            removeTrackingArea(trackingArea)
        }
        let trackingArea = NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(trackingArea)
        self.trackingArea = trackingArea
    }

    override func mouseEntered(with event: NSEvent) {
        guard let rowIndex else { return }
        onHoverRowChanged?(rowIndex)
    }

    override func mouseExited(with event: NSEvent) {
        onHoverRowChanged?(nil)
    }

    override func mouseDown(with event: NSEvent) {
        guard let rowIndex else {
            super.mouseDown(with: event)
            return
        }
        onFocusSidebar?()
        let location = convert(event.locationInWindow, from: nil)
        if isChevronHit(location) {
            onActivateIndex?(rowIndex)
            return
        }
        if event.clickCount >= 2 {
            onActivateIndex?(rowIndex)
        } else {
            onSelectIndex?(rowIndex)
        }
    }

    func configure(
        row: EditorFileTreeRow,
        index: Int,
        isHovered: Bool,
        theme: EditorFileTreeSidebarTheme,
        onFocusSidebar: @escaping () -> Void,
        onSelectIndex: @escaping (Int) -> Void,
        onActivateIndex: @escaping (Int) -> Void,
        onHoverRowChanged: @escaping (Int?) -> Void
    ) {
        self.row = row
        rowIndex = index
        self.theme = theme
        self.onFocusSidebar = onFocusSidebar
        self.onSelectIndex = onSelectIndex
        self.onActivateIndex = onActivateIndex
        self.onHoverRowChanged = onHoverRowChanged
        self.isHovered = isHovered
        refreshRootView()
    }

    func configureEmpty(theme: EditorFileTreeSidebarTheme) {
        row = nil
        rowIndex = nil
        self.theme = theme
        onFocusSidebar = nil
        onSelectIndex = nil
        onActivateIndex = nil
        onHoverRowChanged = nil
        isHovered = false
        refreshRootView()
    }

    private func refreshRootView() {
        guard let theme else { return }
        let rootView = EditorFileTreeCellContentView(row: row, theme: theme, isHovered: isHovered)
        if let hostingView {
            hostingView.rootView = rootView
            return
        }
        let hostingView = EditorFileTreeNonInteractiveHostingView(rootView: rootView)
        hostingView.translatesAutoresizingMaskIntoConstraints = false
        hostingView.sizingOptions = [.minSize, .preferredContentSize]
        addSubview(hostingView)
        NSLayoutConstraint.activate([
            hostingView.leadingAnchor.constraint(equalTo: leadingAnchor),
            hostingView.trailingAnchor.constraint(equalTo: trailingAnchor),
            hostingView.topAnchor.constraint(equalTo: topAnchor),
            hostingView.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
        self.hostingView = hostingView
    }

    func setHovered(_ hovered: Bool) {
        guard hovered != isHovered else { return }
        isHovered = hovered
        refreshRootView()
    }

    private func isChevronHit(_ location: NSPoint) -> Bool {
        guard let row, row.hasChildren || row.isDirectory else { return false }
        let chevronSize: CGFloat = 18
        let leadingPadding = 2 + (CGFloat(row.depth) * 11)
        let chevronRect = NSRect(
            x: max(leadingPadding - 4, 0),
            y: floor((bounds.height - chevronSize) / 2),
            width: chevronSize,
            height: chevronSize
        )
        return chevronRect.contains(location)
    }
}

private struct EditorFileTreeCellContentView: View {
    let row: EditorFileTreeRow?
    let theme: EditorFileTreeSidebarTheme
    let isHovered: Bool

    var body: some View {
        Group {
            if let row {
                EditorFileTreeRowView(
                    row: row,
                    theme: theme,
                    isHovered: isHovered
                )
            } else {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Empty Folder")
                        .font(.system(size: 12, weight: .semibold))
                    Text("Create a file or open another project folder.")
                        .font(.system(size: 11))
                        .foregroundStyle(ChromeForegroundColors.forBackground(theme.backgroundColor).secondary)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 14)
                .padding(.vertical, 16)
            }
        }
    }
}

private struct EditorFileTreeRowView: View {
    let row: EditorFileTreeRow
    let theme: EditorFileTreeSidebarTheme
    let isHovered: Bool

    private var chromeForeground: ChromeForegroundColors {
        ChromeForegroundColors.forBackground(theme.backgroundColor)
    }

    var body: some View {
        HStack(spacing: 0) {
            Rectangle()
                .fill(Color(nsColor: leadingRailColor))
                .frame(width: 2)
                .opacity(row.isSelected || row.isCurrentFile ? 1 : 0)

            HStack(spacing: 6) {
                if row.hasChildren || row.isDirectory {
                    Image(systemName: row.isExpanded ? "chevron.down" : "chevron.right")
                        .font(.system(size: 9, weight: .semibold))
                        .foregroundStyle(chromeForeground.secondary)
                        .frame(width: 10, height: 10)
                } else {
                    Color.clear
                        .frame(width: 10, height: 10)
                }

                EditorSemanticIconView(iconName: row.iconName, isDirectory: row.isDirectory, size: 11)
                    .foregroundStyle(Color(nsColor: iconColor))
                    .frame(width: 12)

                Text(row.displayName)
                    .font(.system(size: 12, weight: row.isDirectory ? .medium : .regular))
                    .foregroundStyle(rowTextColor)
                    .lineLimit(1)

                Spacer(minLength: 6)

                if row.isCurrentFile {
                    Circle()
                        .fill(Color(nsColor: theme.selectionColor).opacity(row.isSelected ? 0.95 : 0.82))
                        .frame(width: 5, height: 5)
                }
            }
            .padding(.leading, CGFloat(row.depth) * 11)
            .padding(.trailing, 8)
            .padding(.vertical, 4)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(selectionBackground)
        .clipShape(.rect(cornerRadius: 6))
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(row.isSelected ? [.isSelected] : [.isButton])
    }

    @ViewBuilder
    private var selectionBackground: some View {
        RoundedRectangle(cornerRadius: 6, style: .continuous)
            .fill(backgroundColor)
    }

    private var backgroundColor: Color {
        if row.isSelected {
            return Color(nsColor: theme.selectionColor).opacity(0.24)
        }
        if isHovered {
            return Color(nsColor: theme.hoverColor)
        }
        return .clear
    }

    private var leadingRailColor: NSColor {
        row.isSelected ? theme.selectionColor : theme.selectionColor.withAlphaComponent(0.78)
    }

    private var iconColor: NSColor {
        if row.isSelected {
            return chromeForeground.primaryNS
        }
        if row.isCurrentFile {
            return theme.selectionColor
        }
        return row.isDirectory ? chromeForeground.secondaryNS : chromeForeground.tertiaryNS
    }

    private var rowTextColor: Color {
        if let vcsKind = row.vcsKind {
            return fileTreeVcsTextColor(for: vcsKind)
        }
        if row.isSelected || row.isCurrentFile {
            return chromeForeground.primary
        }
        return chromeForeground.primary.opacity(0.88)
    }
}

private struct EditorSidebarRowDecorationsView: View {
    let vcsKind: EditorFileTreeVcsKind?
    let diagnosticSeverity: EditorDiagnosticSeverity?

    var body: some View {
        HStack(spacing: 6) {
            if let vcsKind {
                EditorSemanticIconView(iconName: fileTreeVcsIconName(vcsKind), size: 10)
                    .foregroundStyle(fileTreeBadgeColor(for: fileTreeVcsSeverity(vcsKind)))
                    .accessibilityLabel(Text(fileTreeVcsAccessibilityLabel(vcsKind)))
            }
            if let diagnosticSeverity {
                EditorSemanticIconView(iconName: diagnosticSeverity.statusIconName, size: 10)
                    .foregroundStyle(fileTreeBadgeColor(for: diagnosticSeverity))
                    .accessibilityLabel(Text(fileTreeDiagnosticAccessibilityLabel(diagnosticSeverity)))
            }
        }
    }
}

private struct EditorSidebarResizeHandle<ResizeGesture: Gesture>: View {
    let color: NSColor
    let edge: HorizontalEdge
    let gesture: ResizeGesture

    @State private var isHovering = false

    var body: some View {
        Rectangle()
            .fill(Color.clear)
            .frame(width: 8)
            .overlay(alignment: edge == .leading ? .leading : .trailing) {
                Rectangle()
                    .fill(Color(nsColor: color).opacity(isHovering ? 1.0 : 0.9))
                    .frame(width: isHovering ? 2 : 1)
            }
            .contentShape(Rectangle())
            .onHover { hovering in
                isHovering = hovering
                if hovering {
                    NSCursor.resizeLeftRight.set()
                }
            }
            .gesture(gesture)
    }
}

private struct EditorStatusAccessoryView: View {
    let chrome: EditorChromeModel
    let mode: EditorMode
    @ObservedObject var controller: EditorSurfaceController

    private var metadataItems: [EditorStatusItem] {
        [
            chrome.document.languageName.map {
                EditorStatusItem(icon: "curlybraces", text: $0, emphasis: .muted)
            },
            chrome.document.encodingName.map {
                EditorStatusItem(icon: "textformat", text: $0, emphasis: .muted)
            },
            chrome.document.lineEndingName.map {
                EditorStatusItem(icon: "return", text: $0, emphasis: .muted)
            },
        ].compactMap { $0 }
    }

    private var lspStatus: EditorLSPStatusPresentation? {
        chrome.statusBar.items
            .lazy
            .compactMap { EditorLSPStatusPresentation(item: $0) }
            .first
    }

    private var nonLSPStatusItems: [EditorStatusItem] {
        chrome.statusBar.items.filter { EditorLSPStatusPresentation(item: $0) == nil }
    }

    private var chromeForeground: ChromeForegroundColors {
        ChromeForegroundColors.forBackground(chrome.backgroundColor)
    }

    var body: some View {
        Group {
            if let terminalStatus = controller.activeTerminalStatus {
                terminalStatusBody(terminalStatus)
            } else {
                editorStatusBody
            }
        }
        .padding(.horizontal, 14)
        .frame(height: 28)
        .background(Color(nsColor: chromeBackgroundColor(base: chrome.backgroundColor)))
        .overlay(alignment: .top) {
            Divider()
        }
        .accessibilityElement(children: .contain)
    }

    private var editorStatusBody: some View {
        HStack(spacing: 12) {
            ModePill(mode: mode, chromeForeground: chromeForeground)

            if let lspStatus {
                LSPStatusAccessoryView(status: lspStatus, foreground: chromeForeground)
            }

            Spacer(minLength: 12)

            HStack(spacing: 10) {
                ForEach(nonLSPStatusItems) { item in
                    StatusAccessoryItemView(item: item, foreground: chromeForeground)
                }

                ForEach(metadataItems) { item in
                    StatusAccessoryItemView(item: item, foreground: chromeForeground)
                }

                Text(chrome.statusBar.cursorText)
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundStyle(chromeForeground.primary)
                    .lineLimit(1)
                    .padding(.leading, 4)
            }
        }
    }

    @ViewBuilder
    private func terminalStatusBody(_ status: EditorActiveTerminalStatus) -> some View {
        HStack(spacing: 10) {
            TerminalStatusPill(foreground: chromeForeground)

            if let workingDirectory = status.workingDirectory {
                Text(workingDirectory)
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundStyle(chromeForeground.primary)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }

            if let title = status.title {
                if status.workingDirectory != nil {
                    Circle()
                        .fill(chromeForeground.tertiary)
                        .frame(width: 3, height: 3)
                }
                Text(title)
                    .font(.system(size: 11, weight: .regular))
                    .foregroundStyle(chromeForeground.secondary)
                    .lineLimit(1)
                    .truncationMode(.tail)
            }

            Spacer(minLength: 0)
        }
    }
}

private struct StatusAccessoryItemView: View {
    let item: EditorStatusItem
    let foreground: ChromeForegroundColors
    var foregroundStyleOverride: Color? = nil

    private var normalized: (icon: String?, text: String) {
        normalizedStatusItemDisplay(icon: item.icon, text: item.text)
    }

    var body: some View {
        HStack(spacing: 5) {
            if let icon = normalized.icon {
                Group {
                    if icon == "curlybraces" || icon == "textformat" || icon == "return" {
                        Image(systemName: icon)
                            .font(.system(size: 10, weight: .semibold))
                    } else {
                        EditorSemanticIconView(iconName: icon, size: 10)
                    }
                }
            }

            if showsText, !normalized.text.isEmpty {
                Text(normalized.text)
                    .font(textFont)
                    .lineLimit(1)
            }
        }
        .foregroundStyle(foregroundStyleOverride ?? foregroundStyle)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(accessibilityLabel)
    }

    private var textFont: Font {
        switch item.emphasis {
        case .normal:
            return .system(size: 11, weight: .regular)
        case .muted:
            return .system(size: 11, weight: .regular)
        case .strong:
            return .system(size: 11, weight: .semibold)
        }
    }

    private var showsText: Bool {
        guard let icon = normalized.icon else { return true }
        switch icon {
        case "diagnostic_error", "diagnostic_warning", "diagnostic_info", "diagnostic_hint":
            return false
        default:
            return true
        }
    }

    private var accessibilityLabel: Text {
        Text(normalized.text.isEmpty ? item.text : normalized.text)
    }

    private var foregroundStyle: Color {
        if let icon = normalized.icon {
            switch icon {
            case "diagnostic_error":
                return .red
            case "diagnostic_warning":
                return .orange
            case "diagnostic_info":
                return .blue
            case "diagnostic_hint":
                return .teal
            case "copilot", "copilot_disabled", "copilot_init", "copilot_error", "supermaven", "supermaven_disabled", "supermaven_init", "supermaven_error":
                return .accentColor
            default:
                break
            }
        }

        switch item.emphasis {
        case .normal:
            return foreground.primary
        case .muted:
            return foreground.secondary
        case .strong:
            return foreground.primary
        }
    }
}

private struct EditorLSPStatusPresentation: Equatable {
    enum Phase: Equatable {
        case unavailable
        case off(String?)
        case loading(String)
        case ready(String?)
        case error(String?)
    }

    let phase: Phase

    init?(item: EditorStatusItem) {
        let raw = item.text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard raw.hasPrefix("lsp:") else { return nil }
        let payload = raw.dropFirst(4).trimmingCharacters(in: .whitespaces)

        if payload == "unavailable" {
            phase = .unavailable
            return
        }

        if payload.hasPrefix("ready") {
            phase = .ready(Self.extractParentheticalDetail(from: String(payload)))
            return
        }

        if payload.hasPrefix("error") {
            phase = .error(Self.extractParentheticalDetail(from: String(payload)))
            return
        }

        if payload.hasPrefix("off") {
            let detail = payload.dropFirst(3).trimmingCharacters(in: .whitespacesAndNewlines)
            phase = .off(detail.isEmpty ? nil : detail)
            return
        }

        let cleaned = String(payload).trimmingCharacters(in: CharacterSet(charactersIn: "⣾⣽⣻⢿⡿⣟⣯⣷ "))
        phase = .loading(cleaned.isEmpty ? "Language Server" : cleaned)
    }

    private static func extractParentheticalDetail(from text: String) -> String? {
        guard let open = text.firstIndex(of: "("), let close = text.lastIndex(of: ")"), open < close else {
            return nil
        }
        let detail = text[text.index(after: open)..<close].trimmingCharacters(in: .whitespacesAndNewlines)
        return detail.isEmpty ? nil : detail
    }
}

private struct LSPStatusAccessoryView: View {
    let status: EditorLSPStatusPresentation
    let foreground: ChromeForegroundColors

    @State private var isExpanded = true

    var body: some View {
        Button(action: toggleExpanded) {
            Group {
                if isExpanded {
                    expandedBody
                        .transition(.asymmetric(insertion: .opacity.combined(with: .scale(scale: 0.96)), removal: .opacity))
                } else {
                    collapsedBody
                        .transition(.asymmetric(insertion: .opacity.combined(with: .scale(scale: 0.92)), removal: .opacity))
                }
            }
        }
        .buttonStyle(.plain)
        .animation(.spring(response: 0.24, dampingFraction: 0.88), value: isExpanded)
        .accessibilityLabel(accessibilityLabel)
        .accessibilityHint(isExpanded ? "Collapse language server status" : "Expand language server status")
    }

    private var expandedBody: some View {
        HStack(spacing: 8) {
            statusDot

            Text(title)
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(foreground.primary)
                .lineLimit(1)

            if case .loading = status.phase {
                LSPIndeterminateBar()
                    .frame(width: 112, height: 4)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 5)
        .background(
            Capsule(style: .continuous)
                .fill(backgroundColor)
        )
        .overlay {
            Capsule(style: .continuous)
                .strokeBorder(borderColor, lineWidth: 1)
        }
    }

    private var collapsedBody: some View {
        ZStack {
            Circle()
                .fill(backgroundColor)
            Circle()
                .strokeBorder(borderColor, lineWidth: 1)
            statusDot
        }
        .frame(width: 18, height: 18)
    }

    @ViewBuilder
    private var statusDot: some View {
        Circle()
            .fill(dotColor)
            .frame(width: 7, height: 7)
            .overlay {
                if case .loading = status.phase {
                    Circle()
                        .stroke(dotColor.opacity(0.24), lineWidth: 4)
                        .scaleEffect(1.55)
                }
            }
    }

    private var title: String {
        switch status.phase {
        case .unavailable:
            return "Language Server Unavailable"
        case .off(let detail):
            return detail ?? "Language Server Off"
        case .loading(let detail):
            return detail
        case .ready(let detail):
            return detail ?? "Language Server Ready"
        case .error(let detail):
            return detail ?? "Language Server Error"
        }
    }

    private var accessibilityLabel: String {
        switch status.phase {
        case .unavailable:
            return "Language server unavailable"
        case .off(let detail):
            return detail.map { "Language server \($0)" } ?? "Language server off"
        case .loading(let detail):
            return "Language server loading: \(detail)"
        case .ready(let detail):
            return detail.map { "Language server ready: \($0)" } ?? "Language server ready"
        case .error(let detail):
            return detail.map { "Language server error: \($0)" } ?? "Language server error"
        }
    }

    private var dotColor: Color {
        switch status.phase {
        case .unavailable, .off:
            return foreground.secondary
        case .loading:
            return .accentColor
        case .ready:
            return .green
        case .error:
            return .red
        }
    }

    private var backgroundColor: Color {
        switch status.phase {
        case .unavailable, .off:
            return foreground.primary.opacity(0.06)
        case .loading:
            return Color.accentColor.opacity(0.10)
        case .ready:
            return Color.green.opacity(0.10)
        case .error:
            return Color.red.opacity(0.10)
        }
    }

    private var borderColor: Color {
        switch status.phase {
        case .unavailable, .off:
            return foreground.primary.opacity(0.08)
        case .loading:
            return Color.accentColor.opacity(0.18)
        case .ready:
            return Color.green.opacity(0.18)
        case .error:
            return Color.red.opacity(0.18)
        }
    }

    private func toggleExpanded() {
        isExpanded.toggle()
    }
}

private struct LSPIndeterminateBar: View {
    @State private var phase: CGFloat = -0.55

    var body: some View {
        GeometryReader { proxy in
            let width = proxy.size.width
            let fillWidth = max(width * 0.38, 28)

            Capsule(style: .continuous)
                .fill(Color.accentColor.opacity(0.16))
                .overlay(alignment: .leading) {
                    Capsule(style: .continuous)
                        .fill(
                            LinearGradient(
                                colors: [
                                    Color.accentColor.opacity(0.55),
                                    Color.accentColor.opacity(0.95)
                                ],
                                startPoint: .leading,
                                endPoint: .trailing
                            )
                        )
                        .frame(width: fillWidth)
                        .offset(x: phase * max(width - fillWidth, 1))
                }
                .clipShape(Capsule(style: .continuous))
                .onAppear {
                    phase = -0.55
                    withAnimation(.linear(duration: 1.05).repeatForever(autoreverses: false)) {
                        phase = 1.0
                    }
                }
        }
        .accessibilityHidden(true)
    }
}

private struct TerminalStatusPill: View {
    let foreground: ChromeForegroundColors

    var body: some View {
        HStack(spacing: 6) {
            EditorSemanticIconView(iconName: "terminal", size: 10)
            Text("Terminal")
                .font(.system(size: 10, weight: .semibold, design: .rounded))
        }
        .foregroundStyle(foreground.primary)
        .padding(.horizontal, 8)
        .padding(.vertical, 3)
        .background(
            Capsule(style: .continuous)
                .fill(foreground.primary.opacity(0.08))
        )
        .accessibilityLabel("Terminal")
    }
}

private struct ModePill: View {
    let mode: EditorMode
    let chromeForeground: ChromeForegroundColors

    var body: some View {
        Text(label)
            .font(.system(size: 10, weight: .semibold, design: .rounded))
            .foregroundStyle(pillForeground)
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(
                Capsule(style: .continuous)
                    .fill(pillBackground)
            )
            .accessibilityLabel(label)
    }

    private var label: String {
        switch mode {
        case .normal:
            return "Normal"
        case .insert:
            return "Insert"
        case .select:
            return "Select"
        case .command:
            return "Command"
        }
    }

    private var pillForeground: Color {
        switch mode {
        case .normal:
            return chromeForeground.secondary
        case .insert:
            return .accentColor
        case .select:
            return .purple
        case .command:
            return .orange
        }
    }

    private var pillBackground: Color {
        switch mode {
        case .normal:
            return chromeForeground.secondary.opacity(0.14)
        case .insert:
            return Color.accentColor.opacity(0.14)
        case .select:
            return Color.purple.opacity(0.14)
        case .command:
            return Color.orange.opacity(0.14)
        }
    }
}

private final class EditorTitlebarLeadingState: ObservableObject {
    @Published var isSidebarActive: Bool = false
    @Published var sidebarMode: EditorSidebarMode = .files
    @Published var sidebarTitle: String = "Workspace"
    @Published var document: EditorDocumentChrome = .empty
}

/// Manages the window chrome for the editor. Unlike the previous NSToolbar-based approach,
/// this uses NSTitlebarAccessoryViewController (like cmux) so that SwiftUI controls sit
/// directly inside the titlebar area alongside traffic lights - no visible toolbar band.
/// The window titlebar is transparent, and the sidebar content extends up into it.
private struct EditorWindowChromeAccessor: NSViewRepresentable {
    let chrome: EditorChromeModel
    let fileTreeVisible: Bool
    let sidebarMode: EditorSidebarMode
    let sidebarTitle: String
    @Binding var titlebarPadding: CGFloat
    let onToggleFileTree: () -> Void
    let onSelectSidebarMode: (EditorSidebarMode) -> Void
    let onOpenTerminal: () -> Void
    let onOpenAgentPane: () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> NSView {
        let view = NSView(frame: .zero)
        view.isHidden = true
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        if let window = nsView.window {
            context.coordinator.configure(
                window: window,
                chrome: chrome,
                fileTreeVisible: fileTreeVisible,
                sidebarMode: sidebarMode,
                sidebarTitle: sidebarTitle,
                titlebarPadding: $titlebarPadding,
                onToggleFileTree: onToggleFileTree,
                onSelectSidebarMode: onSelectSidebarMode,
                onOpenTerminal: onOpenTerminal,
                onOpenAgentPane: onOpenAgentPane
            )
            return
        }

        DispatchQueue.main.async { [weak nsView] in
            guard let nsView, let window = nsView.window else { return }
            context.coordinator.configure(
                window: window,
                chrome: chrome,
                fileTreeVisible: fileTreeVisible,
                sidebarMode: sidebarMode,
                sidebarTitle: sidebarTitle,
                titlebarPadding: $titlebarPadding,
                onToggleFileTree: onToggleFileTree,
                onSelectSidebarMode: onSelectSidebarMode,
                onOpenTerminal: onOpenTerminal,
                onOpenAgentPane: onOpenAgentPane
            )
        }
    }

    @MainActor
    final class Coordinator: NSObject {
        private let controlsIdentifier = NSUserInterfaceItemIdentifier("TheSwiftPOC.TitlebarControls")
        private let leadingState = EditorTitlebarLeadingState()
        private let containerView = NSView()
        private lazy var leadingHostingView: NonDraggableHostingView<EditorTitlebarLeadingRegionView> = {
            let view = NonDraggableHostingView(
                rootView: EditorTitlebarLeadingRegionView(
                    state: leadingState,
                    foreground: ChromeForegroundColors.forBackground(.windowBackgroundColor),
                    onToggle: {},
                    onSelectSidebarMode: { _ in },
                    onOpenTerminal: {},
                    onOpenAgentPane: {},
                    showsTerminalButton: GhosttyTerminalRegistry.isAvailable
                )
            )
            view.translatesAutoresizingMaskIntoConstraints = true
            view.autoresizingMask = [.width, .height]
            view.setContentCompressionResistancePriority(.required, for: .horizontal)
            view.setContentHuggingPriority(.required, for: .horizontal)
            return view
        }()
        private weak var observedWindow: NSWindow?
        private var lastChrome: EditorChromeModel = .empty
        private var lastFileTreeVisible = false
        private var lastSidebarMode: EditorSidebarMode = .files
        private var lastSidebarTitle = "Workspace"
        private var toggleFileTreeAction: (() -> Void)?
        private var selectSidebarModeAction: ((EditorSidebarMode) -> Void)?
        private var openTerminalAction: (() -> Void)?
        private var openAgentAction: (() -> Void)?
        private var titlebarPaddingBinding: Binding<CGFloat>?
        private var controlsAccessory: NSTitlebarAccessoryViewController?
        private var lastWindowLayoutSignature: String?

        override init() {
            super.init()
            containerView.translatesAutoresizingMaskIntoConstraints = true
            containerView.wantsLayer = true
            containerView.layer?.masksToBounds = false
            containerView.addSubview(leadingHostingView)
        }

        deinit {
            NotificationCenter.default.removeObserver(self)
        }

        func configure(
            window: NSWindow,
            chrome: EditorChromeModel,
            fileTreeVisible: Bool,
            sidebarMode: EditorSidebarMode,
            sidebarTitle: String,
            titlebarPadding: Binding<CGFloat>,
            onToggleFileTree: @escaping () -> Void,
            onSelectSidebarMode: @escaping (EditorSidebarMode) -> Void,
            onOpenTerminal: @escaping () -> Void,
            onOpenAgentPane: @escaping () -> Void
        ) {
            let started = CFAbsoluteTimeGetCurrent()
            let windowChanged = observedWindow !== window
            let chromeChanged = !chrome.matches(lastChrome)
            let fileTreeChanged = fileTreeVisible != lastFileTreeVisible
            let sidebarModeChanged = sidebarMode != lastSidebarMode
            let sidebarTitleChanged = sidebarTitle != lastSidebarTitle
            toggleFileTreeAction = onToggleFileTree
            selectSidebarModeAction = onSelectSidebarMode
            openTerminalAction = onOpenTerminal
            openAgentAction = onOpenAgentPane
            titlebarPaddingBinding = titlebarPadding
            attachWindowObserversIfNeeded(window: window)
            guard windowChanged || chromeChanged || fileTreeChanged || sidebarModeChanged || sidebarTitleChanged else {
                scrollPerfLog(
                    "chrome.configure skipped windowChanged=\(windowChanged) chromeChanged=\(chromeChanged) fileTreeChanged=\(fileTreeChanged) sidebarModeChanged=\(sidebarModeChanged)"
                )
                return
            }
            lastChrome = chrome
            lastFileTreeVisible = fileTreeVisible
            lastSidebarMode = sidebarMode
            lastSidebarTitle = sidebarTitle
            let applyStarted = CFAbsoluteTimeGetCurrent()
            applyWindowChrome(window: window, chrome: chrome)
            syncTitlebarPadding(window: window, binding: titlebarPadding)
            let applyMs = (CFAbsoluteTimeGetCurrent() - applyStarted) * 1000
            let updateStarted = CFAbsoluteTimeGetCurrent()
            updateAccessoryContent(
                chrome: chrome,
                fileTreeVisible: fileTreeVisible,
                sidebarMode: sidebarMode,
                sidebarTitle: sidebarTitle
            )
            let updateMs = (CFAbsoluteTimeGetCurrent() - updateStarted) * 1000
            let totalMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
            scrollPerfLog(
                "chrome.configure windowChanged=\(windowChanged) chromeChanged=\(chromeChanged) fileTreeChanged=\(fileTreeChanged) sidebarModeChanged=\(sidebarModeChanged) applyMs=\(String(format: "%.2f", applyMs)) updateMs=\(String(format: "%.2f", updateMs)) totalMs=\(String(format: "%.2f", totalMs))"
            )
        }

        private func attachWindowObserversIfNeeded(window: NSWindow) {
            guard observedWindow !== window else { return }
            NotificationCenter.default.removeObserver(self)
            observedWindow = window
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(windowDidChangeState(_:)),
                name: NSWindow.didBecomeMainNotification,
                object: window
            )
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(windowDidChangeState(_:)),
                name: NSWindow.didResignMainNotification,
                object: window
            )
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(windowDidChangeState(_:)),
                name: NSWindow.didResizeNotification,
                object: window
            )
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(windowDidChangeState(_:)),
                name: NSWindow.didEnterFullScreenNotification,
                object: window
            )
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(windowDidChangeState(_:)),
                name: NSWindow.didExitFullScreenNotification,
                object: window
            )
        }

        @objc private func windowDidChangeState(_ notification: Notification) {
            guard let window = notification.object as? NSWindow else { return }
            applyWindowChrome(window: window, chrome: lastChrome)
            if let titlebarPaddingBinding {
                syncTitlebarPadding(window: window, binding: titlebarPaddingBinding)
            }
            updateAccessoryContent(
                chrome: lastChrome,
                fileTreeVisible: lastFileTreeVisible,
                sidebarMode: leadingState.sidebarMode,
                sidebarTitle: leadingState.sidebarTitle
            )
        }

        // MARK: - Window chrome (transparent titlebar, no toolbar band)

        private func applyWindowChrome(window: NSWindow, chrome: EditorChromeModel) {
            let backgroundColor = chrome.backgroundColor
            window.title = ""
            window.titleVisibility = .hidden
            window.titlebarAppearsTransparent = true
            window.titlebarSeparatorStyle = .none
            window.isMovableByWindowBackground = false
            window.styleMask.insert(.fullSizeContentView)
            window.backgroundColor = backgroundColor.withAlphaComponent(1)
            window.isDocumentEdited = chrome.document.isModified
            window.appearance = backgroundColor.isLightColor
                ? NSAppearance(named: .aqua)
                : NSAppearance(named: .darkAqua)
            window.representedURL = nil

            // Install titlebar accessory on first window attachment
            installAccessoryIfNeeded(window: window)

            // Match the window background to the editor chrome
            if let contentView = window.contentView {
                contentView.wantsLayer = true
                contentView.layer?.backgroundColor = backgroundColor.cgColor
            }
            logWindowLayout(window: window, chrome: chrome)
        }

        private func syncTitlebarPadding(window: NSWindow, binding: Binding<CGFloat>) {
            let computedTitlebarHeight = window.frame.height - window.contentLayoutRect.height
            let nextPadding = max(28, min(72, computedTitlebarHeight))
            layoutDebugLog(
                String(
                    format: "window.titlebarPadding computed=%.1f clamped=%.1f contentLayoutHeight=%.1f frameHeight=%.1f",
                    computedTitlebarHeight,
                    nextPadding,
                    window.contentLayoutRect.height,
                    window.frame.height
                )
            )
            guard abs(binding.wrappedValue - nextPadding) > 0.5 else { return }
            DispatchQueue.main.async {
                binding.wrappedValue = nextPadding
            }
        }

        private func logWindowLayout(window: NSWindow, chrome: EditorChromeModel) {
            let contentViewFrameText = window.contentView.map { rectText($0.frame) } ?? "nil"
            let contentViewBoundsText = window.contentView.map { rectText($0.bounds) } ?? "nil"
            let safeAreaText = window.contentView.map { edgeInsetsText($0.safeAreaInsets) } ?? "nil"
            let signature = [
                "window",
                "frame=\(rectText(window.frame))",
                "contentLayoutRect=\(rectText(window.contentLayoutRect))",
                "contentViewFrame=\(contentViewFrameText)",
                "contentViewBounds=\(contentViewBoundsText)",
                "safeArea=\(safeAreaText)",
                "fileTreeVisible=\(lastFileTreeVisible)",
                "sidebarMode=\(lastSidebarMode.rawValue)",
                "statusItems=\(chrome.statusBar.items.count)"
            ].joined(separator: " ")
            guard signature != lastWindowLayoutSignature else { return }
            lastWindowLayoutSignature = signature
            layoutDebugLog(signature)
        }

        private func rectText(_ rect: CGRect) -> String {
            String(format: "(x:%.1f y:%.1f w:%.1f h:%.1f)", rect.origin.x, rect.origin.y, rect.size.width, rect.size.height)
        }

        private func edgeInsetsText(_ insets: NSEdgeInsets) -> String {
            String(format: "(t:%.1f l:%.1f b:%.1f r:%.1f)", insets.top, insets.left, insets.bottom, insets.right)
        }

        // MARK: - Titlebar accessory (cmux-style: controls live in the titlebar)

        private func installAccessoryIfNeeded(window: NSWindow) {
            // Only install once per window
            if window.titlebarAccessoryViewControllers.contains(where: { $0.view.identifier == controlsIdentifier }) {
                return
            }
            let accessory = NSTitlebarAccessoryViewController()
            // Place controls on the leading side so they sit beside traffic lights
            accessory.layoutAttribute = .left
            accessory.view = containerView
            accessory.view.identifier = controlsIdentifier
            window.addTitlebarAccessoryViewController(accessory)
            controlsAccessory = accessory
        }

        private func updateAccessoryContent(
            chrome: EditorChromeModel,
            fileTreeVisible: Bool,
            sidebarMode: EditorSidebarMode,
            sidebarTitle: String
        ) {
            let foreground = ChromeForegroundColors.forBackground(chrome.backgroundColor)
            leadingHostingView.rootView = EditorTitlebarLeadingRegionView(
                state: leadingState,
                foreground: foreground,
                onToggle: { self.toggleFileTreeAction?() },
                onSelectSidebarMode: { self.selectSidebarModeAction?($0) },
                onOpenTerminal: { self.openTerminalAction?() },
                onOpenAgentPane: { self.openAgentAction?() },
                showsTerminalButton: GhosttyTerminalRegistry.isAvailable
            )
            leadingState.isSidebarActive = fileTreeVisible
            leadingState.sidebarMode = sidebarMode
            leadingState.sidebarTitle = sidebarTitle
            leadingState.document = chrome.document
            leadingHostingView.invalidateIntrinsicContentSize()

            // Resize the accessory to fit its content
            scheduleSizeUpdate()
        }

        private func scheduleSizeUpdate() {
            DispatchQueue.main.async { [weak self] in
                self?.updateAccessorySize()
            }
        }

        private func updateAccessorySize() {
            leadingHostingView.layoutSubtreeIfNeeded()
            let fitting = leadingHostingView.fittingSize
            guard fitting.width > 0, fitting.height > 0 else { return }

            // Use traffic light button height as the true titlebar height, like cmux does.
            // We only set preferredContentSize on the accessory — never set the hosting
            // view's frame directly, as NSTitlebarAccessoryViewController owns that layout.
            let titlebarHeight: CGFloat = {
                if let window = observedWindow,
                   let closeButton = window.standardWindowButton(.closeButton),
                   let titlebarView = closeButton.superview,
                   titlebarView.frame.height > 0 {
                    return titlebarView.frame.height
                }
                return observedWindow.map { $0.frame.height - $0.contentLayoutRect.height } ?? fitting.height
            }()

            let containerHeight = max(fitting.height, titlebarHeight)
            let yOffset = max(0, (containerHeight - fitting.height) / 2.0)
            controlsAccessory?.preferredContentSize = NSSize(
                width: fitting.width,
                height: containerHeight
            )
            containerView.frame = NSRect(x: 0, y: 0, width: fitting.width, height: containerHeight)
            leadingHostingView.frame = NSRect(x: 0, y: yOffset, width: fitting.width, height: fitting.height)
        }
    }
}

/// NSHostingView subclass that does not swallow mouse-down for window dragging,
/// so button hits in the titlebar work correctly.
private final class NonDraggableHostingView<Content: View>: NSHostingView<Content> {
    override var mouseDownCanMoveWindow: Bool { false }
}

/// Leading titlebar controls: 24×24 cells; icons are flush (no inter-item stack spacing).
private enum EditorTitlebarControlMetrics {
    static let stackSpacing: CGFloat = 0
    static let buttonSize: CGFloat = 24
}

/// The accessory view content — cmux-style compact titlebar controls beside the traffic lights.
private struct EditorTitlebarLeadingRegionView: View {
    @ObservedObject var state: EditorTitlebarLeadingState
    let foreground: ChromeForegroundColors
    let onToggle: () -> Void
    let onSelectSidebarMode: (EditorSidebarMode) -> Void
    let onOpenTerminal: () -> Void
    let onOpenAgentPane: () -> Void
    let showsTerminalButton: Bool

    var body: some View {
        HStack(spacing: EditorTitlebarControlMetrics.stackSpacing) {
            EditorTitlebarSidebarToggleButton(
                isActive: state.isSidebarActive,
                foreground: foreground,
                onToggle: onToggle
            )

            if state.isSidebarActive {
                EditorTitlebarSidebarModeButtons(
                    mode: state.sidebarMode,
                    foreground: foreground,
                    onSelect: onSelectSidebarMode
                )
            }

            EditorTitlebarOpenAgentButton(
                onOpenAgentPane: onOpenAgentPane,
                foreground: foreground
            )

            if showsTerminalButton {
                EditorTitlebarOpenTerminalButton(
                    onOpenTerminal: onOpenTerminal,
                    foreground: foreground
                )
            }
        }
        .fixedSize(horizontal: true, vertical: true)
    }
}

private struct EditorTitlebarSidebarModeButtons: View {
    let mode: EditorSidebarMode
    let foreground: ChromeForegroundColors
    let onSelect: (EditorSidebarMode) -> Void

    var body: some View {
        HStack(spacing: EditorTitlebarControlMetrics.stackSpacing) {
            button(.files, iconName: "folder", systemImage: nil, label: "Show Files")
            button(.buffers, iconName: "buffers", systemImage: nil, label: "Show Open Items")
        }
    }

    private func button(_ target: EditorSidebarMode, iconName: String?, systemImage: String?, label: String) -> some View {
        let isSelected = mode == target
        return Button {
            onSelect(target)
        } label: {
            Group {
                if let iconName {
                    EditorSemanticIconView(iconName: iconName, size: 11)
                } else if let systemImage {
                    Image(systemName: systemImage)
                        .font(.system(size: 11, weight: .semibold))
                }
            }
                .foregroundStyle(isSelected ? foreground.primary : foreground.secondary)
                .frame(width: EditorTitlebarControlMetrics.buttonSize, height: EditorTitlebarControlMetrics.buttonSize)
                .background {
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .fill(isSelected ? foreground.primary.opacity(0.10) : Color.clear)
                }
        }
        .buttonStyle(.plain)
        .help(label)
        .accessibilityLabel(label)
    }
}

private struct EditorTitlebarSidebarTitleView: View {
    let mode: EditorSidebarMode
    let title: String
    let foreground: ChromeForegroundColors

    @ViewBuilder
    private var iconView: some View {
        switch mode {
        case .files:
            EditorSemanticIconView(iconName: "folder", size: 11)
        case .buffers:
            EditorSemanticIconView(iconName: "buffers", size: 11)
        }
    }

    var body: some View {
        HStack(spacing: 6) {
            iconView
                .foregroundStyle(foreground.secondary)
            Text(title)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(foreground.secondary)
                .lineLimit(1)
                .truncationMode(.middle)
        }
        .frame(maxWidth: 180, alignment: .leading)
        .allowsHitTesting(false)
        .accessibilityElement(children: .combine)
    }
}

private struct EditorTitlebarSidebarToggleButton: View {
    let isActive: Bool
    let foreground: ChromeForegroundColors
    let onToggle: () -> Void

    var body: some View {
        Button(action: toggle) {
            Image(systemName: "sidebar.left")
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(isActive ? foreground.primary : foreground.secondary)
                .frame(width: EditorTitlebarControlMetrics.buttonSize, height: EditorTitlebarControlMetrics.buttonSize)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .frame(width: EditorTitlebarControlMetrics.buttonSize, height: EditorTitlebarControlMetrics.buttonSize)
        .contentShape(Rectangle())
        .help("Toggle Sidebar")
        .accessibilityLabel("Toggle Sidebar")
    }

    private func toggle() {
        // Do not wrap in spring animation: that animates chrome width, so the editor surface
        // sees dozens of intermediate sizes per toggle and pays for a full snapshot each time
        // the terminal column count changes.
        onToggle()
    }
}

private struct EditorTitlebarOpenAgentButton: View {
    let onOpenAgentPane: () -> Void
    let foreground: ChromeForegroundColors

    var body: some View {
        Button(action: onOpenAgentPane) {
            Image(systemName: "brain.head.profile")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(foreground.secondary)
                .frame(width: 28, height: 24)
                .background {
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .fill(foreground.primary.opacity(0.08))
                }
        }
        .buttonStyle(.plain)
        .help("Open Agent Pane")
        .accessibilityLabel("Open Agent Pane")
        .accessibilityHint("Open the pi agent in the active pane")
    }
}

private struct EditorTitlebarOpenTerminalButton: View {
    let onOpenTerminal: () -> Void
    let foreground: ChromeForegroundColors

    var body: some View {
        Button(action: onOpenTerminal) {
            EditorSemanticIconView(iconName: "terminal", size: 12)
                .foregroundStyle(foreground.secondary)
                .frame(width: 28, height: 24)
                .background {
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .fill(foreground.primary.opacity(0.08))
                }
        }
        .buttonStyle(.plain)
        .help("New Terminal")
        .accessibilityLabel("New Terminal")
        .accessibilityHint("Open a terminal in the active pane")
    }
}

private struct EditorTitlebarDocumentView: View {
    let document: EditorDocumentChrome
    let foreground: ChromeForegroundColors

    var body: some View {
        HStack(spacing: 8) {
            EditorSemanticIconView(iconName: document.icon, size: 12)
                .foregroundStyle(foreground.secondary)

            Text(document.name)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(foreground.primary)
                .lineLimit(1)
                .truncationMode(.middle)
        }
        .frame(maxWidth: 260, alignment: .leading)
        .allowsHitTesting(false)
        .accessibilityElement(children: .combine)
    }
}

private struct EditorTitlebarVCSView: View {
    let vcsText: String?
    let foreground: ChromeForegroundColors

    private var trimmedVCSText: String? {
        let trimmed = vcsText?.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let trimmed, !trimmed.isEmpty else { return nil }
        return trimmed
    }

    var body: some View {
        Group {
            if let trimmedVCSText {
                HStack(spacing: 6) {
                    EditorSemanticIconView(iconName: "git_branch", size: 11)
                    Text(trimmedVCSText)
                        .font(.system(size: 12, weight: .medium))
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
                .foregroundStyle(foreground.secondary)
                .fixedSize()
                .allowsHitTesting(false)
                .accessibilityElement(children: .combine)
            } else {
                Color.clear
                    .frame(width: 1, height: 1)
                    .allowsHitTesting(false)
                    .accessibilityHidden(true)
            }
        }
    }
}

private func chromeBackgroundColor(base: NSColor) -> NSColor {
    base.usingColorSpace(.sRGB) ?? base
}

private extension NSColor {
    var isLightColor: Bool {
        guard let color = usingColorSpace(.sRGB) else { return false }
        let luminance = (0.299 * color.redComponent) + (0.587 * color.greenComponent) + (0.114 * color.blueComponent)
        return luminance > 0.7
    }

    func adjustedBrightness(by amount: CGFloat) -> NSColor {
        guard let color = usingColorSpace(.sRGB) else { return self }
        return NSColor(
            calibratedRed: min(max(color.redComponent + amount, 0), 1),
            green: min(max(color.greenComponent + amount, 0), 1),
            blue: min(max(color.blueComponent + amount, 0), 1),
            alpha: color.alphaComponent
        )
    }
}

private func fileTreeVcsIconName(_ kind: EditorFileTreeVcsKind) -> String {
    switch kind {
    case .conflict:
        return "git_conflict"
    case .deleted:
        return "git_deleted"
    case .modified:
        return "git_modified"
    case .renamed:
        return "git_renamed"
    case .untracked:
        return "git_untracked"
    }
}

private func fileTreeVcsSeverity(_ kind: EditorFileTreeVcsKind) -> EditorDiagnosticSeverity {
    switch kind {
    case .conflict, .deleted:
        return .error
    case .modified:
        return .warning
    case .renamed:
        return .information
    case .untracked:
        return .hint
    }
}

private func fileTreeVcsAccessibilityLabel(_ kind: EditorFileTreeVcsKind) -> String {
    switch kind {
    case .conflict:
        return "conflict"
    case .deleted:
        return "deleted"
    case .modified:
        return "modified"
    case .renamed:
        return "renamed"
    case .untracked:
        return "untracked"
    }
}

private func fileTreeDiagnosticAccessibilityLabel(_ severity: EditorDiagnosticSeverity) -> String {
    switch severity {
    case .error:
        return "error"
    case .warning:
        return "warning"
    case .information:
        return "information"
    case .hint:
        return "hint"
    }
}

private func fileTreeBadgeColor(for severity: EditorDiagnosticSeverity) -> Color {
    switch severity {
    case .error:
        return .red
    case .warning:
        return .orange
    case .information:
        return .blue
    case .hint:
        return .teal
    }
}

private func fileTreeVcsTextColor(for kind: EditorFileTreeVcsKind) -> Color {
    switch kind {
    case .conflict, .deleted:
        return .red
    case .modified:
        return .orange
    case .renamed:
        return .blue
    case .untracked:
        return .teal
    }
}

private func prettyKeyToken(_ token: String) -> String {
    let parts = token.split(separator: "-").map(String.init)
    guard let rawKey = parts.last else { return token }
    let modifiers = parts.dropLast().map { modifier -> String in
        switch modifier.uppercased() {
        case "C":
            return "⌃"
        case "A":
            return "⌥"
        case "S":
            return "⇧"
        default:
            return "\(modifier)-"
        }
    }.joined()

    let keyLabel: String = switch rawKey.lowercased() {
    case "space":
        "Space"
    case "ret", "return", "enter":
        "↩"
    case "esc":
        "Esc"
    case "tab":
        "Tab"
    case "bs":
        "⌫"
    case "del":
        "⌦"
    case "left":
        "←"
    case "right":
        "→"
    case "up":
        "↑"
    case "down":
        "↓"
    case "pgup":
        "PgUp"
    case "pgdown":
        "PgDn"
    case "home":
        "Home"
    case "end":
        "End"
    default:
        rawKey.count == 1 ? rawKey.uppercased() : rawKey.uppercased()
    }

    return modifiers + keyLabel
}

private func normalizedStatusItemDisplay(icon: String?, text: String) -> (icon: String?, text: String) {
    if let icon {
        return (icon, text)
    }

    let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
    let glyphMappings: [(String, String)] = [
        ("", "diagnostic_error"),
        ("", "diagnostic_warning"),
        ("", "diagnostic_info"),
        ("󰌵", "diagnostic_hint"),
    ]
    for (glyph, iconName) in glyphMappings where trimmed.hasPrefix(glyph) {
        let remainder = trimmed.dropFirst(glyph.count).trimmingCharacters(in: .whitespacesAndNewlines)
        return (iconName, remainder)
    }
    return (nil, text)
}

// MARK: - Titlebar helper views

/// Invisible NSView that tracks the leading inset (traffic lights + accessories) for titlebar alignment.
private final class EditorTitlebarLeadingInsetPassthroughView: NSView {
    override var mouseDownCanMoveWindow: Bool { false }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

private struct EditorTitlebarLeadingInsetReader: NSViewRepresentable {
    @Binding var inset: CGFloat

    func makeNSView(context: Context) -> NSView {
        let view = EditorTitlebarLeadingInsetPassthroughView()
        view.setFrameSize(.zero)
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        DispatchQueue.main.async {
            guard let window = nsView.window else { return }
            // Start past the traffic lights.
            var leading: CGFloat = 78
            // Match cmux more closely: use the larger of the accessory's preferred width and
            // current frame width, since AppKit can report a zero frame while the accessory is
            // still being installed. Add a small gap so the custom titlebar content doesn't
            // visually collide with the leading accessory controls.
            for accessory in window.titlebarAccessoryViewControllers
                where accessory.layoutAttribute == .left || accessory.layoutAttribute == .leading {
                leading += max(accessory.preferredContentSize.width, accessory.view.frame.width)
            }
            leading += 8
            if abs(leading - inset) > 0.5 {
                inset = leading
            }
        }
    }
}

/// Transparent hit-test view covering the titlebar area for window dragging.
private struct EditorWindowDragHandleView: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView {
        let view = EditorWindowDragPassthroughNSView()
        view.setFrameSize(NSSize(width: 10000, height: 10000))
        return view
    }
    func updateNSView(_ nsView: NSView, context: Context) {}
}

/// NSView subclass that allows window dragging through its frame.
private final class EditorWindowDragPassthroughNSView: NSView {
    override var mouseDownCanMoveWindow: Bool { true }
}

/// Monitors double-clicks on the titlebar to perform standard double-click action (zoom/minimize).
private struct EditorTitlebarDoubleClickMonitorView: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView {
        let view = EditorTitlebarDoubleClickTargetNSView()
        view.setFrameSize(NSSize(width: 10000, height: 10000))
        return view
    }
    func updateNSView(_ nsView: NSView, context: Context) {}
}

private final class EditorTitlebarDoubleClickTargetNSView: NSView {
    override func mouseDown(with event: NSEvent) {
        super.mouseDown(with: event)
        guard let window else { return }
        performStandardTitlebarDoubleClick(on: window)
    }

    private func performStandardTitlebarDoubleClick(on window: NSWindow) {
        let globalDefaults = UserDefaults.standard.persistentDomain(forName: UserDefaults.globalDomain) ?? [:]
        let action: String? = globalDefaults["AppleActionOnDoubleClick"] as? String
        switch action?.lowercased() {
        case "minimize", "miniaturize":
            window.miniaturize(nil)
        case "none", "no action":
            break
        default:
            window.zoom(nil)
        }
    }
}

/// Renders the background color in the titlebar area.
private struct EditorTitlebarLayerBackground: View {
    let backgroundColor: NSColor

    var body: some View {
        EditorTitlebarBackgroundNSView(backgroundColor: backgroundColor)
            .allowsHitTesting(false)
    }
}

private struct EditorTitlebarBackgroundNSView: NSViewRepresentable {
    let backgroundColor: NSColor

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        view.wantsLayer = true
        view.layer?.backgroundColor = backgroundColor.cgColor
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        nsView.layer?.backgroundColor = backgroundColor.cgColor
    }
}

/// Folder proxy icon for the titlebar, matching cmux's native Finder-style icon.
private struct EditorSidebarTitleIconView: View {
    let directory: String

    var body: some View {
        Image(nsImage: folderIcon)
            .resizable()
            .aspectRatio(contentMode: .fit)
            .frame(width: 16, height: 16)
            .help("Drag to open in Finder or another app")
            .onTapGesture(count: 2) {
                NSWorkspace.shared.selectFile(nil, inFileViewerRootedAtPath: directory)
            }
    }

    private var folderIcon: NSImage {
        let icon = NSWorkspace.shared.icon(forFile: directory)
        icon.size = NSSize(width: 16, height: 16)
        return icon
    }
}

/// A solid-color scrim that covers the traffic-light zone at the top of the sidebar,
/// preventing list content from showing through beneath the window buttons.
private struct EditorSidebarTopScrim: View {
    let height: CGFloat
    let backgroundColor: NSColor

    var body: some View {
        Rectangle()
            .fill(Color(nsColor: backgroundColor))
            .frame(height: height)
            .allowsHitTesting(false)
    }
}
