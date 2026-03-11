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

    struct PreparedDecorationScene {
        fileprivate let rows: [PreparedDecorationRow?]
    }

    private struct DecorationSceneKey: Equatable {
        let layoutGeneration: UInt64
        let decorationGeneration: UInt64
        let scrollGeneration: UInt64
        let themeGeneration: UInt64
        let contentOffsetX: CGFloat
        let cellWidth: CGFloat
        let cellHeight: CGFloat
    }

    fileprivate struct PreparedSelectionFill {
        let rect: CGRect
        let color: SwiftUI.Color
    }

    fileprivate struct PreparedUnderlineStroke {
        let path: Path
        let color: SwiftUI.Color
    }

    fileprivate struct PreparedDecorationRow {
        var selections: [PreparedSelectionFill]
        var underlines: [PreparedUnderlineStroke]
    }

    private struct CachedDecorationScene {
        var key: DecorationSceneKey
        var rows: [PreparedDecorationRow?]
    }

    struct PreparedCursorScene {
        fileprivate let cursors: [PreparedCursor]
    }

    private struct CursorSceneKey: Equatable {
        let layoutGeneration: UInt64
        let cursorGeneration: UInt64
        let scrollGeneration: UInt64
        let themeGeneration: UInt64
        let contentOffsetX: CGFloat
        let cellWidth: CGFloat
        let cellHeight: CGFloat
        let backingScale: CGFloat
    }

    fileprivate enum PreparedCursorShape {
        case bar(rect: CGRect)
        case underline(rect: CGRect)
        case hollow(rect: CGRect)
        case block(rect: CGRect)
    }

    fileprivate struct PreparedCursor {
        let id: UInt64
        let shape: PreparedCursorShape
        let color: SwiftUI.Color
    }

    private struct CachedCursorScene {
        var key: CursorSceneKey
        var cursors: [PreparedCursor]
    }

    private var textScenes: [UInt64: CachedTextScene] = [:]
    private var decorationScenes: [UInt64: CachedDecorationScene] = [:]
    private var cursorScenes: [UInt64: CachedCursorScene] = [:]

    func pruneScenes(retaining paneIds: Set<UInt64>) {
        textScenes = textScenes.filter { paneIds.contains($0.key) }
        decorationScenes = decorationScenes.filter { paneIds.contains($0.key) }
        cursorScenes = cursorScenes.filter { paneIds.contains($0.key) }
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

    func preparedDecorationScene(
        paneId: UInt64,
        plan: RenderPlan,
        cellSize: CGSize,
        contentOffsetX: CGFloat,
        selectionColor: (RenderSelection) -> SwiftUI.Color,
        underlineColor: (UInt8) -> SwiftUI.Color
    ) -> PreparedDecorationScene {
        let rowCount = Int(plan.viewport().height)
        guard rowCount > 0 else {
            decorationScenes.removeValue(forKey: paneId)
            return PreparedDecorationScene(rows: [])
        }

        let key = DecorationSceneKey(
            layoutGeneration: plan.layout_generation(),
            decorationGeneration: plan.decoration_generation(),
            scrollGeneration: plan.scroll_generation(),
            themeGeneration: plan.theme_generation(),
            contentOffsetX: contentOffsetX,
            cellWidth: cellSize.width,
            cellHeight: cellSize.height
        )
        var cached = decorationScenes.removeValue(forKey: paneId) ?? CachedDecorationScene(
            key: key,
            rows: Array(repeating: nil, count: rowCount)
        )

        let needsFullRebuild = cached.rows.count != rowCount
            || cached.key.layoutGeneration != key.layoutGeneration
            || cached.key.scrollGeneration != key.scrollGeneration
            || cached.key.themeGeneration != key.themeGeneration
            || cached.key.contentOffsetX != key.contentOffsetX
            || cached.key.cellWidth != key.cellWidth
            || cached.key.cellHeight != key.cellHeight
            || plan.damage_is_full()

        if needsFullRebuild {
            cached.rows = Array(repeating: nil, count: rowCount)
            rebuildDecorationRows(
                in: 0..<rowCount,
                scene: &cached,
                plan: plan,
                cellSize: cellSize,
                contentOffsetX: contentOffsetX,
                selectionColor: selectionColor,
                underlineColor: underlineColor
            )
        } else if cached.key.decorationGeneration != key.decorationGeneration {
            let damageRows = clampedDamageRows(plan: plan, rowCount: rowCount)
            rebuildDecorationRows(
                in: damageRows,
                scene: &cached,
                plan: plan,
                cellSize: cellSize,
                contentOffsetX: contentOffsetX,
                selectionColor: selectionColor,
                underlineColor: underlineColor
            )
        }

        cached.key = key
        decorationScenes[paneId] = cached
        return PreparedDecorationScene(rows: cached.rows)
    }

    func drawDecorationScene(
        _ scene: PreparedDecorationScene,
        in context: GraphicsContext
    ) {
        for rowEntry in scene.rows {
            guard let rowEntry else { continue }
            for selection in rowEntry.selections {
                context.fill(Path(selection.rect), with: .color(selection.color))
            }
            for underline in rowEntry.underlines {
                context.stroke(underline.path, with: .color(underline.color), lineWidth: 1.0)
            }
        }
    }

    func preparedCursorScene(
        paneId: UInt64,
        plan: RenderPlan,
        cellSize: CGSize,
        contentOffsetX: CGFloat,
        backingScale: CGFloat,
        cursorColor: (RenderCursor) -> SwiftUI.Color
    ) -> PreparedCursorScene {
        let key = CursorSceneKey(
            layoutGeneration: plan.layout_generation(),
            cursorGeneration: plan.cursor_generation(),
            scrollGeneration: plan.scroll_generation(),
            themeGeneration: plan.theme_generation(),
            contentOffsetX: contentOffsetX,
            cellWidth: cellSize.width,
            cellHeight: cellSize.height,
            backingScale: backingScale
        )
        var cached = cursorScenes.removeValue(forKey: paneId) ?? CachedCursorScene(key: key, cursors: [])
        let needsRebuild = cached.key != key || plan.damage_is_full()
        if needsRebuild {
            cached.cursors = buildPreparedCursors(
                plan: plan,
                cellSize: cellSize,
                contentOffsetX: contentOffsetX,
                backingScale: backingScale,
                cursorColor: cursorColor
            )
        }
        cached.key = key
        cursorScenes[paneId] = cached
        return PreparedCursorScene(cursors: cached.cursors)
    }

    func drawCursorScene(
        _ scene: PreparedCursorScene,
        in context: GraphicsContext,
        pickedCursorId: UInt64?,
        cursorOpacity: Double
    ) {
        let effectiveCursorOpacity = max(0.0, min(1.0, cursorOpacity))
        guard effectiveCursorOpacity > 0.001 else { return }

        for cursor in scene.cursors {
            let isPickedCursor = pickedCursorId == cursor.id
            switch cursor.shape {
            case let .bar(rect):
                context.fill(Path(rect), with: .color(cursor.color.opacity(effectiveCursorOpacity)))
            case let .underline(rect):
                context.fill(Path(rect), with: .color(cursor.color.opacity(effectiveCursorOpacity)))
            case let .hollow(rect):
                context.stroke(Path(rect), with: .color(cursor.color.opacity(effectiveCursorOpacity)), lineWidth: 1.2)
            case let .block(rect):
                let opacity = (isPickedCursor ? 0.65 : 0.5) * effectiveCursorOpacity
                context.fill(Path(rect), with: .color(cursor.color.opacity(opacity)))
            }
        }
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

    private func rebuildDecorationRows(
        in targetRows: Range<Int>,
        scene: inout CachedDecorationScene,
        plan: RenderPlan,
        cellSize: CGSize,
        contentOffsetX: CGFloat,
        selectionColor: (RenderSelection) -> SwiftUI.Color,
        underlineColor: (UInt8) -> SwiftUI.Color
    ) {
        guard !targetRows.isEmpty else { return }
        for row in targetRows {
            scene.rows[row] = PreparedDecorationRow(selections: [], underlines: [])
        }

        let selectionCount = Int(plan.selection_count())
        if selectionCount > 0 {
            for index in 0..<selectionCount {
                let selection = plan.selection_at(UInt(index))
                let selectionRect = selection.rect()
                let row = Int(selectionRect.y)
                guard targetRows.contains(row) else { continue }
                let rect = CGRect(
                    x: contentOffsetX + CGFloat(selectionRect.x) * cellSize.width,
                    y: CGFloat(selectionRect.y) * cellSize.height,
                    width: CGFloat(selectionRect.width) * cellSize.width,
                    height: CGFloat(selectionRect.height) * cellSize.height
                )
                var rowScene = scene.rows[row] ?? PreparedDecorationRow(selections: [], underlines: [])
                rowScene.selections.append(
                    PreparedSelectionFill(
                        rect: rect,
                        color: selectionColor(selection)
                    )
                )
                scene.rows[row] = rowScene
            }
        }

        let underlineCount = Int(plan.diagnostic_underline_count())
        if underlineCount > 0 {
            for index in 0..<underlineCount {
                let underline = plan.diagnostic_underline_at(UInt(index))
                let row = Int(underline.row())
                guard targetRows.contains(row) else { continue }
                let y = CGFloat(underline.row()) * cellSize.height + cellSize.height - 1
                let xStart = contentOffsetX + CGFloat(underline.start_col()) * cellSize.width
                let xEnd = contentOffsetX + CGFloat(underline.end_col()) * cellSize.width
                guard xEnd > xStart else { continue }
                let path = wavyUnderlinePath(xStart: xStart, xEnd: xEnd, y: y)
                var rowScene = scene.rows[row] ?? PreparedDecorationRow(selections: [], underlines: [])
                rowScene.underlines.append(
                    PreparedUnderlineStroke(
                        path: path,
                        color: underlineColor(underline.severity()).opacity(0.7)
                    )
                )
                scene.rows[row] = rowScene
            }
        }
    }

    private func buildPreparedCursors(
        plan: RenderPlan,
        cellSize: CGSize,
        contentOffsetX: CGFloat,
        backingScale: CGFloat,
        cursorColor: (RenderCursor) -> SwiftUI.Color
    ) -> [PreparedCursor] {
        let count = Int(plan.cursor_count())
        guard count > 0 else { return [] }

        func snapToPixel(_ value: CGFloat) -> CGFloat {
            (value * backingScale).rounded(.down) / backingScale
        }

        var cursors: [PreparedCursor] = []
        cursors.reserveCapacity(count)
        for index in 0..<count {
            let cursor = plan.cursor_at(UInt(index))
            let pos = cursor.pos()
            let x = contentOffsetX + CGFloat(pos.col) * cellSize.width
            let y = CGFloat(pos.row) * cellSize.height
            let color = cursorColor(cursor)
            let shape: PreparedCursorShape
            switch cursor.kind() {
            case 1:
                let barWidth: CGFloat = min(2.0, max(1.0, cellSize.width * 0.2))
                shape = .bar(
                    rect: CGRect(
                        x: snapToPixel(x),
                        y: snapToPixel(y),
                        width: max(1.0 / backingScale, barWidth),
                        height: max(1.0 / backingScale, snapToPixel(cellSize.height))
                    )
                )
            case 2:
                shape = .underline(
                    rect: CGRect(x: x, y: y + cellSize.height - 2, width: cellSize.width, height: 2)
                )
            case 3:
                shape = .hollow(rect: CGRect(x: x, y: y, width: cellSize.width, height: cellSize.height))
            case 4:
                continue
            default:
                shape = .block(rect: CGRect(x: x, y: y, width: cellSize.width, height: cellSize.height))
            }
            cursors.append(
                PreparedCursor(
                    id: cursor.id(),
                    shape: shape,
                    color: color
                )
            )
        }
        return cursors
    }

    private func clampedDamageRows(plan: RenderPlan, rowCount: Int) -> Range<Int> {
        guard rowCount > 0 else { return 0..<0 }
        let start = max(0, min(rowCount - 1, Int(plan.damage_start_row())))
        let end = max(start, min(rowCount - 1, Int(plan.damage_end_row())))
        return start..<(end + 1)
    }

    private func wavyUnderlinePath(xStart: CGFloat, xEnd: CGFloat, y: CGFloat) -> Path {
        let waveHeight: CGFloat = 2.0
        let wavelength: CGFloat = 4.0
        var path = Path()
        var x = xStart
        path.move(to: CGPoint(x: x, y: y))
        var up = true
        while x < xEnd {
            let nextX = min(x + wavelength, xEnd)
            let controlY = up ? y - waveHeight : y + waveHeight
            path.addQuadCurve(
                to: CGPoint(x: nextX, y: y),
                control: CGPoint(x: (x + nextX) / 2, y: controlY)
            )
            x = nextX
            up.toggle()
        }
        return path
    }
}
