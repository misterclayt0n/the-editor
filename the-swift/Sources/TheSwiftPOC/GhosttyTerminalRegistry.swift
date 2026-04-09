import AppKit
import Foundation
import QuartzCore

#if canImport(GhosttyKit)
import GhosttyKit
#endif

@MainActor
final class GhosttyTerminalRegistry {
    #if canImport(GhosttyKit)
    static let isAvailable = true
    #else
    static let isAvailable = false
    #endif

    #if canImport(GhosttyKit)
    private weak var controller: EditorSurfaceController?
    private let runtime = GhosttyEmbeddedRuntime.shared
    private var viewsBySurfaceID: [UInt: GhosttyTerminalSurfaceView] = [:]

    init(controller: EditorSurfaceController) {
        self.controller = controller
    }

    func reconcile(
        scene: EditorRenderScene?,
        openItems: EditorPaneOpenItemsState,
        in containerView: NSView,
        editorSurfaceView: NSView
    ) {
        let resolvedBackgroundColor = scene?.backgroundColor ?? controller?.chrome.backgroundColor ?? .windowBackgroundColor
        let colorScheme = ghosttyColorScheme(for: resolvedBackgroundColor)
        runtime.setColorScheme(colorScheme)

        let visibleTerminalPanes = scene?.panes.filter { $0.kind == .clientSurface && $0.clientSurfaceID != nil } ?? []
        let allSurfaceIDs = Set(openItems.groups.flatMap { group in
            group.items.compactMap { item -> UInt? in
                guard item.kind == .terminal else { return nil }
                return item.clientSurfaceID
            }
        })
        .union(visibleTerminalPanes.compactMap(\.clientSurfaceID))

        for surfaceID in allSurfaceIDs where viewsBySurfaceID[surfaceID] == nil {
            let view = GhosttyTerminalSurfaceView(
                clientSurfaceID: surfaceID,
                runtime: runtime,
                workingDirectory: preferredWorkingDirectory(),
                onCloseRequested: { [weak controller] closingSurfaceID in
                    controller?.closeTerminalSurface(closingSurfaceID)
                }
            )
            view.paneIDOwner = controller
            viewsBySurfaceID[surfaceID] = view
        }

        let staleSurfaceIDs = Set(viewsBySurfaceID.keys).subtracting(allSurfaceIDs)
        for surfaceID in staleSurfaceIDs {
            viewsBySurfaceID[surfaceID]?.removeFromSuperview()
            viewsBySurfaceID.removeValue(forKey: surfaceID)
        }

        let visibleSurfaceIDs = Set(visibleTerminalPanes.compactMap(\.clientSurfaceID))

        for (surfaceID, view) in viewsBySurfaceID where !visibleSurfaceIDs.contains(surfaceID) {
            view.updateVisibility(false)
            view.removeFromSuperview()
        }

        if let scene {
            for pane in visibleTerminalPanes {
                guard let surfaceID = pane.clientSurfaceID,
                      let view = viewsBySurfaceID[surfaceID]
                else {
                    continue
                }
                let frame = scene.paneContentRect(for: pane)
                view.paneID = pane.paneID
                view.frame = frame.integral
                view.autoresizingMask = []
                if view.superview !== containerView {
                    containerView.addSubview(view)
                }
                view.updateColorScheme(colorScheme)
                view.updateVisibility(true)
                view.updateFocus(pane.isActive && containerView.window?.firstResponder === view)
            }

            let currentFirstResponder = containerView.window?.firstResponder
            if let activeTerminalPane = visibleTerminalPanes.first(where: { $0.isActive }),
               let surfaceID = activeTerminalPane.clientSurfaceID,
               let activeView = viewsBySurfaceID[surfaceID],
               currentFirstResponder !== activeView,
               currentFirstResponder == nil || currentFirstResponder === editorSurfaceView || currentFirstResponder is GhosttyTerminalSurfaceView {
                containerView.window?.makeFirstResponder(activeView)
            } else if currentFirstResponder is GhosttyTerminalSurfaceView {
                containerView.window?.makeFirstResponder(editorSurfaceView)
            }
        }
    }

