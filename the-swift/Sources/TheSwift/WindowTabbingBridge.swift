import AppKit
import SwiftUI

struct EditorWindowRoute: Codable, Hashable {
    let requestId: UUID
    let filePath: String?
    let bufferId: UInt64?

    init(requestId: UUID = UUID(), filePath: String? = nil, bufferId: UInt64? = nil) {
        self.requestId = requestId
        self.filePath = filePath
        self.bufferId = bufferId
    }
}

private final class WeakWindowRef {
    weak var window: NSWindow?

    init(_ window: NSWindow?) {
        self.window = window
    }
}

private final class NativeTabWindowObserver {
    private weak var window: NSWindow?
    private weak var coordinator: SwiftWindowTabsCoordinator?
    private var notificationTokens: [NSObjectProtocol] = []
    private var titleObservation: NSKeyValueObservation?
    private weak var observedTabGroup: NSWindowTabGroup?
    private var tabGroupWindowsObservation: NSKeyValueObservation?
    private var tabGroupVisibleObservation: NSKeyValueObservation?
    private var tabGroupSelectedObservation: NSKeyValueObservation?

    init(window: NSWindow, coordinator: SwiftWindowTabsCoordinator) {
        self.window = window
        self.coordinator = coordinator

        let center = NotificationCenter.default
        let names: [Notification.Name] = [
            NSWindow.didBecomeKeyNotification,
            NSWindow.didResignKeyNotification,
            NSWindow.didBecomeMainNotification,
            NSWindow.didResignMainNotification
        ]
        notificationTokens = names.map { name in
            center.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in
                self?.coordinator?.relabelNativeTabs(around: self?.window)
            }
        }

        titleObservation = window.observe(\.title, options: [.new]) { [weak self] window, _ in
            self?.coordinator?.windowPresentationDidChange(window)
        }

        refreshTabGroupObservers()
    }

    deinit {
        invalidate()
    }

    var isAlive: Bool {
        window != nil
    }

    var currentWindow: NSWindow? {
        window
    }

    func refreshTabGroupObservers() {
        guard let window else { return }
        let current = window.tabGroup
        if observedTabGroup === current {
            return
        }

        tabGroupWindowsObservation?.invalidate()
        tabGroupVisibleObservation?.invalidate()
        tabGroupSelectedObservation?.invalidate()
        tabGroupWindowsObservation = nil
        tabGroupVisibleObservation = nil
        tabGroupSelectedObservation = nil
        observedTabGroup = current

        guard let current else { return }

        tabGroupWindowsObservation = current.observe(\.windows, options: [.new]) { [weak self] _, _ in
            self?.coordinator?.relabelNativeTabs(around: self?.window)
        }
        tabGroupVisibleObservation = current.observe(\.isTabBarVisible, options: [.new]) { [weak self] _, _ in
            self?.coordinator?.relabelNativeTabs(around: self?.window)
        }
        tabGroupSelectedObservation = current.observe(\.selectedWindow, options: [.new]) { [weak self] _, _ in
            self?.coordinator?.relabelNativeTabs(around: self?.window)
        }
    }

    func invalidate() {
        for token in notificationTokens {
            NotificationCenter.default.removeObserver(token)
        }
        notificationTokens.removeAll()
        titleObservation?.invalidate()
        titleObservation = nil
        tabGroupWindowsObservation?.invalidate()
        tabGroupVisibleObservation?.invalidate()
        tabGroupSelectedObservation?.invalidate()
        tabGroupWindowsObservation = nil
        tabGroupVisibleObservation = nil
        tabGroupSelectedObservation = nil
        observedTabGroup = nil
    }
}

final class SwiftWindowTabsCoordinator {
    static let shared = SwiftWindowTabsCoordinator()
    private static let tabbingIdentifier = "the-swift.editor"
    private static let tabAccessoryIdentifier = NSUserInterfaceItemIdentifier("swiftTabShortcutAccessory")

    private var pendingSourceWindows: [UUID: WeakWindowRef] = [:]
    private var observedWindows: [ObjectIdentifier: NativeTabWindowObserver] = [:]
    private var windowBoundBufferIds: [ObjectIdentifier: UInt64] = [:]
    private weak var pendingRelabelWindow: NSWindow?
    private var relabelScheduled: Bool = false

    private init() {}

    var hasPendingTabOpenRequests: Bool {
        !pendingSourceWindows.isEmpty
    }

