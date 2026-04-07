import AppKit
import Foundation

struct EditorMarkedText {
    let text: String
    let row: Int
    let col: Int
}

struct EditorSceneLine: Hashable {
    let paneID: UInt
    let x: Int
    let row: Int
    let width: Int
    let docLine: Int?
    let firstVisualLine: Bool
    let spans: [EditorSnapshotSpan]
    let textCells: [EditorSnapshotTextCell]

    var cacheSignature: Int {
        var hasher = Hasher()
        hasher.combine(width)
        hasher.combine(docLine)
        hasher.combine(firstVisualLine)
        hasher.combine(textCells.count)
        for cell in textCells {
            hasher.combine(max(cell.col - x, 0))
            hasher.combine(cell.cols)
            hasher.combine(cell.text)
            hasher.combine(cell.isVirtual)
            hasher.combine(cell.style)
        }
        return hasher.finalize()
    }
}

struct EditorRenderScene {
    let info: EditorSnapshotInfo
    let panes: [EditorSnapshotPane]
    let separators: [EditorSnapshotSeparator]
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
                paneID: line.paneID,
                x: line.x,
                width: line.width,
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

    var activePane: EditorSnapshotPane? {
        panes.first(where: { $0.isActive })
    }

    func pane(id: UInt) -> EditorSnapshotPane? {
        panes.first(where: { $0.paneID == id })
    }

    func line(atRow row: Int, paneID: UInt? = nil) -> EditorSceneLine? {
        lines.first(where: { line in
            line.row == row && (paneID == nil || line.paneID == paneID)
        })
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
            panes: snapshot.panes,
            separators: snapshot.separators,
            lines: snapshot.lines.map {
                EditorSceneLine(
                    paneID: $0.paneID,
                    x: $0.x,
                    row: $0.row,
                    width: $0.width,
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
    let paneID: UInt
    let x: Int
    let width: Int
    let themeGeneration: UInt64
    let cellWidthPx: Int
    let cellHeightPx: Int
    let cellBaselinePx: Int
    let signature: Int
}
