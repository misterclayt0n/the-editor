import AppKit
import Foundation
import MetalKit
import QuartzCore
import TheEditorFFI

@MainActor
final class EditorSurfaceView: NSView, @preconcurrency NSTextInputClient {
    weak var controller: EditorSurfaceController?

    private var renderer: MetalEditorRenderer
    private lazy var cursorBlinkController = EditorCursorBlinkController { [weak self] opacity in
        guard let self else { return }
        self.renderer.setActiveCursorBlinkOpacity(opacity)
        self.renderer.drawImmediately()
    }
    private var font: NSFont
    private var fontMetrics: EditorFontMetrics
    private var fallbackCellSize: CGSize
    private var bufferFontSize: CGFloat

    private struct SplitDragState {
        let splitID: UInt
    }

    private struct ScrollbarDragState {
        let paneID: UInt
        let thumbOffsetY: CGFloat
    }

    private struct BufferSelectionDragState {
        let paneID: UInt
        let originLogicalCol: Int
        let originLogicalRow: Int
        let modifiers: UInt8
        let clickCount: Int
    }

    var cellSize: CGSize {
        controller?.scene?.info.surfaceMetrics.cellSizePoints ?? fallbackCellSize
    }
    private var markedText = NSMutableAttributedString()
    private var pendingScrollRows: CGFloat = 0
    private var pendingScrollCols: CGFloat = 0
    private var splitDrag: SplitDragState?
    private var scrollbarDrag: ScrollbarDragState?
    private var bufferSelectionDrag: BufferSelectionDragState?
    private var notificationObservers: [NSObjectProtocol] = []

    override var acceptsFirstResponder: Bool { true }
    override var isFlipped: Bool { true }

