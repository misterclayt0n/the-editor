import AppKit
import Foundation

@MainActor
final class EditorCursorBlinkController {
    private let fadeDuration: TimeInterval = 0.14

    private var phaseTimer: Timer?
    private var animationTimer: Timer?
    private var lastBlinkGeneration: UInt64?
    private var blinkEnabled = false
    private var blinkInterval: TimeInterval = 0.5
    private var blinkDelay: TimeInterval = 0.5
    private var canBlink = false
    private var hasBlinkTarget = false
    private var isTracking = false
    private var targetVisible = true
    private var transitionStartedAt: CFTimeInterval?
    private var transitionFromOpacity: CGFloat = 1
    private var transitionToOpacity: CGFloat = 1

    private(set) var opacity: CGFloat = 1

    private let onOpacityChanged: (CGFloat) -> Void

    init(onOpacityChanged: @escaping (CGFloat) -> Void) {
        self.onOpacityChanged = onOpacityChanged
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

    func stop() {
        stopBlinking(forceVisible: true)
    }

    private func restartBlinkCycle() {
        invalidateTimers()
        targetVisible = true
        setOpacity(1)
        schedulePhaseTimer(after: blinkDelay)
    }

    private func stopBlinking(forceVisible: Bool) {
        invalidateTimers()
        transitionStartedAt = nil
        if forceVisible {
            targetVisible = true
            setOpacity(1)
        }
    }

    private func invalidateTimers() {
        phaseTimer?.invalidate()
        phaseTimer = nil
        animationTimer?.invalidate()
        animationTimer = nil
    }

    private func schedulePhaseTimer(after delay: TimeInterval) {
        phaseTimer?.invalidate()
        let nextDelay = max(delay, 0.016)
        let timer = Timer(timeInterval: nextDelay, repeats: false) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.handlePhaseTimerFire()
            }
        }
        timer.tolerance = min(max(nextDelay * 0.1, 0.01), 0.05)
        RunLoop.main.add(timer, forMode: .common)
        phaseTimer = timer
    }

    private func handlePhaseTimerFire() {
        guard canBlink, hasBlinkTarget, blinkEnabled else {
            stopBlinking(forceVisible: true)
            return
        }
        targetVisible.toggle()
        startOpacityTransition(to: targetVisible ? 1 : 0)
        schedulePhaseTimer(after: blinkInterval)
    }

    private func startOpacityTransition(to targetOpacity: CGFloat) {
        animationTimer?.invalidate()
        transitionStartedAt = CACurrentMediaTime()
        transitionFromOpacity = opacity
        transitionToOpacity = targetOpacity

        let timer = Timer(timeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.stepOpacityTransition()
            }
        }
        timer.tolerance = 1.0 / 120.0
        RunLoop.main.add(timer, forMode: .common)
        animationTimer = timer
        stepOpacityTransition()
    }

    private func stepOpacityTransition() {
        guard let transitionStartedAt else {
            animationTimer?.invalidate()
            animationTimer = nil
            return
        }
        let elapsed = CACurrentMediaTime() - transitionStartedAt
        let progress = min(max(elapsed / fadeDuration, 0), 1)
        let eased = easeInOut(progress)
        let nextOpacity = transitionFromOpacity + CGFloat(eased) * (transitionToOpacity - transitionFromOpacity)
        setOpacity(nextOpacity)
        if progress >= 1 {
            self.transitionStartedAt = nil
            animationTimer?.invalidate()
            animationTimer = nil
            setOpacity(transitionToOpacity)
        }
    }

    private func setOpacity(_ nextOpacity: CGFloat) {
        let clamped = min(max(nextOpacity, 0), 1)
        guard abs(opacity - clamped) > 0.001 else { return }
        opacity = clamped
        onOpacityChanged(clamped)
    }

    private func easeInOut(_ value: Double) -> Double {
        value * value * (3 - (2 * value))
    }
}
