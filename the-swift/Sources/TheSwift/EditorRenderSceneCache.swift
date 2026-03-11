import AppKit
import Foundation
import SwiftUI
import TheEditorFFIBridge

final class EditorRenderSceneCache {
    struct PreparedTextScene {
        fileprivate let rows: [PreparedTextRow?]

        var drawnSpans: Int {
            rows.compactMap(\.self).reduce(0) { $0 + $1.drawnSpans }
        }

        var skippedVirtualSpans: Int {
            rows.compactMap(\.self).reduce(0) { $0 + $1.skippedVirtualSpans }
        }
    }

    private struct TextSceneKey: Equatable {
        let layoutGeneration: UInt64
        let textGeneration: UInt64
        let scrollGeneration: UInt64
        let themeGeneration: UInt64
        let fontName: String
        let pointSize: CGFloat
    }

    fileprivate struct PreparedTextRun {
        let col: UInt16
        let attributed: NSAttributedString
    }

    fileprivate struct PreparedTextRow {
        let row: UInt16
        let runs: [PreparedTextRun]
        let drawnSpans: Int
        let skippedVirtualSpans: Int
    }

    private struct CachedTextScene {
        var key: TextSceneKey
        var rows: [PreparedTextRow?]
    }

    private var textScenes: [UInt64: CachedTextScene] = [:]

    func pruneTextScenes(retaining paneIds: Set<UInt64>) {
        textScenes = textScenes.filter { paneIds.contains($0.key) }
    }

    func preparedTextScene(
        paneId: UInt64,
        plan: RenderPlan,
        nsFont: NSFont,
        colorForSpan: (RenderSpan) -> NSColor
    ) -> PreparedTextScene {
        let rowCount = Int(plan.viewport().height)
        guard rowCount > 0 else {
            textScenes.removeValue(forKey: paneId)
            return PreparedTextScene(rows: [])
        }

        let key = TextSceneKey(
            layoutGeneration: plan.layout_generation(),
            textGeneration: plan.text_generation(),
            scrollGeneration: plan.scroll_generation(),
            themeGeneration: plan.theme_generation(),
            fontName: nsFont.fontName,
            pointSize: nsFont.pointSize
        )

        var cached = textScenes.removeValue(forKey: paneId) ?? CachedTextScene(
            key: key,
            rows: Array(repeating: nil, count: rowCount)
        )

        let needsFullRebuild = cached.rows.count != rowCount
            || cached.key.layoutGeneration != key.layoutGeneration
            || cached.key.scrollGeneration != key.scrollGeneration
            || cached.key.themeGeneration != key.themeGeneration
            || cached.key.fontName != key.fontName
            || cached.key.pointSize != key.pointSize
            || plan.damage_is_full()

        if needsFullRebuild {
            cached.rows = Array(repeating: nil, count: rowCount)
            rebuildRows(
                in: 0..<rowCount,
                scene: &cached,
                plan: plan,
                nsFont: nsFont,
                colorForSpan: colorForSpan
            )
        } else if cached.key.textGeneration != key.textGeneration {
            let start = max(0, min(rowCount - 1, Int(plan.damage_start_row())))
            let end = max(start, min(rowCount - 1, Int(plan.damage_end_row())))
            rebuildRows(
                in: start..<(end + 1),
                scene: &cached,
                plan: plan,
                nsFont: nsFont,
                colorForSpan: colorForSpan
            )
        }

        cached.key = key
        textScenes[paneId] = cached
        return PreparedTextScene(rows: cached.rows)
    }

    func drawTextScene(
        _ scene: PreparedTextScene,
        in context: GraphicsContext,
        cellSize: CGSize,
        contentOffsetX: CGFloat
    ) {
        guard !scene.rows.isEmpty else { return }
        context.withCGContext { cg in
            NSGraphicsContext.saveGraphicsState()
            NSGraphicsContext.current = NSGraphicsContext(cgContext: cg, flipped: true)
            for rowEntry in scene.rows {
                guard let rowEntry else { continue }
                let y = CGFloat(rowEntry.row) * cellSize.height
                for run in rowEntry.runs {
                    run.attributed.draw(
                        at: CGPoint(
                            x: contentOffsetX + CGFloat(run.col) * cellSize.width,
                            y: y
                        )
                    )
                }
            }
            NSGraphicsContext.restoreGraphicsState()
        }
    }

    private func rebuildRows(
        in targetRows: Range<Int>,
        scene: inout CachedTextScene,
        plan: RenderPlan,
        nsFont: NSFont,
        colorForSpan: (RenderSpan) -> NSColor
    ) {
        for row in targetRows {
            scene.rows[row] = PreparedTextRow(
                row: UInt16(row),
                runs: [],
                drawnSpans: 0,
                skippedVirtualSpans: 0
            )
        }

        let lineCount = Int(plan.line_count())
        guard lineCount > 0 else { return }

        for lineIndex in 0..<lineCount {
            let line = plan.line_at(UInt(lineIndex))
            let row = Int(line.row())
            guard targetRows.contains(row) else { continue }

            var runs: [PreparedTextRun] = []
            runs.reserveCapacity(Int(line.span_count()))
            var drawnSpans = 0
            var skippedVirtualSpans = 0

            for spanIndex in 0..<Int(line.span_count()) {
                let span = line.span_at(UInt(spanIndex))
                if span.is_virtual() {
                    skippedVirtualSpans += 1
                    continue
                }
                drawnSpans += 1
                let attrs: [NSAttributedString.Key: Any] = [
                    .font: nsFont,
                    .foregroundColor: colorForSpan(span)
                ]
                runs.append(
                    PreparedTextRun(
                        col: span.col(),
                        attributed: NSAttributedString(
                            string: span.text().toString(),
                            attributes: attrs
                        )
                    )
                )
            }

            scene.rows[row] = PreparedTextRow(
                row: line.row(),
                runs: runs,
                drawnSpans: drawnSpans,
                skippedVirtualSpans: skippedVirtualSpans
            )
        }
    }
}
