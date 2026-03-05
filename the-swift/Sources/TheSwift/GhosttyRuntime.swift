import AppKit
import Foundation
import SwiftUI

#if canImport(GhosttyKit)
import GhosttyKit

private extension NSScreen {
    var ghosttyDisplayID: UInt32? {
        let key = NSDeviceDescriptionKey("NSScreenNumber")
        return (deviceDescription[key] as? NSNumber)?.uint32Value
    }
}

private enum GhosttyPasteboardHelper {
    private static let selectionPasteboard = NSPasteboard(
        name: NSPasteboard.Name("com.mitchellh.ghostty.selection")
    )
    private static let utf8PlainTextType = NSPasteboard.PasteboardType("public.utf8-plain-text")
    private static let shellEscapeCharacters = "\\ ()[]{}<>\"'`!#$&;|*?\t"

    static func pasteboard(for location: ghostty_clipboard_e) -> NSPasteboard? {
        switch location {
        case GHOSTTY_CLIPBOARD_STANDARD:
            return .general
        case GHOSTTY_CLIPBOARD_SELECTION:
            return selectionPasteboard
        default:
            return nil
        }
    }

    static func stringContents(from pasteboard: NSPasteboard) -> String? {
        if let urls = pasteboard.readObjects(forClasses: [NSURL.self]) as? [URL], !urls.isEmpty {
            return urls
                .map { $0.isFileURL ? escapeForShell($0.path) : $0.absoluteString }
                .joined(separator: " ")
        }

        if let value = pasteboard.string(forType: .string) {
            return value
        }

        return pasteboard.string(forType: utf8PlainTextType)
    }

    static func hasString(for location: ghostty_clipboard_e) -> Bool {
        guard let pasteboard = pasteboard(for: location) else { return false }
        if let text = stringContents(from: pasteboard), !text.isEmpty { return true }
        return false
    }

    static func writeString(_ string: String, to location: ghostty_clipboard_e) {
        guard let pasteboard = pasteboard(for: location) else { return }
        pasteboard.clearContents()
        pasteboard.setString(string, forType: .string)
    }

    private static func escapeForShell(_ value: String) -> String {
        var result = value
        for char in shellEscapeCharacters {
            result = result.replacingOccurrences(of: String(char), with: "\\\(char)")
        }
        return result
    }
}

private final class GhosttySurfaceCallbackContext {
    weak var controller: GhosttySurfaceController?

    init(controller: GhosttySurfaceController) {
        self.controller = controller
    }
}

final class GhosttyRuntime {
    static let shared = GhosttyRuntime()

    private(set) var app: ghostty_app_t?
    private var config: ghostty_config_t?
    private var controllers: [UInt64: GhosttySurfaceController] = [:]
    private var appObservers: [NSObjectProtocol] = []

    private init() {
        initialize()
    }

    deinit {
        appObservers.forEach { NotificationCenter.default.removeObserver($0) }
        appObservers.removeAll()

        for controller in controllers.values {
            controller.shutdown()
        }
        controllers.removeAll()

        if let app {
            ghostty_app_free(app)
        }
        if let config {
            ghostty_config_free(config)
        }
    }

    func reconcileTerminalIds(_ terminalIds: Set<UInt64>) {
        let stale = controllers.keys.filter { !terminalIds.contains($0) }
        for terminalId in stale {
            controllers[terminalId]?.shutdown()
            controllers.removeValue(forKey: terminalId)
        }
    }

    fileprivate func controller(for terminalId: UInt64) -> GhosttySurfaceController {
        if let existing = controllers[terminalId] {
            return existing
        }
        let created = GhosttySurfaceController(terminalId: terminalId, runtime: self)
        controllers[terminalId] = created
        return created
    }

    fileprivate func tick() {
        guard let app else { return }
        ghostty_app_tick(app)
    }

    private func initialize() {
        let initResult = ghostty_init(UInt(CommandLine.argc), CommandLine.unsafeArgv)
        guard initResult == GHOSTTY_SUCCESS else {
            let message = "the-swift: ghostty_init failed result=\(initResult)\n"
            if let data = message.data(using: .utf8) {
                FileHandle.standardError.write(data)
            }
            return
        }

        guard let config = ghostty_config_new() else {
            let message = "the-swift: ghostty_config_new failed\n"
            if let data = message.data(using: .utf8) {
                FileHandle.standardError.write(data)
            }
            return
        }

        ghostty_config_load_default_files(config)
        ghostty_config_load_recursive_files(config)
        ghostty_config_finalize(config)

        var runtimeConfig = ghostty_runtime_config_s()
        runtimeConfig.userdata = Unmanaged.passUnretained(self).toOpaque()
        runtimeConfig.supports_selection_clipboard = true
        runtimeConfig.wakeup_cb = { _ in
            DispatchQueue.main.async {
                GhosttyRuntime.shared.tick()
            }
        }
        runtimeConfig.action_cb = { _, _, _ in false }
        runtimeConfig.read_clipboard_cb = { userdata, location, state in
            guard let callbackContext = GhosttyRuntime.callbackContext(from: userdata),
                  let controller = callbackContext.controller else {
                return
            }
            controller.readClipboard(location: location, state: state)
        }
        runtimeConfig.confirm_read_clipboard_cb = { userdata, content, state, request in
            guard let callbackContext = GhosttyRuntime.callbackContext(from: userdata),
                  let controller = callbackContext.controller,
                  let content else {
                return
            }
            controller.confirmReadClipboard(content: content, state: state, request: request)
        }
        runtimeConfig.write_clipboard_cb = { userdata, location, content, len, _ in
            guard let callbackContext = GhosttyRuntime.callbackContext(from: userdata),
                  let controller = callbackContext.controller else {
                return
            }
            controller.writeClipboard(location: location, content: content, len: len)
        }
        runtimeConfig.close_surface_cb = { userdata, needsConfirmClose in
            guard let callbackContext = GhosttyRuntime.callbackContext(from: userdata),
                  let controller = callbackContext.controller else {
                return
            }
            controller.closeRequested(needsConfirmClose: needsConfirmClose)
        }

        guard let app = ghostty_app_new(&runtimeConfig, config) else {
            ghostty_config_free(config)
            let message = "the-swift: ghostty_app_new failed\n"
            if let data = message.data(using: .utf8) {
                FileHandle.standardError.write(data)
            }
            return
        }

        self.config = config
        self.app = app
        installAppFocusObservers(app)
    }

