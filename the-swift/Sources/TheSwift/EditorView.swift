import SwiftUI
import TheEditorFFIBridge

private let embeddedTerminalCropInset: CGFloat = 2
private let inactivePaneDimOpacity: CGFloat = 0.22

struct EditorView: View {
    @Environment(\.openWindow) private var openWindow
    @StateObject private var model: EditorModel
    @ObservedObject private var globalTerminalSwitcher = GlobalTerminalSwitcherController.shared
    private let windowRoute: EditorWindowRoute?
    @State private var columnVisibility: NavigationSplitViewVisibility = .detailOnly
    @SceneStorage("surfaceRailVisible") private var surfaceRailVisible = false

    private static let surfaceRailWidth: CGFloat = 248

    private struct CursorPickState {
        let remove: Bool
        let currentIndex: Int
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
            sidebar(snapshot: fileTreeSnapshot)
        } detail: {
            detailContent(
                editorBackgroundColor: editorBackgroundColor,
                surfaceRailSnapshot: surfaceRailSnapshot
            )
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
                    Button {
                        surfaceRailVisible.toggle()
                    } label: {
                        Image(systemName: "sidebar.right")
                    }
                    .help("Toggle Surface Rail")
                    .accessibilityLabel("Toggle Surface Rail")
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
    private func sidebar(snapshot: FileTreeSnapshot) -> some View {
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
        .navigationSplitViewColumnWidth(min: 180, ideal: 240, max: 360)
    }

    @ViewBuilder
    private func detailContent(
        editorBackgroundColor: SwiftUI.Color,
        surfaceRailSnapshot: SurfaceRailSnapshot
    ) -> some View {
        HStack(spacing: 0) {
            VStack(spacing: 0) {
                GeometryReader { contentProxy in
                    detailGeometryContent(
                        contentProxy: contentProxy,
                        editorBackgroundColor: editorBackgroundColor
                    )
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)

            if surfaceRailVisible {
                SurfaceRailView(
                    snapshot: surfaceRailSnapshot,
                    onSelectBuffer: { bufferIndex in
                        model.selectBufferTab(bufferIndex: bufferIndex)
                    },
                    onSelectTerminal: { terminalId in
                        _ = model.focusTerminalSurface(terminalId: terminalId)
                    }
                )
                .frame(width: Self.surfaceRailWidth)
                .transition(.move(edge: .trailing))
            }
        }
        .background(editorBackgroundColor)
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
        let hoverSnapshot = model.uiTree.hoverSnapshot()
        let signatureSnapshot = model.uiTree.signatureHelpSnapshot()
        let popupTheme = model.popupTheme()
        let isCompletionOpen = completionSnapshot != nil
        let isHoverOpen = hoverSnapshot != nil && !isCompletionOpen
        let isSignatureOpen = signatureSnapshot != nil && !isCompletionOpen && !isHoverOpen
        let shouldTerminalOwnFocus = !isOverlayOpen && !isCompletionOpen && !isHoverOpen && !isSignatureOpen
        let focusedTerminalPaneId = GhosttyRuntime.shared.firstResponderPaneId(in: model.currentHostWindow())
        let effectiveActivePaneId = focusedTerminalPaneId ?? model.framePlan.active_pane_id()
        let cursorPickState = cursorPickState(from: model.uiTree.statuslineSnapshot())
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
                value: "overlay_open=\(isOverlayOpen ? 1 : 0) completion=\(isCompletionOpen ? 1 : 0) hover=\(isHoverOpen ? 1 : 0) signature=\(isSignatureOpen ? 1 : 0) active_pane=\(effectiveActivePaneId)"
            )
        }()
        let terminalPassthroughRects = terminalPaneLayouts.map(\.frame)

        ZStack {
            terminalCanvas(
                cellSize: cellSize,
                bufferFont: bufferFont,
                bufferNSFont: bufferNSFont,
                terminalPaneIds: terminalPaneIds,
                effectiveActivePaneId: effectiveActivePaneId,
                cursorPickState: cursorPickState,
                editorBackgroundColor: editorBackgroundColor
            )

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
                hoverSnapshot: hoverSnapshot,
                signatureSnapshot: signatureSnapshot,
                popupTheme: popupTheme,
                activePaneOrigin: activePaneOrigin,
                cellSize: cellSize,
                contentSize: contentProxy.size
            )
        }
        .background(keyCaptureBackground(isOverlayOpen: isOverlayOpen))
        .overlay(
            scrollCaptureOverlay(
                isOverlayOpen: isOverlayOpen,
                isCompletionOpen: isCompletionOpen,
                isHoverOpen: isHoverOpen,
                isSignatureOpen: isSignatureOpen,
                splitResizeHandles: splitResizeHandles,
                terminalPassthroughRects: terminalPassthroughRects,
                pointerPanes: pointerPanes,
                cellSize: cellSize
            )
        )
        .overlay(
            KeySequenceIndicator(keys: model.pendingKeys, hints: model.pendingKeyHints)
                .padding(.bottom, 28)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom),
            alignment: .bottom
        )
        .onAppear {
            model.updateViewport(pixelSize: contentProxy.size, cellSize: cellSize)
        }
        .onChange(of: contentProxy.size) { newSize in
            model.updateViewport(pixelSize: newSize, cellSize: cellSize)
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
                cursorPickState: cursorPickState
            )
        }
        .background(editorBackgroundColor)
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
        hoverSnapshot: HoverSnapshot?,
        signatureSnapshot: SignatureHelpSnapshot?,
        popupTheme: PopupChromeTheme,
        activePaneOrigin: CGPoint,
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
            } else if let hoverSnapshot {
                HoverPopupView(
                    snapshot: hoverSnapshot,
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
        isHoverOpen: Bool,
        isSignatureOpen: Bool,
        splitResizeHandles: [ScrollCaptureView.SeparatorHandle],
        terminalPassthroughRects: [CGRect],
        pointerPanes: [ScrollCaptureView.PaneHandle],
        cellSize: CGSize
    ) -> some View {
        if !isOverlayOpen && !isCompletionOpen && !isHoverOpen && !isSignatureOpen {
            ScrollCaptureView(
                onScroll: { deltaX, deltaY, precise in
                    model.handlePointerScroll(deltaX: deltaX, deltaY: deltaY, precise: precise)
                },
                onPointer: { event in
                    model.handlePointerEvent(event)
                },
                separators: splitResizeHandles,
                passthroughRects: terminalPassthroughRects,
                panes: pointerPanes,
                cellSize: cellSize,
                onSplitResize: { splitId, point in
                    model.resizeSplit(splitId: splitId, pixelPoint: point)
                }
            )
            .allowsHitTesting(true)
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
        cursorPickState: CursorPickState?
    ) {
        let paneCount = Int(framePlan.pane_count())
        guard paneCount > 0 else {
            drawPlan(
                in: context,
                size: size,
                plan: fallbackPlan,
                cellSize: cellSize,
                bufferFont: bufferFont,
                bufferNSFont: bufferNSFont,
                cursorPickState: cursorPickState
            )
            return
        }

        for index in 0..<paneCount {
            let pane = framePlan.pane_at(UInt(index))
            if terminalPaneIds.contains(pane.pane_id()) {
                continue
            }
            let paneRect = pane.rect()
            let paneSize = CGSize(
                width: CGFloat(paneRect.width) * cellSize.width,
                height: CGFloat(paneRect.height) * cellSize.height
            )
            var paneContext = context
            paneContext.translateBy(
                x: CGFloat(paneRect.x) * cellSize.width,
                y: CGFloat(paneRect.y) * cellSize.height
            )
            drawPlan(
                in: paneContext,
                size: paneSize,
                plan: pane.plan(),
                cellSize: cellSize,
                bufferFont: bufferFont,
                bufferNSFont: bufferNSFont,
                cursorPickState: pane.pane_id() == activePaneId ? cursorPickState : nil
            )
        }

        drawPaneSeparators(
            in: context,
            framePlan: framePlan,
            canvasSize: size,
            cellSize: cellSize
        )
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
        plan: RenderPlan,
        cellSize: CGSize,
        bufferFont: Font,
        bufferNSFont: NSFont,
        cursorPickState: CursorPickState?
    ) {
        let contentOffsetX = CGFloat(plan.content_offset_x()) * cellSize.width
        drawGutter(
            in: context,
            size: size,
            plan: plan,
            cellSize: cellSize,
            font: bufferFont,
            contentOffsetX: contentOffsetX
        )
        drawSelections(in: context, plan: plan, cellSize: cellSize, contentOffsetX: contentOffsetX)
        drawDiagnosticUnderlines(in: context, plan: plan, cellSize: cellSize, contentOffsetX: contentOffsetX)
        let textStats = drawText(
            in: context,
            plan: plan,
            cellSize: cellSize,
            font: bufferFont,
            contentOffsetX: contentOffsetX
        )
        let inlineSummary = drawInlineDiagnostics(
            in: context,
            plan: plan,
            cellSize: cellSize,
            nsFont: bufferNSFont,
            contentOffsetX: contentOffsetX
        )
        let eolSummary = drawEolDiagnostics(
            in: context,
            plan: plan,
            cellSize: cellSize,
            nsFont: bufferNSFont,
            contentOffsetX: contentOffsetX
        )
        drawCursors(
            in: context,
            plan: plan,
            cellSize: cellSize,
            contentOffsetX: contentOffsetX,
            cursorPickState: cursorPickState
        )
        debugDrawSnapshot(
            plan: plan,
            textStats: textStats,
            inlineSummary: inlineSummary,
            eolSummary: eolSummary
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
                        drawLineNumber(in: context, span: span, x: x, y: y, font: font, isActive: isActiveLine)
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
        isActive: Bool
    ) {
        let style = span.style()
        let color: SwiftUI.Color
        if style.has_fg, let themeColor = ColorMapper.color(from: style.fg) {
            color = themeColor
        } else if isActive {
            color = gutterLineNumberColor(active: true)
        } else {
            color = gutterLineNumberColor(active: false)
        }
        let text = Text(span.text().toString()).font(font).foregroundColor(color)
        context.draw(text, at: CGPoint(x: x, y: y), anchor: .topLeading)
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
            let rect = selection.rect()
            let style = selection.style()
            let x = contentOffsetX + CGFloat(rect.x) * cellSize.width
            let y = CGFloat(rect.y) * cellSize.height
            let width = CGFloat(rect.width) * cellSize.width
            let height = CGFloat(rect.height) * cellSize.height
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

    private func drawText(
        in context: GraphicsContext,
        plan: RenderPlan,
        cellSize: CGSize,
        font: Font,
        contentOffsetX: CGFloat
    ) -> DrawTextStats {
        let lineCount = Int(plan.line_count())
        guard lineCount > 0 else {
            return DrawTextStats(drawnSpans: 0, skippedVirtualSpans: 0)
        }

        var drawnSpans = 0
        var skippedVirtualSpans = 0

        for lineIndex in 0..<lineCount {
            let line = plan.line_at(UInt(lineIndex))
            let y = CGFloat(line.row()) * cellSize.height
            let spanCount = Int(line.span_count())

            for spanIndex in 0..<spanCount {
                let span = line.span_at(UInt(spanIndex))
                // Inline diagnostics are rendered explicitly in dedicated passes
                // below; skipping virtual spans avoids double-drawing artifacts.
                if span.is_virtual() {
                    skippedVirtualSpans += 1
                    continue
                }
                drawnSpans += 1
                let x = contentOffsetX + CGFloat(span.col()) * cellSize.width
                let color = colorForSpan(span)
                let text = Text(span.text().toString()).font(font).foregroundColor(color)
                context.draw(text, at: CGPoint(x: x, y: y), anchor: .topLeading)
            }
        }

        return DrawTextStats(drawnSpans: drawnSpans, skippedVirtualSpans: skippedVirtualSpans)
    }

    private func drawCursors(
        in context: GraphicsContext,
        plan: RenderPlan,
        cellSize: CGSize,
        contentOffsetX: CGFloat,
        cursorPickState: CursorPickState?
    ) {
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
            let strokeColor = cursorColor(for: cursor.style(), fallback: fallbackCursorColor)

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
                context.fill(Path(rect), with: .color(strokeColor.opacity(isPickedCursor ? 0.65 : 0.5)))
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

    private func drawDiagnosticText(
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
        let lineNumberStyle = model.uiThemeStyle("ui.linenr")
        if lineNumberStyle.has_bg, let color = ColorMapper.color(from: lineNumberStyle.bg) {
            return color
        }
        return model.editorBackgroundColor()
    }

    private func gutterActiveRowColor() -> SwiftUI.Color {
        let cursorLineStyle = model.uiThemeStyle("ui.cursorline.active")
        if cursorLineStyle.has_bg, let color = ColorMapper.color(from: cursorLineStyle.bg) {
            return color
        }
        return SwiftUI.Color.white.opacity(0.04)
    }

    private func gutterSeparatorColor() -> SwiftUI.Color {
        let separatorStyle = model.uiThemeStyle("ui.background.separator")
        if separatorStyle.has_fg, let color = ColorMapper.color(from: separatorStyle.fg) {
            return color.opacity(0.55)
        }
        return SwiftUI.Color(nsColor: .separatorColor).opacity(0.3)
    }

    private func gutterLineNumberColor(active: Bool) -> SwiftUI.Color {
        let scope = active ? "ui.linenr.selected" : "ui.linenr"
        let style = model.uiThemeStyle(scope)
        if style.has_fg, let color = ColorMapper.color(from: style.fg) {
            return color
        }
        return active ? SwiftUI.Color(nsColor: .secondaryLabelColor) : SwiftUI.Color(nsColor: .tertiaryLabelColor)
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