    func requestOpenBufferInTab(
        bufferId: UInt64,
        filePath: String?,
        from sourceWindow: NSWindow?,
        allowSourceWindowMatch: Bool = true,
        openWindow: (EditorWindowRoute) -> Void
    ) {
        let excludedWindow: NSWindow? = allowSourceWindowMatch ? nil : sourceWindow
        if focusExistingWindowForBufferId(bufferId, excluding: excludedWindow) {
            return
        }
        let route = EditorWindowRoute(filePath: filePath, bufferId: bufferId)
        pendingSourceWindows[route.requestId] = WeakWindowRef(sourceWindow)
        openWindow(route)
    }

    func requestOpenUntitledTab(
        from sourceWindow: NSWindow?,
        openWindow: (EditorWindowRoute) -> Void
    ) {
        let route = EditorWindowRoute(filePath: nil, bufferId: nil)
        pendingSourceWindows[route.requestId] = WeakWindowRef(sourceWindow)
        openWindow(route)
    }

    @discardableResult
    func focusExistingWindowForBufferId(
        _ bufferId: UInt64,
        excluding excludedWindow: NSWindow? = nil
    ) -> Bool {
        guard let existing = existingWindow(forBufferId: bufferId, excluding: excludedWindow) else {
            return false
        }
        existing.makeKeyAndOrderFront(nil)
        relabelNativeTabs(around: existing)
        return true
    }

