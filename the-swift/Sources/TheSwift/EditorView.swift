import Foundation
import SwiftUI
import TheEditorFFIBridge

private let embeddedTerminalCropInset: CGFloat = 2
private let inactivePaneDimOpacity: CGFloat = 0.22

private enum SidebarChromeBaselineCache {
    static var topInset: CGFloat?
}

struct EditorView: View {
    @Environment(\.openWindow) private var openWindow
    @StateObject private var model: EditorModel
    @StateObject private var cursorBlinkController = CursorBlinkController()
    @ObservedObject private var globalTerminalSwitcher = GlobalTerminalSwitcherController.shared
    @State private var renderSceneCache = EditorRenderSceneCache()
    private let windowRoute: EditorWindowRoute?
    @State private var columnVisibility: NavigationSplitViewVisibility = .detailOnly

    private enum SidebarNavigatorMode: String {
        case files
        case surfaces
    }

    @SceneStorage("sidebarNavigatorMode") private var sidebarNavigatorModeRaw = SidebarNavigatorMode.files.rawValue

    private struct CursorPickState {
        let remove: Bool
        let currentIndex: Int
    }

    private struct EolDiagnosticOverlayLayout: Identifiable {
        let id: String
        let x: CGFloat
        let y: CGFloat
        let maxWidth: CGFloat
        let message: String
        let severity: UInt8
    }

    private struct EolDiagnosticOverlayStyle {
        let fontSize: CGFloat
        let horizontalPadding: CGFloat
        let verticalPadding: CGFloat
        let cornerRadius: CGFloat
        let backgroundOpacity: CGFloat
        let borderOpacity: CGFloat
        let textGap: CGFloat
        let rowOffset: CGFloat
    }

    init(filePath: String? = nil, windowRoute: EditorWindowRoute? = nil) {
        self.windowRoute = windowRoute
        _model = StateObject(
            wrappedValue: EditorModel(
                filePath: filePath,
                bufferId: windowRoute?.bufferId
            )
        )
    }

    var body: some View {
        let fileTreeSnapshot = model.fileTreeSnapshot
        let editorBackgroundColor = model.editorBackgroundColor()
        let surfaceRailSnapshot = model.surfaceRailSnapshot()

        NavigationSplitView(columnVisibility: $columnVisibility) {
            sidebar(snapshot: fileTreeSnapshot, surfaceRailSnapshot: surfaceRailSnapshot)
        } detail: {
            detailContent(editorBackgroundColor: editorBackgroundColor)
        }
        .navigationTitle(model.navigationTitle)
        .toolbarBackground(.hidden, for: .windowToolbar)
        .toolbar {
            if let snap = model.uiTree.statuslineSnapshot() {
                ToolbarItem(placement: .navigation) {
                    EditorToolbarLeading(snapshot: snap)
                }
                ToolbarItemGroup(placement: .automatic) {
                    if EditorToolbarVCS.text(from: snap) != nil {
                        EditorToolbarVCS(snapshot: snap)
                    }
                    EditorToolbarTrailing(snapshot: snap, pendingKeys: model.pendingKeys)
                }
            }
        }
        .background(
            WindowTabbingBridge(
                route: windowRoute,
                onWindowShouldClose: { window in
                    model.handleWindowShouldClose(window)
                },
                onWindowChanged: { window in
                    model.setHostWindow(window)
                }
            )
            .frame(width: 0, height: 0)
            .allowsHitTesting(false)
        )
        .onAppear {
            model.setOpenWindowTabHandler { route in
                openWindow(id: TheSwiftApp.editorWindowSceneId, value: route)
            }
            columnVisibility = fileTreeSnapshot.visible ? .all : .detailOnly
        }
        .onChange(of: fileTreeSnapshot.visible) { isVisible in
            columnVisibility = isVisible ? .all : .detailOnly
        }
        .focusedValue(
            \.editorCommandExecutor,
            EditorCommandExecutor(
                executeNamedCommand: { command in
                    model.executeNamedCommand(command)
                },
                selectNativeTabCommand: { indexOneBased in
                    model.selectNativeWindowTab(indexOneBased: indexOneBased)
                }
            )
        )
    }

    @ViewBuilder
    private func sidebar(
        snapshot: FileTreeSnapshot,
        surfaceRailSnapshot: SurfaceRailSnapshot
    ) -> some View {
        SidebarChromeCompensationView {
            VStack(spacing: 0) {
                sidebarNavigatorTabs

                Group {
                    switch sidebarNavigatorMode {
                    case .files:
                        FileTreeSidebarView(
                            snapshot: snapshot,
                            onSetExpanded: { path, expanded in
                                model.fileTreeSetExpanded(path: path, expanded: expanded)
                            },
                            onSelectPath: { path in
                                model.fileTreeSelectPath(path: path)
                            },
                            onOpenSelected: {
                                model.fileTreeOpenSelected()
                            }
                        )
                    case .surfaces:
                        SurfaceRailView(
                            snapshot: surfaceRailSnapshot,
                            onFocusEditorSurface: { paneId in
                                _ = model.focusEditorSurface(paneId: paneId)
                            },
                            onSelectOpenBuffer: { bufferIndex in
                                model.selectOpenBuffer(bufferIndex: bufferIndex)
                            },
                            onSelectTerminal: { terminalId in
                                _ = model.focusTerminalSurface(terminalId: terminalId)
                            },
                            onCloseBuffer: { bufferId in
                                _ = model.closeSurfaceRailBuffer(bufferId: bufferId)
                            },
                            onCloseTerminal: { terminalId in
                                _ = model.closeSurfaceRailTerminal(terminalId: terminalId)
                            }
                        )
                    }
                }
            }
        }
        .navigationSplitViewColumnWidth(min: 180, ideal: 240, max: 360)
    }

    @ViewBuilder
    private func detailContent(editorBackgroundColor: SwiftUI.Color) -> some View {
        VStack(spacing: 0) {
            GeometryReader { contentProxy in
                detailGeometryContent(
                    contentProxy: contentProxy,
                    editorBackgroundColor: editorBackgroundColor
                )
            }
        }
        .background(editorBackgroundColor)
    }

    private var sidebarNavigatorMode: SidebarNavigatorMode {
        get { SidebarNavigatorMode(rawValue: sidebarNavigatorModeRaw) ?? .files }
        nonmutating set { sidebarNavigatorModeRaw = newValue.rawValue }
    }

    private var sidebarNavigatorTabs: some View {
        HStack(spacing: 6) {
            sidebarNavigatorButton(
                mode: .files,
                systemImage: "folder.fill",
                help: "File Tree"
            )
            sidebarNavigatorButton(
                mode: .surfaces,
                systemImage: "square.on.square",
                help: "Open Files and Terminals"
            )
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 12)
        .padding(.top, 10)
        .padding(.bottom, 9)
        .background(.clear)
        .overlay(alignment: .bottom) {
            Divider()
        }
    }

    private func sidebarNavigatorButton(
        mode: SidebarNavigatorMode,
        systemImage: String,
        help: String
    ) -> some View {
        let isSelected = sidebarNavigatorMode == mode

        return Button {
            sidebarNavigatorMode = mode
        } label: {
            Image(systemName: systemImage)
                .font(.system(size: 12, weight: .semibold))
                .frame(width: 32, height: 28)
                .foregroundStyle(isSelected ? .primary : .secondary)
                .background {
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .fill(isSelected ? Color(nsColor: .quaternaryLabelColor).opacity(0.22) : Color(nsColor: .quaternaryLabelColor).opacity(0.08))
                }
        }
        .buttonStyle(.plain)
        .help(help)
        .accessibilityLabel(help)
    }