    private static func callbackContext(from userdata: UnsafeMutableRawPointer?) -> GhosttySurfaceCallbackContext? {
        guard let userdata else { return nil }
        return Unmanaged<GhosttySurfaceCallbackContext>.fromOpaque(userdata).takeUnretainedValue()
    }

    private func installAppFocusObservers(_ app: ghostty_app_t) {
        ghostty_app_set_focus(app, NSApp.isActive)
        appObservers.append(
            NotificationCenter.default.addObserver(
                forName: NSApplication.didBecomeActiveNotification,
                object: nil,
                queue: .main
            ) { _ in
                ghostty_app_set_focus(app, true)
            }
        )
        appObservers.append(
            NotificationCenter.default.addObserver(
                forName: NSApplication.didResignActiveNotification,
                object: nil,
                queue: .main
            ) { _ in
                ghostty_app_set_focus(app, false)
            }
        )
    }
}

private final class GhosttySurfaceController {
    private let terminalId: UInt64
    private unowned let runtime: GhosttyRuntime

    private weak var hostView: GhosttySurfaceHostView?
    private var surface: ghostty_surface_t?
    private var focused = false
    private var lastPixelWidth: UInt32 = 0
    private var lastPixelHeight: UInt32 = 0
    private var lastScaleX: CGFloat = 0
    private var lastScaleY: CGFloat = 0
    private var lastColorScheme: ghostty_color_scheme_e?
    private var surfaceCallbackContext: Unmanaged<GhosttySurfaceCallbackContext>?

    init(terminalId: UInt64, runtime: GhosttyRuntime) {
        self.terminalId = terminalId
        self.runtime = runtime
    }

    deinit {
        shutdown()
    }

    fileprivate var surfaceHandle: ghostty_surface_t? {
        surface
    }

    func attach(to hostView: GhosttySurfaceHostView) {
        self.hostView = hostView
        createSurfaceIfNeeded()
        updateSurfaceSize()
        updateDisplayID()
        setFocused(focused)
        applyColorScheme(hostView.currentColorScheme())
    }

    func detach(from hostView: GhosttySurfaceHostView) {
        guard self.hostView === hostView else {
            return
        }
        self.hostView = nil
        destroySurface()
    }

    func shutdown() {
        hostView = nil
        destroySurface()
    }

    func setFocused(_ focused: Bool) {
        self.focused = focused
        guard let surface else { return }
        ghostty_surface_set_focus(surface, focused)

        if focused,
           let hostView,
           let window = hostView.window,
           window.firstResponder !== hostView {
            window.makeFirstResponder(hostView)
        }
    }

    func setOcclusionVisible(_ visible: Bool) {
        guard let surface else { return }
        ghostty_surface_set_occlusion(surface, visible)
    }

    func updateDisplayID() {
        guard let surface,
              let hostView else {
            return
        }
        guard let displayID = (hostView.window?.screen ?? NSScreen.main)?.ghosttyDisplayID,
              displayID != 0 else {
            return
        }
        ghostty_surface_set_display_id(surface, displayID)
    }

    func updateSurfaceSize() {
        guard let hostView,
              let surface else {
            return
        }

        let size = hostView.bounds.size
        guard size.width > 0, size.height > 0 else {
            return
        }

        let backingSize = hostView.convertToBacking(NSRect(origin: .zero, size: size)).size
        guard backingSize.width > 0, backingSize.height > 0 else {
            return
        }

        let scaleX = backingSize.width / size.width
        let scaleY = backingSize.height / size.height
        let widthPx = UInt32(max(1, Int(floor(backingSize.width))))
        let heightPx = UInt32(max(1, Int(floor(backingSize.height))))

        if abs(scaleX - lastScaleX) > 0.0001 || abs(scaleY - lastScaleY) > 0.0001 {
            ghostty_surface_set_content_scale(surface, scaleX, scaleY)
            lastScaleX = scaleX
            lastScaleY = scaleY
        }

        if widthPx != lastPixelWidth || heightPx != lastPixelHeight {
            ghostty_surface_set_size(surface, widthPx, heightPx)
            lastPixelWidth = widthPx
            lastPixelHeight = heightPx
        }
    }

    func applyColorScheme(_ scheme: ghostty_color_scheme_e) {
        guard let surface else {
            return
        }
        if let last = lastColorScheme, last == scheme {
            return
        }
        ghostty_surface_set_color_scheme(surface, scheme)
        lastColorScheme = scheme
    }

    func isMouseCaptured() -> Bool {
        guard let surface else { return false }
        return ghostty_surface_mouse_captured(surface)
    }

    func handleMouseMove(event: NSEvent) {
        guard let surface,
              let hostView else {
            return
        }
        let point = hostView.convert(event.locationInWindow, from: nil)
        let ghostY = mouseY(for: point, in: hostView)
        if DiagnosticsDebugLog.terminalMouseEnabled {
            let row = mouseRow(forGhostY: ghostY, in: hostView)
            if row <= 2 {
                logMouse(
                    "move term=\(terminalId) pane=\(hostView.paneId) local=(\(fmt(point.x)),\(fmt(point.y))) ghost=(\(fmt(point.x)),\(fmt(ghostY))) row=\(row) bounds=(\(fmt(hostView.bounds.width))x\(fmt(hostView.bounds.height)))"
                )
            }
        }
        ghostty_surface_mouse_pos(surface, point.x, ghostY, modsFromEvent(event))
    }

