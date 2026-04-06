import AppKit
import SwiftUI

private let diagnosticOverlayEdgePadding: CGFloat = 8
private let diagnosticInlinePillHeight: CGFloat = 20
private let diagnosticInlineHorizontalPadding: CGFloat = 8

struct EditorDiagnosticsOverlayView: View {
    @ObservedObject var controller: EditorSurfaceController

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .topLeading) {
                if let scene = controller.scene,
                   let pill = currentLinePill(scene: scene, viewportSize: geometry.size) {
                    EditorDiagnosticInlinePill(
                        diagnostic: pill.diagnostic,
                        extraCount: pill.extraCount,
                        backgroundColor: controller.chrome.backgroundColor
                    )
                    .frame(width: pill.frame.width, height: pill.frame.height)
                    .offset(x: pill.frame.minX, y: pill.frame.minY)
                    .allowsHitTesting(false)
                    .zIndex(2)
                }
            }
            .frame(width: geometry.size.width, height: geometry.size.height, alignment: .topLeading)
        }
        .allowsHitTesting(false)
    }

    private func currentLinePill(scene: EditorRenderScene, viewportSize: CGSize) -> (diagnostic: EditorSnapshotDiagnostic, extraCount: Int, frame: CGRect)? {
        guard let cursor = scene.primaryCursor,
              let activePane = scene.activePane,
              let line = scene.line(atRow: cursor.row, paneID: activePane.paneID),
              let docLine = line.docLine else {
            return nil
        }
        let diagnostics = scene.diagnostics(onDocumentLine: docLine)
        guard let diagnostic = diagnostics.first else { return nil }

        let cellSize = scene.info.surfaceMetrics.cellSizePoints
        let pillText = diagnosticInlineSummary(for: diagnostic, extraCount: max(diagnostics.count - 1, 0))
        let measuredTextWidth = diagnosticInlineTextWidth(for: pillText)
        let width = min(max(measuredTextWidth + diagnosticInlineHorizontalPadding * 2 + 8, 160), 420)
        let paneTextStartCol = activePane.x + activePane.contentOffsetX
        let textEndCol = line.textCells
            .filter { $0.col >= paneTextStartCol && !$0.text.isEmpty }
            .map { $0.col + max($0.cols, 1) }
            .max() ?? max(paneTextStartCol + 1, cursor.col + 1)
        let x = min(
            CGFloat(textEndCol) * cellSize.width + 4,
            max(viewportSize.width - width - diagnosticOverlayEdgePadding, diagnosticOverlayEdgePadding)
        )
        let y = CGFloat(cursor.row) * cellSize.height + max(cellSize.height - diagnosticInlinePillHeight - 1, 1)
        return (
            diagnostic,
            max(diagnostics.count - 1, 0),
            CGRect(x: x, y: y, width: width, height: diagnosticInlinePillHeight)
        )
    }

    private func diagnosticInlineTextWidth(for text: String) -> CGFloat {
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 12, weight: .medium)
        ]
        return ceil((text as NSString).size(withAttributes: attributes).width)
    }
}

private struct EditorDiagnosticInlinePill: View {
    let diagnostic: EditorSnapshotDiagnostic
    let extraCount: Int
    let backgroundColor: NSColor

    var body: some View {
        HStack(spacing: 0) {
            Text(diagnosticInlineSummary(for: diagnostic, extraCount: extraCount))
                .font(.system(size: 12, weight: .medium))
                .lineLimit(1)
        }
        .foregroundStyle(diagnosticColor(for: diagnostic.severity))
        .padding(.horizontal, diagnosticInlineHorizontalPadding)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 7, style: .continuous)
                .fill(Color(nsColor: diagnosticBackgroundColor(base: backgroundColor, severity: diagnostic.severity)))
        )
        .overlay {
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .strokeBorder(diagnosticColor(for: diagnostic.severity).opacity(0.22), lineWidth: 1)
        }
    }
}

private func diagnosticInlineSummary(for diagnostic: EditorSnapshotDiagnostic, extraCount: Int) -> String {
    if extraCount > 0 {
        return "\(diagnostic.message) +\(extraCount)"
    }
    return diagnostic.message
}

private func diagnosticColor(for severity: EditorDiagnosticSeverity) -> Color {
    switch severity {
    case .error:
        return .red
    case .warning:
        return .orange
    case .information:
        return .blue
    case .hint:
        return .teal
    }
}

private func diagnosticBackgroundColor(base: NSColor, severity: EditorDiagnosticSeverity) -> NSColor {
    let accent: NSColor
    switch severity {
    case .error:
        accent = .systemRed
    case .warning:
        accent = .systemOrange
    case .information:
        accent = .systemBlue
    case .hint:
        accent = .systemTeal
    }
    return base.blended(withFraction: 0.12, of: accent) ?? accent.withAlphaComponent(0.14)
}