    @ViewBuilder
    private func detailGeometryContent(
        contentProxy: GeometryProxy,
        editorBackgroundColor: SwiftUI.Color
    ) -> some View {
        let cellSize = model.cellSize
        let bufferFont = model.bufferFont
        let bufferNSFont = model.bufferNSFont
        let isPaletteOpen = model.uiTree.hasCommandPalettePanel
        let isSearchOpen = model.uiTree.hasSearchPromptPanel
        let isFilePickerOpen = model.filePickerSnapshot?.active ?? false
        let isInputPromptOpen = model.uiTree.hasInputPromptPanel
        let terminalSwitcherSnapshot = globalTerminalSwitcher.snapshot(for: model.currentHostWindow())
        let isTerminalSwitcherOpen = terminalSwitcherSnapshot.isOpen
        let isOverlayOpen = isPaletteOpen || isSearchOpen || isFilePickerOpen || isInputPromptOpen || isTerminalSwitcherOpen
        let completionSnapshot = model.uiTree.completionSnapshot()
        let diagnosticPopupSnapshot = model.uiTree.diagnosticSnapshot()
        let hoverSnapshot = model.uiTree.hoverSnapshot()
        let docsPopoverSnapshots = model.uiTree.docsPopoverSnapshots()
        let signatureSnapshot = model.uiTree.signatureHelpSnapshot()
        let popupTheme = model.popupTheme()
        let isCompletionOpen = completionSnapshot != nil
        let isDocsPopupOpen = !docsPopoverSnapshots.isEmpty && !isCompletionOpen
        let isSignatureOpen = signatureSnapshot != nil && !isCompletionOpen && !isDocsPopupOpen
        let shouldTerminalOwnFocus = !isOverlayOpen && !isCompletionOpen && !isDocsPopupOpen && !isSignatureOpen
        let bufferOwnsFocus = model.isHostWindowFocused
            && (!model.isActivePaneTerminal || !shouldTerminalOwnFocus)
        let focusedTerminalPaneId = GhosttyRuntime.shared.firstResponderPaneId(in: model.currentHostWindow())
        let effectiveActivePaneId = focusedTerminalPaneId ?? model.framePlan.active_pane_id()
        let cursorPickState = cursorPickState(from: model.uiTree.statuslineSnapshot())
        let cursorBlinkDescriptor = cursorBlinkDescriptor(
            plan: model.plan,
            bufferOwnsFocus: bufferOwnsFocus
        )
        let activePaneOrigin = panePixelOrigin(model.activePaneRect(), cellSize: cellSize)
        let terminalPaneIds = Set(model.terminalPanes.map(\.paneId))
        let shouldDimInactivePanes = Int(model.framePlan.pane_count()) > 1
        let splitResizeHandles = splitResizeHandles(from: model.splitSeparators, cellSize: cellSize)
        let pointerPanes = pointerPanes(from: model.framePlan, cellSize: cellSize)
        let terminalPaneLayouts = terminalPaneLayouts(
            from: model.terminalPanes,
            framePlan: model.framePlan,
            cellSize: cellSize,
            contentSize: contentProxy.size,
            focusedPaneId: effectiveActivePaneId
        )
        let paneFocusLayouts = paneFocusLayouts(
            framePlan: model.framePlan,
            cellSize: cellSize,
            contentSize: contentProxy.size,
            activePaneId: effectiveActivePaneId
        )
        let paneDragPanes = paneDragPanes(from: paneFocusLayouts)
        let paneDragInteractionRects = paneDragInteractionRects(from: paneFocusLayouts)
        let eolDiagnosticStyle = eolDiagnosticOverlayStyle(
            bufferFontSize: model.bufferFontSize,
            cellSize: cellSize
        )
        let eolDiagnosticOverlays = eolDiagnosticOverlayLayouts(
            plan: model.plan,
            paneOrigin: activePaneOrigin,
            cellSize: cellSize,
            containerSize: contentProxy.size,
            style: eolDiagnosticStyle
        )
        let _ = debugTerminalFocusState(
            model: model,
            focusedTerminalPaneId: focusedTerminalPaneId,
            shouldTerminalOwnFocus: shouldTerminalOwnFocus,
            terminalPaneLayouts: terminalPaneLayouts,
            effectiveActivePaneId: effectiveActivePaneId
        )
        let _ = {
            guard DiagnosticsDebugLog.enabled else { return }
            let windowNumber = model.currentHostWindow()?.windowNumber ?? 0
            DiagnosticsDebugLog.logChanged(
                key: "editor.popup.mount.window\(windowNumber).runtime\(model.runtimeInstanceId).editor\(model.editorId.value)",
                value: "overlay_open=\(isOverlayOpen ? 1 : 0) completion=\(isCompletionOpen ? 1 : 0) diagnostic=\(diagnosticPopupSnapshot != nil ? 1 : 0) hover=\(hoverSnapshot != nil ? 1 : 0) docs=\(isDocsPopupOpen ? 1 : 0) signature=\(isSignatureOpen ? 1 : 0) active_pane=\(effectiveActivePaneId)"
            )
        }()
        let terminalPassthroughRects = terminalPaneLayouts.map(\.frame)
        let docsPopupOrigin = docsPopupPixelPosition(
            anchor: model.docsPopupAnchor,
            framePlan: model.framePlan,
            fallbackPlan: model.plan,
            fallbackPaneOrigin: activePaneOrigin,
            cellSize: cellSize
        )
        let docsPopupPassthroughRects = DocsPopoverLayout.placements(
            popovers: docsPopoverSnapshots,
            cursorOrigin: docsPopupOrigin,
            cellSize: cellSize,
            containerSize: contentProxy.size
        )
        .map(\.frame)

        ZStack {
            terminalCanvas(
                cellSize: cellSize,
                bufferFont: bufferFont,
                bufferNSFont: bufferNSFont,
                terminalPaneIds: terminalPaneIds,
                effectiveActivePaneId: effectiveActivePaneId,
                cursorPickState: cursorPickState,
                cursorOpacity: cursorBlinkController.opacity,
                editorBackgroundColor: editorBackgroundColor
            )

            eolDiagnosticsOverlay(layouts: eolDiagnosticOverlays, style: eolDiagnosticStyle)

            terminalSurfaceLayer(
                terminalPaneLayouts: terminalPaneLayouts,
                cellSize: cellSize,
                shouldTerminalOwnFocus: shouldTerminalOwnFocus
            )

            inactivePaneOverlay(
                paneFocusLayouts: paneFocusLayouts,
                shouldDimInactivePanes: shouldDimInactivePanes
            )

            overlayHost(terminalSwitcherSnapshot: terminalSwitcherSnapshot)

            popupOverlay(
                isOverlayOpen: isOverlayOpen,
                completionSnapshot: completionSnapshot,
                docsPopoverSnapshots: docsPopoverSnapshots,
                signatureSnapshot: signatureSnapshot,
                popupTheme: popupTheme,
                activePaneOrigin: activePaneOrigin,
                docsPopupOrigin: docsPopupOrigin,
                cellSize: cellSize,
                contentSize: contentProxy.size
            )
        }
        .background(keyCaptureBackground(isOverlayOpen: isOverlayOpen))
        .overlay(
            scrollCaptureOverlay(
                isOverlayOpen: isOverlayOpen,
                isCompletionOpen: isCompletionOpen,
                isDocsPopupOpen: isDocsPopupOpen,
                isSignatureOpen: isSignatureOpen,
                splitResizeHandles: splitResizeHandles,
                terminalPassthroughRects: terminalPassthroughRects,
                popupPassthroughRects: docsPopupPassthroughRects,
                pointerPanes: pointerPanes,
                cursorExclusionRects: paneDragInteractionRects,
                cellSize: cellSize
            )
        )
        .overlay(
            paneDragOverlay(
                isOverlayOpen: isOverlayOpen,
                isCompletionOpen: isCompletionOpen,
                isDocsPopupOpen: isDocsPopupOpen,
                isSignatureOpen: isSignatureOpen,
                paneDragPanes: paneDragPanes
            ),
            alignment: .topLeading
        )
        .overlay(
            KeySequenceIndicator(keys: model.pendingKeys, hints: model.pendingKeyHints)
                .padding(.bottom, 28)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom),
            alignment: .bottom
        )
        .onAppear {
            model.updateViewport(pixelSize: contentProxy.size, cellSize: cellSize)
            cursorBlinkController.update(cursorBlinkDescriptor)
        }
        .onChange(of: contentProxy.size) { newSize in
            model.updateViewport(pixelSize: newSize, cellSize: cellSize)
        }
        .onChange(of: cellSize) { newCellSize in
            model.updateViewport(pixelSize: contentProxy.size, cellSize: newCellSize)
        }
        .onChange(of: cursorBlinkDescriptor) { newDescriptor in
            cursorBlinkController.update(newDescriptor)
        }
        .onChange(of: isCompletionOpen) { isOpen in
            guard !isOpen else {
                return
            }
            guard !model.isActivePaneTerminal else {
                return
            }
            DispatchQueue.main.async {
                KeyCaptureFocusBridge.shared.reclaimActive()
            }
        }
        .onChange(of: model.isActivePaneTerminal) { isTerminal in
            guard !isTerminal && !isOverlayOpen else {
                return
            }
            DispatchQueue.main.async {
                KeyCaptureFocusBridge.shared.reclaimActive()
            }
        }
    }

    @ViewBuilder
    private func terminalCanvas(
        cellSize: CGSize,
        bufferFont: Font,
        bufferNSFont: NSFont,
        terminalPaneIds: Set<UInt64>,
        effectiveActivePaneId: UInt64,
        cursorPickState: CursorPickState?,
        cursorOpacity: Double,
        editorBackgroundColor: SwiftUI.Color
    ) -> some View {
        Canvas { context, size in
            drawFrame(
                in: context,
                size: size,
                framePlan: model.framePlan,
                fallbackPlan: model.plan,
                terminalPaneIds: terminalPaneIds,
                activePaneId: effectiveActivePaneId,
                cellSize: cellSize,
                bufferFont: bufferFont,
                bufferNSFont: bufferNSFont,
                cursorPickState: cursorPickState,
                cursorOpacity: cursorOpacity
            )
        }
        .background(editorBackgroundColor)
    }

    @ViewBuilder
    private func eolDiagnosticsOverlay(
        layouts: [EolDiagnosticOverlayLayout],
        style: EolDiagnosticOverlayStyle
    ) -> some View {
        ZStack(alignment: .topLeading) {
            ForEach(layouts) { layout in
                Text(layout.message)
                    .font(FontLoader.bufferFont(size: style.fontSize, weight: .regular))
                    .foregroundStyle(diagnosticColor(severity: layout.severity).opacity(0.76))
                    .lineLimit(1)
                    .truncationMode(.tail)
                .padding(.horizontal, style.horizontalPadding)
                .padding(.vertical, style.verticalPadding)
                .background {
                    RoundedRectangle(cornerRadius: style.cornerRadius, style: .continuous)
                        .fill(
                            diagnosticColor(severity: layout.severity)
                                .opacity(style.backgroundOpacity)
                        )
                }
                .overlay {
                    RoundedRectangle(cornerRadius: style.cornerRadius, style: .continuous)
                        .stroke(
                            diagnosticColor(severity: layout.severity)
                                .opacity(style.borderOpacity),
                            lineWidth: 0.5
                        )
                }
                .frame(maxWidth: layout.maxWidth, alignment: .leading)
                .offset(x: layout.x, y: layout.y)
                .allowsHitTesting(false)
                .accessibilityHidden(true)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    private func cursorBlinkDescriptor(
        plan: RenderPlan,
        bufferOwnsFocus: Bool
    ) -> CursorBlinkDescriptor {
        CursorBlinkDescriptor(
            enabled: bufferOwnsFocus && plan.cursor_blink_enabled(),
            cursorCount: Int(plan.cursor_count()),
            intervalMilliseconds: UInt64(plan.cursor_blink_interval_ms()),
            delayMilliseconds: UInt64(plan.cursor_blink_delay_ms()),
            generation: plan.cursor_blink_generation()
        )
    }

    private func eolDiagnosticOverlayLayouts(
        plan: RenderPlan,
        paneOrigin: CGPoint,
        cellSize: CGSize,
        containerSize: CGSize,
        style: EolDiagnosticOverlayStyle
    ) -> [EolDiagnosticOverlayLayout] {
        let perfEnabled = DiagnosticsDebugLog.editorPerfEnabled
        let perfStart = perfEnabled ? DispatchTime.now().uptimeNanoseconds : 0
        let count = Int(plan.eol_diagnostic_count())
        guard count > 0 else { return [] }

        let contentOffsetX = CGFloat(plan.content_offset_x()) * cellSize.width
        var layouts: [EolDiagnosticOverlayLayout] = []
        layouts.reserveCapacity(count)
        var totalMessageChars = 0
        var maxMessageChars = 0

        for index in 0..<count {
            let entry = plan.eol_diagnostic_at(UInt(index))
            let message = entry.message().toString().trimmingCharacters(in: .whitespacesAndNewlines)
            guard !message.isEmpty else { continue }
            totalMessageChars += message.count
            maxMessageChars = max(maxMessageChars, message.count)

            let x = paneOrigin.x + contentOffsetX + CGFloat(entry.col()) * cellSize.width + style.textGap
            let availableWidth = containerSize.width - x - 8
            guard availableWidth >= 72 else { continue }

            layouts.append(
                EolDiagnosticOverlayLayout(
                    id: "\(entry.row()):\(entry.col()):\(entry.severity()):\(message)",
                    x: x,
                    y: paneOrigin.y + CGFloat(entry.row()) * cellSize.height + style.rowOffset,
                    maxWidth: availableWidth,
                    message: message,
                    severity: entry.severity()
                )
            )
        }

        if perfEnabled {
            let elapsedMs = Double(DispatchTime.now().uptimeNanoseconds - perfStart) / 1_000_000.0
            if DiagnosticsDebugLog.editorPerfShouldLog(durationMs: elapsedMs) {
                DiagnosticsDebugLog.editorPerfLog(
                    String(
                        format: "eol_overlay_layout elapsed=%.2fms raw=%d laid_out=%d lines=%d total_chars=%d max_chars=%d pane_origin=(%.1f,%.1f) content_offset_x=%.1f container=(%.1f,%.1f)",
                        elapsedMs,
                        count,
                        layouts.count,
                        Int(plan.line_count()),
                        totalMessageChars,
                        maxMessageChars,
                        paneOrigin.x,
                        paneOrigin.y,
                        contentOffsetX,
                        containerSize.width,
                        containerSize.height
                    )
                )
            }
        }

        return layouts
    }

    private func eolDiagnosticOverlayStyle(
        bufferFontSize: CGFloat,
        cellSize: CGSize
    ) -> EolDiagnosticOverlayStyle {
        let scale = max(0.45, min(1.8, bufferFontSize / FontZoomLimits.defaultBufferPointSize))
        let fontSize = max(5.5, min(20, min(cellSize.height * 0.72, 11 * scale)))
        return EolDiagnosticOverlayStyle(
            fontSize: fontSize,
            horizontalPadding: max(4, min(10, 6 * scale)),
            verticalPadding: max(1.5, min(5, 2.6 * scale)),
            cornerRadius: max(4, min(10, 6 * scale)),
            backgroundOpacity: 0.10,
            borderOpacity: 0.22,
            textGap: max(3, min(10, 6 * scale)),
            rowOffset: max(0, floor(max(0, cellSize.height - fontSize) * 0.16))
        )
    }

    @ViewBuilder
    private func terminalSurfaceLayer(
        terminalPaneLayouts: [TerminalPaneLayout],
        cellSize: CGSize,
        shouldTerminalOwnFocus: Bool
    ) -> some View {
        ForEach(terminalPaneLayouts) { pane in
            SwiftUI.Color.clear
                .frame(width: pane.frame.width, height: pane.frame.height)
                .overlay {
                    GhosttyPaneView(
                        runtimeId: model.runtimeInstanceId,
                        paneId: pane.paneId,
                        terminalId: pane.terminalId,
                        requestedSize: CGSize(
                            width: pane.frame.width + (embeddedTerminalCropInset * 2),
                            height: pane.frame.height + (embeddedTerminalCropInset * 2)
                        ),
                        cellSize: cellSize,
                        focused: pane.isActive && shouldTerminalOwnFocus,
                        onPointer: { event in
                            model.handlePointerEvent(event)
                        },
                        onCloseRequest: {
                            model.closeSurface()
                        },
                        onNamedCommand: { command in
                            model.executeNamedCommand(command)
                        },
                        onNotification: { notification in
                            model.handleTerminalNotification(
                                terminalId: pane.terminalId,
                                title: notification.title,
                                body: notification.body,
                                terminalTitle: notification.terminalTitle
                            )
                        },
                        onMetadataChange: {
                            model.handleTerminalMetadataUpdate(terminalId: pane.terminalId)
                        }
                    )
                    .frame(
                        width: pane.frame.width + (embeddedTerminalCropInset * 2),
                        height: pane.frame.height + (embeddedTerminalCropInset * 2)
                    )
                }
                .position(x: pane.frame.midX, y: pane.frame.midY)
                .clipped()
        }
    }

    @ViewBuilder
    private func inactivePaneOverlay(
        paneFocusLayouts: [PaneFocusLayout],
        shouldDimInactivePanes: Bool
    ) -> some View {
        ForEach(paneFocusLayouts) { pane in
            if shouldDimInactivePanes && !pane.isActive {
                Rectangle()
                    .fill(SwiftUI.Color.black.opacity(inactivePaneDimOpacity))
                    .frame(width: pane.frame.width, height: pane.frame.height)
                    .position(x: pane.frame.midX, y: pane.frame.midY)
                    .allowsHitTesting(false)
            }
        }
    }

    @ViewBuilder
    private func overlayHost(terminalSwitcherSnapshot: GlobalTerminalSwitcherSnapshot) -> some View {
        UiOverlayHost(
            tree: model.uiTree,
            cellSize: model.cellSize,
            filePickerSnapshot: model.filePickerSnapshot,
            filePickerPreviewModel: model.filePickerPreviewModel,
            pendingKeys: model.pendingKeys,
            onSelectCommand: { index in
                model.selectCommandPalette(index: index)
            },
            onSubmitCommand: { index in
                model.submitCommandPalette(index: index)
            },
            onCloseCommandPalette: {
                model.closeCommandPalette()
            },
            onQueryChange: { query in
                model.setCommandPaletteQuery(query)
            },
            onSearchQueryChange: { query in
                model.setSearchQuery(query)
            },
            onSearchPrev: {
                model.searchPrev()
            },
            onSearchNext: {
                model.searchNext()
            },
            onSearchClose: {
                model.closeSearch()
            },
            onSearchSubmit: {
                model.submitSearch()
            },
            onFilePickerQueryChange: { query in
                model.setFilePickerQuery(query)
            },
            onFilePickerSubmit: { index in
                model.submitFilePicker(index: index)
            },
            onFilePickerClose: {
                model.closeFilePicker()
            },
            onFilePickerSelectionChange: { index in
                model.filePickerSelectIndex(index)
            },
            onFilePickerPreviewWindowRequest: { offset, visibleRows, overscan in
                model.filePickerPreviewWindowRequest(offset: offset, visibleRows: visibleRows, overscan: overscan)
            },
            colorForHighlight: { highlightId in
                model.colorForHighlight(highlightId)
            },
            onInputPromptQueryChange: { query in
                model.setSearchQuery(query)
            },
            onInputPromptClose: {
                model.closeSearch()
            },
            onInputPromptSubmit: {
                model.submitSearch()
            }
        )

        EditorNotificationOverlay(banners: model.notificationBanners)

        if terminalSwitcherSnapshot.isOpen {
            GlobalTerminalSwitcherView(
                snapshot: terminalSwitcherSnapshot,
                onSubmit: { entry in
                    _ = model.submitGlobalTerminalSwitcher(entry: entry)
                },
                onClose: {
                    model.closeGlobalTerminalSwitcher()
                }
            )
        }
    }

    @ViewBuilder
    private func popupOverlay(
        isOverlayOpen: Bool,
        completionSnapshot: CompletionSnapshot?,
        docsPopoverSnapshots: [DocsPopoverSnapshot],
        signatureSnapshot: SignatureHelpSnapshot?,
        popupTheme: PopupChromeTheme,
        activePaneOrigin: CGPoint,
        docsPopupOrigin: CGPoint,
        cellSize: CGSize,
        contentSize: CGSize
    ) -> some View {
        if !isOverlayOpen {
            if let completionSnapshot {
                CompletionPopupView(
                    snapshot: completionSnapshot,
                    cursorOrigin: cursorPixelPosition(
                        plan: model.plan,
                        paneOrigin: activePaneOrigin,
                        cellSize: cellSize
                    ),
                    theme: popupTheme,
                    cellSize: cellSize,
                    containerSize: contentSize,
                    languageHint: model.completionDocsLanguageHint(),
                    onSelect: { index in
                        model.selectCompletion(index: index)
                    },
                    onSubmit: { index in
                        model.submitCompletion(index: index)
                    }
                )
                .allowsHitTesting(true)
            } else if !docsPopoverSnapshots.isEmpty {
                DocsPopoverStackView(
                    popovers: docsPopoverSnapshots,
                    cursorOrigin: docsPopupOrigin,
                    theme: popupTheme,
                    cellSize: cellSize,
                    containerSize: contentSize,
                    languageHint: model.completionDocsLanguageHint()
                )
                .allowsHitTesting(true)
            } else if let signatureSnapshot {
                SignatureHelpPopupView(
                    snapshot: signatureSnapshot,
                    cursorOrigin: cursorPixelPosition(
                        plan: model.plan,
                        paneOrigin: activePaneOrigin,
                        cellSize: cellSize
                    ),
                    theme: popupTheme,
                    cellSize: cellSize,
                    containerSize: contentSize,
                    languageHint: model.completionDocsLanguageHint()
                )
                .allowsHitTesting(true)
            }
        }
    }

    @ViewBuilder
    private func keyCaptureBackground(isOverlayOpen: Bool) -> some View {
        if !isOverlayOpen && !model.isActivePaneTerminal {
            KeyCaptureView(
                onKey: { event in
                    model.handleKeyEvent(event)
                },
                onText: { text, modifiers in
                    model.handleText(text, modifiers: modifiers)
                },
                onCommandDigit: { digit in
                    _ = model.selectNativeWindowTab(indexOneBased: digit)
                },
                onNamedCommand: { command in
                    _ = model.executeNamedCommand(command)
                },
                onScroll: { _, _, _ in },
                modeProvider: {
                    model.mode
                }
            )
            .allowsHitTesting(false)
        }
    }

    @ViewBuilder
    private func scrollCaptureOverlay(
        isOverlayOpen: Bool,
        isCompletionOpen: Bool,
        isDocsPopupOpen: Bool,
        isSignatureOpen: Bool,
        splitResizeHandles: [ScrollCaptureView.SeparatorHandle],
        terminalPassthroughRects: [CGRect],
        popupPassthroughRects: [CGRect],
        pointerPanes: [ScrollCaptureView.PaneHandle],
        cursorExclusionRects: [CGRect],
        cellSize: CGSize
    ) -> some View {
        if !isOverlayOpen && !isCompletionOpen && !isSignatureOpen {
            ScrollCaptureView(
                onScroll: { deltaX, deltaY, precise in
                    model.handlePointerScroll(deltaX: deltaX, deltaY: deltaY, precise: precise)
                },
                onPointer: { event in
                    model.handlePointerEvent(event)
                },
                separators: splitResizeHandles,
                passthroughRects: terminalPassthroughRects,
                popupPassthroughRects: popupPassthroughRects,
                panes: pointerPanes,
                cursorExclusionRects: cursorExclusionRects,
                cellSize: cellSize,
                onSplitResize: { splitId, point in
                    model.resizeSplit(splitId: splitId, pixelPoint: point)
                }
            )
            .allowsHitTesting(true)
        }
    }

    @ViewBuilder
    private func paneDragOverlay(
        isOverlayOpen: Bool,
        isCompletionOpen: Bool,
        isDocsPopupOpen: Bool,
        isSignatureOpen: Bool,
        paneDragPanes: [PaneDragPaneSnapshot]
    ) -> some View {
        if !isOverlayOpen && !isCompletionOpen && !isDocsPopupOpen && !isSignatureOpen && paneDragPanes.count > 1 {
            PaneDragOverlayView(
                panes: paneDragPanes,
                onMovePane: { sourcePaneId, destinationPaneId, directionRaw in
                    _ = model.movePane(
                        sourcePaneId: sourcePaneId,
                        destinationPaneId: destinationPaneId,
                        directionRaw: directionRaw
                    )
                }
            )
        }
    }

    private func splitResizeHandles(
        from separators: [SplitSeparatorSnapshot],
        cellSize: CGSize
    ) -> [ScrollCaptureView.SeparatorHandle] {
        separators.map { separator in
            let linePx: CGFloat
            let spanStartPx: CGFloat
            let spanEndPx: CGFloat
            if separator.axis == 0 {
                linePx = CGFloat(separator.line) * cellSize.width
                spanStartPx = CGFloat(separator.spanStart) * cellSize.height
                spanEndPx = CGFloat(separator.spanEnd) * cellSize.height
            } else {
                linePx = CGFloat(separator.line) * cellSize.height
                spanStartPx = CGFloat(separator.spanStart) * cellSize.width
                spanEndPx = CGFloat(separator.spanEnd) * cellSize.width
            }
            return ScrollCaptureView.SeparatorHandle(
                splitId: separator.splitId,
                axis: separator.axis,
                linePx: linePx,
                spanStartPx: spanStartPx,
                spanEndPx: spanEndPx
            )
        }
    }

    private struct TerminalPaneLayout: Identifiable {
        let paneId: UInt64
        let terminalId: UInt64
        let frame: CGRect
        let isActive: Bool

        // The mounted Ghostty host view is owned per terminal surface/controller,
        // not per pane slot. If a detached terminal is reattached into the same
        // pane, the pane id stays the same while the terminal id changes. Keying
        // by terminal id forces SwiftUI to swap the embedded NSView correctly.
        var id: UInt64 { terminalId }
    }

    private struct PaneFocusLayout: Identifiable {
        let paneId: UInt64
        let frame: CGRect
        let isActive: Bool

        var id: UInt64 { paneId }
    }

    private func terminalPaneLayouts(
        from panes: [TerminalPaneSnapshot],
        framePlan: RenderFramePlan,
        cellSize: CGSize,
        contentSize: CGSize,
        focusedPaneId: UInt64?
    ) -> [TerminalPaneLayout] {
        let gridExtent = framePlanGridExtent(framePlan)
        let activePaneId = focusedPaneId ?? framePlan.active_pane_id()
        return panes.map { pane in
            let x = CGFloat(pane.x) * cellSize.width
            let y = CGFloat(pane.y) * cellSize.height
            let paneMaxX = pane.x + pane.width
            let paneMaxY = pane.y + pane.height
            var width = CGFloat(pane.width) * cellSize.width
            var height = CGFloat(pane.height) * cellSize.height

            // Ghostty expects to own the full pixel size of its host view. The editor
            // core layout is cell-based, so any remainder pixels from the window size
            // live on the outermost pane edges. Let edge panes absorb that remainder
            // so the terminal surface fills the real pane bounds the same way Ghostty's
            // own host views do.
            if paneMaxX == gridExtent.cols {
                width = max(width, contentSize.width - x)
            }
            if paneMaxY == gridExtent.rows {
                height = max(height, contentSize.height - y)
            }

            return TerminalPaneLayout(
                paneId: pane.paneId,
                terminalId: pane.terminalId,
                frame: CGRect(
                    x: x,
                    y: y,
                    width: width,
                    height: height
                ),
                isActive: pane.paneId == activePaneId
            )
        }
    }

    private func paneFocusLayouts(
        framePlan: RenderFramePlan,
        cellSize: CGSize,
        contentSize: CGSize,
        activePaneId: UInt64
    ) -> [PaneFocusLayout] {
        let gridExtent = framePlanGridExtent(framePlan)
        let count = Int(framePlan.pane_count())
        guard count > 0 else { return [] }
        var layouts: [PaneFocusLayout] = []
        layouts.reserveCapacity(count)
        for index in 0..<count {
            let pane = framePlan.pane_at(UInt(index))
            let rect = pane.rect()
            let x = CGFloat(rect.x) * cellSize.width
            let y = CGFloat(rect.y) * cellSize.height
            let paneMaxX = Int(rect.x) + Int(rect.width)
            let paneMaxY = Int(rect.y) + Int(rect.height)
            var width = CGFloat(rect.width) * cellSize.width
            var height = CGFloat(rect.height) * cellSize.height
            if paneMaxX == gridExtent.cols {
                width = max(width, contentSize.width - x)
            }
            if paneMaxY == gridExtent.rows {
                height = max(height, contentSize.height - y)
            }
            layouts.append(
                PaneFocusLayout(
                    paneId: pane.pane_id(),
                    frame: CGRect(x: x, y: y, width: width, height: height),
                    isActive: pane.pane_id() == activePaneId
                )
            )
        }
        return layouts
    }

    private func paneDragPanes(
        from paneFocusLayouts: [PaneFocusLayout]
    ) -> [PaneDragPaneSnapshot] {
        paneFocusLayouts.map { pane in
            PaneDragPaneSnapshot(
                paneId: pane.paneId,
                frame: pane.frame,
                isActive: pane.isActive,
                previewTitle: model.paneDragPreviewTitle(for: pane.paneId)
            )
        }
    }

    private func paneDragInteractionRects(
        from paneFocusLayouts: [PaneFocusLayout]
    ) -> [CGRect] {
        paneFocusLayouts.map { pane in
            PaneDragHandleLayout.interactionFrame(for: pane.frame)
        }
    }

    private func framePlanGridExtent(_ framePlan: RenderFramePlan) -> (cols: Int, rows: Int) {
        let count = Int(framePlan.pane_count())
        var maxCols = 0
        var maxRows = 0
        for index in 0..<count {
            let pane = framePlan.pane_at(UInt(index))
            let rect = pane.rect()
            maxCols = max(maxCols, Int(rect.x) + Int(rect.width))
            maxRows = max(maxRows, Int(rect.y) + Int(rect.height))
        }
        return (cols: maxCols, rows: maxRows)
    }

    private func pointerPanes(
        from framePlan: RenderFramePlan,
        cellSize: CGSize
    ) -> [ScrollCaptureView.PaneHandle] {
        let count = Int(framePlan.pane_count())
        guard count > 0 else { return [] }
        var handles: [ScrollCaptureView.PaneHandle] = []
        handles.reserveCapacity(count)
        for index in 0..<count {
            let pane = framePlan.pane_at(UInt(index))
            if pane.pane_kind() == 1 {
                continue
            }
            let rect = pane.rect()
            let paneRect = CGRect(
                x: CGFloat(rect.x) * cellSize.width,
                y: CGFloat(rect.y) * cellSize.height,
                width: CGFloat(rect.width) * cellSize.width,
                height: CGFloat(rect.height) * cellSize.height
            )
            let contentOffsetXPx = CGFloat(pane.plan().content_offset_x()) * cellSize.width
            handles.append(
                ScrollCaptureView.PaneHandle(
                    paneId: pane.pane_id(),
                    rect: paneRect,
                    contentOffsetXPx: contentOffsetXPx
                )
            )
        }
        return handles
    }

    private func panePixelOrigin(_ rect: Rect?, cellSize: CGSize) -> CGPoint {
        guard let rect else {
            return .zero
        }
        return CGPoint(
            x: CGFloat(rect.x) * cellSize.width,
            y: CGFloat(rect.y) * cellSize.height
        )
    }

    private func paneRect(
        for paneId: UInt64,
        in framePlan: RenderFramePlan
    ) -> Rect? {
        let count = Int(framePlan.pane_count())
        for index in 0..<count {
            let pane = framePlan.pane_at(UInt(index))
            guard pane.pane_id() == paneId else { continue }
            return pane.rect()
        }
        return nil
    }

    private func drawFrame(
        in context: GraphicsContext,
        size: CGSize,
        framePlan: RenderFramePlan,
        fallbackPlan: RenderPlan,
        terminalPaneIds: Set<UInt64>,
        activePaneId: UInt64,
        cellSize: CGSize,
        bufferFont: Font,
        bufferNSFont: NSFont,
        cursorPickState: CursorPickState?,
        cursorOpacity: Double
    ) {
        let perfEnabled = DiagnosticsDebugLog.editorPerfEnabled
        let perfStart = perfEnabled ? DispatchTime.now().uptimeNanoseconds : 0
        let paneCount = Int(framePlan.pane_count())
        var aggregateStats = DrawPlanPerfStats.zero
        var activePlanStats = DrawPlanPerfStats.zero
        guard paneCount > 0 else {
            renderSceneCache.pruneTextScenes(retaining: Set([0]))
            let fallbackStats = drawPlan(
                in: context,
                size: size,
                paneId: 0,
                plan: fallbackPlan,
                cellSize: cellSize,
                bufferFont: bufferFont,
                bufferNSFont: bufferNSFont,
                cursorPickState: cursorPickState,
                cursorOpacity: cursorOpacity
            )
            if perfEnabled {
                let elapsedMs = Double(DispatchTime.now().uptimeNanoseconds - perfStart) / 1_000_000.0
                if DiagnosticsDebugLog.editorPerfShouldLog(durationMs: elapsedMs) {
                    DiagnosticsDebugLog.editorPerfLog(
                        String(
                            format: "canvas_draw total=%.2fms panes=%d terminal_panes=%d active_pane=%llu active_scroll=%d:%d active_lines=%d gutter=%.2fms selections=%.2fms underlines=%.2fms text=%.2fms cursors=%.2fms separators=%.2fms drawn_spans=%d skipped_virtual=%d canvas=(%.0f,%.0f)",
                            elapsedMs,
                            0,
                            terminalPaneIds.count,
                            CUnsignedLongLong(activePaneId),
                            fallbackStats.scrollRow,
                            fallbackStats.scrollCol,
                            fallbackStats.lineCount,
                            fallbackStats.gutterMs,
                            fallbackStats.selectionsMs,
                            fallbackStats.underlinesMs,
                            fallbackStats.textMs,
                            fallbackStats.cursorsMs,
                            0.0,
                            fallbackStats.drawnSpans,
                            fallbackStats.skippedVirtualSpans,
                            size.width,
                            size.height
                        )
                    )
                }
            }
            return
        }

        var retainedPaneIds: Set<UInt64> = []
        for index in 0..<paneCount {
            retainedPaneIds.insert(framePlan.pane_at(UInt(index)).pane_id())
        }
        renderSceneCache.pruneTextScenes(retaining: retainedPaneIds)

        for index in 0..<paneCount {
            let pane = framePlan.pane_at(UInt(index))
            if terminalPaneIds.contains(pane.pane_id()) {
                continue
            }
            let paneRect = pane.rect()
            let panePlan = pane.plan()
            let paneSize = CGSize(
                width: CGFloat(paneRect.width) * cellSize.width,
                height: CGFloat(paneRect.height) * cellSize.height
            )
            var paneContext = context
            paneContext.translateBy(
                x: CGFloat(paneRect.x) * cellSize.width,
                y: CGFloat(paneRect.y) * cellSize.height
            )
            let paneStats = drawPlan(
                in: paneContext,
                size: paneSize,
                paneId: pane.pane_id(),
                plan: panePlan,
                cellSize: cellSize,
                bufferFont: bufferFont,
                bufferNSFont: bufferNSFont,
                cursorPickState: pane.pane_id() == activePaneId ? cursorPickState : nil,
                cursorOpacity: cursorOpacity
            )
            aggregateStats = aggregateStats.adding(paneStats)
            if pane.pane_id() == activePaneId {
                activePlanStats = paneStats
            }
        }

        let separatorsStart = perfEnabled ? DispatchTime.now().uptimeNanoseconds : 0
        drawPaneSeparators(
            in: context,
            framePlan: framePlan,
            canvasSize: size,
            cellSize: cellSize
        )
        let separatorsMs = perfEnabled
            ? Double(DispatchTime.now().uptimeNanoseconds - separatorsStart) / 1_000_000.0
            : 0
        if perfEnabled {
            let elapsedMs = Double(DispatchTime.now().uptimeNanoseconds - perfStart) / 1_000_000.0
            if DiagnosticsDebugLog.editorPerfShouldLog(durationMs: elapsedMs) {
                DiagnosticsDebugLog.editorPerfLog(
                    String(
                        format: "canvas_draw total=%.2fms panes=%d terminal_panes=%d active_pane=%llu active_scroll=%d:%d active_lines=%d gutter=%.2fms selections=%.2fms underlines=%.2fms text=%.2fms cursors=%.2fms separators=%.2fms drawn_spans=%d skipped_virtual=%d canvas=(%.0f,%.0f)",
                        elapsedMs,
                        paneCount,
                        terminalPaneIds.count,
                        CUnsignedLongLong(activePaneId),
                        activePlanStats.scrollRow,
                        activePlanStats.scrollCol,
                        activePlanStats.lineCount,
                        aggregateStats.gutterMs,
                        aggregateStats.selectionsMs,
                        aggregateStats.underlinesMs,
                        aggregateStats.textMs,
                        aggregateStats.cursorsMs,
                        separatorsMs,
                        aggregateStats.drawnSpans,
                        aggregateStats.skippedVirtualSpans,
                        size.width,
                        size.height
                    )
                )
            }
        }
    }

    private struct SplitEdge {
        let from: CGPoint
        let to: CGPoint
    }

    private func drawPaneSeparators(
        in context: GraphicsContext,
        framePlan: RenderFramePlan,
        canvasSize: CGSize,
        cellSize: CGSize
    ) {
        let paneCount = Int(framePlan.pane_count())
        guard paneCount > 1 else { return }

        var maxCol = 0
        var maxRow = 0
        var panes: [RenderFramePane] = []
        panes.reserveCapacity(paneCount)
        for index in 0..<paneCount {
            let pane = framePlan.pane_at(UInt(index))
            panes.append(pane)
            let rect = pane.rect()
            maxCol = max(maxCol, Int(rect.x) + Int(rect.width))
            maxRow = max(maxRow, Int(rect.y) + Int(rect.height))
        }

        var edges: [SplitEdge] = []
        edges.reserveCapacity(paneCount * 2)
        for pane in panes {
            let rect = pane.rect()
            let x0 = CGFloat(rect.x) * cellSize.width
            let y0 = CGFloat(rect.y) * cellSize.height
            let x1 = CGFloat(Int(rect.x) + Int(rect.width)) * cellSize.width
            let y1 = CGFloat(Int(rect.y) + Int(rect.height)) * cellSize.height

            if Int(rect.width) > 0 && Int(rect.x) + Int(rect.width) < maxCol {
                edges.append(SplitEdge(
                    from: CGPoint(x: x1, y: y0),
                    to: CGPoint(x: x1, y: min(y1, canvasSize.height))
                ))
            }
            if Int(rect.height) > 0 && Int(rect.y) + Int(rect.height) < maxRow {
                edges.append(SplitEdge(
                    from: CGPoint(x: x0, y: y1),
                    to: CGPoint(x: min(x1, canvasSize.width), y: y1)
                ))
            }
        }

        let separatorColor = SwiftUI.Color(nsColor: .separatorColor).opacity(0.30)
        for edge in edges {
            let path = Path { path in
                path.move(to: edge.from)
                path.addLine(to: edge.to)
            }
            context.stroke(path, with: .color(separatorColor), lineWidth: 1.0)
        }
    }

    private func cursorPixelPosition(plan: RenderPlan, paneOrigin: CGPoint, cellSize: CGSize) -> CGPoint {
        let contentOffsetX = CGFloat(plan.content_offset_x()) * cellSize.width
        let count = Int(plan.cursor_count())
        guard count > 0 else {
            return CGPoint(x: paneOrigin.x + contentOffsetX, y: paneOrigin.y)
        }
        let pos = plan.cursor_at(0).pos()
        return CGPoint(
            x: paneOrigin.x + contentOffsetX + CGFloat(pos.col) * cellSize.width,
            y: paneOrigin.y + CGFloat(pos.row) * cellSize.height
        )
    }

    private func docsPopupPixelPosition(
        anchor: DocsPopupAnchorSnapshot?,
        framePlan: RenderFramePlan,
        fallbackPlan: RenderPlan,
        fallbackPaneOrigin: CGPoint,
        cellSize: CGSize
    ) -> CGPoint {
        guard let anchor else {
            return cursorPixelPosition(
                plan: fallbackPlan,
                paneOrigin: fallbackPaneOrigin,
                cellSize: cellSize
            )
        }

        let paneOrigin = panePixelOrigin(
            paneRect(for: anchor.paneId, in: framePlan),
            cellSize: cellSize
        )
        return CGPoint(
            x: paneOrigin.x + CGFloat(anchor.col) * cellSize.width,
            y: paneOrigin.y + CGFloat(anchor.row) * cellSize.height
        )
    }

    private func cursorPickState(from snapshot: StatuslineSnapshot?) -> CursorPickState? {
        guard let snapshot else { return nil }

        let modeToken = snapshot
            .left
            .split(whereSeparator: { $0.isWhitespace })
            .first?
            .uppercased()
        let remove: Bool
        switch modeToken {
        case "COL":
            remove = false
        case "REM":
            remove = true
        default:
            return nil
        }

        let prefix = remove ? "remove " : "collapse "
        let rightSegments = snapshot.rightSegments.map(\.text)
        let pickIndex = rightSegments.compactMap { segment in
            parseCursorPickIndex(segment: segment, prefix: prefix)
        }.first
        guard let pickIndex else { return nil }

        return CursorPickState(remove: remove, currentIndex: pickIndex)
    }

    private func parseCursorPickIndex(segment: String, prefix: String) -> Int? {
        let lower = segment.lowercased()
        guard lower.hasPrefix(prefix) else { return nil }
        let remainder = lower.dropFirst(prefix.count)
        let parts = remainder.split(separator: "/", maxSplits: 1)
        guard
            parts.count == 2,
            let current = Int(parts[0]),
            current >= 1
        else {
            return nil
        }
        return current - 1
    }

    private func drawPlan(
        in context: GraphicsContext,
        size: CGSize,
        paneId: UInt64,
        plan: RenderPlan,
        cellSize: CGSize,
        bufferFont: Font,
        bufferNSFont: NSFont,
        cursorPickState: CursorPickState?,
        cursorOpacity: Double
    ) -> DrawPlanPerfStats {
        let perfEnabled = DiagnosticsDebugLog.editorPerfEnabled
        let perfStart = perfEnabled ? DispatchTime.now().uptimeNanoseconds : 0
        func perfNow() -> UInt64 {
            DispatchTime.now().uptimeNanoseconds
        }
        func perfMs(_ start: UInt64, _ end: UInt64) -> Double {
            guard perfEnabled, end >= start else { return 0 }
            return Double(end - start) / 1_000_000.0
        }
        let contentOffsetX = CGFloat(plan.content_offset_x()) * cellSize.width
        let perfAfterStart = perfEnabled ? perfStart : 0
        drawGutter(
            in: context,
            size: size,
            plan: plan,
            cellSize: cellSize,
            font: bufferFont,
            nsFont: bufferNSFont,
            contentOffsetX: contentOffsetX
        )
        let perfAfterGutter = perfEnabled ? perfNow() : 0
        drawSelections(in: context, plan: plan, cellSize: cellSize, contentOffsetX: contentOffsetX)
        let perfAfterSelections = perfEnabled ? perfNow() : 0
        drawDiagnosticUnderlines(in: context, plan: plan, cellSize: cellSize, contentOffsetX: contentOffsetX)
        let perfAfterUnderlines = perfEnabled ? perfNow() : 0
        let textStats = drawText(
            in: context,
            paneId: paneId,
            plan: plan,
            cellSize: cellSize,
            nsFont: bufferNSFont,
            contentOffsetX: contentOffsetX
        )
        let perfAfterText = perfEnabled ? perfNow() : 0
        let inlineSummary = "count=\(Int(plan.inline_diagnostic_line_count())) native=0"
        let eolSummary = "count=\(Int(plan.eol_diagnostic_count())) native=1"
        drawCursors(
            in: context,
            plan: plan,
            cellSize: cellSize,
            contentOffsetX: contentOffsetX,
            cursorPickState: cursorPickState,
            cursorOpacity: cursorOpacity
        )
        let perfAfterCursors = perfEnabled ? perfNow() : 0
        debugDrawSnapshot(
            plan: plan,
            textStats: textStats,
            inlineSummary: inlineSummary,
            eolSummary: eolSummary
        )
        let scroll = plan.scroll()
        return DrawPlanPerfStats(
            gutterMs: perfMs(perfAfterStart, perfAfterGutter),
            selectionsMs: perfMs(perfAfterGutter, perfAfterSelections),
            underlinesMs: perfMs(perfAfterSelections, perfAfterUnderlines),
            textMs: perfMs(perfAfterUnderlines, perfAfterText),
            cursorsMs: perfMs(perfAfterText, perfAfterCursors),
            drawnSpans: textStats.drawnSpans,
            skippedVirtualSpans: textStats.skippedVirtualSpans,
            lineCount: Int(plan.line_count()),
            scrollRow: Int(scroll.row),
            scrollCol: Int(scroll.col)
        )
    }

    // MARK: - Gutter

    private enum GutterSpanKind {
        case lineNumber
        case diagnostic
        case diff
        case other
    }

    private func classifyGutterSpan(_ span: RenderGutterSpan) -> GutterSpanKind {
        let text = span.text().toString()
        let trimmed = text.trimmingCharacters(in: .whitespaces)
        if trimmed.isEmpty { return .other }
        if trimmed == "\u{25CF}" { return .diagnostic }
        if trimmed == "+" || trimmed == "~" || trimmed == "-" { return .diff }
        if trimmed.allSatisfy({ $0.isNumber }) { return .lineNumber }
        return .other
    }

    private func activeCursorRow(plan: RenderPlan) -> UInt16? {
        let count = Int(plan.cursor_count())
        guard count > 0 else { return nil }
        let pos = plan.cursor_at(0).pos()
        return UInt16(clamping: pos.row)
    }

    private func drawGutter(
        in context: GraphicsContext,
        size: CGSize,
        plan: RenderPlan,
        cellSize: CGSize,
        font: Font,
        nsFont: NSFont,
        contentOffsetX: CGFloat
    ) {
        let lineCount = Int(plan.gutter_line_count())
        guard contentOffsetX > 0 else { return }

        let activeRow = activeCursorRow(plan: plan)
        let gutterBackground = gutterBackgroundColor()
        let gutterActiveBackground = gutterActiveRowColor()
        let gutterSeparator = gutterSeparatorColor()

        // Layer 1: Gutter background
        let gutterBg = CGRect(x: 0, y: 0, width: contentOffsetX, height: size.height)
        context.fill(Path(gutterBg), with: .color(gutterBackground))

        // Layer 2: Active line highlight
        if let activeRow {
            let y = CGFloat(activeRow) * cellSize.height
            let rect = CGRect(x: 0, y: y, width: contentOffsetX, height: cellSize.height)
            context.fill(Path(rect), with: .color(gutterActiveBackground))
        }

        // Layer 3: Vertical separator
        let sepPath = Path { p in
            p.move(to: CGPoint(x: contentOffsetX - 0.5, y: 0))
            p.addLine(to: CGPoint(x: contentOffsetX - 0.5, y: size.height))
        }
        context.stroke(sepPath, with: .color(gutterSeparator), lineWidth: 0.5)

        // Layer 4: Per-span rendering
        if lineCount > 0 {
            for lineIndex in 0..<lineCount {
                let line = plan.gutter_line_at(UInt(lineIndex))
                let y = CGFloat(line.row()) * cellSize.height
                let isActiveLine = activeRow == line.row()
                let spanCount = Int(line.span_count())

                for spanIndex in 0..<spanCount {
                    let span = line.span_at(UInt(spanIndex))
                    let x = CGFloat(span.col()) * cellSize.width
                    let kind = classifyGutterSpan(span)

                    switch kind {
                    case .diagnostic:
                        let color = colorForStyle(span.style(), fallback: SwiftUI.Color(nsColor: .systemRed))
                        drawDiagnosticIndicator(in: context, y: y, cellSize: cellSize, gutterWidth: contentOffsetX, color: color)
                    case .diff:
                        let fallback: SwiftUI.Color
                        switch span.text().toString().trimmingCharacters(in: .whitespaces) {
                        case "+": fallback = SwiftUI.Color(nsColor: .systemGreen)
                        case "~": fallback = SwiftUI.Color(nsColor: .systemYellow)
                        default:  fallback = SwiftUI.Color(nsColor: .systemRed)
                        }
                        let color = colorForStyle(span.style(), fallback: fallback)
                        drawDiffBar(in: context, y: y, cellSize: cellSize, gutterWidth: contentOffsetX, color: color)
                    case .lineNumber:
                        drawLineNumber(
                            in: context,
                            span: span,
                            x: x,
                            y: y,
                            font: font,
                            nsFont: nsFont,
                            isActive: isActiveLine
                        )
                    case .other:
                        break
                    }
                }
            }
        }
    }

    private func drawDiagnosticIndicator(
        in context: GraphicsContext,
        y: CGFloat,
        cellSize: CGSize,
        gutterWidth: CGFloat,
        color: SwiftUI.Color
    ) {
        // Row tint: subtle color wash across the entire gutter row
        let tintRect = CGRect(x: 0, y: y, width: gutterWidth, height: cellSize.height)
        context.fill(Path(tintRect), with: .color(color.opacity(0.06)))

        // Left-edge stripe: flush against the left wall, full cell height
        let stripeWidth: CGFloat = 2.0
        let stripeRect = CGRect(x: 0, y: y, width: stripeWidth, height: cellSize.height)
        context.fill(Path(stripeRect), with: .color(color.opacity(0.7)))
    }

    private func drawDiffBar(
        in context: GraphicsContext,
        y: CGFloat,
        cellSize: CGSize,
        gutterWidth: CGFloat,
        color: SwiftUI.Color
    ) {
        // Thin stripe along the separator edge — consecutive lines merge into
        // one continuous colored strip (no vertical inset).
        let barWidth: CGFloat = 2.0
        let barRect = CGRect(
            x: gutterWidth - barWidth - 0.5,
            y: y,
            width: barWidth,
            height: cellSize.height
        )
        context.fill(Path(barRect), with: .color(color.opacity(0.7)))
    }

    private func drawLineNumber(
        in context: GraphicsContext,
        span: RenderGutterSpan,
        x: CGFloat, y: CGFloat,
        font: Font,
        nsFont: NSFont,
        isActive: Bool
    ) {
        _ = font
        let style = span.style()
        let color: SwiftUI.Color
        if style.has_fg, let themeColor = ColorMapper.color(from: style.fg) {
            color = themeColor
        } else if isActive {
            color = gutterLineNumberColor(active: true)
        } else {
            color = gutterLineNumberColor(active: false)
        }
        drawAttributedText(
            in: context,
            text: span.text().toString(),
            at: CGPoint(x: x, y: y),
            nsFont: nsFont,
            color: nsColor(from: color)
        )
    }

    private func drawSelections(
        in context: GraphicsContext,
        plan: RenderPlan,
        cellSize: CGSize,
        contentOffsetX: CGFloat
    ) {
        let count = Int(plan.selection_count())
        guard count > 0 else { return }

        for index in 0..<count {
            let selection = plan.selection_at(UInt(index))
            let selectionRect = selection.rect()
            let style = selection.style()
            let x = contentOffsetX + CGFloat(selectionRect.x) * cellSize.width
            let y = CGFloat(selectionRect.y) * cellSize.height
            let width = CGFloat(selectionRect.width) * cellSize.width
            let height = CGFloat(selectionRect.height) * cellSize.height
            let path = Path(CGRect(x: x, y: y, width: width, height: height))
            let color = if style.has_bg, let bg = ColorMapper.color(from: style.bg) {
                bg.opacity(0.42)
            } else {
                SwiftUI.Color(red: 0.28, green: 0.52, blue: 1.0).opacity(0.36)
            }
            context.fill(path, with: .color(color))
        }
    }

    private struct DrawTextStats {
        let drawnSpans: Int
        let skippedVirtualSpans: Int
    }

    private struct DrawPlanPerfStats {
        let gutterMs: Double
        let selectionsMs: Double
        let underlinesMs: Double
        let textMs: Double
        let cursorsMs: Double
        let drawnSpans: Int
        let skippedVirtualSpans: Int
        let lineCount: Int
        let scrollRow: Int
        let scrollCol: Int

        static let zero = DrawPlanPerfStats(
            gutterMs: 0,
            selectionsMs: 0,
            underlinesMs: 0,
            textMs: 0,
            cursorsMs: 0,
            drawnSpans: 0,
            skippedVirtualSpans: 0,
            lineCount: 0,
            scrollRow: 0,
            scrollCol: 0
        )

        func adding(_ other: DrawPlanPerfStats) -> DrawPlanPerfStats {
            DrawPlanPerfStats(
                gutterMs: gutterMs + other.gutterMs,
                selectionsMs: selectionsMs + other.selectionsMs,
                underlinesMs: underlinesMs + other.underlinesMs,
                textMs: textMs + other.textMs,
                cursorsMs: cursorsMs + other.cursorsMs,
                drawnSpans: drawnSpans + other.drawnSpans,
                skippedVirtualSpans: skippedVirtualSpans + other.skippedVirtualSpans,
                lineCount: lineCount + other.lineCount,
                scrollRow: scrollRow,
                scrollCol: scrollCol
            )
        }
    }

    private func drawText(
        in context: GraphicsContext,
        paneId: UInt64,
        plan: RenderPlan,
        cellSize: CGSize,
        nsFont: NSFont,
        contentOffsetX: CGFloat
    ) -> DrawTextStats {
        let lineCount = Int(plan.line_count())
        guard lineCount > 0 else {
            return DrawTextStats(drawnSpans: 0, skippedVirtualSpans: 0)
        }

        let preparedScene = renderSceneCache.preparedTextScene(
            paneId: paneId,
            plan: plan,
            nsFont: nsFont
        ) { span in
            nsColor(from: colorForSpan(span))
        }
        renderSceneCache.drawTextScene(
            preparedScene,
            in: context,
            cellSize: cellSize,
            contentOffsetX: contentOffsetX
        )
        return DrawTextStats(
            drawnSpans: preparedScene.drawnSpans,
            skippedVirtualSpans: preparedScene.skippedVirtualSpans
        )
    }

    private func drawCursors(
        in context: GraphicsContext,
        plan: RenderPlan,
        cellSize: CGSize,
        contentOffsetX: CGFloat,
        cursorPickState: CursorPickState?,
        cursorOpacity: Double
    ) {
        let effectiveCursorOpacity = max(0.0, min(1.0, cursorOpacity))
        guard effectiveCursorOpacity > 0.001 else { return }
        let count = Int(plan.cursor_count())
        guard count > 0 else { return }

        let pickedCursorId: UInt64? = {
            guard let cursorPickState else { return nil }
            guard cursorPickState.currentIndex >= 0 && cursorPickState.currentIndex < count else { return nil }
            return plan.cursor_at(UInt(cursorPickState.currentIndex)).id()
        }()
        let fallbackCursorColor = SwiftUI.Color.accentColor
        let backingScale = NSApp.keyWindow?.backingScaleFactor
            ?? NSScreen.main?.backingScaleFactor
            ?? 2.0

        func snapToPixel(_ value: CGFloat) -> CGFloat {
            (value * backingScale).rounded(.down) / backingScale
        }

        for index in 0..<count {
            let cursor = plan.cursor_at(UInt(index))
            let pos = cursor.pos()
            let x = contentOffsetX + CGFloat(pos.col) * cellSize.width
            let y = CGFloat(pos.row) * cellSize.height
            let isPickedCursor = pickedCursorId == cursor.id()
            let baseCursorColor = cursorColor(for: cursor.style(), fallback: fallbackCursorColor)
            let strokeColor = baseCursorColor.opacity(effectiveCursorOpacity)

            switch cursor.kind() {
            case 1: // bar
                let barWidth: CGFloat = min(2.0, max(1.0, cellSize.width * 0.2))
                let rect = CGRect(
                    x: snapToPixel(x),
                    y: snapToPixel(y),
                    width: max(1.0 / backingScale, barWidth),
                    height: max(1.0 / backingScale, snapToPixel(cellSize.height))
                )
                context.fill(Path(rect), with: .color(strokeColor))
            case 2: // underline
                let rect = CGRect(x: x, y: y + cellSize.height - 2, width: cellSize.width, height: 2)
                context.fill(Path(rect), with: .color(strokeColor))
            case 3: // hollow
                let rect = CGRect(x: x, y: y, width: cellSize.width, height: cellSize.height)
                context.stroke(Path(rect), with: .color(strokeColor), lineWidth: 1.2)
            case 4: // hidden
                continue
            default: // block
                let rect = CGRect(x: x, y: y, width: cellSize.width, height: cellSize.height)
                context.fill(
                    Path(rect),
                    with: .color(baseCursorColor.opacity((isPickedCursor ? 0.65 : 0.5) * effectiveCursorOpacity))
                )
            }
        }
    }

    // MARK: - Inline Diagnostics

    private func drawInlineDiagnostics(
        in context: GraphicsContext,
        plan: RenderPlan,
        cellSize: CGSize,
        nsFont: NSFont,
        contentOffsetX: CGFloat
    ) -> String {
        let count = Int(plan.inline_diagnostic_line_count())
        guard count > 0 else { return "count=0" }
        var entries: [String] = []

        for i in 0..<count {
            let line = plan.inline_diagnostic_line_at(UInt(i))
            let y = CGFloat(line.row()) * cellSize.height
            let baseX = contentOffsetX + CGFloat(line.col()) * cellSize.width
            let color = diagnosticNSColor(severity: line.severity()).withAlphaComponent(0.6)
            entries.append(
                "\(line.row()):\(line.col()):\(line.severity()):\(debugTruncate(line.text().toString(), limit: 70))"
            )
            drawDiagnosticText(
                in: context,
                text: line.text().toString(),
                at: CGPoint(x: baseX, y: y),
                nsFont: nsFont,
                color: color
            )
        }
        return "count=\(count) [\(entries.joined(separator: " || "))]"
    }

    private func diagnosticColor(severity: UInt8) -> SwiftUI.Color {
        switch severity {
        case 1: return SwiftUI.Color(nsColor: .systemRed)
        case 2: return SwiftUI.Color(nsColor: .systemYellow)
        case 3: return SwiftUI.Color(nsColor: .systemBlue)
        case 4: return SwiftUI.Color(nsColor: .systemGreen)
        default: return SwiftUI.Color.white
        }
    }

    private func diagnosticNSColor(severity: UInt8) -> NSColor {
        switch severity {
        case 1: return .systemRed
        case 2: return .systemYellow
        case 3: return .systemBlue
        case 4: return .systemGreen
        default: return .white
        }
    }

    private func drawAttributedText(
        in context: GraphicsContext,
        text: String,
        at point: CGPoint,
        nsFont: NSFont,
        color: NSColor
    ) {
        guard !text.isEmpty else { return }
        let attrs: [NSAttributedString.Key: Any] = [
            .font: nsFont,
            .foregroundColor: color
        ]
        let attributed = NSAttributedString(string: text, attributes: attrs)
        context.withCGContext { cg in
            NSGraphicsContext.saveGraphicsState()
            NSGraphicsContext.current = NSGraphicsContext(cgContext: cg, flipped: true)
            attributed.draw(at: point)
            NSGraphicsContext.restoreGraphicsState()
        }
    }

    private func drawDiagnosticText(
        in context: GraphicsContext,
        text: String,
        at point: CGPoint,
        nsFont: NSFont,
        color: NSColor
    ) {
        drawAttributedText(
            in: context,
            text: text,
            at: point,
            nsFont: nsFont,
            color: color
        )
    }

    // MARK: - End-of-Line Diagnostics

    private func drawEolDiagnostics(
        in context: GraphicsContext,
        plan: RenderPlan,
        cellSize: CGSize,
        nsFont: NSFont,
        contentOffsetX: CGFloat
    ) -> String {
        let count = Int(plan.eol_diagnostic_count())
        guard count > 0 else { return "count=0" }
        var entries: [String] = []

        for i in 0..<count {
            let eol = plan.eol_diagnostic_at(UInt(i))
            let y = CGFloat(eol.row()) * cellSize.height
            let baseX = contentOffsetX + CGFloat(eol.col()) * cellSize.width
            let color = diagnosticNSColor(severity: eol.severity()).withAlphaComponent(0.6)
            entries.append(
                "\(eol.row()):\(eol.col()):\(eol.severity()):\(debugTruncate(eol.message().toString(), limit: 70))"
            )
            drawDiagnosticText(
                in: context,
                text: eol.message().toString(),
                at: CGPoint(x: baseX, y: y),
                nsFont: nsFont,
                color: color
            )
        }
        return "count=\(count) [\(entries.joined(separator: " || "))]"
    }

    // MARK: - Diagnostic Underlines

    private func drawDiagnosticUnderlines(
        in context: GraphicsContext,
        plan: RenderPlan,
        cellSize: CGSize,
        contentOffsetX: CGFloat
    ) {
        let count = Int(plan.diagnostic_underline_count())
        guard count > 0 else { return }

        for i in 0..<count {
            let underline = plan.diagnostic_underline_at(UInt(i))
            let y = CGFloat(underline.row()) * cellSize.height + cellSize.height - 1
            let xStart = contentOffsetX + CGFloat(underline.start_col()) * cellSize.width
            let xEnd = contentOffsetX + CGFloat(underline.end_col()) * cellSize.width
            let color = diagnosticColor(severity: underline.severity()).opacity(0.7)

            // Draw wavy underline
            let width = xEnd - xStart
            guard width > 0 else { continue }
            let waveHeight: CGFloat = 2.0
            let wavelength: CGFloat = 4.0
            var path = Path()
            var x = xStart
            path.move(to: CGPoint(x: x, y: y))
            var up = true
            while x < xEnd {
                let nextX = min(x + wavelength, xEnd)
                let controlY = up ? y - waveHeight : y + waveHeight
                path.addQuadCurve(
                    to: CGPoint(x: nextX, y: y),
                    control: CGPoint(x: (x + nextX) / 2, y: controlY)
                )
                x = nextX
                up.toggle()
            }
            context.stroke(path, with: .color(color), lineWidth: 1.0)
        }
    }

    private func colorForStyle(_ style: Style, fallback: SwiftUI.Color) -> SwiftUI.Color {
        guard style.has_fg, let color = ColorMapper.color(from: style.fg) else {
            return fallback
        }
        return color
    }

    private func gutterBackgroundColor() -> SwiftUI.Color {
        model.gutterBackgroundColor()
    }

    private func gutterActiveRowColor() -> SwiftUI.Color {
        model.gutterActiveRowColor()
    }

    private func gutterSeparatorColor() -> SwiftUI.Color {
        model.gutterSeparatorColor()
    }

    private func gutterLineNumberColor(active: Bool) -> SwiftUI.Color {
        model.gutterLineNumberColor(active: active)
    }

    private func cursorColor(for style: Style, fallback: SwiftUI.Color) -> SwiftUI.Color {
        if style.has_bg, let bg = ColorMapper.color(from: style.bg) {
            return bg
        }
        if style.has_fg, let fg = ColorMapper.color(from: style.fg) {
            return fg
        }
        return fallback
    }

    private func colorForSpan(_ span: RenderSpan) -> SwiftUI.Color {
        let baseTextColor = model.editorTextColor()
        if span.has_highlight() {
            if let color = model.colorForHighlight(span.highlight()) {
                return color
            }
        }
        if span.is_virtual() {
            return model.editorVirtualTextColor()
        }
        return baseTextColor
    }

    private func nsColor(from color: SwiftUI.Color) -> NSColor {
        NSColor(color)
    }

    private func debugDrawSnapshot(
        plan: RenderPlan,
        textStats: DrawTextStats,
        inlineSummary: String,
        eolSummary: String
    ) {
        guard DiagnosticsDebugLog.enabled else { return }
        let cursorCount = Int(plan.cursor_count())
        let cursorSummary: String
        if cursorCount > 0 {
            let pos = plan.cursor_at(0).pos()
            cursorSummary = "\(pos.row):\(pos.col)"
        } else {
            cursorSummary = "none"
        }
        DiagnosticsDebugLog.logChanged(
            key: "view.draw",
            value: "cursor=\(cursorSummary) drawn_spans=\(textStats.drawnSpans) skipped_virtual=\(textStats.skippedVirtualSpans) inline=\(inlineSummary) eol=\(eolSummary)"
        )
    }

    private func debugTerminalFocusState(
        model: EditorModel,
        focusedTerminalPaneId: UInt64?,
        shouldTerminalOwnFocus: Bool,
        terminalPaneLayouts: [TerminalPaneLayout],
        effectiveActivePaneId: UInt64
    ) {
        guard DiagnosticsDebugLog.enabled else { return }
        let frameActivePaneId = model.framePlan.active_pane_id()
        let windowNumber = model.currentHostWindow()?.windowNumber ?? 0
        let snapshotSummary = model.terminalPanes
            .map { "p\($0.paneId):t\($0.terminalId):s\($0.isActive ? 1 : 0)" }
            .joined(separator: ",")
        let layoutSummary = terminalPaneLayouts
            .map { "p\($0.paneId):t\($0.terminalId):l\($0.isActive ? 1 : 0)" }
            .joined(separator: ",")
        DiagnosticsDebugLog.logChanged(
            key: "editor.view.terminal_focus.window\(windowNumber).runtime\(model.runtimeInstanceId).editor\(model.editorId.value)",
            value: "frame_active=\(frameActivePaneId) responder=\(focusedTerminalPaneId ?? 0) effective_active=\(effectiveActivePaneId) terminal_focus_owns=\(shouldTerminalOwnFocus ? 1 : 0) snapshots=[\(snapshotSummary)] layouts=[\(layoutSummary)]"
        )
    }

    private func debugTruncate(_ text: String, limit: Int) -> String {
        let normalized = text
            .replacingOccurrences(of: "\n", with: "\\n")
            .replacingOccurrences(of: "\t", with: "\\t")
        if normalized.count <= limit {
            return normalized
        }
        let idx = normalized.index(normalized.startIndex, offsetBy: limit)
        return "\(normalized[..<idx])..."
    }
}

private struct SidebarChromeCompensationView<Content: View>: View {
    @State private var topInset: CGFloat = 0
    @State private var baselineTopInset: CGFloat?
    @State private var tabBarVisibilityKnown = false
    @State private var nativeTabBarVisible = false
    private let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        GeometryReader { proxy in
            let safeAreaTopInset = proxy.safeAreaInsets.top
            content
                .padding(.top, -topInsetCompensation(for: safeAreaTopInset))
                .onAppear {
                    updateTopInset(safeAreaTopInset)
                }
                .onChange(of: safeAreaTopInset) { newValue in
                    updateTopInset(newValue)
                }
                .background(
                    WindowTabBarVisibilityProbe { visible in
                        tabBarVisibilityKnown = true
                        nativeTabBarVisible = visible
                        if !visible {
                            learnBaseline(from: topInset)
                        }
                    }
                    .frame(width: 0, height: 0)
                    .allowsHitTesting(false)
                )
        }
    }

    private var effectiveBaselineTopInset: CGFloat? {
        baselineTopInset ?? SidebarChromeBaselineCache.topInset
    }

    private func topInsetCompensation(for currentTopInset: CGFloat) -> CGFloat {
        guard nativeTabBarVisible else { return 0 }
        guard let baseline = effectiveBaselineTopInset else { return 0 }
        return max(0, currentTopInset - baseline)
    }

    private func updateTopInset(_ newValue: CGFloat) {
        if abs(topInset - newValue) > 0.5 {
            topInset = newValue
        }
        learnBaseline(from: newValue)
    }

    private func learnBaseline(from currentTopInset: CGFloat) {
        guard tabBarVisibilityKnown else { return }
        guard !nativeTabBarVisible else { return }
        guard currentTopInset > 1 else { return }
        guard abs((effectiveBaselineTopInset ?? .zero) - currentTopInset) > 0.5 else {
            return
        }

        baselineTopInset = currentTopInset
        SidebarChromeBaselineCache.topInset = currentTopInset
    }
}