    func handleMouseButton(event: NSEvent, state: ghostty_input_mouse_state_e, button: ghostty_input_mouse_button_e) {
        guard let surface,
              let hostView else {
            return
        }
        let point = hostView.convert(event.locationInWindow, from: nil)
        let mods = modsFromEvent(event)
        let ghostY = mouseY(for: point, in: hostView)
        if DiagnosticsDebugLog.terminalMouseEnabled {
            let row = mouseRow(forGhostY: ghostY, in: hostView)
            logMouse(
                "button term=\(terminalId) pane=\(hostView.paneId) state=\(state.rawValue) btn=\(button.rawValue) click=\(event.clickCount) local=(\(fmt(point.x)),\(fmt(point.y))) ghost=(\(fmt(point.x)),\(fmt(ghostY))) row=\(row) bounds=(\(fmt(hostView.bounds.width))x\(fmt(hostView.bounds.height))) mods=\(mods.rawValue)"
            )
        }
        ghostty_surface_mouse_pos(surface, point.x, ghostY, mods)
        _ = ghostty_surface_mouse_button(surface, state, button, mods)
    }

    func handleScroll(event: NSEvent) {
        guard let surface else {
            return
        }

        var x = event.scrollingDeltaX
        var y = event.scrollingDeltaY
        let precision = event.hasPreciseScrollingDeltas
        if precision {
            x *= 2
            y *= 2
        }

        var mods: Int32 = 0
        if precision {
            mods |= 0b0000_0001
        }

        let momentum: Int32
        switch event.momentumPhase {
        case .began:
            momentum = Int32(GHOSTTY_MOUSE_MOMENTUM_BEGAN.rawValue)
        case .stationary:
            momentum = Int32(GHOSTTY_MOUSE_MOMENTUM_STATIONARY.rawValue)
        case .changed:
            momentum = Int32(GHOSTTY_MOUSE_MOMENTUM_CHANGED.rawValue)
        case .ended:
            momentum = Int32(GHOSTTY_MOUSE_MOMENTUM_ENDED.rawValue)
        case .cancelled:
            momentum = Int32(GHOSTTY_MOUSE_MOMENTUM_CANCELLED.rawValue)
        case .mayBegin:
            momentum = Int32(GHOSTTY_MOUSE_MOMENTUM_MAY_BEGIN.rawValue)
        default:
            momentum = Int32(GHOSTTY_MOUSE_MOMENTUM_NONE.rawValue)
        }
        mods |= momentum << 1

        ghostty_surface_mouse_scroll(
            surface,
            x,
            y,
            ghostty_input_scroll_mods_t(mods)
        )
    }

    func sendText(_ text: String) {
        guard let surface,
              let data = text.data(using: .utf8),
              !data.isEmpty else {
            return
        }

        data.withUnsafeBytes { bytes in
            guard let pointer = bytes.baseAddress?.assumingMemoryBound(to: CChar.self) else {
                return
            }
            ghostty_surface_text(surface, pointer, UInt(bytes.count))
        }
    }

    func performBindingAction(_ action: String) -> Bool {
        guard let surface else {
            return false
        }
        return action.withCString { cString in
            ghostty_surface_binding_action(surface, cString, UInt(strlen(cString)))
        }
    }

    func readClipboard(location: ghostty_clipboard_e, state: UnsafeMutableRawPointer?) {
        guard let surface else {
            return
        }

        let pasteboard = GhosttyPasteboardHelper.pasteboard(for: location)
        let value = pasteboard.flatMap { GhosttyPasteboardHelper.stringContents(from: $0) } ?? ""
        value.withCString { pointer in
            ghostty_surface_complete_clipboard_request(surface, pointer, state, false)
        }
    }

    func confirmReadClipboard(
        content: UnsafePointer<CChar>,
        state: UnsafeMutableRawPointer?,
        request: ghostty_clipboard_request_e
    ) {
        _ = request
        guard let surface else {
            return
        }
        ghostty_surface_complete_clipboard_request(surface, content, state, true)
    }

    func writeClipboard(
        location: ghostty_clipboard_e,
        content: UnsafePointer<ghostty_clipboard_content_s>?,
        len: Int
    ) {
        guard let content, len > 0 else {
            return
        }

        let buffer = UnsafeBufferPointer(start: content, count: Int(len))
        var fallback: String?
        for item in buffer {
            guard let dataPtr = item.data else { continue }
            let value = String(cString: dataPtr)

            if let mimePtr = item.mime {
                let mime = String(cString: mimePtr)
                if mime.hasPrefix("text/plain") {
                    GhosttyPasteboardHelper.writeString(value, to: location)
                    return
                }
            }

            if fallback == nil {
                fallback = value
            }
        }

        if let fallback {
            GhosttyPasteboardHelper.writeString(fallback, to: location)
        }
    }

    func closeRequested(needsConfirmClose: Bool) {
        DispatchQueue.main.async { [weak self] in
            guard let self, let hostView = self.hostView else {
                return
            }
            hostView.handleRuntimeCloseRequest(needsConfirmClose: needsConfirmClose)
        }
    }

