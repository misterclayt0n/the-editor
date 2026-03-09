import AppKit
import Foundation
import SwiftUI

struct DocsPopoverStackView: View {
    let popovers: [DocsPopoverSnapshot]
    let cursorOrigin: CGPoint
    let theme: PopupChromeTheme
    let cellSize: CGSize
    let containerSize: CGSize
    let languageHint: String

    private let minDocsWidth: CGFloat = 220
    private let maxDocsWidth: CGFloat = 520
    private let minDocsHeight: CGFloat = 64
    private let maxDocsHeight: CGFloat = 360
    private let docsLineHeight: CGFloat = 18
    private let stackGap: CGFloat = 14
    private let outerInset: CGFloat = 8

    var body: some View {
        let placements = computePlacements()

        ZStack(alignment: .topLeading) {
            ForEach(placements) { placement in
                DocsPopoverView(
                    snapshot: placement.snapshot,
                    width: placement.width,
                    height: placement.height,
                    theme: theme,
                    languageHint: languageHint
                )
                .offset(x: placement.x, y: placement.y)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .onDisappear {
            DispatchQueue.main.async {
                KeyCaptureFocusBridge.shared.reclaimActive()
            }
        }
    }

    private struct DocsPopoverPlacement: Identifiable {
        let snapshot: DocsPopoverSnapshot
        let x: CGFloat
        let y: CGFloat
        let width: CGFloat
        let height: CGFloat

        var id: String { snapshot.id }
    }

    private struct DocsPopoverMeasurement {
        let width: CGFloat
        let height: CGFloat
    }

    private func estimatedDocsWidth(for docs: String) -> CGFloat {
        let longestLine = docs
            .split(separator: "\n", omittingEmptySubsequences: false)
            .map { line in
                line.replacingOccurrences(of: "\t", with: "    ").count
            }
            .filter { $0 > 0 }
            .prefix(16)
            .max() ?? 48
        let targetColumns = min(72, max(34, longestLine))
        let width = CGFloat(targetColumns) * 6.9 + 24
        return min(max(minDocsWidth, width), maxDocsWidth)
    }

    private func estimatedDocsHeight(for docs: String, width: CGFloat) -> CGFloat {
        let columns = max(30, Int((width - 22) / 6.8))
        var wrappedLines = 0
        for rawLine in docs.split(separator: "\n", omittingEmptySubsequences: false) {
            let line = rawLine.replacingOccurrences(of: "\t", with: "    ")
            if line.trimmingCharacters(in: .whitespaces).isEmpty {
                wrappedLines += 1
                continue
            }

            var currentColumn = 0
            var lineWraps = 1
            for word in line.split(separator: " ", omittingEmptySubsequences: false) {
                let wordLen = max(1, word.count)
                if currentColumn == 0 {
                    currentColumn = wordLen
                    continue
                }
                if currentColumn + 1 + wordLen <= columns {
                    currentColumn += 1 + wordLen
                } else {
                    lineWraps += 1
                    currentColumn = wordLen
                }
            }
            wrappedLines += lineWraps
        }

        let lineCount = min(18, max(3, wrappedLines))
        let height = CGFloat(lineCount) * docsLineHeight + 14
        return min(max(minDocsHeight, height), maxDocsHeight)
    }

    private func measurePopover(_ popover: DocsPopoverSnapshot) -> DocsPopoverMeasurement {
        let availableWidth = max(1, containerSize.width - (outerInset * 2))
        let width = min(estimatedDocsWidth(for: popover.docsText), availableWidth)
        let availableHeight = max(1, containerSize.height - (outerInset * 2))
        let height = min(estimatedDocsHeight(for: popover.docsText, width: width), availableHeight)
        return DocsPopoverMeasurement(width: width, height: height)
    }

    private func computePlacements() -> [DocsPopoverPlacement] {
        guard !popovers.isEmpty else { return [] }

        let measurements = popovers.map(measurePopover)
        let maxPopoverWidth = measurements.map(\.width).max() ?? minDocsWidth
        let initialX = cursorOrigin.x + max(10, cellSize.width * 0.7)
        let maxX = max(outerInset, containerSize.width - maxPopoverWidth - outerInset)
        let sharedX = min(max(initialX, outerInset), maxX)

        let totalHeight = measurements.reduce(CGFloat.zero) { partialResult, measurement in
            partialResult + measurement.height
        } + CGFloat(max(0, measurements.count - 1)) * stackGap

        let belowAnchorY = min(
            containerSize.height - outerInset,
            cursorOrigin.y + max(12, cellSize.height * 1.05)
        )
        let aboveAnchorY = max(
            outerInset,
            cursorOrigin.y - max(8, cellSize.height * 0.45)
        )
        let availableBelow = max(0, containerSize.height - belowAnchorY - outerInset)
        let availableAbove = max(0, aboveAnchorY - outerInset)
        let placeBelow = totalHeight <= availableBelow || availableBelow >= availableAbove

        var placements: [DocsPopoverPlacement] = []
        placements.reserveCapacity(popovers.count)

        if placeBelow {
            var y = belowAnchorY
            for (popover, measurement) in zip(popovers, measurements) {
                placements.append(
                    DocsPopoverPlacement(
                        snapshot: popover,
                        x: sharedX,
                        y: y,
                        width: measurement.width,
                        height: measurement.height
                    )
                )
                y += measurement.height + stackGap
            }
        } else {
            var nextBottom = aboveAnchorY
            for (popover, measurement) in zip(popovers, measurements) {
                let y = nextBottom - measurement.height
                placements.append(
                    DocsPopoverPlacement(
                        snapshot: popover,
                        x: sharedX,
                        y: y,
                        width: measurement.width,
                        height: measurement.height
                    )
                )
                nextBottom = y - stackGap
            }
        }

        return shiftPlacementsIntoBounds(placements)
    }

    private func shiftPlacementsIntoBounds(
        _ placements: [DocsPopoverPlacement]
    ) -> [DocsPopoverPlacement] {
        guard let minY = placements.map(\.y).min(),
              let maxY = placements.map({ $0.y + $0.height }).max()
        else {
            return placements
        }

        let lowerBound = outerInset
        let upperBound = max(lowerBound, containerSize.height - outerInset)
        var deltaY: CGFloat = 0

        if minY < lowerBound {
            deltaY = lowerBound - minY
        }
        if maxY + deltaY > upperBound {
            deltaY += upperBound - (maxY + deltaY)
        }

        guard deltaY != 0 else { return placements }

        return placements.map { placement in
            DocsPopoverPlacement(
                snapshot: placement.snapshot,
                x: placement.x,
                y: placement.y + deltaY,
                width: placement.width,
                height: placement.height
            )
        }
    }
}

private struct DocsPopoverView: View {
    let snapshot: DocsPopoverSnapshot
    let width: CGFloat
    let height: CGFloat
    let theme: PopupChromeTheme
    let languageHint: String

    var body: some View {
        let chrome = chromeStyle()

        OverlayDocsPanelView(
            docs: snapshot.docsText,
            width: width,
            height: height,
            languageHint: languageHint,
            theme: theme
        )
        .overlay {
            RoundedRectangle(cornerRadius: chrome.cornerRadius, style: .continuous)
                .fill(chrome.overlayTint)
                .allowsHitTesting(false)
        }
        .overlay {
            RoundedRectangle(cornerRadius: chrome.cornerRadius, style: .continuous)
                .stroke(chrome.borderColor, lineWidth: chrome.borderWidth)
                .allowsHitTesting(false)
        }
        .shadow(color: chrome.shadowColor, radius: chrome.shadowRadius, x: 0, y: chrome.shadowYOffset)
    }

    private struct DocsPopoverChromeStyle {
        let overlayTint: Color
        let borderColor: Color
        let borderWidth: CGFloat
        let shadowColor: Color
        let shadowRadius: CGFloat
        let shadowYOffset: CGFloat
        let cornerRadius: CGFloat
    }

    private func chromeStyle() -> DocsPopoverChromeStyle {
        switch snapshot.kind {
        case .diagnostic:
            let accent = accentColor()
            return DocsPopoverChromeStyle(
                overlayTint: accent.opacity(0.05),
                borderColor: accent.opacity(0.34),
                borderWidth: 0.9,
                shadowColor: accent.opacity(0.12),
                shadowRadius: 12,
                shadowYOffset: 3,
                cornerRadius: 8
            )
        case .hover:
            return DocsPopoverChromeStyle(
                overlayTint: .clear,
                borderColor: .clear,
                borderWidth: 0,
                shadowColor: .clear,
                shadowRadius: 0,
                shadowYOffset: 0,
                cornerRadius: 8
            )
        }
    }

    private func accentColor() -> Color {
        switch snapshot.tone {
        case .error:
            return Color(nsColor: .systemRed)
        case .warning:
            return Color(nsColor: .systemOrange)
        case .information:
            return Color(nsColor: .systemBlue)
        case .hint:
            return Color(nsColor: .systemGreen)
        case .neutral:
            return theme.accentColor
        }
    }
}