    init?(controller: EditorSurfaceController) {
        self.controller = controller
        let initialBufferFontSize = controller.bufferFontSize
        let initialFont = Self.makeEditorFont(size: initialBufferFontSize)
        let initialFontMetrics = EditorFontMetrics(font: initialFont)
        guard let renderer = Self.makeRenderer(fontMetrics: initialFontMetrics) else {
            return nil
        }
        self.renderer = renderer
        self.font = initialFont
        self.fontMetrics = initialFontMetrics
        self.fallbackCellSize = initialFontMetrics.cellSize
        self.bufferFontSize = initialBufferFontSize
        super.init(frame: .zero)
        wantsLayer = true
        addSubview(renderer.view)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private static func makeEditorFont(size: CGFloat) -> NSFont {
        NSFont.monospacedSystemFont(ofSize: size, weight: .regular)
    }

    private static func makeRenderer(fontMetrics: EditorFontMetrics) -> MetalEditorRenderer? {
        MetalEditorRenderer(fontMetrics: fontMetrics, scaleProvider: {
            NSScreen.main?.backingScaleFactor ?? 2
        })
    }

    func updateBufferFontSize(_ pointSize: CGFloat) {
        guard abs(pointSize - bufferFontSize) > 0.001 else { return }
        let nextFont = Self.makeEditorFont(size: pointSize)
        let nextFontMetrics = EditorFontMetrics(font: nextFont)
        guard let nextRenderer = Self.makeRenderer(fontMetrics: nextFontMetrics) else { return }

        let previousRendererView = renderer.view
        renderer = nextRenderer
        font = nextFont
        fontMetrics = nextFontMetrics
        fallbackCellSize = nextFontMetrics.cellSize
        bufferFontSize = pointSize

        previousRendererView.removeFromSuperview()
        addSubview(nextRenderer.view)
        applyRendererGeometry()
        synchronizeSurfaceConfiguration(forceDraw: true)
        inputContext?.invalidateCharacterCoordinates()
        window?.invalidateCursorRects(for: self)
        refreshCursorBlinkState(reset: true)
    }

    private func describe(_ drag: BufferSelectionDragState) -> String {
        "pane=\(drag.paneID) origin=(\(drag.originLogicalCol),\(drag.originLogicalRow)) modifiers=\(drag.modifiers) clickCount=\(drag.clickCount)"
    }

    private func pointText(_ point: CGPoint) -> String {
        String(format: "(%.1f,%.1f)", point.x, point.y)
    }

    override func layout() {
        super.layout()
        applyRendererGeometry()
        synchronizeSurfaceConfiguration()
    }

    override func viewWillMove(toWindow newWindow: NSWindow?) {
        if newWindow == nil || newWindow != window {
            removeBlinkObservers()
            cursorBlinkController.stop()
        }
        super.viewWillMove(toWindow: newWindow)
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        installBlinkObservers()
        refreshCursorBlinkState()
    }

    override func becomeFirstResponder() -> Bool {
        let accepted = super.becomeFirstResponder()
        refreshCursorBlinkState(reset: true)
        return accepted
    }

    override func resignFirstResponder() -> Bool {
        let accepted = super.resignFirstResponder()
        refreshCursorBlinkState()
        return accepted
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        applyRendererGeometry()
        synchronizeSurfaceConfiguration(forceDraw: true)
    }

    private func installBlinkObservers() {
        removeBlinkObservers()
        let center = NotificationCenter.default
        if let window {
            notificationObservers.append(center.addObserver(
                forName: NSWindow.didBecomeKeyNotification,
                object: window,
                queue: .main
            ) { [weak self] _ in
                Task { @MainActor [weak self] in
                    self?.refreshCursorBlinkState(reset: true)
                }
            })
            notificationObservers.append(center.addObserver(
                forName: NSWindow.didResignKeyNotification,
                object: window,
                queue: .main
            ) { [weak self] _ in
                Task { @MainActor [weak self] in
                    self?.refreshCursorBlinkState()
                }
            })
        }
        notificationObservers.append(center.addObserver(
            forName: NSApplication.didBecomeActiveNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.refreshCursorBlinkState(reset: true)
            }
        })
        notificationObservers.append(center.addObserver(
            forName: NSApplication.didResignActiveNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.refreshCursorBlinkState()
            }
        })
    }

    private func removeBlinkObservers() {
        let center = NotificationCenter.default
        for observer in notificationObservers {
            center.removeObserver(observer)
        }
        notificationObservers.removeAll()
    }

    private func refreshCursorBlinkState(reset: Bool = false) {
        let isTracking = splitDrag != nil || scrollbarDrag != nil || bufferSelectionDrag != nil
        cursorBlinkController.update(
            scene: controller?.scene,
            isFirstResponder: window?.firstResponder === self,
            windowIsKey: window?.isKeyWindow == true,
            appIsActive: NSApp.isActive,
            isTracking: isTracking
        )
        if reset {
            cursorBlinkController.reset()
        }
    }

    private func applyRendererGeometry() {
        let backingScale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 1
        let backingBounds = convertToBacking(bounds)

        CATransaction.begin()
        CATransaction.setDisableActions(true)
        renderer.view.frame = bounds
        renderer.view.drawableSize = backingBounds.size
        layer?.contentsScale = backingScale
        renderer.view.layer?.contentsScale = backingScale
        renderer.view.layer?.contentsGravity = .topLeft
        CATransaction.commit()
    }

    private func synchronizeSurfaceConfiguration(forceDraw: Bool = false) {
        guard let controller else { return }
        let backingScale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 1
        let changed = controller.configureSurface(
            size: bounds.size,
            backingScale: backingScale,
            fontMetrics: fontMetrics
        )
        if changed || forceDraw || window?.inLiveResize == true || controller.isInteractiveResizeActive {
            renderer.drawImmediately()
        }
    }

    func update(scene: EditorRenderScene) {
        renderer.update(scene: scene)
        refreshCursorBlinkState()
        window?.invalidateCursorRects(for: self)
    }

    override func resetCursorRects() {
        discardCursorRects()
        guard let scene = controller?.scene else {
            addCursorRect(bounds, cursor: .iBeam)
            return
        }

        for pane in scene.panes {
            let paneRect = scene.paneRect(for: pane)
            guard paneRect.width > 0, paneRect.height > 0 else { continue }
            if pane.kind != .editorBuffer {
                addCursorRect(paneRect, cursor: .arrow)
                continue
            }
            let contentRect = scene.paneContentRect(for: pane)
            let gutterWidth = CGFloat(pane.contentOffsetX) * scene.info.surfaceMetrics.cellSizePoints.width
            let headerHeight = max(contentRect.minY - paneRect.minY, 0)
            if headerHeight > 0 {
                addCursorRect(
                    NSRect(x: paneRect.minX, y: paneRect.minY, width: paneRect.width, height: headerHeight),
                    cursor: .arrow
                )
            }
            if gutterWidth > 0 {
                addCursorRect(
                    NSRect(x: contentRect.minX, y: contentRect.minY, width: min(gutterWidth, contentRect.width), height: contentRect.height),
                    cursor: .arrow
                )
            }
            let textRect = NSRect(
                x: contentRect.minX + min(gutterWidth, contentRect.width),
                y: contentRect.minY,
                width: max(contentRect.width - gutterWidth, 0),
                height: contentRect.height
            )
            if textRect.width > 0, textRect.height > 0 {
                addCursorRect(textRect, cursor: .iBeam)
            }
        }

        for separator in scene.separators {
            let rect = separatorHitRect(separator, cellSize: scene.info.surfaceMetrics.cellSizePoints)
            let cursor: NSCursor = separator.axis == .vertical ? .resizeLeftRight : .resizeUpDown
            addCursorRect(rect, cursor: cursor)
        }

        for pane in scene.panes {
            guard let geometry = scrollbarGeometry(for: pane, cellSize: scene.info.surfaceMetrics.cellSizePoints) else { continue }
            addCursorRect(geometry.trackRect.insetBy(dx: -2, dy: 0), cursor: .arrow)
        }
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        bufferSelectionDrag = nil
        let point = convert(event.locationInWindow, from: nil)
        if let separator = separator(at: point) {
            splitDrag = SplitDragState(splitID: separator.splitID)
            refreshCursorBlinkState(reset: true)
            return
        }
        if let geometry = scrollbarGeometry(at: point) {
            if !geometry.pane.isActive {
                controller?.setActivePane(geometry.pane.paneID)
            }
            let thumbOffsetY = geometry.thumbRect.contains(point)
                ? point.y - geometry.thumbRect.minY
                : geometry.thumbRect.height * 0.5
            scrollbarDrag = ScrollbarDragState(paneID: geometry.pane.paneID, thumbOffsetY: thumbOffsetY)
            refreshCursorBlinkState(reset: true)
            updateScrollPosition(for: geometry.pane.paneID, pointerY: point.y, thumbOffsetY: thumbOffsetY)
            return
        }
        if let hit = bufferTextHit(at: point) {
            let modifiers = pointerModifiers(from: event.modifierFlags)
            selectionDebugLog(
                "surface.mouseDown point=\(pointText(point)) pane=\(hit.paneID) logical=(\(hit.logicalCol),\(hit.logicalRow)) modifiers=\(modifiers) clickCount=\(event.clickCount)"
            )
            cursorBlinkController.reset()
            controller?.clickBufferPosition(
                paneID: hit.paneID,
                logicalCol: hit.logicalCol,
                logicalRow: hit.logicalRow,
                modifiers: modifiers,
                clickCount: event.clickCount
            )
            bufferSelectionDrag = BufferSelectionDragState(
                paneID: hit.paneID,
                originLogicalCol: hit.logicalCol,
                originLogicalRow: hit.logicalRow,
                modifiers: modifiers,
                clickCount: event.clickCount
            )
            refreshCursorBlinkState(reset: true)
            return
        }
        activatePaneIfNeeded(at: point)
    }

    override func scrollWheel(with event: NSEvent) {
        guard let controller else {
            super.scrollWheel(with: event)
            return
        }

        let started = CFAbsoluteTimeGetCurrent()
        let point = convert(event.locationInWindow, from: nil)
        let hoveredPane = pane(at: point)
        activatePaneIfNeeded(at: point)
        guard hoveredPane?.kind == .editorBuffer else { return }

        if let drag = bufferSelectionDrag {
            selectionDebugLog(
                "surface.scroll cancelSelectionDrag point=\(pointText(point)) hoveredPane=\(hoveredPane.map { String($0.paneID) } ?? "nil") drag=\(describe(drag))"
            )
            bufferSelectionDrag = nil
            refreshCursorBlinkState(reset: true)
        }

        let direction: CGFloat = event.isDirectionInvertedFromDevice ? -1 : 1
        var deltaX = direction * event.scrollingDeltaX
        var deltaY = direction * event.scrollingDeltaY

        if event.hasPreciseScrollingDeltas {
            deltaX *= 2
            deltaY *= 2

            let absX = abs(deltaX)
            let absY = abs(deltaY)
            if absX > absY {
                deltaY = 0
            } else if absY > absX {
                deltaX = 0
            }
        }

        let cellWidth = max(cellSize.width, 1)
        let cellHeight = max(cellSize.height, 1)
        let rowDelta: Int
        let colDelta: Int
        if event.hasPreciseScrollingDeltas {
            pendingScrollRows += deltaY / cellHeight
            rowDelta = Int(pendingScrollRows.rounded(.towardZero))
            pendingScrollRows -= CGFloat(rowDelta)

            pendingScrollCols += deltaX / cellWidth
            colDelta = Int(pendingScrollCols.rounded(.towardZero))
            pendingScrollCols -= CGFloat(colDelta)
        } else {
            rowDelta = Int(deltaY.rounded(.towardZero))
            colDelta = Int(deltaX.rounded(.towardZero))
            pendingScrollRows = 0
            pendingScrollCols = 0
        }

        let softWrapEnabled = controller.scene?.info.softWrapEnabled ?? false
        let effectiveColDelta: Int
        if softWrapEnabled {
            pendingScrollCols = 0
            effectiveColDelta = 0
        } else {
            effectiveColDelta = colDelta
        }

        let totalMs: () -> String = {
            String(format: "%.2f", (CFAbsoluteTimeGetCurrent() - started) * 1000)
        }
        guard rowDelta != 0 || effectiveColDelta != 0 else {
            scrollPerfLog(
                "surface.scroll precise=\(event.hasPreciseScrollingDeltas) phase=\(event.phase.rawValue) momentum=\(event.momentumPhase.rawValue) deltaX=\(String(format: "%.2f", deltaX)) deltaY=\(String(format: "%.2f", deltaY)) rowDelta=0 colDelta=0 totalMs=\(totalMs())"
            )
            return
        }
        controller.scroll(byRows: rowDelta, cols: effectiveColDelta)
        scrollPerfLog(
            "surface.scroll precise=\(event.hasPreciseScrollingDeltas) phase=\(event.phase.rawValue) momentum=\(event.momentumPhase.rawValue) deltaX=\(String(format: "%.2f", deltaX)) deltaY=\(String(format: "%.2f", deltaY)) rowDelta=\(rowDelta) colDelta=\(effectiveColDelta) totalMs=\(totalMs())"
        )
    }

    override func mouseDragged(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        refreshCursorBlinkState()
        if let drag = splitDrag, let controller, let scene = controller.scene {
            let coords = clampedCellCoordinates(for: point, scene: scene)
            controller.resizeSplit(drag.splitID, x: coords.x, y: coords.y)
            return
        }
        if let drag = scrollbarDrag {
            updateScrollPosition(for: drag.paneID, pointerY: point.y, thumbOffsetY: drag.thumbOffsetY)
            return
        }
        if let drag = bufferSelectionDrag,
           let hit = bufferTextHit(at: point, preferredPaneID: drag.paneID, clampToTextRect: true) {
            selectionDebugLog(
                "surface.mouseDragged point=\(pointText(point)) drag=\(describe(drag)) logical=(\(hit.logicalCol),\(hit.logicalRow))"
            )
            controller?.dragBufferSelection(
                paneID: drag.paneID,
                dragOriginCol: drag.originLogicalCol,
                dragOriginRow: drag.originLogicalRow,
                logicalCol: hit.logicalCol,
                logicalRow: hit.logicalRow,
                modifiers: drag.modifiers,
                clickCount: drag.clickCount
            )
            return
        }
        super.mouseDragged(with: event)
    }

    override func mouseUp(with event: NSEvent) {
        splitDrag = nil
        scrollbarDrag = nil
        bufferSelectionDrag = nil
        refreshCursorBlinkState(reset: true)
        super.mouseUp(with: event)
    }

    override func keyDown(with event: NSEvent) {
        guard let controller else {
            super.keyDown(with: event)
            return
        }
        guard controller.shouldHandleEditorKeyboardInput(from: self) else {
            return
        }

        cursorBlinkController.reset()
        if controller.currentMode == .insert {
            if event.modifierFlags.intersection([.control, .option]).isEmpty == false,
               let keyEvent = translateRawEvent(event) {
                controller.handleKey(keyEvent)
                return
            }
            if let special = translateSpecialEvent(event) {
                if special.kind == THE_EDITOR_KEY_ESCAPE.rawValue {
                    cancelMarkedTextComposition()
                }
                controller.handleKey(special)
                return
            }
            interpretKeyEvents([event])
            return
        }

        guard let keyEvent = translateRawEvent(event) else {
            super.keyDown(with: event)
            return
        }
        controller.handleKey(keyEvent)
    }

    private func translateSpecialEvent(_ event: NSEvent) -> the_editor_key_event_t? {
        switch Int(event.keyCode) {
        case 53, 36, 76, 51, 48, 117, 114, 115, 119, 116, 121, 123, 124, 125, 126, 122, 120, 99, 118, 96, 97, 98, 100, 101, 109, 103, 111:
            return translateRawEvent(event)
        default:
            return nil
        }
    }

    private func translateRawEvent(_ event: NSEvent) -> the_editor_key_event_t? {
        var keyEvent = the_editor_key_event_t(kind: THE_EDITOR_KEY_OTHER.rawValue, codepoint: 0, modifiers: modifierBits(for: event, includeShift: true))
        switch Int(event.keyCode) {
        case 53: keyEvent.kind = THE_EDITOR_KEY_ESCAPE.rawValue
        case 36: keyEvent.kind = THE_EDITOR_KEY_ENTER.rawValue
        case 76: keyEvent.kind = THE_EDITOR_KEY_NUMPAD_ENTER.rawValue
        case 51: keyEvent.kind = THE_EDITOR_KEY_BACKSPACE.rawValue
        case 48: keyEvent.kind = THE_EDITOR_KEY_TAB.rawValue
        case 117: keyEvent.kind = THE_EDITOR_KEY_DELETE.rawValue
        case 114: keyEvent.kind = THE_EDITOR_KEY_INSERT.rawValue
        case 115: keyEvent.kind = THE_EDITOR_KEY_HOME.rawValue
        case 119: keyEvent.kind = THE_EDITOR_KEY_END.rawValue
        case 116: keyEvent.kind = THE_EDITOR_KEY_PAGE_UP.rawValue
        case 121: keyEvent.kind = THE_EDITOR_KEY_PAGE_DOWN.rawValue
        case 123: keyEvent.kind = THE_EDITOR_KEY_LEFT.rawValue
        case 124: keyEvent.kind = THE_EDITOR_KEY_RIGHT.rawValue
        case 125: keyEvent.kind = THE_EDITOR_KEY_DOWN.rawValue
        case 126: keyEvent.kind = THE_EDITOR_KEY_UP.rawValue
        case 122: keyEvent.kind = THE_EDITOR_KEY_F1.rawValue
        case 120: keyEvent.kind = THE_EDITOR_KEY_F2.rawValue
        case 99: keyEvent.kind = THE_EDITOR_KEY_F3.rawValue
        case 118: keyEvent.kind = THE_EDITOR_KEY_F4.rawValue
        case 96: keyEvent.kind = THE_EDITOR_KEY_F5.rawValue
        case 97: keyEvent.kind = THE_EDITOR_KEY_F6.rawValue
        case 98: keyEvent.kind = THE_EDITOR_KEY_F7.rawValue
        case 100: keyEvent.kind = THE_EDITOR_KEY_F8.rawValue
        case 101: keyEvent.kind = THE_EDITOR_KEY_F9.rawValue
        case 109: keyEvent.kind = THE_EDITOR_KEY_F10.rawValue
        case 103: keyEvent.kind = THE_EDITOR_KEY_F11.rawValue
        case 111: keyEvent.kind = THE_EDITOR_KEY_F12.rawValue
        default:
            let scalarSource: String?
            if event.modifierFlags.intersection([.control, .option]).isEmpty {
                scalarSource = event.characters
            } else {
                scalarSource = event.charactersIgnoringModifiers
            }
            guard let scalar = scalarSource?.unicodeScalars.first else { return nil }
            keyEvent.kind = THE_EDITOR_KEY_CHAR.rawValue
            keyEvent.codepoint = scalar.value
            keyEvent.modifiers = modifierBits(for: event, includeShift: false)
        }
        return keyEvent
    }

    private func modifierBits(for event: NSEvent, includeShift: Bool) -> UInt8 {
        var bits: UInt8 = 0
        if event.modifierFlags.contains(.control) {
            bits |= UInt8(THE_EDITOR_MODIFIER_CTRL)
        }
        if event.modifierFlags.contains(.option) {
            bits |= UInt8(THE_EDITOR_MODIFIER_ALT)
        }
        if includeShift && event.modifierFlags.contains(.shift) {
            bits |= UInt8(THE_EDITOR_MODIFIER_SHIFT)
        }
        return bits
    }

    private func separatorHitRect(_ separator: EditorSnapshotSeparator, cellSize: CGSize) -> CGRect {
        let hitThickness: CGFloat = 8
        switch separator.axis {
        case .vertical:
            return CGRect(
                x: CGFloat(separator.line) * cellSize.width - hitThickness / 2,
                y: CGFloat(separator.spanStart) * cellSize.height,
                width: hitThickness,
                height: CGFloat(max(separator.spanEnd - separator.spanStart, 1)) * cellSize.height
            )
        case .horizontal:
            return CGRect(
                x: CGFloat(separator.spanStart) * cellSize.width,
                y: CGFloat(separator.line) * cellSize.height - hitThickness / 2,
                width: CGFloat(max(separator.spanEnd - separator.spanStart, 1)) * cellSize.width,
                height: hitThickness
            )
        }
    }

    private func pane(at point: CGPoint) -> EditorSnapshotPane? {
        guard let scene = controller?.scene else { return nil }
        return scene.panes.first(where: { scene.paneRect(for: $0).contains(point) })
    }

    private func scrollbarGeometry(
        for pane: EditorSnapshotPane,
        cellSize: CGSize
    ) -> (pane: EditorSnapshotPane, trackRect: CGRect, thumbRect: CGRect, maxScrollRow: Int)? {
        guard let scene = controller?.scene, pane.kind == .editorBuffer else { return nil }
        let contentRect = scene.paneContentRect(for: pane)
        let visibleRows = max(scene.paneVisibleRowCapacity(for: pane), 1)
        let docLines = max(pane.documentLineCount, 1)
        let maxScrollRow = max(docLines - 1, 0)
        let totalRowsForThumb = max(pane.documentLineCount, visibleRows)
        guard maxScrollRow > 0, contentRect.height > 0 else { return nil }

        let trackWidth = min(max(floor(cellSize.width * 0.55), 6), 8)
        let inset = max(2, floor(cellSize.width * 0.18))
        let trackRect = CGRect(
            x: contentRect.maxX - inset - trackWidth,
            y: contentRect.minY + inset,
            width: trackWidth,
            height: max(contentRect.height - inset * 2, trackWidth)
        )
        let thumbHeight = max(trackWidth * 2, floor(trackRect.height * (CGFloat(visibleRows) / CGFloat(totalRowsForThumb))))
        let travel = max(trackRect.height - thumbHeight, 0)
        let progress = CGFloat(min(max(pane.scrollRow, 0), maxScrollRow)) / CGFloat(maxScrollRow)
        let thumbRect = CGRect(
            x: trackRect.minX,
            y: trackRect.minY + progress * travel,
            width: trackRect.width,
            height: thumbHeight
        )
        return (pane, trackRect, thumbRect, maxScrollRow)
    }

    private func scrollbarGeometry(at point: CGPoint) -> (pane: EditorSnapshotPane, trackRect: CGRect, thumbRect: CGRect, maxScrollRow: Int)? {
        guard let scene = controller?.scene else { return nil }
        let metrics = scene.info.surfaceMetrics.cellSizePoints
        return scene.panes.compactMap { scrollbarGeometry(for: $0, cellSize: metrics) }.first(where: {
            $0.thumbRect.insetBy(dx: -3, dy: 0).contains(point) || $0.trackRect.contains(point)
        })
    }

    private func separator(at point: CGPoint) -> EditorSnapshotSeparator? {
        guard let scene = controller?.scene else { return nil }
        let metrics = scene.info.surfaceMetrics.cellSizePoints
        return scene.separators.first(where: { separatorHitRect($0, cellSize: metrics).contains(point) })
    }

    private func activatePaneIfNeeded(at point: CGPoint) {
        guard let controller, let pane = pane(at: point), !pane.isActive else { return }
        controller.setActivePane(pane.paneID)
    }

    private func bufferTextHit(
        at point: CGPoint,
        preferredPaneID: UInt? = nil,
        clampToTextRect: Bool = false
    ) -> (paneID: UInt, logicalCol: Int, logicalRow: Int)? {
        guard let scene = controller?.scene else {
            return nil
        }
        let selectedPane: EditorSnapshotPane?
        if let preferredPaneID {
            selectedPane = scene.panes.first(where: { $0.paneID == preferredPaneID && $0.kind == .editorBuffer })
        } else {
            selectedPane = pane(at: point)
        }
        guard let pane = selectedPane, pane.kind == .editorBuffer else {
            return nil
        }
        let metrics = scene.info.surfaceMetrics.cellSizePoints
        let contentRect = scene.paneContentRect(for: pane)
        let gutterWidth = CGFloat(pane.contentOffsetX) * metrics.width
        let textRect = CGRect(
            x: contentRect.minX + gutterWidth,
            y: contentRect.minY,
            width: max(contentRect.width - gutterWidth, 0),
            height: contentRect.height
        )
        guard metrics.width > 0, metrics.height > 0, textRect.width > 0, textRect.height > 0 else {
            return nil
        }
        let samplePoint: CGPoint
        if clampToTextRect {
            let maxX = textRect.maxX - 0.001
            let maxY = textRect.maxY - 0.001
            samplePoint = CGPoint(
                x: min(max(point.x, textRect.minX), maxX),
                y: min(max(point.y, textRect.minY), maxY)
            )
        } else {
            guard textRect.contains(point) else {
                return nil
            }
            samplePoint = point
        }
        let logicalCol = max(Int(floor((samplePoint.x - textRect.minX) / metrics.width)), 0)
        let logicalRow = max(Int(floor((samplePoint.y - contentRect.minY) / metrics.height)), 0)
        return (pane.paneID, logicalCol, logicalRow)
    }

    private func pointerModifiers(from flags: NSEvent.ModifierFlags) -> UInt8 {
        var modifiers: UInt8 = 0
        if flags.contains(.control) {
            modifiers |= 0b0000_0001
        }
        if flags.contains(.option) {
            modifiers |= 0b0000_0010
        }
        if flags.contains(.shift) {
            modifiers |= 0b0000_0100
        }
        return modifiers
    }

    private func updateScrollPosition(for paneID: UInt, pointerY: CGFloat, thumbOffsetY: CGFloat) {
        guard let controller, let scene = controller.scene else { return }
        let metrics = scene.info.surfaceMetrics.cellSizePoints
        guard let geometry = scene.panes.compactMap({ scrollbarGeometry(for: $0, cellSize: metrics) }).first(where: { $0.pane.paneID == paneID }) else {
            return
        }
        if !geometry.pane.isActive {
            controller.setActivePane(paneID)
        }
        let availableTravel = max(geometry.trackRect.height - geometry.thumbRect.height, 0)
        guard availableTravel > 0 else {
            controller.setScrollRow(0)
            return
        }
        let thumbMinY = geometry.trackRect.minY
        let thumbMaxY = geometry.trackRect.maxY - geometry.thumbRect.height
        let thumbY = min(max(pointerY - thumbOffsetY, thumbMinY), thumbMaxY)
        let progress = (thumbY - thumbMinY) / availableTravel
        let row = Int((progress * CGFloat(geometry.maxScrollRow)).rounded())
        controller.setScrollRow(row)
    }

    private func clampedCellCoordinates(for point: CGPoint, scene: EditorRenderScene) -> (x: Int, y: Int) {
        let metrics = scene.info.surfaceMetrics.cellSizePoints
        let cellWidth = max(metrics.width, 1)
        let cellHeight = max(metrics.height, 1)
        let maxX = max(scene.info.viewportWidth - 1, 0)
        let maxY = max(scene.info.viewportHeight - 1, 0)
        let x = min(max(Int(floor(point.x / cellWidth)), 0), maxX)
        let y = min(max(Int(floor(point.y / cellHeight)), 0), maxY)
        return (x, y)
    }

    // MARK: NSTextInputClient

    func hasMarkedText() -> Bool {
        markedText.length > 0
    }

    func markedRange() -> NSRange {
        guard markedText.length > 0 else { return NSRange(location: NSNotFound, length: 0) }
        return NSRange(location: 0, length: markedText.length)
    }

    func selectedRange() -> NSRange {
        controller?.primarySelectionUTF16Range() ?? NSRange(location: 0, length: 0)
    }

    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        switch string {
        case let value as NSAttributedString:
            markedText = NSMutableAttributedString(attributedString: value)
        case let value as String:
            markedText = NSMutableAttributedString(string: value)
        default:
            markedText = NSMutableAttributedString()
        }
        controller?.updateMarkedText(markedText.string)
    }

    func unmarkText() {
        if markedText.length > 0 {
            markedText.mutableString.setString("")
            controller?.clearMarkedText()
        }
    }

    private func cancelMarkedTextComposition() {
        inputContext?.discardMarkedText()
        unmarkText()
    }

    func validAttributesForMarkedText() -> [NSAttributedString.Key] {
        []
    }

    func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? {
        let text = controller?.primarySelectionText() ?? ""
        actualRange?.pointee = NSRange(location: 0, length: text.utf16.count)
        return NSAttributedString(string: text)
    }

    func characterIndex(for point: NSPoint) -> Int {
        selectedRange().location
    }

    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
        actualRange?.pointee = range
        guard let scene = controller?.scene, let cursor = scene.primaryCursor else {
            return convert(NSRect(x: 0, y: 0, width: 0, height: cellSize.height), to: nil)
        }
        let metrics = scene.info.surfaceMetrics
        let cellSize = metrics.cellSizePoints
        let origin = scene.displayOrigin(col: cursor.col, row: cursor.row)
        let local = NSRect(
            x: origin.x,
            y: origin.y,
            width: cellSize.width,
            height: cellSize.height
        )
        let windowRect = convert(local, to: nil)
        return window?.convertToScreen(windowRect) ?? windowRect
    }

    func insertText(_ string: Any, replacementRange: NSRange) {
        guard controller?.shouldHandleEditorKeyboardInput(from: self) == true else { return }
        let text: String
        switch string {
        case let value as NSAttributedString:
            text = value.string
        case let value as String:
            text = value
        default:
            return
        }
        unmarkText()
        cursorBlinkController.reset()
        controller?.insertText(text)
    }

    override func doCommand(by selector: Selector) {
        guard let controller else { return }
        guard controller.shouldHandleEditorKeyboardInput(from: self) else { return }
        let event: the_editor_key_event_t?
        switch selector {
        case #selector(moveLeft(_:)):
            event = the_editor_key_event_t(kind: THE_EDITOR_KEY_LEFT.rawValue, codepoint: 0, modifiers: 0)
        case #selector(moveRight(_:)):
            event = the_editor_key_event_t(kind: THE_EDITOR_KEY_RIGHT.rawValue, codepoint: 0, modifiers: 0)
        case #selector(moveUp(_:)):
            event = the_editor_key_event_t(kind: THE_EDITOR_KEY_UP.rawValue, codepoint: 0, modifiers: 0)
        case #selector(moveDown(_:)):
            event = the_editor_key_event_t(kind: THE_EDITOR_KEY_DOWN.rawValue, codepoint: 0, modifiers: 0)
        case #selector(deleteBackward(_:)):
            event = the_editor_key_event_t(kind: THE_EDITOR_KEY_BACKSPACE.rawValue, codepoint: 0, modifiers: 0)
        case #selector(insertNewline(_:)):
            event = the_editor_key_event_t(kind: THE_EDITOR_KEY_ENTER.rawValue, codepoint: 0, modifiers: 0)
        case #selector(insertTab(_:)):
            event = the_editor_key_event_t(kind: THE_EDITOR_KEY_TAB.rawValue, codepoint: 0, modifiers: 0)
        case #selector(cancelOperation(_:)):
            cancelMarkedTextComposition()
            event = the_editor_key_event_t(kind: THE_EDITOR_KEY_ESCAPE.rawValue, codepoint: 0, modifiers: 0)
        default:
            event = nil
        }

        if let event {
            cursorBlinkController.reset()
            controller.handleKey(event)
        }
    }
}
