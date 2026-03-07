import Foundation
import QuartzCore
import SwiftUI

struct CursorBlinkDescriptor: Equatable {
    let enabled: Bool
    let cursorCount: Int
    let intervalMilliseconds: UInt64
    let delayMilliseconds: UInt64
    let generation: UInt64

    var shouldBlink: Bool {
        enabled && cursorCount > 0
    }

    static let disabled = CursorBlinkDescriptor(
        enabled: false,
        cursorCount: 0,
        intervalMilliseconds: 0,
        delayMilliseconds: 0,
        generation: 0
    )
}

@MainActor
final class CursorBlinkController: ObservableObject {
    @Published private(set) var opacity: Double = 1.0

    private var descriptor: CursorBlinkDescriptor = .disabled
    private var blinkTask: Task<Void, Never>?

    deinit {
        blinkTask?.cancel()
    }

    func update(_ descriptor: CursorBlinkDescriptor) {
        guard descriptor != self.descriptor else {
            return
        }

        self.descriptor = descriptor
        blinkTask?.cancel()
        blinkTask = nil

        guard descriptor.shouldBlink else {
            opacity = 1.0
            return
        }

        opacity = 1.0
        let intervalMs = descriptor.intervalMilliseconds
        let delayMs = descriptor.delayMilliseconds

        blinkTask = Task { [weak self] in
            guard let self else { return }

            do {
                try await Task.sleep(nanoseconds: delayMs * 1_000_000)
            } catch {
                return
            }

            while !Task.isCancelled {
                await self.animateOpacity(from: 1.0, to: 0.0, durationMs: intervalMs)
                guard !Task.isCancelled else { return }

                await self.animateOpacity(from: 0.0, to: 1.0, durationMs: intervalMs)
                guard !Task.isCancelled else { return }
            }
        }
    }

    private func animateOpacity(from: Double, to: Double, durationMs: UInt64) async {
        let duration = Double(durationMs) / 1_000
        let startTime = CACurrentMediaTime()

        while !Task.isCancelled {
            let elapsed = CACurrentMediaTime() - startTime
            let progress = min(1.0, elapsed / duration)
            let newOpacity = from + (to - from) * Self.easeInOut(progress)

            if opacity != newOpacity {
                opacity = newOpacity
            }

            if progress >= 1.0 { break }

            do {
                try await Task.sleep(nanoseconds: 16_000_000)
            } catch {
                return
            }
        }
    }

    private static func easeInOut(_ t: Double) -> Double {
        if t < 0.5 {
            return 2.0 * t * t
        } else {
            return 1.0 - pow(-2.0 * t + 2.0, 2.0) / 2.0
        }
    }
}
