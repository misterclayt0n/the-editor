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

struct EditorChromeView: View {
    @ObservedObject var controller: EditorSurfaceController

    @AppStorage("swift.fileTreeSidebarWidth") private var storedFileTreeWidth: Double = 280
    @GestureState private var fileTreeDragTranslation: CGFloat = 0
    @State private var isFileTreeResizeActive = false

    private let minimumFileTreeWidth: CGFloat = 180
    private let maximumFileTreeWidth: CGFloat = 460

    var body: some View {
        HStack(spacing: 0) {
            if controller.fileTree.isVisible {
                sidebarColumn
                EditorSidebarResizeHandle(
                    color: fileTreeTheme.separatorColor,
                    gesture: fileTreeResizeGesture
                )
            }

            mainColumn
        }
        .background(
            EditorWindowChromeAccessor(
                chrome: controller.chrome,
                fileTreeVisible: controller.fileTree.isVisible,
                fileTreeWidth: titlebarSidebarRegionWidth,
                fileTreeBackgroundColor: fileTreeTheme.backgroundColor,
                fileTreeSeparatorColor: fileTreeTheme.separatorColor,
                onToggleFileTree: controller.toggleFileTree
            )
        )
        .overlay(alignment: .bottom) {
            if let pendingKeys = controller.pendingKeys {
                EditorPendingKeyIndicatorView(pendingKeys: pendingKeys)
                    .padding(.bottom, 38)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .animation(.spring(response: 0.24, dampingFraction: 0.88), value: controller.pendingKeys?.pendingDisplay)
    }

    private var sidebarColumn: some View {
        EditorFileTreeSidebarView(
            tree: controller.fileTree,
            theme: fileTreeTheme,
            onSelectIndex: controller.clickFileTreeIndex,
            onActivateIndex: controller.activateFileTreeIndex,
            onVisibleRowsChanged: controller.setFileTreeVisibleRows,
            onFocusSidebar: { controller.setFileTreeActive(true) }
        )
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .onAppear {
            let widthText = String(format: "%.1f", fileTreeWidth)
            scrollPerfLog("fileTree.sidebar appear rows=\(controller.fileTree.rows.count) width=\(widthText)")
        }
        .onDisappear {
            scrollPerfLog("fileTree.sidebar disappear")
        }
        .background(Color(nsColor: fileTreeTheme.backgroundColor))
        .frame(width: fileTreeWidth)
        .background(Color(nsColor: fileTreeTheme.backgroundColor))
    }

    private var mainColumn: some View {
        VStack(spacing: 0) {
            ZStack {
                EditorSurfaceRepresentable(controller: controller)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)

                EditorDiagnosticsOverlayView(controller: controller)
                EditorDocsPanelsView(controller: controller)
                EditorCompletionMenuView(controller: controller)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)

            EditorStatusAccessoryView(chrome: controller.chrome, mode: controller.currentMode)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var fileTreeTheme: EditorFileTreeSidebarTheme {
        EditorFileTreeSidebarTheme.resolve(scene: controller.scene, chrome: controller.chrome)
    }

    private var fileTreeWidth: CGFloat {
        clampFileTreeWidth(CGFloat(storedFileTreeWidth) + fileTreeDragTranslation)
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

    private func clampFileTreeWidth(_ width: CGFloat) -> CGFloat {
        min(max(width, minimumFileTreeWidth), maximumFileTreeWidth)
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
    let onSelectIndex: (Int) -> Void
    let onActivateIndex: (Int) -> Void
    let onVisibleRowsChanged: (Int) -> Void
    let onFocusSidebar: () -> Void

    @State private var hoveredRowID: String?
    @State private var reportedVisibleRows: Int = 1
    @State private var scrollViewportHeight: CGFloat = 1
    @State private var observedContentMinY: CGFloat = 0
    @State private var observedTopRow: Int = 0
    @State private var lastScrollLogSignature: String?
    @State private var lastProgrammaticScrollSignature: String?

    private let headerHeight: CGFloat = 30
    private let rowHeight: CGFloat = 24
    private let scrollContentVerticalPadding: CGFloat = 6
    private let scrollCoordinateSpaceName = "EditorFileTreeScrollView"

    var body: some View {
        VStack(spacing: 0) {
            header

            ScrollViewReader { proxy in
                ScrollView(.vertical, showsIndicators: true) {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        fileTreeScrollSentinel

                        if !tree.rows.isEmpty {
                            fileTreeScrollObservationSentinel
                        }

                        if tree.rows.isEmpty {
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
                        } else {
                            ForEach(Array(tree.rows.enumerated()), id: \.element.id) { index, row in
                                EditorFileTreeRowView(
                                    row: row,
                                    theme: theme,
                                    isHovered: hoveredRowID == row.id,
                                    onSelect: { onSelectIndex(index) },
                                    onActivate: { onActivateIndex(index) }
                                )
                                .id(row.id)
                                .onHover { isHovering in
                                    hoveredRowID = isHovering ? row.id : (hoveredRowID == row.id ? nil : hoveredRowID)
                                }
                            }
                        }
                    }
                    .padding(.horizontal, 6)
                    .padding(.vertical, scrollContentVerticalPadding)
                }
                .coordinateSpace(name: scrollCoordinateSpaceName)
                .onPreferenceChange(EditorFileTreeContentMinYPreferenceKey.self) { minY in
                    updateObservedScrollPosition(contentMinY: minY, source: "geometry")
                }
                .scrollContentBackground(.hidden)
                .scrollIndicators(.visible)
                .background(Color.clear)
                .simultaneousGesture(TapGesture().onEnded {
                    onFocusSidebar()
                })
                .background {
                    GeometryReader { proxy in
                        Color.clear
                            .onChange(of: proxy.size.height, initial: true) { _, newHeight in
                                scrollViewportHeight = newHeight
                                updateVisibleRows(for: newHeight)
                            }
                    }
                }
                .overlay(alignment: .trailing) {
                    fileTreeScrollbarThumb
                        .padding(.trailing, 2)
                }
                .onAppear {
                    scheduleProgrammaticScroll(proxy: proxy, reason: "appear")
                }
                .onChange(of: tree.scrollOffset, initial: true) { _, _ in
                    logScrollObservation(source: "snapshot", requestedTopRow: tree.scrollOffset)
                    scheduleProgrammaticScroll(proxy: proxy, reason: "scrollOffset-change")
                }
                .onChange(of: tree.selectedIndex, initial: true) { _, _ in
                    logScrollObservation(source: "selected-change", requestedTopRow: tree.scrollOffset)
                    scheduleProgrammaticScroll(proxy: proxy, reason: "selected-change")
                }
                .onChange(of: tree.rows.count, initial: true) { _, _ in
                    logScrollObservation(source: "rows-change", requestedTopRow: tree.scrollOffset)
                    scheduleProgrammaticScroll(proxy: proxy, reason: "rows-change")
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Color(nsColor: theme.backgroundColor))
        .environment(\.colorScheme, theme.backgroundColor.isLightColor ? .light : .dark)
    }

    private var fileTreeScrollSentinel: some View {
        Color.clear
            .frame(height: 0)
            .background {
                GeometryReader { proxy in
                    Color.clear.preference(
                        key: EditorFileTreeContentMinYPreferenceKey.self,
                        value: proxy.frame(in: .named(scrollCoordinateSpaceName)).minY
                    )
                }
            }
    }

    private var fileTreeScrollObservationSentinel: some View {
        Color.clear
            .frame(height: 0)
    }

    private func updateObservedScrollPosition(contentMinY: CGFloat, source: String) {
        observedContentMinY = contentMinY
        let contentOffset = max(-(contentMinY - scrollContentVerticalPadding), 0)
        let topRow = min(max(Int(floor(contentOffset / rowHeight)), 0), max(tree.rows.count - 1, 0))
        guard topRow != observedTopRow || source != "geometry" else { return }
        observedTopRow = topRow
        logScrollObservation(source: source, requestedTopRow: tree.scrollOffset)
    }

    private func scheduleProgrammaticScroll(proxy: ScrollViewProxy, reason: String) {
        let targetIndex = tree.scrollOffset
        let signature = [reason, String(targetIndex), String(tree.rows.count), String(tree.selectedIndex ?? -1)].joined(separator: ":")
        guard signature != lastProgrammaticScrollSignature else { return }
        lastProgrammaticScrollSignature = signature
        guard tree.rows.indices.contains(targetIndex) else {
            scrollPerfLog(
                "fileTree.scrollTo skipped reason=\(reason) target=\(targetIndex) rows=\(tree.rows.count) selected=\(String(describing: tree.selectedIndex))"
            )
            return
        }
        let rowID = tree.rows[targetIndex].id
        scrollPerfLog(
            "fileTree.scrollTo scheduled reason=\(reason) target=\(targetIndex) rowID=\(rowID) selected=\(String(describing: tree.selectedIndex)) observedTop=\(observedTopRow) rows=\(tree.rows.count)"
        )
        DispatchQueue.main.async {
            proxy.scrollTo(rowID, anchor: .top)
            scrollPerfLog(
                "fileTree.scrollTo executed reason=\(reason) target=\(targetIndex) rowID=\(rowID) rows=\(tree.rows.count)"
            )
        }
    }

    private func logScrollObservation(source: String, requestedTopRow: Int) {
        let topRow = min(max(observedTopRow, 0), max(tree.rows.count - 1, 0))
        let visibleRows = max(reportedVisibleRows, 1)
        let bottomRow = tree.rows.isEmpty ? 0 : min(topRow + visibleRows - 1, tree.rows.count - 1)
        let contentOffset = max(-(observedContentMinY - scrollContentVerticalPadding), 0)
        let offsetText = String(format: "%.1f", contentOffset)
        let drift = topRow - requestedTopRow
        let signature = [
            source,
            String(topRow),
            String(bottomRow),
            String(requestedTopRow),
            String(tree.selectedIndex ?? -1),
            String(tree.rows.count),
            String(visibleRows),
            String(drift),
        ].joined(separator: ":")
        guard signature != lastScrollLogSignature else { return }
        lastScrollLogSignature = signature
        scrollPerfLog(
            "fileTree.scrollObserved source=\(source) offsetY=\(offsetText) top=\(topRow) bottom=\(bottomRow) rustScroll=\(requestedTopRow) drift=\(drift) selected=\(String(describing: tree.selectedIndex)) visibleRows=\(visibleRows) rows=\(tree.rows.count)"
        )
    }

    private var header: some View {
        HStack(spacing: 8) {
            Image(systemName: "folder.fill")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(Color(nsColor: theme.selectionColor).opacity(0.85))
            Text(rootTitle)
                .font(.system(size: 12, weight: .semibold))
                .lineLimit(1)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 12)
        .frame(maxWidth: .infinity, minHeight: headerHeight, maxHeight: headerHeight, alignment: .leading)
        .background(Color(nsColor: theme.headerColor))
        .contentShape(Rectangle())
        .onTapGesture {
            onFocusSidebar()
        }
        .help(tree.root ?? rootTitle)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(Color(nsColor: theme.separatorColor))
                .frame(height: 1)
        }
    }

    private var rootTitle: String {
        guard let root = tree.root, !root.isEmpty else { return "Workspace" }
        let url = URL(fileURLWithPath: root)
        let name = url.lastPathComponent
        return name.isEmpty ? root : name
    }

    @ViewBuilder
    private var fileTreeScrollbarThumb: some View {
        let totalRows = max(tree.rows.count, reportedVisibleRows)
        let visibleRows = max(reportedVisibleRows, 1)
        if totalRows > visibleRows {
            let trackHeight = max(scrollViewportHeight - 8, 1)
            let thumbHeight = max(26, trackHeight * (CGFloat(visibleRows) / CGFloat(totalRows)))
            let maxOffset = max(totalRows - visibleRows, 1)
            let progress = CGFloat(min(max(tree.scrollOffset, 0), maxOffset)) / CGFloat(maxOffset)
            let travel = max(trackHeight - thumbHeight, 0)
            RoundedRectangle(cornerRadius: 2.5, style: .continuous)
                .fill(Color(nsColor: theme.separatorColor).opacity(0.92))
                .frame(width: 4, height: thumbHeight)
                .frame(maxHeight: .infinity, alignment: .top)
                .offset(y: 4 + (travel * progress))
                .allowsHitTesting(false)
        }
    }

    private func updateVisibleRows(for height: CGFloat) {
        let contentHeight = max(height - 12, 1)
        let rows = max(Int(floor(contentHeight / rowHeight)), 1)
        guard rows != reportedVisibleRows else { return }
        let heightText = String(format: "%.1f", height)
        scrollPerfLog(
            "fileTree.visibleRows height=\(heightText) rows=\(rows) previous=\(reportedVisibleRows) scrollOffset=\(tree.scrollOffset) totalRows=\(tree.rows.count)"
        )
        reportedVisibleRows = rows
        onVisibleRowsChanged(rows)
    }
}

private struct EditorFileTreeContentMinYPreferenceKey: PreferenceKey {
    static let defaultValue: CGFloat = 0

    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) {
        value = nextValue()
    }
}

private struct EditorFileTreeRowView: View {
    let row: EditorFileTreeRow
    let theme: EditorFileTreeSidebarTheme
    let isHovered: Bool
    let onSelect: () -> Void
    let onActivate: () -> Void

    var body: some View {
        Button(action: onSelect) {
            HStack(spacing: 0) {
                Rectangle()
                    .fill(Color(nsColor: leadingRailColor))
                    .frame(width: 2)
                    .opacity(row.isSelected || row.isCurrentFile ? 1 : 0)

                HStack(spacing: 6) {
                    if row.hasChildren || row.isDirectory {
                        Button(action: onActivate) {
                            Image(systemName: row.isExpanded ? "chevron.down" : "chevron.right")
                                .font(.system(size: 9, weight: .semibold))
                                .foregroundStyle(.secondary)
                                .frame(width: 10, height: 10)
                        }
                        .buttonStyle(.plain)
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

                    EditorFileTreeRowDecorationsView(row: row)

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
        }
        .buttonStyle(.plain)
        .simultaneousGesture(TapGesture(count: 2).onEnded {
            onActivate()
        })
        .accessibilityAddTraits(row.isSelected ? [.isSelected] : [])
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

private struct EditorFileTreeRowDecorationsView: View {
    let row: EditorFileTreeRow

    var body: some View {
        HStack(spacing: 6) {
            if let vcsKind = row.vcsKind {
                Image(systemName: symbolName(for: fileTreeVcsIconName(vcsKind), isDirectory: false))
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(fileTreeBadgeColor(for: fileTreeVcsSeverity(vcsKind)))
                    .accessibilityLabel(Text(fileTreeVcsAccessibilityLabel(vcsKind)))
            }
            if let diagnosticSeverity = row.diagnosticSeverity {
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
    let gesture: ResizeGesture

    @State private var isHovering = false

    var body: some View {
        Rectangle()
            .fill(Color.clear)
            .frame(width: 8)
            .overlay(alignment: .trailing) {
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

            if !normalized.text.isEmpty {
                Text(normalized.text)
                    .font(textFont)
                    .lineLimit(1)
            }
        }
        .foregroundStyle(foregroundStyle)
        .accessibilityElement(children: .combine)
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
    let onToggleFileTree: () -> Void

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
                onToggleFileTree: onToggleFileTree
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
                onToggleFileTree: onToggleFileTree
            )
        }
    }

    @MainActor
    final class Coordinator: NSObject, NSToolbarDelegate {
        private let toolbarIdentifier = NSToolbar.Identifier("TheSwiftPOC.TitlebarToolbar")
        private let leadingItemIdentifier = NSToolbarItem.Identifier("TheSwiftPOC.LeadingRegion")
        private let vcsItemIdentifier = NSToolbarItem.Identifier("TheSwiftPOC.VCSInfo")
        private let leadingState = EditorTitlebarLeadingState()
        private lazy var fileTreeHostingView = NSHostingView(rootView: EditorTitlebarLeadingRegionView(state: leadingState, onToggle: {}))
        private let vcsHostingView = NSHostingView(rootView: EditorTitlebarVCSView(vcsText: nil))
        private let sidebarTitlebarBackgroundView = NSView(frame: .zero)
        private let sidebarTitlebarSeparatorView = EditorTitlebarSidebarSeparatorView(frame: .zero)
        private weak var observedWindow: NSWindow?
        private var lastChrome: EditorChromeModel = .empty
        private var lastFileTreeVisible = false
        private var lastFileTreeWidth: CGFloat = 52
        private var lastFileTreeBackgroundColor: NSColor = .windowBackgroundColor
        private var lastFileTreeSeparatorColor: NSColor = .separatorColor
        private var toggleFileTreeAction: (() -> Void)?
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
            onToggleFileTree: @escaping () -> Void
        ) {
            let started = CFAbsoluteTimeGetCurrent()
            let windowChanged = observedWindow !== window
            let chromeChanged = !chrome.matches(lastChrome)
            let fileTreeChanged = fileTreeVisible != lastFileTreeVisible
            let widthChanged = abs(fileTreeWidth - lastFileTreeWidth) > 0.5
            let sidebarColorChanged = !fileTreeBackgroundColor.isEqual(lastFileTreeBackgroundColor)
            let separatorColorChanged = !fileTreeSeparatorColor.isEqual(lastFileTreeSeparatorColor)
            toggleFileTreeAction = onToggleFileTree
            attachWindowObserversIfNeeded(window: window)
            installToolbarIfNeeded(window: window)
            guard windowChanged || chromeChanged || fileTreeChanged || widthChanged || sidebarColorChanged || separatorColorChanged else {
                scrollPerfLog("chrome.configure skipped windowChanged=\(windowChanged) chromeChanged=\(chromeChanged) fileTreeChanged=\(fileTreeChanged) widthChanged=\(widthChanged)")
                return
            }
            lastChrome = chrome
            lastFileTreeVisible = fileTreeVisible
            lastFileTreeWidth = fileTreeWidth
            lastFileTreeBackgroundColor = fileTreeBackgroundColor
            lastFileTreeSeparatorColor = fileTreeSeparatorColor
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
            updateToolbarContent(window: window, chrome: chrome, fileTreeVisible: fileTreeVisible, fileTreeWidth: fileTreeWidth)
            let toolbarMs = (CFAbsoluteTimeGetCurrent() - toolbarStarted) * 1000
            let totalMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
            scrollPerfLog(
                "chrome.configure windowChanged=\(windowChanged) chromeChanged=\(chromeChanged) fileTreeChanged=\(fileTreeChanged) widthChanged=\(widthChanged) applyMs=\(String(format: "%.2f", applyMs)) toolbarMs=\(String(format: "%.2f", toolbarMs)) totalMs=\(String(format: "%.2f", totalMs))"
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
            updateToolbarContent(window: window, chrome: lastChrome, fileTreeVisible: lastFileTreeVisible, fileTreeWidth: lastFileTreeWidth)
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

        private func updateToolbarContent(window: NSWindow, chrome: EditorChromeModel, fileTreeVisible: Bool, fileTreeWidth: CGFloat) {
            fileTreeHostingView.rootView = EditorTitlebarLeadingRegionView(state: leadingState) {
                self.toggleFileTreeAction?()
            }
            withAnimation(.spring(response: 0.24, dampingFraction: 0.88)) {
                leadingState.isSidebarActive = fileTreeVisible
                leadingState.sidebarWidth = fileTreeWidth
                leadingState.document = chrome.document
            }
            vcsHostingView.rootView = EditorTitlebarVCSView(vcsText: chrome.document.vcsText)
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

    var body: some View {
        HStack(spacing: 0) {
            HStack(spacing: 0) {
                EditorTitlebarSidebarToggleButton(isActive: state.isSidebarActive, onToggle: onToggle)
                    .padding(.leading, 10)
                Spacer(minLength: 0)
            }
            .frame(width: max(state.sidebarWidth, 52), height: 24, alignment: .leading)

            EditorTitlebarDocumentView(document: state.document)
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