    private func createSurfaceIfNeeded() {
        guard surface == nil,
              let app = runtime.app,
              let hostView else {
            return
        }

        var surfaceConfig = ghostty_surface_config_new()
        surfaceConfig.platform_tag = GHOSTTY_PLATFORM_MACOS
        surfaceConfig.platform = ghostty_platform_u(
            macos: ghostty_platform_macos_s(nsview: Unmanaged.passUnretained(hostView).toOpaque())
        )
        let callbackContext = Unmanaged.passRetained(GhosttySurfaceCallbackContext(controller: self))
        surfaceConfig.userdata = callbackContext.toOpaque()
        surfaceCallbackContext?.release()
        surfaceCallbackContext = callbackContext
        surfaceConfig.context = GHOSTTY_SURFACE_CONTEXT_SPLIT
        let scaleFactor = Double(hostView.window?.backingScaleFactor ?? hostView.layer?.contentsScale ?? 1.0)
        surfaceConfig.scale_factor = scaleFactor

        guard let created = ghostty_surface_new(app, &surfaceConfig) else {
            surfaceCallbackContext?.release()
            surfaceCallbackContext = nil
            return
        }

        surface = created
        updateDisplayID()
        updateSurfaceSize()
        ghostty_surface_set_focus(created, focused)
        applyColorScheme(hostView.currentColorScheme())
        ghostty_surface_refresh(created)
    }

    private func destroySurface() {
        guard let surface else {
            return
        }
        let callbackContext = surfaceCallbackContext
        surfaceCallbackContext = nil

        ghostty_surface_set_focus(surface, false)
        ghostty_surface_free(surface)
        callbackContext?.release()

        self.surface = nil
        lastPixelWidth = 0
        lastPixelHeight = 0
        lastScaleX = 0
        lastScaleY = 0
        lastColorScheme = nil
    }

    private func modsFromEvent(_ event: NSEvent) -> ghostty_input_mods_e {
        var mods = GHOSTTY_MODS_NONE.rawValue
        if event.modifierFlags.contains(.shift) { mods |= GHOSTTY_MODS_SHIFT.rawValue }
        if event.modifierFlags.contains(.control) { mods |= GHOSTTY_MODS_CTRL.rawValue }
        if event.modifierFlags.contains(.option) { mods |= GHOSTTY_MODS_ALT.rawValue }
        if event.modifierFlags.contains(.command) { mods |= GHOSTTY_MODS_SUPER.rawValue }
        return ghostty_input_mods_e(rawValue: mods)
    }

    /// Ghostty pointer APIs use a top-origin Y coordinate.
    private func mouseY(for point: CGPoint, in hostView: GhosttySurfaceHostView) -> CGFloat {
        hostView.bounds.height - point.y
    }

    private func mouseRow(forGhostY ghostY: CGFloat, in hostView: GhosttySurfaceHostView) -> Int {
        let cellHeight = max(1, hostView.cellSize.height)
        return max(0, Int(floor(ghostY / cellHeight)))
    }

    private func fmt(_ value: CGFloat) -> String {
        String(format: "%.1f", value)
    }

    private func logMouse(_ message: @autoclosure () -> String) {
        DiagnosticsDebugLog.terminalMouseLog(message())
    }
}

final class GhosttySurfaceHostView: NSView {
    override var acceptsFirstResponder: Bool { true }

    var paneId: UInt64 = 0
    var cellSize: CGSize = .init(width: 1, height: 1)
    var onPointer: ((MouseBridgeEvent) -> Void)?
    var onCloseRequest: (() -> Bool)?

    private var controller: GhosttySurfaceController?
    private var keyTextAccumulator: [String]? = nil
    private var markedText = NSMutableAttributedString()
    private var lastPerformKeyEvent: TimeInterval?
    private var screenObserver: NSObjectProtocol?
    private var occlusionObserver: NSObjectProtocol?
    private var closeConfirmationAlert: NSAlert?

    deinit {
        if let observer = screenObserver {
            NotificationCenter.default.removeObserver(observer)
        }
        if let observer = occlusionObserver {
            NotificationCenter.default.removeObserver(observer)
        }
        if let controller {
            controller.detach(from: self)
        }
    }

    fileprivate func bind(controller: GhosttySurfaceController) {
        guard self.controller !== controller else {
            controller.attach(to: self)
            return
        }

        if let current = self.controller {
            current.detach(from: self)
        }
        self.controller = controller
        controller.attach(to: self)
    }

    private func ensureSurfaceReadyForInput() -> ghostty_surface_t? {
        guard let controller else {
            return nil
        }
        controller.attach(to: self)
        controller.updateSurfaceSize()
        controller.updateDisplayID()
        controller.applyColorScheme(currentColorScheme())
        return controller.surfaceHandle
    }

    func updateFocus(_ focused: Bool) {
        controller?.setFocused(focused)
    }

    func currentColorScheme() -> ghostty_color_scheme_e {
        let bestMatch = effectiveAppearance.bestMatch(from: [.darkAqua, .aqua])
        return bestMatch == .darkAqua ? GHOSTTY_COLOR_SCHEME_DARK : GHOSTTY_COLOR_SCHEME_LIGHT
    }

    private func updateOcclusion() {
        let visible = isHiddenOrHasHiddenAncestor == false
            && (window?.occlusionState.contains(.visible) ?? false || window?.isKeyWindow == true)
        controller?.setOcclusionVisible(visible)
    }

    fileprivate func handleRuntimeCloseRequest(needsConfirmClose: Bool) {
        guard needsConfirmClose else {
            _ = onCloseRequest?()
            return
        }
        presentCloseConfirmation()
    }

