import AppKit
import CoreText
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
    private var lastAutoFocusedSurfaceID: UInt?

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

        let visibleTerminalPanes = scene?.panes.filter {
            $0.kind == .clientSurface
                && $0.clientSurfaceID != nil
        } ?? []
        let allSurfaceIDs = Set(openItems.groups.flatMap { group in
            group.items.compactMap { item -> UInt? in
                guard item.kind == .terminal else { return nil }
                return item.clientSurfaceID
            }
        })
        .union(visibleTerminalPanes.compactMap(\.clientSurfaceID))

        for surfaceID in allSurfaceIDs where viewsBySurfaceID[surfaceID] == nil {
            let workingDirectory = preferredWorkingDirectory()
            controller?.registerTerminalSurface(surfaceID, preferredWorkingDirectory: workingDirectory)
            let view = GhosttyTerminalSurfaceView(
                clientSurfaceID: surfaceID,
                runtime: runtime,
                controller: controller,
                workingDirectory: workingDirectory,
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
                view.updateInactiveAppearance(dimmed: !pane.isActive)
                view.updateFocus(pane.isActive && containerView.window?.firstResponder === view)
            }

            let currentFirstResponder = containerView.window?.firstResponder
            let currentTerminalSurfaceID = (currentFirstResponder as? GhosttyTerminalSurfaceView)?.clientSurfaceID
            if let activeTerminalPane = visibleTerminalPanes.first(where: { $0.isActive }),
               let surfaceID = activeTerminalPane.clientSurfaceID,
               let activeView = viewsBySurfaceID[surfaceID] {
                if currentFirstResponder === activeView {
                    lastAutoFocusedSurfaceID = surfaceID
                } else {
                    let currentTerminalIsVisible = currentTerminalSurfaceID.map { visibleSurfaceIDs.contains($0) } ?? false
                    let shouldMoveFocus = currentFirstResponder == nil
                        || currentFirstResponder === editorSurfaceView
                        || !currentTerminalIsVisible
                        || lastAutoFocusedSurfaceID != surfaceID
                    if shouldMoveFocus {
                        containerView.window?.makeFirstResponder(activeView)
                        lastAutoFocusedSurfaceID = surfaceID
                    }
                }
            } else {
                lastAutoFocusedSurfaceID = nil
            }
        } else {
            lastAutoFocusedSurfaceID = nil
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
        loadDefaultConfigFilesWithLegacyFallback(primaryConfig)

        var runtimeConfig = ghostty_runtime_config_s()
        runtimeConfig.userdata = Unmanaged.passUnretained(self).toOpaque()
        runtimeConfig.supports_selection_clipboard = true
        runtimeConfig.wakeup_cb = { userdata in
            guard let userdata else { return }
            let runtime = Unmanaged<GhosttyEmbeddedRuntime>.fromOpaque(userdata).takeUnretainedValue()
            runtime.scheduleTick()
        }
        runtimeConfig.action_cb = theEditorRuntimeActionCallback
        // Some GhosttyKit builds import this callback as returning `Void` in Swift even
        // though the C ABI returns `bool`. Store the C-compatible shim explicitly so the
        // project compiles against both importer variants.
        runtimeConfig.read_clipboard_cb = unsafeBitCast(
            theEditorRuntimeReadClipboardCallback as @convention(c) (
                UnsafeMutableRawPointer?,
                ghostty_clipboard_e,
                UnsafeMutableRawPointer?
            ) -> Bool,
            to: ghostty_runtime_read_clipboard_cb.self
        )
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

    private func loadDefaultConfigFilesWithLegacyFallback(_ config: ghostty_config_t) {
        // Match cmux's load ordering as closely as this GhosttyKit build allows.
        // Our current embedded kit exposes default + recursive loading, but not
        // ghostty_config_load_file, so the legacy single-file compatibility path
        // used by cmux is not available here yet.
        ghostty_config_load_default_files(config)
        ghostty_config_load_recursive_files(config)
        ghostty_config_finalize(config)
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

private struct GhosttyTextSnapshot {
    let tlPxX: Double
    let tlPxY: Double
    let offsetStart: UInt32
    let offsetLen: UInt32
    let text: String
}

private func theEditorRuntimeReadClipboardCallback(
    _ userdata: UnsafeMutableRawPointer?,
    _ location: ghostty_clipboard_e,
    _ state: UnsafeMutableRawPointer?
) -> Bool {
    guard let userdata else { return false }
    let userdataBits = UInt(bitPattern: userdata)
    let stateBits = state.map { UInt(bitPattern: $0) }
    DispatchQueue.main.async {
        guard let userdata = UnsafeMutableRawPointer(bitPattern: userdataBits) else { return }
        let context = Unmanaged<GhosttySurfaceCallbackContext>.fromOpaque(userdata).takeUnretainedValue()
        let state = stateBits.flatMap(UnsafeMutableRawPointer.init(bitPattern:))
        let text = GhosttyPasteboardBridge.readString(from: location)
        context.view?.completeClipboardRequest(text: text, state: state, confirmed: false)
    }
    return true
}

private func theEditorRuntimeActionCallback(
    _ app: ghostty_app_t?,
    _ target: ghostty_target_s,
    _ action: ghostty_action_s
) -> Bool {
    _ = app
    guard target.tag == GHOSTTY_TARGET_SURFACE,
          let userdata = ghostty_surface_userdata(target.target.surface)
    else {
        return false
    }

    let userdataBits = UInt(bitPattern: userdata)
    switch action.tag {
    case GHOSTTY_ACTION_SET_TITLE:
        let title = action.action.set_title.title.map { String(cString: $0) } ?? ""
        DispatchQueue.main.async {
            guard let userdata = UnsafeMutableRawPointer(bitPattern: userdataBits) else { return }
            let context = Unmanaged<GhosttySurfaceCallbackContext>.fromOpaque(userdata).takeUnretainedValue()
            context.controller?.updateTerminalTitle(title, for: context.clientSurfaceID)
        }
        return true
    case GHOSTTY_ACTION_PWD:
        let workingDirectory = action.action.pwd.pwd.map { String(cString: $0) } ?? ""
        DispatchQueue.main.async {
            guard let userdata = UnsafeMutableRawPointer(bitPattern: userdataBits) else { return }
            let context = Unmanaged<GhosttySurfaceCallbackContext>.fromOpaque(userdata).takeUnretainedValue()
            context.controller?.updateTerminalWorkingDirectory(workingDirectory, for: context.clientSurfaceID)
        }
        return true
    default:
        return false
    }
}

private final class GhosttySurfaceCallbackContext {
    let clientSurfaceID: UInt
    let onCloseRequested: (UInt) -> Void
    weak var controller: EditorSurfaceController?
    weak var view: GhosttyTerminalSurfaceView?

    init(clientSurfaceID: UInt, controller: EditorSurfaceController?, onCloseRequested: @escaping (UInt) -> Void) {
        self.clientSurfaceID = clientSurfaceID
        self.controller = controller
        self.onCloseRequested = onCloseRequested
    }
}

@MainActor
private final class GhosttyTerminalSurfaceView: NSView, @preconcurrency NSTextInputClient {
    let clientSurfaceID: UInt
    weak var paneIDOwner: EditorSurfaceController?
    var paneID: UInt = 0

    private let runtime: GhosttyEmbeddedRuntime
    private var surface: ghostty_surface_t?
    private var callbackContext: Unmanaged<GhosttySurfaceCallbackContext>?
    private var isSurfaceVisible = false
    private var isSurfaceFocused = false
    private var isInactiveDimmed = false
    private let inactiveDimmingLayer = CALayer()
    private var pendingWorkingDirectory: String?
    private let pendingCommand: String?
    private var lastKnownMousePointInView: NSPoint?
    private var trackingArea: NSTrackingArea?
    private var lastPerformKeyEventTimestamp: TimeInterval?
    private var keyTextAccumulator: [String]?
    private var markedText = NSMutableAttributedString()

    private var fallbackCellSize: NSSize {
        NSSize(width: 9, height: 18)
    }

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
        controller: EditorSurfaceController?,
        workingDirectory: String?,
        command: String? = nil,
        onCloseRequested: @escaping (UInt) -> Void
    ) {
        self.clientSurfaceID = clientSurfaceID
        self.runtime = runtime
        self.pendingWorkingDirectory = workingDirectory
        self.pendingCommand = command?.trimmingCharacters(in: .whitespacesAndNewlines)
        super.init(frame: NSRect(x: 0, y: 0, width: 800, height: 600))
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
        layer?.isOpaque = false
        layer?.masksToBounds = true
        inactiveDimmingLayer.backgroundColor = NSColor.black.withAlphaComponent(0.06).cgColor
        inactiveDimmingLayer.isHidden = true
        inactiveDimmingLayer.zPosition = 1
        layer?.addSublayer(inactiveDimmingLayer)
        let context = GhosttySurfaceCallbackContext(
            clientSurfaceID: clientSurfaceID,
            controller: controller,
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
        synchronizeInactiveAppearance()
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
        super.updateTrackingAreas()

        if let trackingArea {
            removeTrackingArea(trackingArea)
        }

        trackingArea = NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .mouseMoved, .inVisibleRect, .activeAlways],
            owner: self,
            userInfo: nil
        )

        if let trackingArea {
            addTrackingArea(trackingArea)
        }
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

    func updateInactiveAppearance(dimmed: Bool) {
        guard isInactiveDimmed != dimmed else { return }
        isInactiveDimmed = dimmed
        synchronizeInactiveAppearance()
    }

    private func synchronizeInactiveAppearance() {
        inactiveDimmingLayer.frame = bounds
        inactiveDimmingLayer.isHidden = !isInactiveDimmed || bounds.isEmpty
    }

    func updateFocus(_ focused: Bool) {
        guard let surface else { return }
        guard isSurfaceFocused != focused else { return }
        isSurfaceFocused = focused
        ghostty_surface_set_focus(surface, focused)
        if focused {
            synchronizeDisplayID()
        }
    }

    private func keyIsBinding(_ event: NSEvent) -> Bool {
        guard let surface else { return false }
        var keyEvent = ghosttyKeyEvent(action: GHOSTTY_ACTION_PRESS, for: event)
        var flags = ghostty_binding_flags_e(0)
        let matchesBinding: Bool = (event.characters ?? "").withCString { pointer in
            keyEvent.text = pointer
            return ghostty_surface_key_is_binding(surface, keyEvent, &flags)
        }
        return matchesBinding
    }

    /// Editor split commands (⌘D / ⌘⇧D) are implemented in SwiftUI menus; the embedded terminal
    /// would otherwise consume them as Ghostty bindings or raw key input.
    private func handleEditorSplitShortcut(_ event: NSEvent) -> Bool {
        guard event.type == .keyDown else { return false }
        guard event.charactersIgnoringModifiers?.lowercased() == "d" else { return false }
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        guard let controller = paneIDOwner else { return false }
        if flags == [.command] {
            controller.splitActivePaneVertical()
            return true
        }
        if flags == [.command, .shift] {
            controller.splitActivePaneHorizontal()
            return true
        }
        return false
    }

    private func requestPointerFocusRecovery() {
        paneIDOwner?.setActivePane(paneID)
    }

    override func mouseDown(with event: NSEvent) {
        requestPointerFocusRecovery()
        window?.makeFirstResponder(self)
        guard let surface else { return }
        let eventPoint = convert(event.locationInWindow, from: nil)
        trackMousePointIfUsable(eventPoint)
        let point = preferredPointerPoint(from: eventPoint) ?? eventPoint
        logSelectionState(
            "leftDown.before",
            event: event,
            surface: surface,
            mousePositionSent: false,
            consumed: nil,
            includeTextSnapshots: false
        )
        let sentMousePosition = event.clickCount == 1
        if sentMousePosition {
            ghostty_surface_mouse_pos(surface, point.x, bounds.height - point.y, mods(from: event.modifierFlags))
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
        requestPointerFocusRecovery()
        window?.makeFirstResponder(self)
        guard let surface else { return }
        if !ghostty_surface_mouse_captured(surface) {
            super.rightMouseDown(with: event)
            return
        }

        let eventPoint = convert(event.locationInWindow, from: nil)
        trackMousePointIfUsable(eventPoint)
        let point = preferredPointerPoint(from: eventPoint) ?? eventPoint
        ghostty_surface_mouse_pos(surface, point.x, bounds.height - point.y, mods(from: event.modifierFlags))
        _ = ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_PRESS, GHOSTTY_MOUSE_RIGHT, mods(from: event.modifierFlags))
    }

    override func rightMouseUp(with event: NSEvent) {
        guard let surface else { return }
        if !ghostty_surface_mouse_captured(surface) {
            super.rightMouseUp(with: event)
            return
        }

        _ = ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_RELEASE, GHOSTTY_MOUSE_RIGHT, mods(from: event.modifierFlags))
    }

    override func otherMouseDown(with event: NSEvent) {
        guard event.buttonNumber == 2 else {
            super.otherMouseDown(with: event)
            return
        }

        requestPointerFocusRecovery()
        window?.makeFirstResponder(self)
        guard let surface else { return }
        let eventPoint = convert(event.locationInWindow, from: nil)
        trackMousePointIfUsable(eventPoint)
        let point = preferredPointerPoint(from: eventPoint) ?? eventPoint
        ghostty_surface_mouse_pos(surface, point.x, bounds.height - point.y, mods(from: event.modifierFlags))
        _ = ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_PRESS, GHOSTTY_MOUSE_MIDDLE, mods(from: event.modifierFlags))
    }

    override func otherMouseUp(with event: NSEvent) {
        guard event.buttonNumber == 2 else {
            super.otherMouseUp(with: event)
            return
        }

        guard let surface else { return }
        _ = ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_RELEASE, GHOSTTY_MOUSE_MIDDLE, mods(from: event.modifierFlags))
    }

    override func mouseEntered(with event: NSEvent) {
        super.mouseEntered(with: event)
        sendMousePosition(event)
    }

    override func mouseExited(with event: NSEvent) {
        guard let surface else { return }
        if NSEvent.pressedMouseButtons != 0 {
            return
        }
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
        ghostty_surface_mouse_scroll(surface, x, y, scrollMods(from: event))
    }

    override func keyDown(with event: NSEvent) {
        guard let surface else {
            interpretKeyEvents([event])
            return
        }

        if handleEditorSplitShortcut(event) {
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

        let action = event.isARepeat ? GHOSTTY_ACTION_REPEAT : GHOSTTY_ACTION_PRESS
        keyTextAccumulator = []
        defer { keyTextAccumulator = nil }

        let markedTextBefore = markedText.length > 0
        lastPerformKeyEventTimestamp = nil
        interpretKeyEvents([translationEvent])
        syncPreedit(clearIfNeeded: markedTextBefore)

        if let list = keyTextAccumulator, !list.isEmpty {
            for text in list {
                _ = keyAction(action, event: event, translationEvent: translationEvent, text: text)
            }
        } else {
            _ = keyAction(
                action,
                event: event,
                translationEvent: translationEvent,
                text: ghosttyText(for: translationEvent),
                composing: markedText.length > 0 || markedTextBefore
            )
        }
    }

    override func keyUp(with event: NSEvent) {
        _ = keyAction(GHOSTTY_ACTION_RELEASE, event: event)
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard event.type == .keyDown else { return false }
        guard window?.firstResponder === self else { return false }

        if handleEditorSplitShortcut(event) {
            return true
        }

        if keyIsBinding(event) {
            keyDown(with: event)
            return true
        }

        let equivalent: String
        switch event.charactersIgnoringModifiers {
        case "\r":
            guard event.modifierFlags.contains(.control) else { return false }
            equivalent = "\r"
        case "/":
            guard event.modifierFlags.contains(.control),
                  event.modifierFlags.isDisjoint(with: [.shift, .command, .option])
            else {
                return false
            }
            equivalent = "_"
        default:
            guard event.timestamp != 0 else { return false }
            if !event.modifierFlags.contains(.command) && !event.modifierFlags.contains(.control) {
                lastPerformKeyEventTimestamp = nil
                return false
            }
            if let lastPerformKeyEventTimestamp {
                self.lastPerformKeyEventTimestamp = nil
                if lastPerformKeyEventTimestamp == event.timestamp {
                    equivalent = event.characters ?? ""
                    break
                }
            }
            lastPerformKeyEventTimestamp = event.timestamp
            return false
        }

        guard let finalEvent = NSEvent.keyEvent(
            with: .keyDown,
            location: event.locationInWindow,
            modifierFlags: event.modifierFlags,
            timestamp: event.timestamp,
            windowNumber: event.windowNumber,
            context: nil,
            characters: equivalent,
            charactersIgnoringModifiers: equivalent,
            isARepeat: event.isARepeat,
            keyCode: event.keyCode
        ) else {
            return false
        }

        keyDown(with: finalEvent)
        return true
    }

    override func flagsChanged(with event: NSEvent) {
        let mod: UInt32
        switch event.keyCode {
        case 0x39: mod = GHOSTTY_MODS_CAPS.rawValue
        case 0x38, 0x3C: mod = GHOSTTY_MODS_SHIFT.rawValue
        case 0x3B, 0x3E: mod = GHOSTTY_MODS_CTRL.rawValue
        case 0x3A, 0x3D: mod = GHOSTTY_MODS_ALT.rawValue
        case 0x37, 0x36: mod = GHOSTTY_MODS_SUPER.rawValue
        default: return
        }

        if hasMarkedText() { return }

        let currentMods = mods(from: event.modifierFlags)
        var action = GHOSTTY_ACTION_RELEASE
        if currentMods.rawValue & mod != 0 {
            let sidePressed: Bool
            switch event.keyCode {
            case 0x3C:
                sidePressed = event.modifierFlags.rawValue & UInt(NX_DEVICERSHIFTKEYMASK) != 0
            case 0x3E:
                sidePressed = event.modifierFlags.rawValue & UInt(NX_DEVICERCTLKEYMASK) != 0
            case 0x3D:
                sidePressed = event.modifierFlags.rawValue & UInt(NX_DEVICERALTKEYMASK) != 0
            case 0x36:
                sidePressed = event.modifierFlags.rawValue & UInt(NX_DEVICERCMDKEYMASK) != 0
            default:
                sidePressed = true
            }
            if sidePressed {
                action = GHOSTTY_ACTION_PRESS
            }
        }

        _ = keyAction(action, event: event)
    }

    func hasMarkedText() -> Bool {
        markedText.length > 0
    }

    func markedRange() -> NSRange {
        guard markedText.length > 0 else { return NSRange(location: NSNotFound, length: 0) }
        return NSRange(location: 0, length: markedText.length)
    }

    func selectedRange() -> NSRange {
        guard let surface else { return NSRange(location: 0, length: 0) }
        var text = ghostty_text_s()
        guard ghostty_surface_read_selection(surface, &text) else { return NSRange(location: 0, length: 0) }
        defer { ghostty_surface_free_text(surface, &text) }
        return NSRange(location: Int(text.offset_start), length: Int(text.offset_len))
    }

    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        switch string {
        case let value as NSAttributedString:
            markedText = NSMutableAttributedString(attributedString: value)
        case let value as String:
            markedText = NSMutableAttributedString(string: value)
        default:
            return
        }

        if keyTextAccumulator == nil {
            syncPreedit()
        }
    }

    func unmarkText() {
        if markedText.length > 0 {
            markedText.mutableString.setString("")
            syncPreedit()
        }
    }

    func validAttributesForMarkedText() -> [NSAttributedString.Key] {
        []
    }

    func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? {
        guard let surface else { return nil }
        guard range.length > 0 else { return nil }

        var text = ghostty_text_s()
        guard ghostty_surface_read_selection(surface, &text) else { return nil }
        defer { ghostty_surface_free_text(surface, &text) }

        var attributes: [NSAttributedString.Key: Any] = [:]
        if let fontRaw = ghostty_surface_quicklook_font(surface) {
            let font = Unmanaged<CTFont>.fromOpaque(fontRaw)
            attributes[.font] = font.takeUnretainedValue()
            font.release()
        }

        let stringData = Data(bytes: text.text, count: Int(text.text_len))
        let string = String(data: stringData, encoding: .utf8) ?? String(cString: text.text)
        return NSAttributedString(string: string, attributes: attributes)
    }

    func characterIndex(for point: NSPoint) -> Int {
        0
    }

    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
        guard let surface else {
            return NSRect(x: frame.origin.x, y: frame.origin.y, width: 0, height: 0)
        }

        var x: Double = 0
        var y: Double = 0
        var width: Double = fallbackCellSize.width
        var height: Double = fallbackCellSize.height

        if range.length > 0 && !NSEqualRanges(range, selectedRange()) {
            var text = ghostty_text_s()
            if ghostty_surface_read_selection(surface, &text) {
                x = text.tl_px_x - 2
                y = text.tl_px_y + 2
                ghostty_surface_free_text(surface, &text)
            } else {
                ghostty_surface_ime_point(surface, &x, &y, &width, &height)
            }
        } else {
            ghostty_surface_ime_point(surface, &x, &y, &width, &height)
        }

        if range.length == 0, width > 0 {
            width = 0
            x += fallbackCellSize.width * Double(range.location + range.length)
        }

        let viewRect = NSRect(
            x: x,
            y: frame.size.height - y,
            width: width,
            height: max(height, fallbackCellSize.height)
        )
        let windowRect = convert(viewRect, to: nil)
        guard let window else { return windowRect }
        return window.convertToScreen(windowRect)
    }

    func insertText(_ string: Any, replacementRange: NSRange) {
        guard NSApp.currentEvent != nil else { return }

        let chars: String
        switch string {
        case let value as NSAttributedString:
            chars = value.string
        case let value as String:
            chars = value
        default:
            return
        }

        unmarkText()

        if var accumulator = keyTextAccumulator {
            accumulator.append(chars)
            keyTextAccumulator = accumulator
            return
        }

        let len = chars.utf8CString.count
        guard let surface, len > 0 else { return }
        chars.withCString { pointer in
            ghostty_surface_text(surface, pointer, UInt(len - 1))
        }
    }

    override func doCommand(by selector: Selector) {
        if let lastPerformKeyEventTimestamp,
           let current = NSApp.currentEvent,
           lastPerformKeyEventTimestamp == current.timestamp {
            NSApp.sendEvent(current)
            return
        }

        guard let surface else { return }
        switch selector {
        case #selector(moveToBeginningOfDocument(_:)):
            _ = ghostty_surface_binding_action(surface, "scroll_to_top", UInt(strlen("scroll_to_top")))
        case #selector(moveToEndOfDocument(_:)):
            _ = ghostty_surface_binding_action(surface, "scroll_to_bottom", UInt(strlen("scroll_to_bottom")))
        default:
            break
        }
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
                if let pendingCommand, !pendingCommand.isEmpty {
                    pendingCommand.withCString { command in
                        surfaceConfig.command = command
                        surface = ghostty_surface_new(app, &surfaceConfig)
                    }
                } else {
                    surface = ghostty_surface_new(app, &surfaceConfig)
                }
            }
        } else if let pendingCommand, !pendingCommand.isEmpty {
            pendingCommand.withCString { command in
                surfaceConfig.command = command
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
        consumed: Bool?,
        includeTextSnapshots: Bool = true
    ) {
        guard ghosttySelectionLoggingEnabled, event.clickCount >= 2 else { return }
        let rawPoint = convert(event.locationInWindow, from: nil)
        let currentPoint = currentMousePointInView()
        let cachedPoint = lastKnownMousePointInView
        let preferredPoint = resolvedPointerPointForLogging(from: rawPoint, currentPoint: currentPoint, cachedPoint: cachedPoint)
        let selectionSnapshot = includeTextSnapshots ? readSelectionSnapshot(surface) : nil
        let quicklookSnapshot = includeTextSnapshots ? readQuicklookSnapshot(surface) : nil
        let selection = selectionSnapshot.map { debugText($0.text) } ?? "-"
        let quicklook = quicklookSnapshot.map { debugText($0.text) } ?? "-"
        let selectionMeta = selectionSnapshot.map { "@x=\(String(format: "%.1f", $0.tlPxX)) y=\(String(format: "%.1f", $0.tlPxY)) start=\($0.offsetStart) len=\($0.offsetLen)" } ?? "-"
        let quicklookMeta = quicklookSnapshot.map { "@x=\(String(format: "%.1f", $0.tlPxX)) y=\(String(format: "%.1f", $0.tlPxY)) start=\($0.offsetStart) len=\($0.offsetLen)" } ?? "-"
        let hasSelectionText: String
        if includeTextSnapshots {
            hasSelectionText = ghostty_surface_has_selection(surface) ? "1" : "0"
        } else {
            hasSelectionText = "-"
        }
        let consumedText = consumed.map { $0 ? "1" : "0" } ?? "-"
        ghosttySelectionLog(
            "surface=\(clientSurfaceID) pane=\(paneID) phase=\(phase) clickCount=\(event.clickCount) sentPos=\(mousePositionSent ? 1 : 0) consumed=\(consumedText) raw=\(debugPoint(rawPoint)) current=\(debugPoint(currentPoint)) cached=\(debugPoint(cachedPoint)) preferred=\(debugPoint(preferredPoint)) hasSelection=\(hasSelectionText) selection=\(selection) selectionMeta=\(selectionMeta) quicklook=\(quicklook) quicklookMeta=\(quicklookMeta)"
        )
    }

    private func readSelectionSnapshot(_ surface: ghostty_surface_t) -> GhosttyTextSnapshot? {
        guard ghostty_surface_has_selection(surface) else { return nil }
        return readTextSnapshot(surface: surface) { text in
            ghostty_surface_read_selection(surface, text)
        }
    }

    private func readQuicklookSnapshot(_ surface: ghostty_surface_t) -> GhosttyTextSnapshot? {
        readTextSnapshot(surface: surface) { text in
            ghostty_surface_quicklook_word(surface, text)
        }
    }

    private func readTextSnapshot(
        surface: ghostty_surface_t,
        reader: (UnsafeMutablePointer<ghostty_text_s>) -> Bool
    ) -> GhosttyTextSnapshot? {
        var text = ghostty_text_s()
        guard reader(&text), let pointer = text.text else {
            return nil
        }
        defer { ghostty_surface_free_text(surface, &text) }
        let textData = Data(bytes: pointer, count: Int(text.text_len))
        let decodedText = String(data: textData, encoding: .utf8) ?? String(cString: pointer)
        return GhosttyTextSnapshot(
            tlPxX: text.tl_px_x,
            tlPxY: text.tl_px_y,
            offsetStart: text.offset_start,
            offsetLen: text.offset_len,
            text: decodedText
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

    private func scrollMods(from event: NSEvent) -> ghostty_input_scroll_mods_t {
        var mods: ghostty_input_scroll_mods_t = 0
        if event.hasPreciseScrollingDeltas {
            mods |= 0b0000_0001
        }

        let momentum: ghostty_input_scroll_mods_t
        switch event.momentumPhase {
        case .began:
            momentum = ghostty_input_scroll_mods_t(GHOSTTY_MOUSE_MOMENTUM_BEGAN.rawValue)
        case .stationary:
            momentum = ghostty_input_scroll_mods_t(GHOSTTY_MOUSE_MOMENTUM_STATIONARY.rawValue)
        case .changed:
            momentum = ghostty_input_scroll_mods_t(GHOSTTY_MOUSE_MOMENTUM_CHANGED.rawValue)
        case .ended:
            momentum = ghostty_input_scroll_mods_t(GHOSTTY_MOUSE_MOMENTUM_ENDED.rawValue)
        case .cancelled:
            momentum = ghostty_input_scroll_mods_t(GHOSTTY_MOUSE_MOMENTUM_CANCELLED.rawValue)
        case .mayBegin:
            momentum = ghostty_input_scroll_mods_t(GHOSTTY_MOUSE_MOMENTUM_MAY_BEGIN.rawValue)
        default:
            momentum = ghostty_input_scroll_mods_t(GHOSTTY_MOUSE_MOMENTUM_NONE.rawValue)
        }
        mods |= momentum << 1
        return mods
    }

    private func keyAction(
        _ action: ghostty_input_action_e,
        event: NSEvent,
        translationEvent: NSEvent? = nil,
        text: String? = nil,
        composing: Bool = false
    ) -> Bool {
        guard let surface else { return false }

        var keyEvent = ghosttyKeyEvent(action: action, for: event, translationModifiers: translationEvent?.modifierFlags)
        keyEvent.composing = composing
        if let text, !text.isEmpty, let codepoint = text.utf8.first, codepoint >= 0x20 {
            return text.withCString { pointer in
                keyEvent.text = pointer
                return ghostty_surface_key(surface, keyEvent)
            }
        }
        return ghostty_surface_key(surface, keyEvent)
    }

    private func syncPreedit(clearIfNeeded: Bool = true) {
        guard let surface else { return }

        if markedText.length > 0 {
            let string = markedText.string
            let len = string.utf8CString.count
            if len > 0 {
                string.withCString { pointer in
                    ghostty_surface_preedit(surface, pointer, UInt(len - 1))
                }
            }
        } else if clearIfNeeded {
            ghostty_surface_preedit(surface, nil, 0)
        }
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
