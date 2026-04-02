import AppKit
import Foundation

struct EditorMarkedText {
    let text: String
    let row: Int
    let col: Int
}

struct EditorSceneLine: Hashable {
    let row: Int
    let docLine: Int?
    let firstVisualLine: Bool
    let spans: [EditorSnapshotSpan]
    let textCells: [EditorSnapshotTextCell]

    var cacheSignature: Int {
        var hasher = Hasher()
        hasher.combine(row)
        hasher.combine(docLine)
        hasher.combine(firstVisualLine)
        hasher.combine(spans)
        hasher.combine(textCells)
        return hasher.finalize()
    }
}

struct EditorRenderScene {
    let info: EditorSnapshotInfo
    let lines: [EditorSceneLine]
    let cursors: [EditorSnapshotCursor]
    let selections: [EditorSnapshotSelection]
    let overlays: [EditorSnapshotOverlay]
    let markedText: EditorMarkedText?

    var backgroundColor: NSColor {
        NSColor.textBackgroundColor
    }

    var gutterBackgroundColor: NSColor {
        NSColor.controlBackgroundColor
    }

    var visibleLineKeys: Set<EditorLineCacheKey> {
        Set(lines.map { line in
            EditorLineCacheKey(
                row: line.row,
                layoutGeneration: info.layoutGeneration,
                textGeneration: info.textGeneration,
                scrollGeneration: info.scrollGeneration,
                themeGeneration: info.themeGeneration,
                cellWidthPx: info.surfaceMetrics.cellWidthPx,
                cellHeightPx: info.surfaceMetrics.cellHeightPx,
                cellBaselinePx: info.surfaceMetrics.cellBaselinePx,
                signature: line.cacheSignature
            )
        })
    }

    var primaryCursor: EditorSnapshotCursor? {
        cursors.first
    }

    static func from(snapshot: EditorSnapshot, markedText: EditorMarkedText?) -> EditorRenderScene {
        EditorRenderScene(
            info: snapshot.info,
            lines: snapshot.lines.map {
                EditorSceneLine(
                    row: $0.row,
                    docLine: $0.docLine,
                    firstVisualLine: $0.firstVisualLine,
                    spans: $0.spans,
                    textCells: $0.textCells
                )
            },
            cursors: snapshot.cursors,
            selections: snapshot.selections,
            overlays: snapshot.overlays,
            markedText: markedText
        )
    }
}

struct EditorLineCacheKey: Hashable {
    let row: Int
    let layoutGeneration: UInt64
    let textGeneration: UInt64
    let scrollGeneration: UInt64
    let themeGeneration: UInt64
    let cellWidthPx: Int
    let cellHeightPx: Int
    let cellBaselinePx: Int
    let signature: Int
}
