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
            let cellSize = scene.info.surfaceMetrics.cellSizePoints
            for pane in visibleTerminalPanes {
                guard let surfaceID = pane.clientSurfaceID,
                      let view = viewsBySurfaceID[surfaceID]
                else {
                    continue
                }
                let frame = CGRect(
                    x: CGFloat(pane.x) * cellSize.width,
                    y: CGFloat(pane.y) * cellSize.height,
                    width: CGFloat(pane.width) * cellSize.width,
                    height: CGFloat(pane.height) * cellSize.height
                )
                view.paneID = pane.paneID
                view.frame = frame.integral
                view.autoresizingMask = []
                if view.superview !== containerView {
                    containerView.addSubview(view)
                }
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
        runtimeConfig.supports_selection_clipboard = false
        runtimeConfig.wakeup_cb = { userdata in
            guard let userdata else { return }
            let runtime = Unmanaged<GhosttyEmbeddedRuntime>.fromOpaque(userdata).takeUnretainedValue()
            runtime.scheduleTick()
        }
        runtimeConfig.action_cb = { _, _, _ in false }
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

func ghosttyLog(_ message: String) {
    fputs("[the-swift:ghostty] \(message)\n", stderr)
}

private final class GhosttySurfaceCallbackContext {
    let clientSurfaceID: UInt
    let onCloseRequested: (UInt) -> Void

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
        callbackContext = Unmanaged.passRetained(GhosttySurfaceCallbackContext(
            clientSurfaceID: clientSurfaceID,
            onCloseRequested: onCloseRequested
        ))
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
        sendMousePosition(event)
        guard let surface else { return }
        ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_PRESS, GHOSTTY_MOUSE_LEFT, mods(from: event.modifierFlags))
    }

    override func mouseUp(with event: NSEvent) {
        sendMousePosition(event)
        guard let surface else { return }
        ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_RELEASE, GHOSTTY_MOUSE_LEFT, mods(from: event.modifierFlags))
    }

    override func rightMouseDown(with event: NSEvent) {
        paneIDOwner?.setActivePane(paneID)
        window?.makeFirstResponder(self)
        sendMousePosition(event)
        guard let surface else { return }
        ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_PRESS, GHOSTTY_MOUSE_RIGHT, mods(from: event.modifierFlags))
    }

    override func rightMouseUp(with event: NSEvent) {
        sendMousePosition(event)
        guard let surface else { return }
        ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_RELEASE, GHOSTTY_MOUSE_RIGHT, mods(from: event.modifierFlags))
    }

    override func mouseDragged(with event: NSEvent) {
        sendMousePosition(event)
    }

    override func mouseMoved(with event: NSEvent) {
        sendMousePosition(event)
    }

    override func scrollWheel(with event: NSEvent) {
        sendMousePosition(event)
        guard let surface else { return }
        ghostty_surface_mouse_scroll(surface, event.scrollingDeltaX, event.scrollingDeltaY, 0)
    }

    override func keyDown(with event: NSEvent) {
        guard let surface else {
            super.keyDown(with: event)
            return
        }
        var keyEvent = ghostty_input_key_s()
        keyEvent.action = event.isARepeat ? GHOSTTY_ACTION_REPEAT : GHOSTTY_ACTION_PRESS
        keyEvent.mods = mods(from: event.modifierFlags)
        keyEvent.consumed_mods = consumedMods(from: event.modifierFlags)
        keyEvent.keycode = UInt32(event.keyCode)
        keyEvent.text = nil
        keyEvent.unshifted_codepoint = event.charactersIgnoringModifiers?.unicodeScalars.first.map(\.value) ?? 0
        keyEvent.composing = false
        _ = ghostty_surface_key(surface, keyEvent)
    }

    override func keyUp(with event: NSEvent) {
        guard let surface else {
            super.keyUp(with: event)
            return
        }
        var keyEvent = ghostty_input_key_s()
        keyEvent.action = GHOSTTY_ACTION_RELEASE
        keyEvent.mods = mods(from: event.modifierFlags)
        keyEvent.consumed_mods = consumedMods(from: event.modifierFlags)
        keyEvent.keycode = UInt32(event.keyCode)
        keyEvent.text = nil
        keyEvent.unshifted_codepoint = event.charactersIgnoringModifiers?.unicodeScalars.first.map(\.value) ?? 0
        keyEvent.composing = false
        _ = ghostty_surface_key(surface, keyEvent)
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
        let point = convert(event.locationInWindow, from: nil)
        ghostty_surface_mouse_pos(surface, point.x, bounds.height - point.y, mods(from: event.modifierFlags))
    }

    private func mods(from flags: NSEvent.ModifierFlags) -> ghostty_input_mods_e {
        let flags = flags.intersection(.deviceIndependentFlagsMask)
        var raw = GHOSTTY_MODS_NONE.rawValue
        if flags.contains(.shift) { raw |= GHOSTTY_MODS_SHIFT.rawValue }
        if flags.contains(.control) { raw |= GHOSTTY_MODS_CTRL.rawValue }
        if flags.contains(.option) { raw |= GHOSTTY_MODS_ALT.rawValue }
        if flags.contains(.command) { raw |= GHOSTTY_MODS_SUPER.rawValue }
        return ghostty_input_mods_e(rawValue: raw)
    }

    private func consumedMods(from flags: NSEvent.ModifierFlags) -> ghostty_input_mods_e {
        let flags = flags.intersection(.deviceIndependentFlagsMask)
        var raw = GHOSTTY_MODS_NONE.rawValue
        if flags.contains(.shift) { raw |= GHOSTTY_MODS_SHIFT.rawValue }
        if flags.contains(.option) { raw |= GHOSTTY_MODS_ALT.rawValue }
        return ghostty_input_mods_e(rawValue: raw)
    }
}
#endif