    func registerWindow(_ window: NSWindow?, route: EditorWindowRoute?) {
        guard let window else { return }
        window.tabbingMode = .preferred
        window.tabbingIdentifier = Self.tabbingIdentifier
        observeWindowIfNeeded(window)
        let key = ObjectIdentifier(window)
        if let route, let bufferId = route.bufferId {
            windowBoundBufferIds[key] = bufferId
            enforceUniqueBindings(preferredWindow: window, preferredBufferId: bufferId)
        }

        guard let route else { return }
        guard let sourceWindow = pendingSourceWindows.removeValue(forKey: route.requestId)?.window else { return }
        guard sourceWindow !== window else { return }
        sourceWindow.tabbingMode = .preferred
        sourceWindow.tabbingIdentifier = Self.tabbingIdentifier

        if sourceWindow.tabbedWindows?.contains(where: { $0 === window }) == true {
            return
        }

        window.orderOut(nil)
        DispatchQueue.main.async {
            guard sourceWindow !== window else { return }
            if sourceWindow.tabbedWindows?.contains(where: { $0 === window }) == true {
                return
            }
            sourceWindow.addTabbedWindow(window, ordered: .above)
            window.makeKeyAndOrderFront(nil)
            self.relabelNativeTabs(around: sourceWindow)
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) { [weak self, weak sourceWindow] in
                self?.relabelNativeTabs(around: sourceWindow)
            }
        }
    }

    func relabelNativeTabs(around window: NSWindow?) {
        guard let window else { return }
        pendingRelabelWindow = window
        guard !relabelScheduled else { return }
        relabelScheduled = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.relabelScheduled = false
            let anchorWindow = self.pendingRelabelWindow
            self.pendingRelabelWindow = nil
            self.performRelabelNativeTabs(around: anchorWindow)
        }
    }

    private func performRelabelNativeTabs(around window: NSWindow?) {
        guard let window else { return }
        let windows = (window.tabbedWindows?.isEmpty == false) ? (window.tabbedWindows ?? [window]) : [window]
        pruneDeadObservers()
        for (index, tabWindow) in windows.enumerated() {
            observedWindows[ObjectIdentifier(tabWindow)]?.refreshTabGroupObservers()
            syncNativeTabTitle(for: tabWindow)
            let oneBased = index + 1
            updateShortcutAccessoryLabel((oneBased <= 9) ? "⌘\(oneBased)" : nil, for: tabWindow)
        }
    }

    private func syncNativeTabTitle(for window: NSWindow) {
        let title = window.title
        let tab = window.tab
        if tab.title != title {
            tab.title = title
        }

        let color: NSColor = (window.tabGroup?.selectedWindow === window || window.isKeyWindow)
            ? .labelColor
            : .secondaryLabelColor
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: NSFont.smallSystemFontSize, weight: .semibold),
            .foregroundColor: color
        ]
        tab.attributedTitle = NSAttributedString(string: title, attributes: attrs)
    }

    func windowPresentationDidChange(_ window: NSWindow?) {
        guard let window else { return }
        syncNativeTabTitle(for: window)
        relabelNativeTabs(around: window)
    }

    func windowBoundBufferIdDidChange(_ window: NSWindow?, bufferId: UInt64?) {
        guard let window else { return }
        let key = ObjectIdentifier(window)
        if let bufferId {
            windowBoundBufferIds[key] = bufferId
        } else {
            windowBoundBufferIds.removeValue(forKey: key)
        }
        enforceUniqueBindings(preferredWindow: window, preferredBufferId: bufferId)
    }

    func reconcileBufferBindings(
        liveBufferIds: Set<UInt64>,
        preferredWindow: NSWindow?,
        preferredBufferId: UInt64?
    ) {
        pruneDeadObservers()
        if liveBufferIds.isEmpty {
            windowBoundBufferIds.removeAll()
            return
        }

        windowBoundBufferIds = windowBoundBufferIds.filter { _, bufferId in
            liveBufferIds.contains(bufferId)
        }

        if let preferredWindow, let preferredBufferId, liveBufferIds.contains(preferredBufferId) {
            windowBoundBufferIds[ObjectIdentifier(preferredWindow)] = preferredBufferId
        }

        enforceUniqueBindings(preferredWindow: preferredWindow, preferredBufferId: preferredBufferId)
    }

    func isBufferBoundToAnotherWindow(_ bufferId: UInt64, excluding window: NSWindow?) -> Bool {
        pruneDeadObservers()
        let excludedKey = window.map(ObjectIdentifier.init)
        for (windowKey, boundBufferId) in windowBoundBufferIds {
            guard boundBufferId == bufferId else { continue }
            if let excludedKey, windowKey == excludedKey {
                continue
            }
            return true
        }
        return false
    }

    func hasRegisteredEditorWindow(excluding window: NSWindow?) -> Bool {
        pruneDeadObservers()
        let excludedKey = window.map(ObjectIdentifier.init)
        for (windowKey, observer) in observedWindows {
            guard observer.currentWindow != nil else { continue }
            if let excludedKey, windowKey == excludedKey {
                continue
            }
            return true
        }
        return false
    }

    func selectNativeTab(indexOneBased: Int, around window: NSWindow?) -> Bool {
        guard indexOneBased >= 1 else { return false }
        guard let window else { return false }
        let windows = (window.tabbedWindows?.isEmpty == false) ? (window.tabbedWindows ?? [window]) : [window]
        guard !windows.isEmpty else { return false }
        let targetIndex = min(indexOneBased - 1, windows.count - 1)
        guard targetIndex >= 0, targetIndex < windows.count else { return false }
        let target = windows[targetIndex]
        guard target !== window || windows.count > 1 else { return false }
        target.makeKeyAndOrderFront(nil)
        relabelNativeTabs(around: target)
        return true
    }

    private func updateShortcutAccessoryLabel(_ text: String?, for window: NSWindow) {
        let tab = window.tab
        let label: NSTextField
        if let existing = tab.accessoryView as? NSTextField,
           existing.identifier == Self.tabAccessoryIdentifier {
            label = existing
        } else {
            label = NSTextField(labelWithString: "")
            label.identifier = Self.tabAccessoryIdentifier
            label.font = .monospacedDigitSystemFont(ofSize: 10, weight: .semibold)
            label.alignment = .center
            label.lineBreakMode = .byClipping
            label.backgroundColor = .clear
            label.isBezeled = false
            label.isEditable = false
            label.drawsBackground = false
            label.frame = NSRect(x: 0, y: 0, width: 24, height: 12)
            tab.accessoryView = label
        }

        if let text, !text.isEmpty {
            if label.stringValue != text {
                label.stringValue = text
            }
            label.isHidden = false
            let active = (window.tabGroup?.selectedWindow === window) || (window.isKeyWindow && (window.tabbedWindows?.count ?? 1) <= 1)
            let textColor = active ? NSColor.labelColor : NSColor.secondaryLabelColor
            if label.textColor != textColor {
                label.textColor = textColor
            }
        } else {
            if !label.stringValue.isEmpty {
                label.stringValue = ""
            }
            label.isHidden = true
        }
    }

    private func observeWindowIfNeeded(_ window: NSWindow) {
        pruneDeadObservers()
        let key = ObjectIdentifier(window)
        if let observer = observedWindows[key] {
            observer.refreshTabGroupObservers()
            return
        }
        observedWindows[key] = NativeTabWindowObserver(window: window, coordinator: self)
    }

    private func pruneDeadObservers() {
        observedWindows = observedWindows.filter { $0.value.isAlive }
        var aliveKeys = Set(observedWindows.keys)
        for window in NSApp.windows {
            aliveKeys.insert(ObjectIdentifier(window))
        }
        windowBoundBufferIds = windowBoundBufferIds.filter { aliveKeys.contains($0.key) }
    }

    private func existingWindow(
        forBufferId bufferId: UInt64,
        excluding excludedWindow: NSWindow?
    ) -> NSWindow? {
        pruneDeadObservers()
        var seen: Set<ObjectIdentifier> = []

        let candidateWindows: [NSWindow] = {
            var windows: [NSWindow] = []
            windows.reserveCapacity(observedWindows.count + NSApp.windows.count)
            for observer in observedWindows.values {
                if let window = observer.currentWindow {
                    windows.append(window)
                }
            }
            windows.append(contentsOf: NSApp.windows)
            return windows
        }()

        for window in candidateWindows {
            let key = ObjectIdentifier(window)
            if seen.contains(key) {
                continue
            }
            seen.insert(key)
            if let excludedWindow, window === excludedWindow {
                continue
            }
            guard windowBoundBufferIds[key] == bufferId else {
                continue
            }
            return window
        }
        return nil
    }

    private func enforceUniqueBindings(preferredWindow: NSWindow?, preferredBufferId: UInt64?) {
        guard !windowBoundBufferIds.isEmpty else { return }
        let preferredWindowKey = preferredWindow.map(ObjectIdentifier.init)
        var grouped: [UInt64: [ObjectIdentifier]] = [:]
        for (windowKey, bufferId) in windowBoundBufferIds {
            grouped[bufferId, default: []].append(windowKey)
        }

        var removals: Set<ObjectIdentifier> = []
        for (bufferId, keys) in grouped where keys.count > 1 {
            let winner = preferredBindingWindow(
                forBufferId: bufferId,
                keys: keys,
                preferredWindowKey: preferredWindowKey,
                preferredBufferId: preferredBufferId
            )
            for key in keys where key != winner {
                removals.insert(key)
            }
        }

        guard !removals.isEmpty else { return }
        for key in removals {
            windowBoundBufferIds.removeValue(forKey: key)
        }
    }

    private func preferredBindingWindow(
        forBufferId bufferId: UInt64,
        keys: [ObjectIdentifier],
        preferredWindowKey: ObjectIdentifier?,
        preferredBufferId: UInt64?
    ) -> ObjectIdentifier {
        if let preferredWindowKey,
           preferredBufferId == bufferId,
           keys.contains(preferredWindowKey) {
            return preferredWindowKey
        }

        return keys.max(by: { lhs, rhs in
            bindingWindowPriority(for: lhs) < bindingWindowPriority(for: rhs)
        }) ?? keys[0]
    }

    private func bindingWindowPriority(for key: ObjectIdentifier) -> Int {
        guard let window = windowForKey(key) else { return 0 }
        if window.tabGroup?.selectedWindow === window {
            return 4
        }
        if window.isKeyWindow {
            return 3
        }
        if window.isMainWindow {
            return 2
        }
        return 1
    }

    private func windowForKey(_ key: ObjectIdentifier) -> NSWindow? {
        if let observed = observedWindows[key]?.currentWindow {
            return observed
        }
        for window in NSApp.windows where ObjectIdentifier(window) == key {
            return window
        }
        return nil
    }
}

