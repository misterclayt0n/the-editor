import AppKit
import Foundation

@MainActor
final class EditorCursorBlinkController {
    private var timer: Timer?
    private var lastBlinkGeneration: UInt64?
    private var blinkEnabled = false
    private var blinkInterval: TimeInterval = 0.5
    private var blinkDelay: TimeInterval = 0.5
    private var canBlink = false
    private var hasBlinkTarget = false
    private var isTracking = false

    private(set) var isCursorVisible = true

    private let onVisibilityChanged: (Bool) -> Void

    init(onVisibilityChanged: @escaping (Bool) -> Void) {
        self.onVisibilityChanged = onVisibilityChanged
    }

    func update(
        scene: EditorRenderScene?,
        isFirstResponder: Bool,
        windowIsKey: Bool,
        appIsActive: Bool,
        isTracking: Bool
    ) {
        let hasBlinkTarget = scene?.primaryCursor != nil
        let blinkEnabled = scene?.info.cursorBlinkEnabled ?? false
        let blinkInterval = max(Double(scene?.info.cursorBlinkIntervalMs ?? 500) / 1000, 0.05)
        let blinkDelay = max(Double(scene?.info.cursorBlinkDelayMs ?? 500) / 1000, 0)
        let blinkGeneration = scene?.info.cursorBlinkGeneration
        let canBlink = hasBlinkTarget && blinkEnabled && isFirstResponder && windowIsKey && appIsActive && !isTracking

        let configChanged = self.blinkEnabled != blinkEnabled
            || abs(self.blinkInterval - blinkInterval) > 0.001
            || abs(self.blinkDelay - blinkDelay) > 0.001
            || self.hasBlinkTarget != hasBlinkTarget
        let generationChanged = lastBlinkGeneration != blinkGeneration
        let eligibilityChanged = self.canBlink != canBlink
        let trackingChanged = self.isTracking != isTracking

        self.lastBlinkGeneration = blinkGeneration
        self.blinkEnabled = blinkEnabled
        self.blinkInterval = blinkInterval
        self.blinkDelay = blinkDelay
        self.hasBlinkTarget = hasBlinkTarget
        self.canBlink = canBlink
        self.isTracking = isTracking

        guard canBlink else {
            stopBlinking(forceVisible: true)
            return
        }

        if configChanged || generationChanged || eligibilityChanged || trackingChanged {
            restartBlinkCycle()
        }
    }

    func reset() {
        guard canBlink else {
            stopBlinking(forceVisible: true)
            return
        }
        restartBlinkCycle()
    }

    private func restartBlinkCycle() {
        timer?.invalidate()
        timer = nil
        setCursorVisible(true)
        scheduleTimer(after: blinkDelay)
    }

    private func stopBlinking(forceVisible: Bool) {
        timer?.invalidate()
        timer = nil
        if forceVisible {
            setCursorVisible(true)
        }
    }

    private func scheduleTimer(after delay: TimeInterval) {
        timer?.invalidate()
        let nextDelay = max(delay, 0.016)
        let timer = Timer(timeInterval: nextDelay, repeats: false) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.handleTimerFire()
            }
        }
        timer.tolerance = min(max(nextDelay * 0.1, 0.01), 0.05)
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
    }

    private func handleTimerFire() {
        guard canBlink, hasBlinkTarget, blinkEnabled else {
            stopBlinking(forceVisible: true)
            return
        }
        setCursorVisible(!isCursorVisible)
        scheduleTimer(after: blinkInterval)
    }

    private func setCursorVisible(_ visible: Bool) {
        guard isCursorVisible != visible else { return }
        isCursorVisible = visible
        onVisibilityChanged(visible)
    }
}