    private func presentCloseConfirmation() {
        guard closeConfirmationAlert == nil else {
            return
        }

        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = "Close Terminal?"
        alert.informativeText = "The terminal still has a running process. If you close the terminal the process will be killed."
        alert.addButton(withTitle: "Cancel")
        alert.addButton(withTitle: "Close")
        closeConfirmationAlert = alert

        if let window {
            alert.beginSheetModal(for: window) { [weak self] response in
                guard let self else { return }
                self.closeConfirmationAlert = nil
                if response == .alertSecondButtonReturn {
                    _ = self.onCloseRequest?()
                }
            }
            return
        }

        let response = alert.runModal()
        closeConfirmationAlert = nil
        if response == .alertSecondButtonReturn {
            _ = onCloseRequest?()
        }
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()

        if let observer = screenObserver {
            NotificationCenter.default.removeObserver(observer)
            screenObserver = nil
        }
        if let observer = occlusionObserver {
            NotificationCenter.default.removeObserver(observer)
            occlusionObserver = nil
        }

        controller?.attach(to: self)
        controller?.updateDisplayID()
        controller?.updateSurfaceSize()
        controller?.applyColorScheme(currentColorScheme())
        updateOcclusion()

        if let window {
            screenObserver = NotificationCenter.default.addObserver(
                forName: NSWindow.didChangeScreenNotification,
                object: window,
                queue: .main
            ) { [weak self] _ in
                self?.controller?.updateDisplayID()
                self?.controller?.updateSurfaceSize()
            }

            occlusionObserver = NotificationCenter.default.addObserver(
                forName: NSWindow.didChangeOcclusionStateNotification,
                object: window,
                queue: .main
            ) { [weak self] _ in
                self?.updateOcclusion()
            }
        }
    }

    override func viewDidMoveToSuperview() {
        super.viewDidMoveToSuperview()
        updateOcclusion()
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        controller?.applyColorScheme(currentColorScheme())
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        controller?.updateDisplayID()
        controller?.updateSurfaceSize()
    }

    override func layout() {
        super.layout()
        controller?.updateSurfaceSize()
        updateOcclusion()
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        logMouseEvent("host.mouseDown", event: event)
        // Send only pane identity to Rust so it can update active pane safely.
        dispatchPointer(kind: 0, button: 1, event: event)
        controller?.setFocused(true)
        controller?.handleMouseButton(event: event, state: GHOSTTY_MOUSE_PRESS, button: GHOSTTY_MOUSE_LEFT)
    }

    override func mouseUp(with event: NSEvent) {
        logMouseEvent("host.mouseUp", event: event)
        controller?.handleMouseButton(event: event, state: GHOSTTY_MOUSE_RELEASE, button: GHOSTTY_MOUSE_LEFT)
    }

    override func rightMouseDown(with event: NSEvent) {
        guard controller?.isMouseCaptured() == true else {
            super.rightMouseDown(with: event)
            return
        }

        window?.makeFirstResponder(self)
        dispatchPointer(kind: 0, button: 3, event: event)
        controller?.setFocused(true)
        controller?.handleMouseButton(event: event, state: GHOSTTY_MOUSE_PRESS, button: GHOSTTY_MOUSE_RIGHT)
    }

    override func rightMouseUp(with event: NSEvent) {
        guard controller?.isMouseCaptured() == true else {
            super.rightMouseUp(with: event)
            return
        }
        controller?.handleMouseButton(event: event, state: GHOSTTY_MOUSE_RELEASE, button: GHOSTTY_MOUSE_RIGHT)
    }

    override func otherMouseDown(with event: NSEvent) {
        guard event.buttonNumber == 2 else {
            super.otherMouseDown(with: event)
            return
        }

        window?.makeFirstResponder(self)
        dispatchPointer(kind: 0, button: 2, event: event)
        controller?.setFocused(true)
        controller?.handleMouseButton(event: event, state: GHOSTTY_MOUSE_PRESS, button: GHOSTTY_MOUSE_MIDDLE)
    }

    override func otherMouseUp(with event: NSEvent) {
        guard event.buttonNumber == 2 else {
            super.otherMouseUp(with: event)
            return
        }
        controller?.handleMouseButton(event: event, state: GHOSTTY_MOUSE_RELEASE, button: GHOSTTY_MOUSE_MIDDLE)
    }

    override func mouseDragged(with event: NSEvent) {
        logMouseEvent("host.mouseDragged", event: event)
        controller?.handleMouseMove(event: event)
    }

    override func rightMouseDragged(with event: NSEvent) {
        controller?.handleMouseMove(event: event)
    }

    override func otherMouseDragged(with event: NSEvent) {
        controller?.handleMouseMove(event: event)
    }

    override func mouseMoved(with event: NSEvent) {
        controller?.handleMouseMove(event: event)
    }

    override func mouseEntered(with event: NSEvent) {
        controller?.handleMouseMove(event: event)
    }

    override func mouseExited(with event: NSEvent) {
        guard let surface = controller?.surfaceHandle else {
            return
        }
        if NSEvent.pressedMouseButtons != 0 {
            return
        }
        let mods = modsFromEvent(event)
        ghostty_surface_mouse_pos(surface, -1, -1, mods)
    }