    func tearDown() {
        for view in viewsBySurfaceID.values {
            view.removeFromSuperview()
        }
        viewsBySurfaceID.removeAll()
    }

    private func preferredWorkingDirectory() -> String? {
        guard let controller else {
            return FileManager.default.homeDirectoryForCurrentUser.path
        }
        if let absolutePath = controller.chrome.document.absolutePath {
            let url = URL(fileURLWithPath: absolutePath)
            let path = url.hasDirectoryPath ? url.path : url.deletingLastPathComponent().path
            if !path.isEmpty {
                return path
            }
        }
        return FileManager.default.homeDirectoryForCurrentUser.path
    }

    private func ghosttyColorScheme(for color: NSColor) -> ghostty_color_scheme_e {
        let resolved = color.usingColorSpace(.sRGB) ?? color
        let luminance = (0.299 * resolved.redComponent) + (0.587 * resolved.greenComponent) + (0.114 * resolved.blueComponent)
        return luminance > 0.7 ? GHOSTTY_COLOR_SCHEME_LIGHT : GHOSTTY_COLOR_SCHEME_DARK
    }
    #else
    init(controller: EditorSurfaceController) {}
    func reconcile(scene: EditorRenderScene?, openItems: EditorPaneOpenItemsState, in containerView: NSView, editorSurfaceView: NSView) {}
    func tearDown() {}
    #endif
}

@MainActor
final class GhosttyTerminalOverlayContainerView: NSView {
    override var isFlipped: Bool { true }
}

#if canImport(GhosttyKit)

@MainActor
private final class GhosttyEmbeddedRuntime {
    static let shared = GhosttyEmbeddedRuntime()

    private var app: ghostty_app_t?
    private var config: ghostty_config_t?
    private var tickScheduled = false
    private var didInitializeLibrary = false
    private var appObservers: [NSObjectProtocol] = []
    private var colorScheme: ghostty_color_scheme_e = GHOSTTY_COLOR_SCHEME_DARK

    private init() {
        initializeIfNeeded()
    }

    deinit {
        MainActor.assumeIsolated {
            let center = NotificationCenter.default
            for observer in appObservers {
                center.removeObserver(observer)
            }
            if let app {
                ghostty_app_free(app)
            }
            if let config {
                ghostty_config_free(config)
            }
        }
    }

    var isReady: Bool {
        app != nil
    }

    func appHandle() -> ghostty_app_t? {
        initializeIfNeeded()
        return app
    }

    var currentColorScheme: ghostty_color_scheme_e {
        colorScheme
    }

    func setColorScheme(_ colorScheme: ghostty_color_scheme_e) {
        self.colorScheme = colorScheme
        guard let app else { return }
        ghostty_app_set_color_scheme(app, colorScheme)
    }

