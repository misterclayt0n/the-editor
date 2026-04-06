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
    let diagnostics: [EditorSnapshotDiagnostic]
    let diagnosticUnderlines: [EditorSnapshotDiagnosticUnderline]
    let markedText: EditorMarkedText?

    var backgroundColor: NSColor {
        info.backgroundColor?.color ?? NSColor.textBackgroundColor
    }

    var gutterBackgroundColor: NSColor {
        info.gutterBackgroundColor?.color ?? backgroundColor
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

    func line(atRow row: Int) -> EditorSceneLine? {
        lines.first(where: { $0.row == row })
    }

    func diagnostic(index: Int) -> EditorSnapshotDiagnostic? {
        diagnostics.first(where: { $0.index == index })
    }

    func diagnostics(onDocumentLine docLine: Int) -> [EditorSnapshotDiagnostic] {
        diagnostics.filter { $0.startLine == docLine }
            .sorted { lhs, rhs in
                if lhs.severity.sortRank != rhs.severity.sortRank {
                    return lhs.severity.sortRank > rhs.severity.sortRank
                }
                if lhs.startCharacter != rhs.startCharacter {
                    return lhs.startCharacter < rhs.startCharacter
                }
                return lhs.index < rhs.index
            }
    }

    func highestSeverityDiagnostic(onDocumentLine docLine: Int) -> EditorSnapshotDiagnostic? {
        diagnostics(onDocumentLine: docLine).first
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
            diagnostics: snapshot.diagnostics,
            diagnosticUnderlines: snapshot.diagnosticUnderlines,
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