    override func scrollWheel(with event: NSEvent) {
        controller?.handleScroll(event: event)
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard event.type == .keyDown else { return false }
        guard let fr = window?.firstResponder as? NSView,
              fr === self || fr.isDescendant(of: self) else { return false }
        if isCloseSurfaceKeyEquivalent(event),
           controller?.performBindingAction("close_surface") == true {
            return true
        }
        // Let app-level shortcuts (menu key equivalents) run before terminal bindings.
        if EditorNamedCommand.shouldDeferKeyEquivalentToApp(event) {
            lastPerformKeyEvent = nil
            return false
        }
        guard let surface = ensureSurfaceReadyForInput() else { return false }

        if hasMarkedText(), !event.modifierFlags.intersection(.deviceIndependentFlagsMask).contains(.command) {
            return false
        }

        let bindingFlags: ghostty_binding_flags_e? = {
            var keyEvent = ghosttyKeyEvent(for: event, surface: surface)
            let text = event.characters ?? ""
            var flags = ghostty_binding_flags_e(0)
            let isBinding = text.withCString { ptr in
                keyEvent.text = ptr
                return ghostty_surface_key_is_binding(surface, keyEvent, &flags)
            }
            return isBinding ? flags : nil
        }()

        if bindingFlags != nil {
            keyDown(with: event)
            return true
        }

        let equivalent: String
        if !event.modifierFlags.contains(.command) {
            lastPerformKeyEvent = nil
            return false
        }
        if let lastPerformKeyEvent, lastPerformKeyEvent == event.timestamp {
            self.lastPerformKeyEvent = nil
            equivalent = event.characters ?? ""
        } else {
            lastPerformKeyEvent = event.timestamp
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

    override func keyDown(with event: NSEvent) {
        guard let surface = ensureSurfaceReadyForInput() else {
            super.keyDown(with: event)
            return
        }

        let action = event.isARepeat ? GHOSTTY_ACTION_REPEAT : GHOSTTY_ACTION_PRESS

        let translationModsGhostty = ghostty_surface_key_translation_mods(surface, modsFromEvent(event))
        var translationMods = event.modifierFlags
        for flag in [NSEvent.ModifierFlags.shift, .control, .option, .command] {
            let hasFlag: Bool
            switch flag {
            case .shift:
                hasFlag = (translationModsGhostty.rawValue & GHOSTTY_MODS_SHIFT.rawValue) != 0
            case .control:
                hasFlag = (translationModsGhostty.rawValue & GHOSTTY_MODS_CTRL.rawValue) != 0
            case .option:
                hasFlag = (translationModsGhostty.rawValue & GHOSTTY_MODS_ALT.rawValue) != 0
            case .command:
                hasFlag = (translationModsGhostty.rawValue & GHOSTTY_MODS_SUPER.rawValue) != 0
            default:
                hasFlag = translationMods.contains(flag)
            }
            if hasFlag {
                translationMods.insert(flag)
            } else {
                translationMods.remove(flag)
            }
        }

        let translationEvent: NSEvent
        if translationMods == event.modifierFlags {
            translationEvent = event
        } else {
            translationEvent = NSEvent.keyEvent(
                with: event.type,
                location: event.locationInWindow,
                modifierFlags: translationMods,
                timestamp: event.timestamp,
                windowNumber: event.windowNumber,
                context: nil,
                characters: event.characters(byApplyingModifiers: translationMods) ?? "",
                charactersIgnoringModifiers: event.charactersIgnoringModifiers ?? "",
                isARepeat: event.isARepeat,
                keyCode: event.keyCode
            ) ?? event
        }

        keyTextAccumulator = []
        defer { keyTextAccumulator = nil }

        let markedTextBefore = markedText.length > 0
        let keyboardIdBefore: String? = if !markedTextBefore {
            KeyboardLayout.id
        } else {
            nil
        }

        interpretKeyEvents([translationEvent])

        if !markedTextBefore, let kbBefore = keyboardIdBefore, kbBefore != KeyboardLayout.id {
            syncPreedit(clearIfNeeded: markedTextBefore)
            return
        }

        syncPreedit(clearIfNeeded: markedTextBefore)

        var keyEvent = ghostty_input_key_s()
        keyEvent.action = action
        keyEvent.keycode = UInt32(event.keyCode)
        keyEvent.mods = modsFromEvent(event)
        keyEvent.consumed_mods = consumedModsFromFlags(translationMods)
        keyEvent.unshifted_codepoint = unshiftedCodepoint(from: event)
        keyEvent.composing = markedText.length > 0 || markedTextBefore

        let accumulatedText = keyTextAccumulator ?? []
        if !accumulatedText.isEmpty {
            keyEvent.composing = false
            for text in accumulatedText {
                if shouldSendText(text) {
                    text.withCString { ptr in
                        keyEvent.text = ptr
                        _ = ghostty_surface_key(surface, keyEvent)
                    }
                } else {
                    keyEvent.text = nil
                    _ = ghostty_surface_key(surface, keyEvent)
                }
            }
            return
        }

        if let text = textForKeyEvent(translationEvent) {
            if shouldSendText(text) {
                text.withCString { ptr in
                    keyEvent.text = ptr
                    _ = ghostty_surface_key(surface, keyEvent)
                }
            } else {
                keyEvent.text = nil
                _ = ghostty_surface_key(surface, keyEvent)
            }
            return
        }

        keyEvent.text = nil
        _ = ghostty_surface_key(surface, keyEvent)
    }

    override func keyUp(with event: NSEvent) {
        guard let surface = ensureSurfaceReadyForInput() else {
            super.keyUp(with: event)
            return
        }

        var keyEvent = ghosttyKeyEvent(for: event, surface: surface)
        keyEvent.action = GHOSTTY_ACTION_RELEASE
        keyEvent.text = nil
        keyEvent.composing = false
        _ = ghostty_surface_key(surface, keyEvent)
    }

    override func flagsChanged(with event: NSEvent) {
        guard let surface = controller?.surfaceHandle else {
            super.flagsChanged(with: event)
            return
        }

        var keyEvent = ghostty_input_key_s()
        keyEvent.action = GHOSTTY_ACTION_PRESS
        keyEvent.keycode = UInt32(event.keyCode)
        keyEvent.mods = modsFromEvent(event)
        keyEvent.consumed_mods = GHOSTTY_MODS_NONE
        keyEvent.text = nil
        keyEvent.composing = false
        _ = ghostty_surface_key(surface, keyEvent)
    }

    override func doCommand(by selector: Selector) {
        if let lastPerformKeyEvent,
           let current = NSApp.currentEvent,
           lastPerformKeyEvent == current.timestamp {
            NSApp.sendEvent(current)
            return
        }
        // Prevent system beep for unhandled commands.
    }

    @objc func paste(_ sender: Any?) {
        if controller?.performBindingAction("paste_from_clipboard") == true {
            return
        }

        guard let text = NSPasteboard.general.string(forType: .string),
              !text.isEmpty else {
            return
        }
        controller?.sendText(text)
    }

    @objc func pasteAsPlainText(_ sender: Any?) {
        _ = controller?.performBindingAction("paste_from_clipboard")
    }

    func validateUserInterfaceItem(_ item: NSValidatedUserInterfaceItem) -> Bool {
        switch item.action {
        case #selector(paste(_:)), #selector(pasteAsPlainText(_:)):
            return GhosttyPasteboardHelper.hasString(for: GHOSTTY_CLIPBOARD_STANDARD)
        default:
            return true
        }
    }

    private func dispatchPointer(kind: UInt8, button: UInt8, event: NSEvent) {
        let _ = convert(event.locationInWindow, from: nil)
        onPointer?(
            MouseBridgeEvent(
                kind: kind,
                button: button,
                logicalCol: UInt16.max,
                logicalRow: UInt16.max,
                modifiers: pointerModifierBits(from: event.modifierFlags),
                clickCount: UInt8(clamping: event.clickCount),
                surfaceId: paneId
            )
        )
    }

    private func isCloseSurfaceKeyEquivalent(_ event: NSEvent) -> Bool {
        let relevantFlags = event.modifierFlags.intersection([.command, .shift, .option, .control])
        guard relevantFlags == [.command] else { return false }
        return (event.charactersIgnoringModifiers ?? "").lowercased() == "w"
    }

    private func logMouseEvent(_ name: String, event: NSEvent) {
        guard DiagnosticsDebugLog.terminalMouseEnabled else { return }
        let point = convert(event.locationInWindow, from: nil)
        let ghostY = bounds.height - point.y
        let row = max(0, Int(floor(ghostY / max(1, cellSize.height))))
        let topDistance = max(0, bounds.height - point.y)
        let nearEdge = topDistance <= max(64, cellSize.height * 3) || point.y <= max(64, cellSize.height * 3)
        guard nearEdge || name == "host.mouseDown" || name == "host.mouseUp" else { return }
        DiagnosticsDebugLog.terminalMouseLog(
            "\(name) pane=\(paneId) local=(\(String(format: "%.1f", point.x)),\(String(format: "%.1f", point.y))) ghostY=\(String(format: "%.1f", ghostY)) row=\(row) topDist=\(String(format: "%.1f", topDistance)) bounds=(\(String(format: "%.1f", bounds.width))x\(String(format: "%.1f", bounds.height))) click=\(event.clickCount)"
        )
    }

    private func modsFromEvent(_ event: NSEvent) -> ghostty_input_mods_e {
        var mods = GHOSTTY_MODS_NONE.rawValue
        if event.modifierFlags.contains(.shift) { mods |= GHOSTTY_MODS_SHIFT.rawValue }
        if event.modifierFlags.contains(.control) { mods |= GHOSTTY_MODS_CTRL.rawValue }
        if event.modifierFlags.contains(.option) { mods |= GHOSTTY_MODS_ALT.rawValue }
        if event.modifierFlags.contains(.command) { mods |= GHOSTTY_MODS_SUPER.rawValue }
        return ghostty_input_mods_e(rawValue: mods)
    }

    private func consumedModsFromFlags(_ flags: NSEvent.ModifierFlags) -> ghostty_input_mods_e {
        var mods = GHOSTTY_MODS_NONE.rawValue
        if flags.contains(.shift) { mods |= GHOSTTY_MODS_SHIFT.rawValue }
        if flags.contains(.option) { mods |= GHOSTTY_MODS_ALT.rawValue }
        return ghostty_input_mods_e(rawValue: mods)
    }

    private func pointerModifierBits(from flags: NSEvent.ModifierFlags) -> UInt8 {
        var bits: UInt8 = 0
        if flags.contains(.control) {
            bits |= 0b0000_0001
        }
        if flags.contains(.option) {
            bits |= 0b0000_0010
        }
        if flags.contains(.shift) {
            bits |= 0b0000_0100
        }
        return bits
    }

    private func textForKeyEvent(_ event: NSEvent) -> String? {
        guard let chars = event.characters, !chars.isEmpty else { return nil }

        if chars.count == 1, let scalar = chars.unicodeScalars.first {
            let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)

            if scalar.value < 0x20 {
                if flags.contains(.control) {
                    return event.characters(byApplyingModifiers: event.modifierFlags.subtracting(.control))
                }

                if scalar.value == 0x1B,
                   flags == [.shift],
                   event.charactersIgnoringModifiers == "`" {
                    return "~"
                }
            }

            if scalar.value >= 0xF700 && scalar.value <= 0xF8FF {
                return nil
            }
        }

        return chars
    }

    private func unshiftedCodepoint(from event: NSEvent) -> UInt32 {
        if let layoutChars = KeyboardLayout.character(forKeyCode: event.keyCode),
           layoutChars.count == 1,
           let layoutScalar = layoutChars.unicodeScalars.first,
           layoutScalar.value >= 0x20,
           !(layoutScalar.value >= 0xF700 && layoutScalar.value <= 0xF8FF) {
            return layoutScalar.value
        }

        guard let chars = (event.characters(byApplyingModifiers: []) ?? event.charactersIgnoringModifiers ?? event.characters),
              let scalar = chars.unicodeScalars.first else { return 0 }
        return scalar.value
    }

    private func shouldSendText(_ text: String) -> Bool {
        guard let first = text.utf8.first else { return false }
        return first >= 0x20
    }

    private func ghosttyKeyEvent(for event: NSEvent, surface: ghostty_surface_t) -> ghostty_input_key_s {
        var keyEvent = ghostty_input_key_s()
        keyEvent.action = GHOSTTY_ACTION_PRESS
        keyEvent.keycode = UInt32(event.keyCode)
        keyEvent.mods = modsFromEvent(event)

        let translationModsGhostty = ghostty_surface_key_translation_mods(surface, modsFromEvent(event))
        var translationMods = event.modifierFlags
        for flag in [NSEvent.ModifierFlags.shift, .control, .option, .command] {
            let hasFlag: Bool
            switch flag {
            case .shift:
                hasFlag = (translationModsGhostty.rawValue & GHOSTTY_MODS_SHIFT.rawValue) != 0
            case .control:
                hasFlag = (translationModsGhostty.rawValue & GHOSTTY_MODS_CTRL.rawValue) != 0
            case .option:
                hasFlag = (translationModsGhostty.rawValue & GHOSTTY_MODS_ALT.rawValue) != 0
            case .command:
                hasFlag = (translationModsGhostty.rawValue & GHOSTTY_MODS_SUPER.rawValue) != 0
            default:
                hasFlag = translationMods.contains(flag)
            }
            if hasFlag {
                translationMods.insert(flag)
            } else {
                translationMods.remove(flag)
            }
        }

        keyEvent.consumed_mods = consumedModsFromFlags(translationMods)
        keyEvent.text = nil
        keyEvent.composing = false
        keyEvent.unshifted_codepoint = unshiftedCodepoint(from: event)
        return keyEvent
    }

}

extension GhosttySurfaceHostView: NSTextInputClient {
    func hasMarkedText() -> Bool {
        markedText.length > 0
    }