    func scheduleTick() {
        guard !tickScheduled else { return }
        tickScheduled = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.tickScheduled = false
            guard let app = self.app else { return }
            ghostty_app_tick(app)
        }
    }

    private func initializeIfNeeded() {
        guard app == nil else { return }
        if !didInitializeLibrary {
            didInitializeLibrary = true
            let result = ghostty_init(UInt(CommandLine.argc), CommandLine.unsafeArgv)
            if result != GHOSTTY_SUCCESS {
                ghosttyLog("ghostty_init failed result=\(result)")
                return
            }
        }

        guard let primaryConfig = ghostty_config_new() else {
            ghosttyLog("ghostty_config_new failed for primary config")
            return
        }
        ghostty_config_load_default_files(primaryConfig)
        ghostty_config_load_recursive_files(primaryConfig)
        ghostty_config_finalize(primaryConfig)

        var runtimeConfig = ghostty_runtime_config_s()
        runtimeConfig.userdata = Unmanaged.passUnretained(self).toOpaque()
        runtimeConfig.supports_selection_clipboard = true
        runtimeConfig.wakeup_cb = { userdata in
            guard let userdata else { return }
            let runtime = Unmanaged<GhosttyEmbeddedRuntime>.fromOpaque(userdata).takeUnretainedValue()
            runtime.scheduleTick()
        }
        runtimeConfig.action_cb = { _, _, _ in false }
        runtimeConfig.read_clipboard_cb = { userdata, location, state in
            guard let userdata else { return }
            let context = Unmanaged<GhosttySurfaceCallbackContext>.fromOpaque(userdata).takeUnretainedValue()
            DispatchQueue.main.async {
                let text = GhosttyPasteboardBridge.readString(from: location)
                context.view?.completeClipboardRequest(text: text, state: state, confirmed: false)
            }
        }
        runtimeConfig.confirm_read_clipboard_cb = { userdata, content, state, _ in
            guard let userdata else { return }
            let context = Unmanaged<GhosttySurfaceCallbackContext>.fromOpaque(userdata).takeUnretainedValue()
            let text = content.map { String(cString: $0) } ?? ""
            DispatchQueue.main.async {
                context.view?.completeClipboardRequest(text: text, state: state, confirmed: true)
            }
        }
        runtimeConfig.write_clipboard_cb = { _, location, content, len, _ in
            GhosttyPasteboardBridge.write(contents: content, count: Int(len), to: location)
        }
        runtimeConfig.close_surface_cb = { userdata, _ in
            guard let userdata else { return }
            let context = Unmanaged<GhosttySurfaceCallbackContext>.fromOpaque(userdata).takeUnretainedValue()
            DispatchQueue.main.async {
                context.onCloseRequested(context.clientSurfaceID)
            }
        }

        if let createdApp = ghostty_app_new(&runtimeConfig, primaryConfig) {
            app = createdApp
            config = primaryConfig
        } else {
            ghosttyLog("ghostty_app_new failed for primary config")
            logConfigDiagnostics(primaryConfig, label: "primary")
            ghostty_config_free(primaryConfig)
            guard let fallbackConfig = ghostty_config_new() else {
                ghosttyLog("ghostty_config_new failed for fallback config")
                return
            }
            ghostty_config_finalize(fallbackConfig)
            guard let createdApp = ghostty_app_new(&runtimeConfig, fallbackConfig) else {
                ghosttyLog("ghostty_app_new failed for fallback config")
                logConfigDiagnostics(fallbackConfig, label: "fallback")
                ghostty_config_free(fallbackConfig)
                return
            }
            app = createdApp
            config = fallbackConfig
        }

        guard let app else {
            ghosttyLog("ghostty runtime failed to initialize app")
            return
        }
        ghostty_app_set_focus(app, NSApp.isActive)
        ghostty_app_set_color_scheme(app, colorScheme)
        let center = NotificationCenter.default
        appObservers.append(center.addObserver(
            forName: NSApplication.didBecomeActiveNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let app = self?.app else { return }
                ghostty_app_set_focus(app, true)
            }
        })
        appObservers.append(center.addObserver(
            forName: NSApplication.didResignActiveNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let app = self?.app else { return }
                ghostty_app_set_focus(app, false)
            }
        })
    }

    private func logConfigDiagnostics(_ config: ghostty_config_t, label: String) {
        let count = Int(ghostty_config_diagnostics_count(config))
        guard count > 0 else { return }
        for index in 0..<count {
            let diagnostic = ghostty_config_get_diagnostic(config, UInt32(index))
            let message = diagnostic.message.map { String(cString: $0) } ?? "(null)"
            ghosttyLog("config[\(label)] diagnostic[\(index)]=\(message)")
        }
    }
}

private let ghosttyLoggingEnabled = ProcessInfo.processInfo.environment["THE_EDITOR_GHOSTTY_LOG"] == "1"
private let ghosttySelectionLoggingEnabled = ProcessInfo.processInfo.environment["THE_EDITOR_GHOSTTY_SELECTION_LOG"] == "1"

func ghosttyLog(_ message: String) {
    guard ghosttyLoggingEnabled else { return }
    fputs("[the-swift:ghostty] \(message)\n", stderr)
}

func ghosttySelectionLog(_ message: String) {
    guard ghosttySelectionLoggingEnabled else { return }
    fputs("[the-swift:ghostty:selection] \(message)\n", stderr)
}

