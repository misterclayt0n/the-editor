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

enum EditorPaneItemStripMetrics {
    static let height: CGFloat = 24
    static let horizontalInset: CGFloat = 8
    static let verticalInset: CGFloat = 1
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
    let paneItemStripPaneIDs: Set<UInt>

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

    func paneContainingCell(col: Int, row: Int) -> EditorSnapshotPane? {
        panes.first(where: { pane in
            col >= pane.x
                && col < (pane.x + pane.width)
                && row >= pane.y
                && row < (pane.y + pane.height)
        })
    }

    func showsPaneItemStrip(for paneID: UInt) -> Bool {
        paneItemStripPaneIDs.contains(paneID)
    }

    func paneRect(for pane: EditorSnapshotPane) -> CGRect {
        let cellSize = info.surfaceMetrics.cellSizePoints
        return CGRect(
            x: CGFloat(pane.x) * cellSize.width,
            y: CGFloat(pane.y) * cellSize.height,
            width: CGFloat(pane.width) * cellSize.width,
            height: CGFloat(pane.height) * cellSize.height
        )
    }

    func paneHeaderHeight(for pane: EditorSnapshotPane) -> CGFloat {
        showsPaneItemStrip(for: pane.paneID) ? min(EditorPaneItemStripMetrics.height, paneRect(for: pane).height) : 0
    }

    func paneContentRect(for pane: EditorSnapshotPane) -> CGRect {
        let rect = paneRect(for: pane)
        let headerHeight = paneHeaderHeight(for: pane)
        return CGRect(
            x: rect.minX,
            y: rect.minY + headerHeight,
            width: rect.width,
            height: max(rect.height - headerHeight, 0)
        )
    }

    func displayOrigin(col: Int, row: Int, paneID: UInt? = nil) -> CGPoint {
        let cellSize = info.surfaceMetrics.cellSizePoints
        let base = CGPoint(
            x: CGFloat(col) * cellSize.width,
            y: CGFloat(row) * cellSize.height
        )
        let pane = paneID.flatMap { self.pane(id: $0) } ?? paneContainingCell(col: col, row: row)
        guard let pane else { return base }
        return CGPoint(x: base.x, y: base.y + paneHeaderHeight(for: pane))
    }

    func displayRect(x: Int, y: Int, width: Int, height: Int, paneID: UInt? = nil) -> CGRect {
        let cellSize = info.surfaceMetrics.cellSizePoints
        let origin = displayOrigin(col: x, row: y, paneID: paneID)
        return CGRect(
            x: origin.x,
            y: origin.y,
            width: CGFloat(width) * cellSize.width,
            height: CGFloat(height) * cellSize.height
        )
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
        let paneItemStripPaneIDs = Set(snapshot.openItems.groups.compactMap { group in
            let showsStrip = group.items.count > 1 || group.items.contains(where: { $0.kind != .buffer })
            return showsStrip ? group.paneID : nil
        })
        return EditorRenderScene(
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
            markedText: markedText,
            paneItemStripPaneIDs: paneItemStripPaneIDs
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
