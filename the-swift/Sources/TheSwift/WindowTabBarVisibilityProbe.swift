import AppKit
import SwiftUI

struct WindowTabBarVisibilityProbe: NSViewRepresentable {
    let onVisibilityChanged: (Bool) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(onVisibilityChanged: onVisibilityChanged)
    }

    func makeNSView(context: Context) -> WindowTabBarProbeView {
        let view = WindowTabBarProbeView(frame: .zero)
        view.onWindowChanged = { [weak coordinator = context.coordinator] window in
            coordinator?.attach(window: window)
        }
        return view
    }

    func updateNSView(_ nsView: WindowTabBarProbeView, context: Context) {
        context.coordinator.onVisibilityChanged = onVisibilityChanged
        context.coordinator.attach(window: nsView.window)
    }

    static func dismantleNSView(_ nsView: WindowTabBarProbeView, coordinator: Coordinator) {
        coordinator.invalidate()
        nsView.onWindowChanged = nil
    }

    final class Coordinator: NSObject {
        var onVisibilityChanged: (Bool) -> Void
        private weak var window: NSWindow?
        private weak var observedTabGroup: NSWindowTabGroup?
        private var windowTabGroupObservation: NSKeyValueObservation?
        private var tabGroupVisibleObservation: NSKeyValueObservation?

        init(onVisibilityChanged: @escaping (Bool) -> Void) {
            self.onVisibilityChanged = onVisibilityChanged
        }

        func attach(window: NSWindow?) {
            if self.window !== window {
                self.window = window
                rebindWindowObservers()
            }
            emitVisibility()
        }

        func invalidate() {
            windowTabGroupObservation?.invalidate()
            tabGroupVisibleObservation?.invalidate()
            windowTabGroupObservation = nil
            tabGroupVisibleObservation = nil
            observedTabGroup = nil
            window = nil
        }

        private func rebindWindowObservers() {
            windowTabGroupObservation?.invalidate()
            tabGroupVisibleObservation?.invalidate()
            windowTabGroupObservation = nil
            tabGroupVisibleObservation = nil
            observedTabGroup = nil

            guard let window else { return }
            windowTabGroupObservation = window.observe(\.tabGroup, options: [.initial, .new]) { [weak self] window, _ in
                self?.observeTabGroup(window.tabGroup)
                self?.emitVisibility()
            }
        }

        private func observeTabGroup(_ tabGroup: NSWindowTabGroup?) {
            if observedTabGroup === tabGroup {
                return
            }
            tabGroupVisibleObservation?.invalidate()
            tabGroupVisibleObservation = nil
            observedTabGroup = tabGroup
            guard let tabGroup else { return }
            tabGroupVisibleObservation = tabGroup.observe(\.isTabBarVisible, options: [.initial, .new]) { [weak self] tabGroup, _ in
                self?.onVisibilityChanged(tabGroup.isTabBarVisible)
            }
        }

        private func emitVisibility() {
            onVisibilityChanged(window?.tabGroup?.isTabBarVisible ?? false)
        }
    }
}

final class WindowTabBarProbeView: NSView {
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