private enum GhosttyPasteboardBridge {
    static func pasteboard(for location: ghostty_clipboard_e) -> NSPasteboard? {
        switch location {
        case GHOSTTY_CLIPBOARD_STANDARD:
            return .general
        case GHOSTTY_CLIPBOARD_SELECTION:
            return NSPasteboard(name: NSPasteboard.Name("com.mitchellh.ghostty.selection"))
        default:
            return nil
        }
    }

    static func readString(from location: ghostty_clipboard_e) -> String {
        guard let pasteboard = pasteboard(for: location) else { return "" }
        return pasteboard.string(forType: .string) ?? ""
    }

    static func write(
        contents: UnsafePointer<ghostty_clipboard_content_s>?,
        count: Int,
        to location: ghostty_clipboard_e
    ) {
        let values: [(mime: String?, value: String)] = {
            guard let contents, count > 0 else { return [] }
            return (0..<count).compactMap { index in
                let item = contents[index]
                guard let dataPointer = item.data else { return nil }
                let mime = item.mime.map { String(cString: $0) }
                return (mime, String(cString: dataPointer))
            }
        }()

        DispatchQueue.main.async {
            guard let pasteboard = pasteboard(for: location) else { return }
            pasteboard.clearContents()
            if let plainText = values.first(where: { ($0.mime ?? "").hasPrefix("text/plain") })?.value {
                pasteboard.setString(plainText, forType: .string)
            } else if let fallback = values.first?.value {
                pasteboard.setString(fallback, forType: .string)
            }
        }
    }
}

private final class GhosttySurfaceCallbackContext {
    let clientSurfaceID: UInt
    let onCloseRequested: (UInt) -> Void
    weak var view: GhosttyTerminalSurfaceView?

    init(clientSurfaceID: UInt, onCloseRequested: @escaping (UInt) -> Void) {
        self.clientSurfaceID = clientSurfaceID
        self.onCloseRequested = onCloseRequested
    }
}

@MainActor
private final class GhosttyTerminalSurfaceView: NSView {
    let clientSurfaceID: UInt
    weak var paneIDOwner: EditorSurfaceController?
    var paneID: UInt = 0

    private let runtime: GhosttyEmbeddedRuntime
    private var surface: ghostty_surface_t?
    private var callbackContext: Unmanaged<GhosttySurfaceCallbackContext>?
    private var isSurfaceVisible = false
    private var pendingWorkingDirectory: String?
    private var lastKnownMousePointInView: NSPoint?

    override var acceptsFirstResponder: Bool { true }

    override func makeBackingLayer() -> CALayer {
        let metalLayer = CAMetalLayer()
        metalLayer.pixelFormat = .bgra8Unorm
        metalLayer.isOpaque = false
        metalLayer.framebufferOnly = false
        return metalLayer
    }

    init(
        clientSurfaceID: UInt,
        runtime: GhosttyEmbeddedRuntime,
        workingDirectory: String?,
        onCloseRequested: @escaping (UInt) -> Void
    ) {
        self.clientSurfaceID = clientSurfaceID
        self.runtime = runtime
        self.pendingWorkingDirectory = workingDirectory
        super.init(frame: NSRect(x: 0, y: 0, width: 800, height: 600))
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
        layer?.isOpaque = false
        layer?.masksToBounds = true
        let context = GhosttySurfaceCallbackContext(
            clientSurfaceID: clientSurfaceID,
            onCloseRequested: onCloseRequested
        )
        context.view = self
        callbackContext = Unmanaged.passRetained(context)
        createSurfaceIfNeeded()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        MainActor.assumeIsolated {
            if let surface {
                ghostty_surface_free(surface)
            }
            callbackContext?.release()
        }
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        createSurfaceIfNeeded()
        synchronizeSurfaceGeometry()
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        createSurfaceIfNeeded()
        synchronizeSurfaceGeometry()
    }

    override func layout() {
        super.layout()
        createSurfaceIfNeeded()
        synchronizeSurfaceGeometry()
    }

    override func becomeFirstResponder() -> Bool {
        let accepted = super.becomeFirstResponder()
        updateFocus(true)
        return accepted
    }

    override func resignFirstResponder() -> Bool {
        let accepted = super.resignFirstResponder()
        updateFocus(false)
        return accepted
    }