    func markedRange() -> NSRange {
        guard markedText.length > 0 else { return NSRange(location: NSNotFound, length: 0) }
        return NSRange(location: 0, length: markedText.length)
    }

    func selectedRange() -> NSRange {
        NSRange(location: NSNotFound, length: 0)
    }

    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        switch string {
        case let attributed as NSAttributedString:
            markedText = NSMutableAttributedString(attributedString: attributed)
        case let plain as String:
            markedText = NSMutableAttributedString(string: plain)
        default:
            break
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
        _ = range
        _ = actualRange
        return nil
    }

    func characterIndex(for point: NSPoint) -> Int {
        _ = point
        return 0
    }

    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
        _ = actualRange
        guard let window else {
            return NSRect(x: frame.origin.x, y: frame.origin.y, width: 0, height: 0)
        }

        var x: Double = 0
        var y: Double = 0
        var w: Double = cellSize.width
        var h: Double = cellSize.height
        if let surface = controller?.surfaceHandle {
            ghostty_surface_ime_point(surface, &x, &y, &w, &h)
        }

        let width = range.length == 0 ? 0 : w
        let viewRect = NSRect(
            x: x,
            y: frame.size.height - y,
            width: width,
            height: max(h, cellSize.height)
        )
        let winRect = convert(viewRect, to: nil)
        return window.convertToScreen(winRect)
    }

