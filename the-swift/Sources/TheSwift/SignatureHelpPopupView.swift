import Foundation
import SwiftUI
import class TheEditorFFIBridge.App

struct SignatureHelpPopupView: View {
    let snapshot: SignatureHelpSnapshot
    let cursorOrigin: CGPoint
    let cellSize: CGSize
    let containerSize: CGSize
    let languageHint: String

    private let minDocsWidth: CGFloat = 220
    private let maxDocsWidth: CGFloat = 460
    private let minDocsHeight: CGFloat = 64
    private let maxDocsHeight: CGFloat = 320
    private let docsLineHeight: CGFloat = 18

    var body: some View {
        let placement = computePlacement()

        ZStack(alignment: .topLeading) {
            OverlayDocsPanelView(
                docs: snapshot.docsText,
                width: placement.width,
                height: placement.height,
                languageHint: languageHint
            )
            .offset(x: placement.x, y: placement.y)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .onDisappear {
            DispatchQueue.main.async {
                KeyCaptureFocusBridge.shared.reclaimActive()
            }
        }
    }

    private struct Placement {
        let x: CGFloat
        let y: CGFloat
        let width: CGFloat
        let height: CGFloat
    }

    private struct SharedRect: Decodable {
        let x: Int
        let y: Int
        let width: Int
        let height: Int
    }

    private struct SharedPlacement: Decodable {
        let panel: SharedRect
    }

    private func cellsForWidth(_ width: CGFloat) -> Int {
        let cellWidth = max(1, cellSize.width)
        return max(1, Int(ceil(width / cellWidth)))
    }

    private func cellsForHeight(_ height: CGFloat) -> Int {
        let cellHeight = max(1, cellSize.height)
        return max(1, Int(ceil(height / cellHeight)))
    }

    private func pixelsForCols(_ cols: Int) -> CGFloat {
        CGFloat(max(0, cols)) * max(1, cellSize.width)
    }

    private func pixelsForRows(_ rows: Int) -> CGFloat {
        CGFloat(max(0, rows)) * max(1, cellSize.height)
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
        let targetColumns = min(64, max(34, longestLine))
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

        let lineCount = min(16, max(3, wrappedLines))
        let height = CGFloat(lineCount) * docsLineHeight + 14
        return min(max(minDocsHeight, height), maxDocsHeight)
    }

    private func computePlacement() -> Placement {
        let areaWidth = max(1, containerSize.width)
        let areaHeight = max(1, containerSize.height)
        let areaCols = max(1, Int(floor(areaWidth / max(1, cellSize.width))))
        let areaRows = max(1, Int(floor(areaHeight / max(1, cellSize.height))))
        let cursorCol = Int(floor(cursorOrigin.x / max(1, cellSize.width)))
        let cursorRow = Int(floor(cursorOrigin.y / max(1, cellSize.height)))

        let docsWidth = min(estimatedDocsWidth(for: snapshot.docsText), areaWidth)
        let docsHeight = min(estimatedDocsHeight(for: snapshot.docsText, width: docsWidth), areaHeight)
        let docsWidthCells = min(areaCols, cellsForWidth(docsWidth))
        let docsHeightCells = min(areaRows, cellsForHeight(docsHeight))

        let layoutJSON = App.signature_help_popup_layout_json(
            UInt(areaCols),
            UInt(areaRows),
            Int64(cursorCol),
            Int64(cursorRow),
            UInt(docsWidthCells),
            UInt(docsHeightCells)
        ).toString()

        let placementData = Data(layoutJSON.utf8)
        let shared = (try? JSONDecoder().decode(SharedPlacement.self, from: placementData))
            ?? SharedPlacement(
                panel: SharedRect(x: 0, y: 0, width: docsWidthCells, height: docsHeightCells)
            )

        return Placement(
            x: pixelsForCols(shared.panel.x),
            y: pixelsForRows(shared.panel.y),
            width: pixelsForCols(shared.panel.width),
            height: pixelsForRows(shared.panel.height)
        )
    }
}