    override func updateTrackingAreas() {
        trackingAreas.forEach(removeTrackingArea)
        addTrackingArea(NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .mouseMoved, .inVisibleRect, .activeAlways],
            owner: self,
            userInfo: nil
        ))
        super.updateTrackingAreas()
    }

    func updateVisibility(_ visible: Bool) {
        isHidden = !visible
        guard isSurfaceVisible != visible else { return }
        isSurfaceVisible = visible
        if let surface {
            ghostty_surface_set_occlusion(surface, visible)
            if visible {
                ghostty_surface_refresh(surface)
            }
        }
    }

    func updateColorScheme(_ colorScheme: ghostty_color_scheme_e) {
        guard let surface else { return }
        ghostty_surface_set_color_scheme(surface, colorScheme)
    }

    func updateFocus(_ focused: Bool) {
        guard let surface else { return }
        ghostty_surface_set_focus(surface, focused)
        if focused {
            synchronizeDisplayID()
        }
    }

    override func mouseDown(with event: NSEvent) {
        paneIDOwner?.setActivePane(paneID)
        window?.makeFirstResponder(self)
        guard let surface else { return }
        logSelectionState("leftDown.before", event: event, surface: surface, mousePositionSent: false, consumed: nil)
        trackMousePointIfUsable(convert(event.locationInWindow, from: nil))
        let sentMousePosition = event.clickCount == 1
        if sentMousePosition {
            sendMousePosition(event)
        }
        let consumed = ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_PRESS, GHOSTTY_MOUSE_LEFT, mods(from: event.modifierFlags))
        logSelectionState("leftDown.after", event: event, surface: surface, mousePositionSent: sentMousePosition, consumed: consumed)
    }

    override func mouseUp(with event: NSEvent) {
        guard let surface else { return }
        let consumed = ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_RELEASE, GHOSTTY_MOUSE_LEFT, mods(from: event.modifierFlags))
        logSelectionState("leftUp.after", event: event, surface: surface, mousePositionSent: false, consumed: consumed)
        guard ghosttySelectionLoggingEnabled else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self, let surface = self.surface else { return }
            self.logSelectionState("leftUp.async", event: event, surface: surface, mousePositionSent: false, consumed: consumed)
        }
    }

    override func rightMouseDown(with event: NSEvent) {
        paneIDOwner?.setActivePane(paneID)
        window?.makeFirstResponder(self)
        guard let surface else { return }
        ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_PRESS, GHOSTTY_MOUSE_RIGHT, mods(from: event.modifierFlags))
    }

    override func rightMouseUp(with event: NSEvent) {
        guard let surface else { return }
        ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_RELEASE, GHOSTTY_MOUSE_RIGHT, mods(from: event.modifierFlags))
    }

    override func mouseEntered(with event: NSEvent) {
        super.mouseEntered(with: event)
        sendMousePosition(event)
    }

    override func mouseExited(with event: NSEvent) {
        guard let surface else { return }
        ghostty_surface_mouse_pos(surface, -1, -1, mods(from: event.modifierFlags))
    }

    override func mouseDragged(with event: NSEvent) {
        sendMousePosition(event)
    }

    override func mouseMoved(with event: NSEvent) {
        sendMousePosition(event)
    }

    override func rightMouseDragged(with event: NSEvent) {
        sendMousePosition(event)
    }

    override func scrollWheel(with event: NSEvent) {
        guard let surface else { return }
        var x = event.scrollingDeltaX
        var y = event.scrollingDeltaY
        let precision = event.hasPreciseScrollingDeltas
        if precision {
            x *= 2
            y *= 2
        }
        ghostty_surface_mouse_scroll(surface, x, y, precision ? 1 : 0)
    }

    override func keyDown(with event: NSEvent) {
        guard let surface else {
            super.keyDown(with: event)
            return
        }

        let translationModifiers = translatedModifierFlags(for: event, surface: surface)
        let translationEvent: NSEvent
        if translationModifiers == event.modifierFlags {
            translationEvent = event
        } else {
            translationEvent = NSEvent.keyEvent(
                with: event.type,
                location: event.locationInWindow,
                modifierFlags: translationModifiers,
                timestamp: event.timestamp,
                windowNumber: event.windowNumber,
                context: nil,
                characters: event.characters(byApplyingModifiers: translationModifiers) ?? "",
                charactersIgnoringModifiers: event.charactersIgnoringModifiers ?? "",
                isARepeat: event.isARepeat,
                keyCode: event.keyCode
            ) ?? event
        }

        var keyEvent = ghosttyKeyEvent(
            action: event.isARepeat ? GHOSTTY_ACTION_REPEAT : GHOSTTY_ACTION_PRESS,
            for: event,
            translationModifiers: translationEvent.modifierFlags
        )
        if let text = ghosttyText(for: translationEvent),
           let firstByte = text.utf8.first,
           firstByte >= 0x20 {
            text.withCString { pointer in
                keyEvent.text = pointer
                _ = ghostty_surface_key(surface, keyEvent)
            }
            return
        }

        _ = ghostty_surface_key(surface, keyEvent)
    }

    override func keyUp(with event: NSEvent) {
        guard let surface else {
            super.keyUp(with: event)
            return
        }
        let keyEvent = ghosttyKeyEvent(action: GHOSTTY_ACTION_RELEASE, for: event)
        _ = ghostty_surface_key(surface, keyEvent)
    }

    func completeClipboardRequest(text: String, state: UnsafeMutableRawPointer?, confirmed: Bool) {
        guard let surface, let state else { return }
        text.withCString { pointer in
            ghostty_surface_complete_clipboard_request(surface, pointer, state, confirmed)
        }
    }

    private func createSurfaceIfNeeded() {
        guard surface == nil else { return }
        guard let app = runtime.appHandle() else {
            ghosttyLog("surface \(clientSurfaceID) waiting for runtime app")
            return
        }

        var surfaceConfig = ghostty_surface_config_new()
        surfaceConfig.platform_tag = GHOSTTY_PLATFORM_MACOS
        surfaceConfig.platform = ghostty_platform_u(macos: ghostty_platform_macos_s(
            nsview: Unmanaged.passUnretained(self).toOpaque()
        ))
        surfaceConfig.userdata = callbackContext?.toOpaque()
        surfaceConfig.scale_factor = Double(window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2)
        surfaceConfig.context = GHOSTTY_SURFACE_CONTEXT_SPLIT
        surfaceConfig.wait_after_command = false

        if let pendingWorkingDirectory, !pendingWorkingDirectory.isEmpty {
            pendingWorkingDirectory.withCString { cwd in
                surfaceConfig.working_directory = cwd
                surface = ghostty_surface_new(app, &surfaceConfig)
            }
        } else {
            surface = ghostty_surface_new(app, &surfaceConfig)
        }

        guard let surface else {
            ghosttyLog("ghostty_surface_new failed for clientSurfaceID=\(clientSurfaceID)")
            return
        }

        updateColorScheme(runtime.currentColorScheme)
        synchronizeDisplayID()
        synchronizeSurfaceGeometry()
        updateVisibility(isSurfaceVisible)
        ghostty_surface_refresh(surface)
    }

    private func synchronizeSurfaceGeometry() {
        guard let surface else { return }
        if let window {
            CATransaction.begin()
            CATransaction.setDisableActions(true)
            layer?.contentsScale = window.backingScaleFactor
            CATransaction.commit()
        }
        let backingBounds = convertToBacking(bounds)
        let xScale = bounds.width > 0 ? backingBounds.width / bounds.width : (window?.backingScaleFactor ?? 1)
        let yScale = bounds.height > 0 ? backingBounds.height / bounds.height : (window?.backingScaleFactor ?? 1)
        ghostty_surface_set_content_scale(surface, xScale, yScale)
        let widthPx = UInt32(max(Int(backingBounds.width.rounded()), 1))
        let heightPx = UInt32(max(Int(backingBounds.height.rounded()), 1))
        ghostty_surface_set_size(surface, widthPx, heightPx)
        synchronizeDisplayID()
        ghostty_surface_refresh(surface)
    }

    private func synchronizeDisplayID() {
        guard let surface,
              let screenNumber = (window?.screen ?? NSScreen.main)?.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber
        else {
            return
        }
        ghostty_surface_set_display_id(surface, screenNumber.uint32Value)
    }

    private func sendMousePosition(_ event: NSEvent) {
        guard let surface else { return }
        let eventPoint = convert(event.locationInWindow, from: nil)
        trackMousePointIfUsable(eventPoint)
        let point = preferredPointerPoint(from: eventPoint) ?? eventPoint
        ghostty_surface_mouse_pos(surface, point.x, bounds.height - point.y, mods(from: event.modifierFlags))
    }

    private func logSelectionState(
        _ phase: String,
        event: NSEvent,
        surface: ghostty_surface_t,
        mousePositionSent: Bool,
        consumed: Bool?
    ) {
        guard ghosttySelectionLoggingEnabled, event.clickCount >= 2 else { return }
        let rawPoint = convert(event.locationInWindow, from: nil)
        let currentPoint = currentMousePointInView()
        let cachedPoint = lastKnownMousePointInView
        let preferredPoint = resolvedPointerPointForLogging(from: rawPoint, currentPoint: currentPoint, cachedPoint: cachedPoint)
        let selection = selectionDebugText(surface)
        let quicklook = quicklookDebugText(surface)
        let consumedText = consumed.map { $0 ? "1" : "0" } ?? "-"
        ghosttySelectionLog(
            "surface=\(clientSurfaceID) pane=\(paneID) phase=\(phase) clickCount=\(event.clickCount) sentPos=\(mousePositionSent ? 1 : 0) consumed=\(consumedText) raw=\(debugPoint(rawPoint)) current=\(debugPoint(currentPoint)) cached=\(debugPoint(cachedPoint)) preferred=\(debugPoint(preferredPoint)) hasSelection=\(ghostty_surface_has_selection(surface) ? 1 : 0) selection=\(selection) quicklook=\(quicklook)"
        )
    }

    private func resolvedPointerPointForLogging(
        from eventPoint: NSPoint,
        currentPoint: NSPoint?,
        cachedPoint: NSPoint?
    ) -> NSPoint? {
        if pointIsUsableForPointer(eventPoint) {
            return eventPoint
        }
        if let currentPoint, pointIsUsableForPointer(currentPoint) {
            return currentPoint
        }
        return cachedPoint
    }

    private func selectionDebugText(_ surface: ghostty_surface_t) -> String {
        guard ghostty_surface_has_selection(surface) else { return "-" }
        var text = ghostty_text_s()
        guard ghostty_surface_read_selection(surface, &text), let pointer = text.text else {
            return "<unreadable>"
        }
        defer { ghostty_surface_free_text(surface, &text) }
        return debugText(String(cString: pointer))
    }

    private func quicklookDebugText(_ surface: ghostty_surface_t) -> String {
        var text = ghostty_text_s()
        guard ghostty_surface_quicklook_word(surface, &text), let pointer = text.text else {
            return "-"
        }
        defer { ghostty_surface_free_text(surface, &text) }
        return debugText(String(cString: pointer))
    }

    private func debugPoint(_ point: NSPoint?) -> String {
        guard let point else { return "-" }
        return String(format: "(%.1f,%.1f)", point.x, point.y)
    }

    private func debugText(_ text: String, limit: Int = 120) -> String {
        let compact = text
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\n", with: "\\n")
            .replacingOccurrences(of: "\r", with: "\\r")
        if compact.count <= limit {
            return "\"\(compact)\""
        }
        let endIndex = compact.index(compact.startIndex, offsetBy: limit)
        return "\"\(compact[..<endIndex])…\""
    }

    private func pointIsUsableForPointer(_ point: NSPoint) -> Bool {
        point.x >= 0 && point.y >= 0 && point.x <= bounds.width && point.y <= bounds.height
    }

    private func trackMousePointIfUsable(_ point: NSPoint) {
        guard pointIsUsableForPointer(point) else { return }
        lastKnownMousePointInView = point
    }

    private func preferredPointerPoint(from eventPoint: NSPoint? = nil) -> NSPoint? {
        if let eventPoint, pointIsUsableForPointer(eventPoint) {
            lastKnownMousePointInView = eventPoint
            return eventPoint
        }
        if let currentPoint = currentMousePointInView(), pointIsUsableForPointer(currentPoint) {
            lastKnownMousePointInView = currentPoint
            return currentPoint
        }
        return lastKnownMousePointInView ?? eventPoint
    }

    private func currentMousePointInView() -> NSPoint? {
        guard let window else { return nil }
        return convert(window.mouseLocationOutsideOfEventStream, from: nil)
    }

    private func ghosttyKeyEvent(
        action: ghostty_input_action_e,
        for event: NSEvent,
        translationModifiers: NSEvent.ModifierFlags? = nil
    ) -> ghostty_input_key_s {
        var keyEvent = ghostty_input_key_s()
        keyEvent.action = action
        keyEvent.keycode = UInt32(event.keyCode)
        keyEvent.mods = mods(from: event.modifierFlags)
        keyEvent.consumed_mods = consumedMods(from: translationModifiers ?? event.modifierFlags)
        keyEvent.text = nil
        keyEvent.composing = false
        if let chars = event.characters(byApplyingModifiers: []),
           let codepoint = chars.unicodeScalars.first {
            keyEvent.unshifted_codepoint = codepoint.value
        } else {
            keyEvent.unshifted_codepoint = 0
        }
        return keyEvent
    }

    private func ghosttyText(for event: NSEvent) -> String? {
        guard let characters = event.characters, !characters.isEmpty else {
            return nil
        }
        if characters.count == 1,
           let scalar = characters.unicodeScalars.first {
            if scalar.value < 0x20 {
                return event.characters(byApplyingModifiers: event.modifierFlags.subtracting(.control))
            }
            if scalar.value >= 0xF700 && scalar.value <= 0xF8FF {
                return nil
            }
        }
        return characters
    }

    private func translatedModifierFlags(for event: NSEvent, surface: ghostty_surface_t) -> NSEvent.ModifierFlags {
        let translatedMods = ghostty_surface_key_translation_mods(surface, mods(from: event.modifierFlags))
        let translatedFlags = modifierFlags(from: translatedMods)
        var flags = event.modifierFlags
        for flag in [NSEvent.ModifierFlags.shift, .control, .option, .command] {
            if translatedFlags.contains(flag) {
                flags.insert(flag)
            } else {
                flags.remove(flag)
            }
        }
        return flags
    }

    private func modifierFlags(from mods: ghostty_input_mods_e) -> NSEvent.ModifierFlags {
        var flags = NSEvent.ModifierFlags()
        if mods.rawValue & GHOSTTY_MODS_SHIFT.rawValue != 0 { flags.insert(.shift) }
        if mods.rawValue & GHOSTTY_MODS_CTRL.rawValue != 0 { flags.insert(.control) }
        if mods.rawValue & GHOSTTY_MODS_ALT.rawValue != 0 { flags.insert(.option) }
        if mods.rawValue & GHOSTTY_MODS_SUPER.rawValue != 0 { flags.insert(.command) }
        if mods.rawValue & GHOSTTY_MODS_CAPS.rawValue != 0 { flags.insert(.capsLock) }
        return flags
    }

    private func mods(from flags: NSEvent.ModifierFlags) -> ghostty_input_mods_e {
        let flags = flags.intersection(.deviceIndependentFlagsMask)
        var raw = GHOSTTY_MODS_NONE.rawValue
        if flags.contains(.shift) { raw |= GHOSTTY_MODS_SHIFT.rawValue }
        if flags.contains(.control) { raw |= GHOSTTY_MODS_CTRL.rawValue }
        if flags.contains(.option) { raw |= GHOSTTY_MODS_ALT.rawValue }
        if flags.contains(.command) { raw |= GHOSTTY_MODS_SUPER.rawValue }
        if flags.contains(.capsLock) { raw |= GHOSTTY_MODS_CAPS.rawValue }
        return ghostty_input_mods_e(rawValue: raw)
    }

    private func consumedMods(from flags: NSEvent.ModifierFlags) -> ghostty_input_mods_e {
        mods(from: flags.subtracting([.control, .command]))
    }
}
#endif