struct WindowTabbingBridge: NSViewRepresentable {
    let route: EditorWindowRoute?
    let onWindowShouldClose: (NSWindow) -> Bool
    let onWindowChanged: (NSWindow?) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(
            onWindowShouldClose: onWindowShouldClose,
            onWindowChanged: onWindowChanged
        )
    }

    func makeNSView(context: Context) -> WindowProbeView {
        let view = WindowProbeView()
        view.onWindowChanged = { [weak coordinator = context.coordinator] window in
            coordinator?.handleWindowChanged(window)
        }
        return view
    }

    func updateNSView(_ nsView: WindowProbeView, context: Context) {
        context.coordinator.onWindowShouldClose = onWindowShouldClose
        context.coordinator.onWindowChanged = onWindowChanged
        context.coordinator.route = route
        context.coordinator.handleWindowChanged(nsView.window)
    }

    static func dismantleNSView(_ nsView: WindowProbeView, coordinator: Coordinator) {
        coordinator.invalidate()
        coordinator.onWindowChanged(nil)
        nsView.onWindowChanged = nil
    }

    private final class WindowCloseInterceptor: NSObject, NSWindowDelegate {
        private weak var window: NSWindow?
        private var originalDelegate: NSWindowDelegate?
        private let shouldClose: (NSWindow) -> Bool

        init(window: NSWindow, shouldClose: @escaping (NSWindow) -> Bool) {
            self.window = window
            self.shouldClose = shouldClose
            self.originalDelegate = window.delegate
            super.init()
            install()
        }

        deinit {
            invalidate()
        }

        var isAlive: Bool {
            window != nil
        }

        func install() {
            guard let window else { return }
            if window.delegate !== self {
                if let delegate = window.delegate, delegate !== self {
                    originalDelegate = delegate
                }
                window.delegate = self
            }
        }

        func invalidate() {
            guard let window else { return }
            if window.delegate === self {
                window.delegate = originalDelegate
            }
        }

        func windowShouldClose(_ sender: NSWindow) -> Bool {
            if !shouldClose(sender) {
                return false
            }
            return originalDelegate?.windowShouldClose?(sender) ?? true
        }

        override func responds(to aSelector: Selector!) -> Bool {
            if super.responds(to: aSelector) {
                return true
            }
            return originalDelegate?.responds(to: aSelector) ?? false
        }

        override func forwardingTarget(for aSelector: Selector!) -> Any? {
            if super.responds(to: aSelector) {
                return nil
            }
            return originalDelegate
        }
    }

    final class Coordinator: NSObject {
        var onWindowShouldClose: (NSWindow) -> Bool
        var onWindowChanged: (NSWindow?) -> Void
        var route: EditorWindowRoute?
        private weak var lastWindow: NSWindow?
        private var appliedRouteIds: Set<UUID> = []
        private var closeInterceptors: [ObjectIdentifier: WindowCloseInterceptor] = [:]

        init(
            onWindowShouldClose: @escaping (NSWindow) -> Bool,
            onWindowChanged: @escaping (NSWindow?) -> Void
        ) {
            self.onWindowShouldClose = onWindowShouldClose
            self.onWindowChanged = onWindowChanged
        }

        func handleWindowChanged(_ window: NSWindow?) {
            updateCloseInterceptors(activeWindow: window)

            if lastWindow !== window {
                if DiagnosticsDebugLog.enabled {
                    DiagnosticsDebugLog.log(
                        "window.bridge.changed old=\(lastWindow?.windowNumber ?? 0) new=\(window?.windowNumber ?? 0)"
                    )
                }
                lastWindow = window
                onWindowChanged(window)
            }

            if let route {
                guard let window else { return }
                if appliedRouteIds.contains(route.requestId) { return }
                SwiftWindowTabsCoordinator.shared.registerWindow(window, route: route)
                appliedRouteIds.insert(route.requestId)
            } else {
                SwiftWindowTabsCoordinator.shared.registerWindow(window, route: nil)
            }
        }

        func invalidate() {
            for interceptor in closeInterceptors.values {
                interceptor.invalidate()
            }
            closeInterceptors.removeAll()
        }

        private func updateCloseInterceptors(activeWindow: NSWindow?) {
            closeInterceptors = closeInterceptors.filter { $0.value.isAlive }
            guard let activeWindow else { return }
            let key = ObjectIdentifier(activeWindow)
            if let interceptor = closeInterceptors[key] {
                interceptor.install()
                return
            }
            let interceptor = WindowCloseInterceptor(window: activeWindow) { [weak self] window in
                self?.onWindowShouldClose(window) ?? true
            }
            closeInterceptors[key] = interceptor
        }
    }
}

final class WindowProbeView: NSView {
    var onWindowChanged: ((NSWindow?) -> Void)?

    override var intrinsicContentSize: NSSize {
        .zero
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        onWindowChanged?(window)
    }

    override func viewDidMoveToSuperview() {
        super.viewDidMoveToSuperview()
        onWindowChanged?(window)
    }
}
