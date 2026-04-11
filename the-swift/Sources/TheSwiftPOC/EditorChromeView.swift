import AppKit
import SwiftUI

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
    @AppStorage("swift.piSidebarWidth") private var storedPiSidebarWidth: Double = 360
    @AppStorage("swift.sidebar.mode") private var storedSidebarModeRaw: String = EditorSidebarMode.files.rawValue
    @GestureState private var fileTreeDragTranslation: CGFloat = 0
    @GestureState private var piSidebarDragTranslation: CGFloat = 0
    @State private var isFileTreeResizeActive = false
    @State private var isPiSidebarResizeActive = false
    @StateObject private var piSidebarSession: EditorPiSidebarSession

    private let minimumFileTreeWidth: CGFloat = 180
    private let maximumFileTreeWidth: CGFloat = 460
    private let minimumPiSidebarWidth: CGFloat = 280
    private let maximumPiSidebarWidth: CGFloat = 720

    init(controller: EditorSurfaceController) {
        self.controller = controller
        _piSidebarSession = StateObject(wrappedValue: EditorPiSidebarSession(workingDirectory: controller.preferredPiWorkingDirectory()))
    }

    private var sidebarMode: EditorSidebarMode {
        EditorSidebarMode(rawValue: storedSidebarModeRaw) ?? .files
    }

    var body: some View {
        HStack(spacing: 0) {
            if controller.fileTree.isVisible {
                sidebarColumn
                EditorSidebarResizeHandle(
                    color: sidebarTheme.separatorColor,
                    edge: .trailing,
                    gesture: fileTreeResizeGesture
                )
            }

            mainColumn

            if controller.isPiSidebarVisible, GhosttyTerminalRegistry.isAvailable {
                EditorSidebarResizeHandle(
                    color: sidebarTheme.separatorColor,
                    edge: .leading,
                    gesture: piSidebarResizeGesture
                )
                piSidebarColumn
            }
        }
        .background(
            EditorWindowChromeAccessor(
                chrome: controller.chrome,
                fileTreeVisible: controller.fileTree.isVisible,
                fileTreeWidth: titlebarSidebarRegionWidth,
                fileTreeBackgroundColor: sidebarTheme.backgroundColor,
                fileTreeSeparatorColor: sidebarTheme.separatorColor,
                isPiSidebarVisible: controller.isPiSidebarVisible,
                onToggleFileTree: controller.toggleFileTree,
                onOpenTerminal: controller.openTerminalInActivePane,
                onTogglePiSidebar: controller.togglePiSidebar
            )
        )
        .overlay(alignment: .bottom) {
            if let pendingKeys = controller.pendingKeys {
                EditorPendingKeyIndicatorView(pendingKeys: pendingKeys)
                    .padding(.bottom, 38)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .onAppear {
            piSidebarSession.setVisible(controller.isPiSidebarVisible)
        }
        .onChange(of: controller.isPiSidebarVisible) { _, isVisible in
            piSidebarSession.setVisible(isVisible)
            if isVisible {
                DispatchQueue.main.async {
                    piSidebarSession.focus()
                }
            }
        }
        .animation(.spring(response: 0.24, dampingFraction: 0.88), value: controller.pendingKeys?.pendingDisplay)
        .animation(.spring(response: 0.24, dampingFraction: 0.88), value: controller.isPiSidebarVisible)
    }

    private var sidebarColumn: some View {
        Group {
            switch sidebarMode {
            case .files:
                EditorFileTreeSidebarView(
                    tree: controller.fileTree,
                    theme: sidebarTheme,
                    sidebarMode: sidebarMode,
                    onSelectSidebarMode: selectSidebarMode,
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
                    sidebarMode: sidebarMode,
                    onSelectSidebarMode: selectSidebarMode,
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

    private var piSidebarColumn: some View {
        GhosttyPiSidebarRepresentable(session: piSidebarSession)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(Color(nsColor: sidebarTheme.backgroundColor))
            .frame(width: piSidebarWidth)
            .background(Color(nsColor: sidebarTheme.backgroundColor))
            .onDisappear {
                piSidebarSession.setVisible(false)
            }
    }

    private var mainColumn: some View {
        VStack(spacing: 0) {
            ZStack(alignment: .topLeading) {
                EditorSurfaceRepresentable(controller: controller)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)

                EditorDiagnosticsOverlayView(controller: controller)
                EditorDocsPanelsView(controller: controller)
                EditorCompletionMenuView(controller: controller)
                EditorPaneItemStripsOverlayView(controller: controller)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)

            EditorStatusAccessoryView(chrome: controller.chrome, mode: controller.currentMode)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var sidebarTheme: EditorFileTreeSidebarTheme {
        EditorFileTreeSidebarTheme.resolve(scene: controller.scene, chrome: controller.chrome)
    }

    private var fileTreeWidth: CGFloat {
        clampFileTreeWidth(CGFloat(storedFileTreeWidth) + fileTreeDragTranslation)
    }

    private var piSidebarWidth: CGFloat {
        clampPiSidebarWidth(CGFloat(storedPiSidebarWidth) + piSidebarDragTranslation)
    }

    private var titlebarSidebarRegionWidth: CGFloat {
        controller.fileTree.isVisible ? fileTreeWidth + 8 : 52
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

    private var piSidebarResizeGesture: some Gesture {
        DragGesture(minimumDistance: 0, coordinateSpace: .global)
            .updating($piSidebarDragTranslation) { value, state, _ in
                let translation = value.startLocation.x - value.location.x
                state = translation
                let startWidth = CGFloat(storedPiSidebarWidth)
                let proposedWidth = startWidth + translation
                let startText = String(format: "%.1f", startWidth)
                let translationText = String(format: "%.1f", translation)
                let proposedText = String(format: "%.1f", proposedWidth)
                let clampedText = String(format: "%.1f", clampPiSidebarWidth(proposedWidth))
                let startXText = String(format: "%.1f", value.startLocation.x)
                let locationXText = String(format: "%.1f", value.location.x)
                scrollPerfLog(
                    "pi.sidebar.resize updating start=\(startText) translation=\(translationText) proposed=\(proposedText) clamped=\(clampedText) startX=\(startXText) currentX=\(locationXText)"
                )
            }
            .onChanged { _ in
                guard !isPiSidebarResizeActive else { return }
                isPiSidebarResizeActive = true
                controller.beginInteractiveResize(reason: "piSidebar")
            }
            .onEnded { value in
                let translation = value.startLocation.x - value.location.x
                let committedWidth = clampPiSidebarWidth(CGFloat(storedPiSidebarWidth) + translation)
                let storedText = String(format: "%.1f", storedPiSidebarWidth)
                let translationText = String(format: "%.1f", translation)
                let committedText = String(format: "%.1f", committedWidth)
                let startXText = String(format: "%.1f", value.startLocation.x)
                let locationXText = String(format: "%.1f", value.location.x)
                scrollPerfLog(
                    "pi.sidebar.resize ended stored=\(storedText) translation=\(translationText) committed=\(committedText) startX=\(startXText) currentX=\(locationXText)"
                )
                storedPiSidebarWidth = committedWidth
                if isPiSidebarResizeActive {
                    isPiSidebarResizeActive = false
                    controller.endInteractiveResize(reason: "piSidebar")
                }
            }
    }

    private func clampFileTreeWidth(_ width: CGFloat) -> CGFloat {
        min(max(width, minimumFileTreeWidth), maximumFileTreeWidth)
    }

    private func clampPiSidebarWidth(_ width: CGFloat) -> CGFloat {
        min(max(width, minimumPiSidebarWidth), maximumPiSidebarWidth)
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

@MainActor
final class EditorPiSidebarSession: ObservableObject {
    let hostView: GhosttyPiSidebarHostView

    init(workingDirectory: String) {
        hostView = GhosttyPiSidebarHostView(workingDirectory: workingDirectory, command: "pi")
    }

    func setVisible(_ visible: Bool) {
        hostView.setSurfaceVisible(visible)
    }

    func focus() {
        hostView.focusSurface()
    }
}

private struct GhosttyPiSidebarRepresentable: NSViewRepresentable {
    @ObservedObject var session: EditorPiSidebarSession

    func makeNSView(context: Context) -> GhosttyPiSidebarHostView {
        session.hostView
    }

    func updateNSView(_ nsView: GhosttyPiSidebarHostView, context: Context) {}
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

private struct EditorSidebarHeaderView: View {
    let systemImage: String
    let title: String
    let helpText: String?
    let theme: EditorFileTreeSidebarTheme
    let sidebarMode: EditorSidebarMode
    let onSelectSidebarMode: (EditorSidebarMode) -> Void
    let onActivate: () -> Void

    private let headerHeight: CGFloat = 30

    var body: some View {
        HStack(spacing: 10) {
            Button(action: onActivate) {
                HStack(spacing: 8) {
                    Image(systemName: systemImage)
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(Color(nsColor: theme.selectionColor).opacity(0.85))
                    Text(title)
                        .font(.system(size: 12, weight: .semibold))
                        .lineLimit(1)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help(helpText ?? title)

            EditorSidebarModeSwitcher(
                mode: sidebarMode,
                theme: theme,
                onSelect: onSelectSidebarMode
            )
        }
        .padding(.horizontal, 12)
        .frame(maxWidth: .infinity, minHeight: headerHeight, maxHeight: headerHeight, alignment: .leading)
        .background(Color(nsColor: theme.headerColor))
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(Color(nsColor: theme.separatorColor))
                .frame(height: 1)
        }
    }
}

private struct EditorSidebarModeSwitcher: View {
    let mode: EditorSidebarMode
    let theme: EditorFileTreeSidebarTheme
    let onSelect: (EditorSidebarMode) -> Void

    var body: some View {
        HStack(spacing: 3) {
            modeButton(.files, icon: "folder.fill", label: "Files")
            modeButton(.buffers, icon: "square.stack.3d.up.fill", label: "Buffers")
        }
        .padding(3)
        .background(
            RoundedRectangle(cornerRadius: 7, style: .continuous)
                .fill(Color(nsColor: theme.backgroundColor).opacity(0.9))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 7, style: .continuous)
                .stroke(Color(nsColor: theme.separatorColor).opacity(0.65), lineWidth: 1)
        )
    }

    private func modeButton(_ target: EditorSidebarMode, icon: String, label: String) -> some View {
        let isSelected = mode == target
        return Button {
            onSelect(target)
        } label: {
            Image(systemName: icon)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(isSelected ? Color.primary : Color.secondary)
                .frame(width: 20, height: 20)
                .background(
                    RoundedRectangle(cornerRadius: 5, style: .continuous)
                        .fill(isSelected ? Color(nsColor: theme.selectionColor).opacity(0.28) : .clear)
                )
                .contentShape(RoundedRectangle(cornerRadius: 5, style: .continuous))
        }
        .buttonStyle(.plain)
        .help(label)
        .accessibilityLabel(label)
    }
}

private struct EditorFileTreeSidebarView: View {
    let tree: EditorFileTreeState
    let theme: EditorFileTreeSidebarTheme
    let sidebarMode: EditorSidebarMode
    let onSelectSidebarMode: (EditorSidebarMode) -> Void
    let onSelectIndex: (Int) -> Void
    let onActivateIndex: (Int) -> Void
    let onVisibleRowsChanged: (Int) -> Void
    let onScrollOffsetChanged: (Int) -> Void
    let onFocusSidebar: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            EditorSidebarHeaderView(
                systemImage: "folder.fill",
                title: rootTitle,
                helpText: tree.root ?? rootTitle,
                theme: theme,
                sidebarMode: sidebarMode,
                onSelectSidebarMode: onSelectSidebarMode,
                onActivate: onFocusSidebar
            )

            EditorFileTreeListRepresentable(
                tree: tree,
                theme: theme,
                onSelectIndex: onSelectIndex,
                onActivateIndex: onActivateIndex,
                onVisibleRowsChanged: onVisibleRowsChanged,
                onScrollOffsetChanged: onScrollOffsetChanged,
                onFocusSidebar: onFocusSidebar
            )
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Color(nsColor: theme.backgroundColor))
        .environment(\.colorScheme, theme.backgroundColor.isLightColor ? .light : .dark)
    }

    private var rootTitle: String {
        guard let root = tree.root, !root.isEmpty else { return "Workspace" }
        let url = URL(fileURLWithPath: root)
        let name = url.lastPathComponent
        return name.isEmpty ? root : name
    }
}

private struct EditorOpenItemsSidebarView: View {
    let openItems: EditorPaneOpenItemsState
    let scene: EditorRenderScene?
    let uniqueBufferCount: Int
    let theme: EditorFileTreeSidebarTheme
    let sidebarMode: EditorSidebarMode
    let onSelectSidebarMode: (EditorSidebarMode) -> Void
    let onActivateItem: (EditorPaneOpenItemRow) -> Void
    let onCloseItem: (EditorPaneOpenItemRow) -> Void
    let onFocusSidebar: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            EditorSidebarHeaderView(
                systemImage: "square.stack.3d.up.fill",
                title: "Open Items",
                helpText: "Open items grouped by pane",
                theme: theme,
                sidebarMode: sidebarMode,
                onSelectSidebarMode: onSelectSidebarMode,
                onActivate: onFocusSidebar
            )

            ScrollView(.vertical, showsIndicators: true) {
                LazyVStack(alignment: .leading, spacing: 10) {
                    if openItems.groups.isEmpty {
                        VStack(alignment: .leading, spacing: 6) {
                            Text("No Open Items")
                                .font(.system(size: 12, weight: .semibold))
                            Text("Open buffers and pane-local items will appear here.")
                                .font(.system(size: 11))
                                .foregroundStyle(.secondary)
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
                .padding(.vertical, 6)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Color(nsColor: theme.backgroundColor))
        .environment(\.colorScheme, theme.backgroundColor.isLightColor ? .light : .dark)
    }

    private func canClose(_ item: EditorPaneOpenItemRow) -> Bool {
        switch item.kind {
        case .buffer:
            return uniqueBufferCount > 1
        case .terminal:
            return true
        }
    }
}

private struct EditorOpenItemsGroupHeaderView: View {
    let title: String
    let count: Int
    let isActivePane: Bool
    let theme: EditorFileTreeSidebarTheme

    var body: some View {
        HStack(spacing: 6) {
            Circle()
                .fill(Color(nsColor: isActivePane ? theme.selectionColor : theme.separatorColor))
                .frame(width: 5, height: 5)
            Text(title)
                .font(.system(size: 10, weight: .bold))
                .foregroundStyle(isActivePane ? .primary : .secondary)
                .tracking(0.4)
            Text("\(count)")
                .font(.system(size: 10, weight: .medium))
                .foregroundStyle(.secondary.opacity(0.85))
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

    var body: some View {
        HStack(spacing: 0) {
            Rectangle()
                .fill(Color(nsColor: leadingRailColor))
                .frame(width: 2)
                .opacity(item.isActive ? 1 : 0)

            Button(action: onActivate) {
                HStack(spacing: 8) {
                    Image(systemName: symbolName(for: item.iconName, isDirectory: false))
                        .font(.system(size: 11, weight: .regular))
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
                                .foregroundStyle(.secondary)
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
                            .fill(Color(nsColor: item.isActive ? theme.selectionColor : .secondaryLabelColor).opacity(item.isActive ? 0.95 : 0.82))
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

    private var iconColor: NSColor {
        if item.isActive {
            return .labelColor
        }
        return .tertiaryLabelColor
    }

    private var closeButtonForegroundColor: NSColor {
        if !canClose {
            return .tertiaryLabelColor
        }
        if item.isActive || isHovered {
            return .labelColor
        }
        return .secondaryLabelColor
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
        item.isActive ? .primary : .primary.opacity(0.88)
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
        let headerHeight: CGFloat
        let controlHeight: CGFloat
        let controlOriginY: CGFloat

        var id: UInt { group.id }
    }

    @ObservedObject var controller: EditorSurfaceController

    private let horizontalInset = EditorPaneItemStripMetrics.horizontalInset

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

                            EditorPaneItemTabStripView(
                                group: entry.group,
                                paneLabel: paneLocationLabel(for: entry.group.paneID, groupIndex: entry.groupIndex, scene: scene),
                                theme: theme,
                                controlHeight: entry.controlHeight,
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
            return LayoutEntry(
                groupIndex: groupIndex,
                group: group,
                frame: paneFrame(for: pane, in: scene),
                headerHeight: scene.paneHeaderHeight(for: pane),
                controlHeight: scene.paneTabControlHeight(for: pane),
                controlOriginY: scene.paneTabControlOriginY(for: pane)
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
        case .terminal:
            return true
        }
    }

    private func paneFrame(for pane: EditorSnapshotPane, in scene: EditorRenderScene) -> CGRect {
        scene.paneRect(for: pane)
    }
}

private struct EditorPaneItemTabStripView: View {
    let group: EditorPaneOpenItemGroup
    let paneLabel: String
    let theme: EditorFileTreeSidebarTheme
    let controlHeight: CGFloat
    let canCloseItem: (EditorPaneOpenItemRow) -> Bool
    let onActivateItem: (EditorPaneOpenItemRow) -> Void
    let onCloseItem: (EditorPaneOpenItemRow) -> Void

    var body: some View {
        HStack(spacing: 0) {
            ForEach(group.items) { item in
                EditorPaneItemTabView(
                    item: item,
                    theme: theme,
                    isActivePane: group.isActivePane,
                    controlHeight: controlHeight,
                    canClose: canCloseItem(item),
                    onActivate: { onActivateItem(item) },
                    onClose: { onCloseItem(item) }
                )
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
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
    let theme: EditorFileTreeSidebarTheme
    let isActivePane: Bool
    let controlHeight: CGFloat
    let canClose: Bool
    let onActivate: () -> Void
    let onClose: () -> Void

    @State private var isHovered = false

    var body: some View {
        HStack(spacing: 5) {
            Button(action: onActivate) {
                HStack(spacing: 7) {
                    Image(systemName: symbolName(for: item.iconName, isDirectory: false))
                        .font(.system(size: 11, weight: .medium))
                    Text(item.title)
                        .font(.system(size: 11.5, weight: item.isActive ? .semibold : .medium))
                        .lineLimit(1)
                    if item.isModified {
                        Circle()
                            .fill(Color(nsColor: theme.selectionColor).opacity(0.92))
                            .frame(width: 5, height: 5)
                    }
                }
                .foregroundStyle(foregroundColor)
                .padding(.leading, 12)
                .padding(.trailing, canClose ? 0 : 10)
                .frame(maxWidth: .infinity, minHeight: controlHeight, maxHeight: controlHeight, alignment: .leading)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            if canClose {
                Button(action: onClose) {
                    Image(systemName: "xmark")
                        .font(.system(size: 8.5, weight: .bold))
                        .foregroundStyle(Color(nsColor: closeButtonForegroundColor))
                        .frame(width: 16, height: 16)
                        .background(
                            Circle()
                                .fill(Color(nsColor: closeButtonBackgroundColor))
                        )
                }
                .buttonStyle(.plain)
                .padding(.trailing, 8)
                .help("Close \(item.title)")
                .accessibilityLabel("Close \(item.title)")
            }
        }
        .frame(maxWidth: .infinity, minHeight: controlHeight, maxHeight: controlHeight, alignment: .leading)
        .background(tabBackground)
        .overlay(tabBorder)
        .contentShape(Rectangle())
        .onHover { hovering in
            isHovered = hovering
        }
        .help(item.filePath ?? item.title)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(item.title)
    }

    private var foregroundColor: Color {
        if item.isActive {
            return .primary
        }
        return isActivePane ? .secondary.opacity(0.95) : .secondary.opacity(0.82)
    }

    @ViewBuilder
    private var tabBackground: some View {
        UnevenRoundedRectangle(
            topLeadingRadius: 8,
            bottomLeadingRadius: 0,
            bottomTrailingRadius: 0,
            topTrailingRadius: 8,
            style: .continuous
        )
        .fill(item.isActive ? activeBackgroundColor : inactiveBackgroundColor)
    }

    @ViewBuilder
    private var tabBorder: some View {
        UnevenRoundedRectangle(
            topLeadingRadius: 8,
            bottomLeadingRadius: 0,
            bottomTrailingRadius: 0,
            topTrailingRadius: 8,
            style: .continuous
        )
        .stroke(item.isActive ? activeBorderColor : inactiveBorderColor, lineWidth: 1)
    }

    private var activeBackgroundColor: Color {
        Color(nsColor: theme.backgroundColor)
    }

    private var inactiveBackgroundColor: Color {
        Color(nsColor: theme.backgroundColor)
            .opacity(isActivePane ? 0.72 : 0.58)
    }

    private var activeBorderColor: Color {
        Color(nsColor: theme.selectionColor).opacity(0.72)
    }

    private var inactiveBorderColor: Color {
        Color(nsColor: theme.separatorColor).opacity(0.68)
    }

    private var closeButtonForegroundColor: NSColor {
        if item.isActive || isHovered {
            return .labelColor
        }
        return .secondaryLabelColor
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
                        .foregroundStyle(.secondary)
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
                        .foregroundStyle(.secondary)
                        .frame(width: 10, height: 10)
                } else {
                    Color.clear
                        .frame(width: 10, height: 10)
                }

                Image(systemName: symbolName(for: row.iconName, isDirectory: row.isDirectory))
                    .font(.system(size: 11, weight: row.isDirectory ? .medium : .regular))
                    .foregroundStyle(Color(nsColor: iconColor))
                    .frame(width: 12)

                Text(row.displayName)
                    .font(.system(size: 12, weight: row.isDirectory ? .medium : .regular))
                    .foregroundStyle(rowTextColor)
                    .lineLimit(1)

                Spacer(minLength: 6)

                EditorSidebarRowDecorationsView(
                    vcsKind: row.vcsKind,
                    diagnosticSeverity: row.diagnosticSeverity
                )

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
            return .labelColor
        }
        if row.isCurrentFile {
            return theme.selectionColor
        }
        return row.isDirectory ? .secondaryLabelColor : .tertiaryLabelColor
    }

    private var rowTextColor: Color {
        if row.isSelected || row.isCurrentFile {
            return .primary
        }
        return .primary.opacity(0.88)
    }
}

private struct EditorSidebarRowDecorationsView: View {
    let vcsKind: EditorFileTreeVcsKind?
    let diagnosticSeverity: EditorDiagnosticSeverity?

    var body: some View {
        HStack(spacing: 6) {
            if let vcsKind {
                Image(systemName: symbolName(for: fileTreeVcsIconName(vcsKind), isDirectory: false))
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(fileTreeBadgeColor(for: fileTreeVcsSeverity(vcsKind)))
                    .accessibilityLabel(Text(fileTreeVcsAccessibilityLabel(vcsKind)))
            }
            if let diagnosticSeverity {
                Image(systemName: diagnosticSeverity.symbolName)
                    .font(.system(size: 10, weight: .semibold))
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

    var body: some View {
        HStack(spacing: 12) {
            ModePill(mode: mode)

            if let lspStatus {
                LSPStatusAccessoryView(status: lspStatus)
            }

            Spacer(minLength: 12)

            HStack(spacing: 10) {
                ForEach(nonLSPStatusItems) { item in
                    StatusAccessoryItemView(item: item)
                }

                ForEach(metadataItems) { item in
                    StatusAccessoryItemView(item: item)
                }

                Text(chrome.statusBar.cursorText)
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundStyle(.primary)
                    .lineLimit(1)
                    .padding(.leading, 4)
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
}

private struct StatusAccessoryItemView: View {
    let item: EditorStatusItem

    private var normalized: (icon: String?, text: String) {
        normalizedStatusItemDisplay(icon: item.icon, text: item.text)
    }

    var body: some View {
        HStack(spacing: 5) {
            if let icon = normalized.icon {
                Image(systemName: symbolName(for: icon, isDirectory: false))
                    .font(.system(size: 10, weight: .semibold))
            }

            if showsText, !normalized.text.isEmpty {
                Text(normalized.text)
                    .font(textFont)
                    .lineLimit(1)
            }
        }
        .foregroundStyle(foregroundStyle)
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
            case "pi":
                return .purple
            case "copilot", "copilot_disabled", "copilot_init", "copilot_error", "supermaven", "supermaven_disabled", "supermaven_init", "supermaven_error":
                return .accentColor
            default:
                break
            }
        }

        switch item.emphasis {
        case .normal:
            return .primary
        case .muted:
            return .secondary
        case .strong:
            return .primary
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
                .foregroundStyle(.primary)
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
            return .secondary
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
            return Color.primary.opacity(0.05)
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
            return Color.primary.opacity(0.06)
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

private struct ModePill: View {
    let mode: EditorMode

    var body: some View {
        Text(label)
            .font(.system(size: 10, weight: .semibold, design: .rounded))
            .foregroundStyle(foreground)
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(
                Capsule(style: .continuous)
                    .fill(background)
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

    private var foreground: Color {
        switch mode {
        case .normal:
            return .secondary
        case .insert:
            return .accentColor
        case .select:
            return .purple
        case .command:
            return .orange
        }
    }

    private var background: Color {
        switch mode {
        case .normal:
            return Color.secondary.opacity(0.12)
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
    @Published var sidebarWidth: CGFloat = 52
    @Published var document: EditorDocumentChrome = .empty
}

private final class EditorTitlebarSidebarSeparatorView: NSView {
    private let lineLayer = CALayer()
    var separatorColor: NSColor = .separatorColor {
        didSet { updateAppearance() }
    }
    private var isHovering = false {
        didSet { updateAppearance() }
    }
    private var trackingAreaRef: NSTrackingArea?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
        lineLayer.actions = ["bounds": NSNull(), "position": NSNull(), "backgroundColor": NSNull()]
        layer?.addSublayer(lineLayer)
        updateAppearance()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func layout() {
        super.layout()
        layoutLine()
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let trackingAreaRef {
            removeTrackingArea(trackingAreaRef)
        }
        let trackingArea = NSTrackingArea(
            rect: bounds,
            options: [.activeAlways, .mouseEnteredAndExited, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(trackingArea)
        trackingAreaRef = trackingArea
    }

    override func mouseEntered(with event: NSEvent) {
        isHovering = true
        NSCursor.resizeLeftRight.set()
        super.mouseEntered(with: event)
    }

    override func mouseExited(with event: NSEvent) {
        isHovering = false
        super.mouseExited(with: event)
    }

    override func resetCursorRects() {
        addCursorRect(bounds, cursor: .resizeLeftRight)
    }

    private func updateAppearance() {
        lineLayer.backgroundColor = separatorColor.withAlphaComponent(isHovering ? 1.0 : 0.9).cgColor
        layoutLine()
    }

    private func layoutLine() {
        let lineWidth: CGFloat = isHovering ? 2 : 1
        lineLayer.frame = NSRect(x: max(bounds.width - lineWidth, 0), y: 0, width: lineWidth, height: bounds.height)
    }
}

private struct EditorWindowChromeAccessor: NSViewRepresentable {
    let chrome: EditorChromeModel
    let fileTreeVisible: Bool
    let fileTreeWidth: CGFloat
    let fileTreeBackgroundColor: NSColor
    let fileTreeSeparatorColor: NSColor
    let isPiSidebarVisible: Bool
    let onToggleFileTree: () -> Void
    let onOpenTerminal: () -> Void
    let onTogglePiSidebar: () -> Void

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
                fileTreeWidth: fileTreeWidth,
                fileTreeBackgroundColor: fileTreeBackgroundColor,
                fileTreeSeparatorColor: fileTreeSeparatorColor,
                isPiSidebarVisible: isPiSidebarVisible,
                onToggleFileTree: onToggleFileTree,
                onOpenTerminal: onOpenTerminal,
                onTogglePiSidebar: onTogglePiSidebar
            )
            return
        }

        DispatchQueue.main.async { [weak nsView] in
            guard let nsView, let window = nsView.window else { return }
            context.coordinator.configure(
                window: window,
                chrome: chrome,
                fileTreeVisible: fileTreeVisible,
                fileTreeWidth: fileTreeWidth,
                fileTreeBackgroundColor: fileTreeBackgroundColor,
                fileTreeSeparatorColor: fileTreeSeparatorColor,
                isPiSidebarVisible: isPiSidebarVisible,
                onToggleFileTree: onToggleFileTree,
                onOpenTerminal: onOpenTerminal,
                onTogglePiSidebar: onTogglePiSidebar
            )
        }
    }

    @MainActor
    final class Coordinator: NSObject, NSToolbarDelegate {
        private let toolbarIdentifier = NSToolbar.Identifier("TheSwiftPOC.TitlebarToolbar")
        private let leadingItemIdentifier = NSToolbarItem.Identifier("TheSwiftPOC.LeadingRegion")
        private let vcsItemIdentifier = NSToolbarItem.Identifier("TheSwiftPOC.VCSInfo")
        private let leadingState = EditorTitlebarLeadingState()
        private lazy var fileTreeHostingView = NSHostingView(rootView: EditorTitlebarLeadingRegionView(state: leadingState, onToggle: {}, onOpenTerminal: {}, showsTerminalButton: GhosttyTerminalRegistry.isAvailable))
        private let vcsHostingView = NSHostingView(rootView: EditorTitlebarTrailingRegionView(vcsText: nil, isPiSidebarVisible: false, onTogglePiSidebar: {}, showsPiButton: GhosttyTerminalRegistry.isAvailable))
        private let sidebarTitlebarBackgroundView = NSView(frame: .zero)
        private let sidebarTitlebarSeparatorView = EditorTitlebarSidebarSeparatorView(frame: .zero)
        private weak var observedWindow: NSWindow?
        private var lastChrome: EditorChromeModel = .empty
        private var lastFileTreeVisible = false
        private var lastFileTreeWidth: CGFloat = 52
        private var lastFileTreeBackgroundColor: NSColor = .windowBackgroundColor
        private var lastFileTreeSeparatorColor: NSColor = .separatorColor
        private var lastPiSidebarVisible = false
        private var toggleFileTreeAction: (() -> Void)?
        private var openTerminalAction: (() -> Void)?
        private var togglePiSidebarAction: (() -> Void)?
        private lazy var toolbar: NSToolbar = {
            let toolbar = NSToolbar(identifier: toolbarIdentifier)
            toolbar.delegate = self
            toolbar.displayMode = .iconOnly
            toolbar.allowsUserCustomization = false
            toolbar.autosavesConfiguration = false
            toolbar.showsBaselineSeparator = false
            return toolbar
        }()

        override init() {
            super.init()
            fileTreeHostingView.translatesAutoresizingMaskIntoConstraints = false
            fileTreeHostingView.setContentCompressionResistancePriority(.required, for: .horizontal)
            fileTreeHostingView.setContentHuggingPriority(.required, for: .horizontal)
            vcsHostingView.translatesAutoresizingMaskIntoConstraints = false
            vcsHostingView.setContentCompressionResistancePriority(.required, for: .horizontal)
            vcsHostingView.setContentHuggingPriority(.required, for: .horizontal)
            sidebarTitlebarBackgroundView.wantsLayer = true
            sidebarTitlebarBackgroundView.autoresizingMask = [.height]
            sidebarTitlebarSeparatorView.autoresizingMask = [.height]
        }

        deinit {
            NotificationCenter.default.removeObserver(self)
        }

        func configure(
            window: NSWindow,
            chrome: EditorChromeModel,
            fileTreeVisible: Bool,
            fileTreeWidth: CGFloat,
            fileTreeBackgroundColor: NSColor,
            fileTreeSeparatorColor: NSColor,
            isPiSidebarVisible: Bool,
            onToggleFileTree: @escaping () -> Void,
            onOpenTerminal: @escaping () -> Void,
            onTogglePiSidebar: @escaping () -> Void
        ) {
            let started = CFAbsoluteTimeGetCurrent()
            let windowChanged = observedWindow !== window
            let chromeChanged = !chrome.matches(lastChrome)
            let fileTreeChanged = fileTreeVisible != lastFileTreeVisible
            let widthChanged = abs(fileTreeWidth - lastFileTreeWidth) > 0.5
            let sidebarColorChanged = !fileTreeBackgroundColor.isEqual(lastFileTreeBackgroundColor)
            let separatorColorChanged = !fileTreeSeparatorColor.isEqual(lastFileTreeSeparatorColor)
            let piSidebarChanged = isPiSidebarVisible != lastPiSidebarVisible
            toggleFileTreeAction = onToggleFileTree
            openTerminalAction = onOpenTerminal
            togglePiSidebarAction = onTogglePiSidebar
            attachWindowObserversIfNeeded(window: window)
            installToolbarIfNeeded(window: window)
            guard windowChanged || chromeChanged || fileTreeChanged || widthChanged || sidebarColorChanged || separatorColorChanged || piSidebarChanged else {
                scrollPerfLog("chrome.configure skipped windowChanged=\(windowChanged) chromeChanged=\(chromeChanged) fileTreeChanged=\(fileTreeChanged) widthChanged=\(widthChanged) piSidebarChanged=\(piSidebarChanged)")
                return
            }
            lastChrome = chrome
            lastFileTreeVisible = fileTreeVisible
            lastFileTreeWidth = fileTreeWidth
            lastFileTreeBackgroundColor = fileTreeBackgroundColor
            lastFileTreeSeparatorColor = fileTreeSeparatorColor
            lastPiSidebarVisible = isPiSidebarVisible
            let applyStarted = CFAbsoluteTimeGetCurrent()
            applyWindowChrome(window: window, chrome: chrome)
            layoutSidebarTitlebarBackground(
                window: window,
                visible: fileTreeVisible,
                width: fileTreeWidth,
                backgroundColor: fileTreeBackgroundColor,
                separatorColor: fileTreeSeparatorColor
            )
            let applyMs = (CFAbsoluteTimeGetCurrent() - applyStarted) * 1000
            let toolbarStarted = CFAbsoluteTimeGetCurrent()
            updateToolbarContent(window: window, chrome: chrome, fileTreeVisible: fileTreeVisible, fileTreeWidth: fileTreeWidth, isPiSidebarVisible: isPiSidebarVisible)
            let toolbarMs = (CFAbsoluteTimeGetCurrent() - toolbarStarted) * 1000
            let totalMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
            scrollPerfLog(
                "chrome.configure windowChanged=\(windowChanged) chromeChanged=\(chromeChanged) fileTreeChanged=\(fileTreeChanged) widthChanged=\(widthChanged) piSidebarChanged=\(piSidebarChanged) applyMs=\(String(format: "%.2f", applyMs)) toolbarMs=\(String(format: "%.2f", toolbarMs)) totalMs=\(String(format: "%.2f", totalMs))"
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
        }

        private func installToolbarIfNeeded(window: NSWindow) {
            if window.toolbar?.identifier != toolbarIdentifier {
                window.toolbar = toolbar
            }
            window.toolbarStyle = .unifiedCompact
        }

        @objc private func windowDidChangeState(_ notification: Notification) {
            guard let window = notification.object as? NSWindow else { return }
            applyWindowChrome(window: window, chrome: lastChrome)
            layoutSidebarTitlebarBackground(
                window: window,
                visible: lastFileTreeVisible,
                width: lastFileTreeWidth,
                backgroundColor: lastFileTreeBackgroundColor,
                separatorColor: lastFileTreeSeparatorColor
            )
            updateToolbarContent(window: window, chrome: lastChrome, fileTreeVisible: lastFileTreeVisible, fileTreeWidth: lastFileTreeWidth, isPiSidebarVisible: lastPiSidebarVisible)
        }

        private func applyWindowChrome(window: NSWindow, chrome: EditorChromeModel) {
            let backgroundColor = chrome.backgroundColor
            window.titleVisibility = .hidden
            window.titlebarAppearsTransparent = false
            window.titlebarSeparatorStyle = .none
            window.toolbarStyle = .unifiedCompact
            window.backgroundColor = backgroundColor.withAlphaComponent(1)
            window.isDocumentEdited = chrome.document.isModified
            window.appearance = backgroundColor.isLightColor
                ? NSAppearance(named: .aqua)
                : NSAppearance(named: .darkAqua)

            let title = windowTitle(for: chrome)
            if window.title != title {
                window.title = title
            }

            if let absolutePath = chrome.document.absolutePath, !absolutePath.isEmpty {
                window.representedURL = URL(fileURLWithPath: absolutePath)
            } else {
                window.representedURL = nil
            }

            applyTitlebarBackground(window: window, color: backgroundColor)
        }

        private func updateToolbarContent(window: NSWindow, chrome: EditorChromeModel, fileTreeVisible: Bool, fileTreeWidth: CGFloat, isPiSidebarVisible: Bool) {
            fileTreeHostingView.rootView = EditorTitlebarLeadingRegionView(
                state: leadingState,
                onToggle: { self.toggleFileTreeAction?() },
                onOpenTerminal: { self.openTerminalAction?() },
                showsTerminalButton: GhosttyTerminalRegistry.isAvailable
            )
            withAnimation(.spring(response: 0.24, dampingFraction: 0.88)) {
                leadingState.isSidebarActive = fileTreeVisible
                leadingState.sidebarWidth = fileTreeWidth
                leadingState.document = chrome.document
            }
            vcsHostingView.rootView = EditorTitlebarTrailingRegionView(
                vcsText: chrome.document.vcsText,
                isPiSidebarVisible: isPiSidebarVisible,
                onTogglePiSidebar: { self.togglePiSidebarAction?() },
                showsPiButton: GhosttyTerminalRegistry.isAvailable
            )
            fileTreeHostingView.invalidateIntrinsicContentSize()
            vcsHostingView.invalidateIntrinsicContentSize()
            window.toolbar?.validateVisibleItems()
        }

        private func layoutSidebarTitlebarBackground(
            window: NSWindow,
            visible: Bool,
            width: CGFloat,
            backgroundColor: NSColor,
            separatorColor: NSColor
        ) {
            guard let titlebarContainer = titlebarContainer(for: window) else { return }
            if sidebarTitlebarBackgroundView.superview !== titlebarContainer {
                sidebarTitlebarBackgroundView.removeFromSuperview()
                titlebarContainer.addSubview(sidebarTitlebarBackgroundView, positioned: .below, relativeTo: nil)
            }
            if sidebarTitlebarSeparatorView.superview !== titlebarContainer {
                sidebarTitlebarSeparatorView.removeFromSuperview()
                titlebarContainer.addSubview(sidebarTitlebarSeparatorView)
            }

            let resolvedWidth = visible ? width : 0
            let handleWidth: CGFloat = 8
            let backgroundWidth = max(resolvedWidth - handleWidth, 0)
            let separatorFrame = NSRect(x: backgroundWidth, y: 0, width: handleWidth, height: titlebarContainer.bounds.height)
            let backgroundFrame = NSRect(x: 0, y: 0, width: backgroundWidth, height: titlebarContainer.bounds.height)

            sidebarTitlebarBackgroundView.isHidden = backgroundWidth <= 0.5
            sidebarTitlebarBackgroundView.layer?.backgroundColor = backgroundColor.cgColor
            sidebarTitlebarBackgroundView.frame = backgroundFrame

            sidebarTitlebarSeparatorView.isHidden = resolvedWidth <= 0.5
            sidebarTitlebarSeparatorView.separatorColor = separatorColor
            sidebarTitlebarSeparatorView.frame = separatorFrame
        }

        private func applyTitlebarBackground(window: NSWindow, color: NSColor) {
            if #available(macOS 26.0, *) {
                if let titlebarView = titlebarView(for: window) {
                    titlebarView.wantsLayer = true
                    titlebarView.layer?.backgroundColor = color.cgColor
                }
                titlebarBackgroundView(for: window)?.isHidden = true
            } else {
                window.titlebarAppearsTransparent = true
                guard let titlebarContainer = titlebarContainer(for: window) else { return }
                titlebarContainer.wantsLayer = true
                titlebarContainer.layer?.backgroundColor = color.cgColor
                hideFirstEffectView(in: titlebarContainer)
            }
        }

        private func windowTitle(for chrome: EditorChromeModel) -> String {
            if let relativePath = chrome.document.relativePath, !relativePath.isEmpty {
                return "\(relativePath)/\(chrome.document.name)"
            }
            return chrome.document.name
        }

        private func titlebarContainer(for window: NSWindow) -> NSView? {
            if !window.styleMask.contains(.fullScreen) {
                guard let contentView = window.contentView else { return nil }
                return firstViewFromRoot(startingAt: contentView, classNameContains: "NSTitlebarContainerView")
            }

            for appWindow in NSApplication.shared.windows {
                guard NSStringFromClass(type(of: appWindow)).contains("NSToolbarFullScreenWindow") else { continue }
                guard appWindow.parent == window else { continue }
                guard let contentView = appWindow.contentView else { continue }
                return firstViewFromRoot(startingAt: contentView, classNameContains: "NSTitlebarContainerView")
            }
            return nil
        }

        private func titlebarView(for window: NSWindow) -> NSView? {
            titlebarContainer(for: window).flatMap { firstView(from: $0, classNameContains: "NSTitlebarView") }
        }

        private func titlebarBackgroundView(for window: NSWindow) -> NSView? {
            titlebarContainer(for: window).flatMap { firstView(from: $0, classNameContains: "NSTitlebarBackgroundView") }
        }

        private func firstViewFromRoot(startingAt view: NSView, classNameContains needle: String) -> NSView? {
            var root = view
            while let superview = root.superview {
                root = superview
            }
            return firstView(from: root, classNameContains: needle)
        }

        private func firstView(from root: NSView, classNameContains needle: String) -> NSView? {
            if NSStringFromClass(type(of: root)).contains(needle) {
                return root
            }
            for subview in root.subviews {
                if let match = firstView(from: subview, classNameContains: needle) {
                    return match
                }
            }
            return nil
        }

        private func hideFirstEffectView(in root: NSView) {
            if let effectView = firstEffectView(in: root) {
                effectView.isHidden = true
            }
        }

        private func firstEffectView(in root: NSView) -> NSVisualEffectView? {
            if let effectView = root as? NSVisualEffectView {
                return effectView
            }
            for subview in root.subviews {
                if let match = firstEffectView(in: subview) {
                    return match
                }
            }
            return nil
        }

        func toolbarAllowedItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
            [leadingItemIdentifier, .flexibleSpace, vcsItemIdentifier]
        }

        func toolbarDefaultItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
            [leadingItemIdentifier, .flexibleSpace, vcsItemIdentifier]
        }

        func toolbar(
            _ toolbar: NSToolbar,
            itemForItemIdentifier itemIdentifier: NSToolbarItem.Identifier,
            willBeInsertedIntoToolbar flag: Bool
        ) -> NSToolbarItem? {
            let item = NSToolbarItem(itemIdentifier: itemIdentifier)
            item.isBordered = false

            switch itemIdentifier {
            case leadingItemIdentifier:
                item.view = fileTreeHostingView
                item.visibilityPriority = .high
                return item
            case vcsItemIdentifier:
                item.view = vcsHostingView
                item.visibilityPriority = .low
                return item
            default:
                return nil
            }
        }
    }
}

private struct EditorTitlebarLeadingRegionView: View {
    @ObservedObject var state: EditorTitlebarLeadingState
    let onToggle: () -> Void
    let onOpenTerminal: () -> Void
    let showsTerminalButton: Bool

    var body: some View {
        HStack(spacing: 0) {
            HStack(spacing: 0) {
                EditorTitlebarSidebarToggleButton(isActive: state.isSidebarActive, onToggle: onToggle)
                    .padding(.leading, 10)
                Spacer(minLength: 0)
            }
            .frame(width: max(state.sidebarWidth, 52), height: 24, alignment: .leading)

            HStack(spacing: 8) {
                EditorTitlebarDocumentView(document: state.document)
                if showsTerminalButton {
                    EditorTitlebarOpenTerminalButton(onOpenTerminal: onOpenTerminal)
                }
            }
            .padding(.leading, 8)
        }
        .fixedSize(horizontal: true, vertical: true)
        .animation(.spring(response: 0.24, dampingFraction: 0.88), value: state.sidebarWidth)
        .animation(.spring(response: 0.24, dampingFraction: 0.88), value: state.document)
    }
}

private struct EditorTitlebarSidebarToggleButton: View {
    let isActive: Bool
    let onToggle: () -> Void

    var body: some View {
        Button(action: toggle) {
            Image(systemName: "sidebar.left")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(isActive ? .primary : .secondary)
                .frame(width: 28, height: 24)
                .background {
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .fill(isActive ? Color.primary.opacity(0.10) : Color.clear)
                }
        }
        .buttonStyle(.plain)
        .help("Toggle Sidebar")
        .accessibilityLabel("Toggle Sidebar")
    }

    private func toggle() {
        withAnimation(.spring(response: 0.24, dampingFraction: 0.88)) {
            onToggle()
        }
    }
}

private struct EditorTitlebarOpenTerminalButton: View {
    let onOpenTerminal: () -> Void

    var body: some View {
        Button(action: onOpenTerminal) {
            Label("New Terminal", systemImage: "terminal")
                .labelStyle(.iconOnly)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.secondary)
                .frame(width: 28, height: 24)
                .background {
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .fill(Color.primary.opacity(0.08))
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

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: symbolName(for: document.icon, isDirectory: false))
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.secondary)

            Text(document.name)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.primary)
                .lineLimit(1)
        }
        .fixedSize()
        .allowsHitTesting(false)
        .accessibilityElement(children: .combine)
    }
}

private struct EditorTitlebarTrailingRegionView: View {
    let vcsText: String?
    let isPiSidebarVisible: Bool
    let onTogglePiSidebar: () -> Void
    let showsPiButton: Bool

    var body: some View {
        HStack(spacing: 8) {
            if showsPiButton {
                EditorTitlebarPiSidebarButton(isActive: isPiSidebarVisible, onToggle: onTogglePiSidebar)
            }
            EditorTitlebarVCSView(vcsText: vcsText)
        }
        .fixedSize(horizontal: true, vertical: true)
    }
}

private struct EditorTitlebarPiSidebarButton: View {
    let isActive: Bool
    let onToggle: () -> Void

    var body: some View {
        Button(action: onToggle) {
            Text("π")
                .font(.system(size: 13, weight: .semibold, design: .rounded))
                .foregroundStyle(isActive ? .primary : .secondary)
                .frame(width: 28, height: 24)
                .background {
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .fill(isActive ? Color.primary.opacity(0.12) : Color.primary.opacity(0.06))
                }
        }
        .buttonStyle(.plain)
        .help("Toggle PI Sidebar")
        .accessibilityLabel("Toggle PI Sidebar")
        .accessibilityValue(isActive ? "visible" : "hidden")
    }
}

private struct EditorTitlebarVCSView: View {
    let vcsText: String?

    private var trimmedVCSText: String? {
        let trimmed = vcsText?.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let trimmed, !trimmed.isEmpty else { return nil }
        return trimmed
    }

    var body: some View {
        Group {
            if let trimmedVCSText {
                HStack(spacing: 6) {
                    Image(systemName: symbolName(for: "git_branch", isDirectory: false))
                        .font(.system(size: 11, weight: .semibold))
                    Text(trimmedVCSText)
                        .font(.system(size: 12, weight: .medium))
                        .lineLimit(1)
                }
                .foregroundStyle(.secondary)
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

private func symbolName(for icon: String, isDirectory: Bool) -> String {
    switch icon {
    case "folder", "folder_open", "folder_search":
        return isDirectory ? "folder.fill" : "folder"
    case "book":
        return "book.closed"
    case "swift":
        return "swift"
    case "rust", "file_rust":
        return "gearshape.2"
    case "file_markdown":
        return "doc.text"
    case "terminal":
        return "terminal"
    case "image":
        return "photo"
    case "json", "file_toml", "settings", "tool_hammer":
        return "doc.badge.gearshape"
    case "git_branch":
        return "point.topleft.down.curvedto.point.bottomright.up"
    case "git_conflict":
        return "exclamationmark.octagon.fill"
    case "git_deleted":
        return "trash.fill"
    case "git_modified":
        return "circle.fill"
    case "git_renamed":
        return "arrow.left.arrow.right"
    case "git_untracked":
        return "plus.circle.fill"
    case "diagnostic_error":
        return "xmark.circle.fill"
    case "diagnostic_warning":
        return "exclamationmark.triangle.fill"
    case "diagnostic_info":
        return "info.circle.fill"
    case "diagnostic_hint":
        return "lightbulb.fill"
    case "pi":
        return "sparkles"
    case "curlybraces", "textformat", "return":
        return icon
    case "copilot", "supermaven":
        return "wand.and.stars"
    case "copilot_disabled", "supermaven_disabled":
        return "slash.circle"
    case "copilot_init", "supermaven_init":
        return "arrow.triangle.2.circlepath"
    case "copilot_error", "supermaven_error":
        return "exclamationmark.circle"
    default:
        return isDirectory ? "folder" : "doc"
    }
}