    func insertText(_ string: Any, replacementRange: NSRange) {
        _ = replacementRange
        guard NSApp.currentEvent != nil else { return }

        var chars = ""
        switch string {
        case let attributed as NSAttributedString:
            chars = attributed.string
        case let plain as String:
            chars = plain
        default:
            return
        }

        unmarkText()

        if keyTextAccumulator != nil {
            keyTextAccumulator?.append(chars)
            return
        }

        controller?.sendText(chars)
    }

    private func syncPreedit(clearIfNeeded: Bool = true) {
        guard let surface = controller?.surfaceHandle else { return }

        if markedText.length > 0 {
            let str = markedText.string
            let len = str.utf8CString.count
            if len > 0 {
                str.withCString { ptr in
                    ghostty_surface_preedit(surface, ptr, UInt(len - 1))
                }
            }
        } else if clearIfNeeded {
            ghostty_surface_preedit(surface, nil, 0)
        }
    }
}

struct GhosttyPaneView: NSViewRepresentable {
    let paneId: UInt64
    let terminalId: UInt64
    let cellSize: CGSize
    let focused: Bool
    let onPointer: (MouseBridgeEvent) -> Void
    let onCloseRequest: () -> Bool

    func makeNSView(context: Context) -> GhosttySurfaceHostView {
        let view = GhosttySurfaceHostView(frame: .zero)
        view.wantsLayer = true
        view.layer?.backgroundColor = NSColor.clear.cgColor
        view.paneId = paneId
        view.cellSize = cellSize
        view.onPointer = onPointer
        view.onCloseRequest = onCloseRequest
        let controller = GhosttyRuntime.shared.controller(for: terminalId)
        view.bind(controller: controller)
        view.updateFocus(focused)
        return view
    }

    func updateNSView(_ nsView: GhosttySurfaceHostView, context: Context) {
        nsView.paneId = paneId
        nsView.cellSize = cellSize
        nsView.onPointer = onPointer
        nsView.onCloseRequest = onCloseRequest
        let controller = GhosttyRuntime.shared.controller(for: terminalId)
        nsView.bind(controller: controller)
        nsView.updateFocus(focused)
    }
}

#else

final class GhosttyRuntime {
    static let shared = GhosttyRuntime()

    private init() {}

    func reconcileTerminalIds(_ terminalIds: Set<UInt64>) {
        _ = terminalIds
    }
}

final class MissingGhosttyView: NSView {
    private let label = NSTextField(labelWithString: "GhosttyKit.xcframework is missing")

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor.black.cgColor

        label.textColor = NSColor.secondaryLabelColor
        label.font = NSFont.systemFont(ofSize: 11)
        label.translatesAutoresizingMaskIntoConstraints = false
        addSubview(label)
        NSLayoutConstraint.activate([
            label.centerXAnchor.constraint(equalTo: centerXAnchor),
            label.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
    }

    required init?(coder: NSCoder) {
        nil
    }
}

struct GhosttyPaneView: NSViewRepresentable {
    let paneId: UInt64
    let terminalId: UInt64
    let cellSize: CGSize
    let focused: Bool
    let onPointer: (MouseBridgeEvent) -> Void
    let onCloseRequest: () -> Bool

    func makeNSView(context: Context) -> MissingGhosttyView {
        _ = paneId
        _ = terminalId
        _ = cellSize
        _ = focused
        _ = onPointer
        _ = onCloseRequest
        return MissingGhosttyView(frame: .zero)
    }

    func updateNSView(_ nsView: MissingGhosttyView, context: Context) {
        _ = nsView
    }
}

#endif
