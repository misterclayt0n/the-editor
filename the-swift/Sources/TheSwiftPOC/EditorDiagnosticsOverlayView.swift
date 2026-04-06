import AppKit
import SwiftUI

private let diagnosticOverlayEdgePadding: CGFloat = 8
private let diagnosticInlinePillHeight: CGFloat = 24
private let diagnosticInlineHorizontalPadding: CGFloat = 10
private let diagnosticPopoverPreferredWidth: CGFloat = 420

extension EditorSurfaceController {
    var focusedDiagnostics: [EditorSnapshotDiagnostic] {
        guard let scene else { return [] }
        if !hoveredDiagnosticIndices.isEmpty {
            return hoveredDiagnosticIndices
                .compactMap(scene.diagnostic(index:))
                .sorted(by: diagnosticSort)
        }
        guard let cursor = scene.primaryCursor,
              let docLine = scene.line(atRow: cursor.row)?.docLine else {
            return []
        }
        return scene.diagnostics(onDocumentLine: docLine)
    }

    var currentCursorLineDiagnostics: [EditorSnapshotDiagnostic] {
        guard let scene,
              let cursor = scene.primaryCursor,
              let docLine = scene.line(atRow: cursor.row)?.docLine else {
            return []
        }
        return scene.diagnostics(onDocumentLine: docLine)
    }
}

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

                if let scene = controller.scene,
                   let popover = hoveredPopover(scene: scene, viewportSize: geometry.size) {
                    EditorPopoverPanel(frame: popover.frame, backgroundColor: controller.chrome.backgroundColor) {
                        EditorDiagnosticPopoverContent(diagnostics: popover.diagnostics)
                    }
                    .allowsHitTesting(false)
                    .zIndex(5)
                }
            }
            .frame(width: geometry.size.width, height: geometry.size.height, alignment: .topLeading)
        }
        .allowsHitTesting(false)
    }

    private func currentLinePill(scene: EditorRenderScene, viewportSize: CGSize) -> (diagnostic: EditorSnapshotDiagnostic, extraCount: Int, frame: CGRect)? {
        guard let cursor = scene.primaryCursor,
              let line = scene.line(atRow: cursor.row),
              let docLine = line.docLine else {
            return nil
        }
        let diagnostics = scene.diagnostics(onDocumentLine: docLine)
        guard let diagnostic = diagnostics.first else { return nil }

        let cellSize = scene.info.surfaceMetrics.cellSizePoints
        let pillText = diagnosticInlineSummary(for: diagnostic, extraCount: max(diagnostics.count - 1, 0))
        let measuredTextWidth = diagnosticInlineTextWidth(for: pillText)
        let width = min(max(measuredTextWidth + diagnosticInlineHorizontalPadding * 2 + 22, 180), 420)
        let textEndCol = line.textCells
            .filter { $0.col >= scene.info.contentOffsetX && !$0.text.isEmpty }
            .map { $0.col + max($0.cols, 1) }
            .max() ?? max(scene.info.contentOffsetX + 1, cursor.col + 1)
        let x = min(
            CGFloat(textEndCol) * cellSize.width + 8,
            max(viewportSize.width - width - diagnosticOverlayEdgePadding, diagnosticOverlayEdgePadding)
        )
        let y = CGFloat(cursor.row) * cellSize.height + max((cellSize.height - diagnosticInlinePillHeight) * 0.5, 0)
        return (
            diagnostic,
            max(diagnostics.count - 1, 0),
            CGRect(x: x, y: y, width: width, height: diagnosticInlinePillHeight)
        )
    }

    private func hoveredPopover(scene: EditorRenderScene, viewportSize: CGSize) -> (diagnostics: [EditorSnapshotDiagnostic], frame: CGRect)? {
        let diagnostics = controller.hoveredDiagnosticIndices
            .compactMap(scene.diagnostic(index:))
            .sorted(by: diagnosticSort)
        guard !diagnostics.isEmpty,
              let anchorFrame = controller.hoveredDiagnosticAnchorFrame else {
            return nil
        }
        let width = min(diagnosticPopoverPreferredWidth, max(viewportSize.width - diagnosticOverlayEdgePadding * 2, 240))
        let rowHeight: CGFloat = 50
        let height = min(max(56 + CGFloat(diagnostics.count) * rowHeight, 72), 240)
        let preferredX = anchorFrame.maxX + 10
        let x: CGFloat
        if preferredX + width + diagnosticOverlayEdgePadding <= viewportSize.width {
            x = preferredX
        } else {
            x = max(anchorFrame.minX - width - 10, diagnosticOverlayEdgePadding)
        }
        let y = min(
            max(anchorFrame.minY - 6, diagnosticOverlayEdgePadding),
            max(viewportSize.height - height - diagnosticOverlayEdgePadding, diagnosticOverlayEdgePadding)
        )
        return (diagnostics, CGRect(x: x, y: y, width: width, height: height))
    }

    private func diagnosticInlineTextWidth(for text: String) -> CGFloat {
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 12, weight: .medium)
        ]
        return ceil((text as NSString).size(withAttributes: attributes).width)
    }
}

struct DiagnosticStatusAccessoryView: View {
    let diagnostics: [EditorSnapshotDiagnostic]

    var body: some View {
        guard let diagnostic = diagnostics.first else {
            return AnyView(EmptyView())
        }
        return AnyView(
            HStack(spacing: 6) {
                Image(systemName: diagnostic.severity.symbolName)
                    .font(.system(size: 10, weight: .semibold))
                Text(diagnosticInlineSummary(for: diagnostic, extraCount: max(diagnostics.count - 1, 0)))
                    .font(.system(size: 11, weight: .medium))
                    .lineLimit(1)
            }
            .foregroundStyle(diagnosticColor(for: diagnostic.severity))
            .padding(.horizontal, 10)
            .padding(.vertical, 5)
            .background(
                Capsule(style: .continuous)
                    .fill(Color(nsColor: diagnosticBackgroundColor(base: NSColor.windowBackgroundColor, severity: diagnostic.severity)))
            )
        )
    }
}

private struct EditorDiagnosticInlinePill: View {
    let diagnostic: EditorSnapshotDiagnostic
    let extraCount: Int
    let backgroundColor: NSColor

    var body: some View {
        HStack(spacing: 7) {
            Image(systemName: diagnostic.severity.symbolName)
                .font(.system(size: 10, weight: .semibold))
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
            RoundedRectangle(cornerRadius: 7, style: .continuous)
                .strokeBorder(diagnosticColor(for: diagnostic.severity).opacity(0.28), lineWidth: 1)
        }
        .shadow(color: .black.opacity(0.08), radius: 4, y: 2)
    }
}

private struct EditorDiagnosticPopoverContent: View {
    let diagnostics: [EditorSnapshotDiagnostic]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                ForEach(diagnostics) { diagnostic in
                    VStack(alignment: .leading, spacing: 8) {
                        HStack(alignment: .firstTextBaseline, spacing: 8) {
                            Image(systemName: diagnostic.severity.symbolName)
                                .font(.system(size: 11, weight: .semibold))
                                .foregroundStyle(diagnosticColor(for: diagnostic.severity))
                            Text(diagnostic.message)
                                .font(.system(size: 13, weight: .medium))
                                .foregroundStyle(.primary)
                                .fixedSize(horizontal: false, vertical: true)
                        }

                        HStack(spacing: 6) {
                            if let source = diagnostic.source, !source.isEmpty {
                                EditorDiagnosticBadge(text: source)
                            }
                            if let code = diagnostic.code, !code.isEmpty {
                                EditorDiagnosticBadge(text: code)
                            }
                            EditorDiagnosticBadge(text: diagnosticSeverityLabel(diagnostic.severity))
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    if diagnostic.id != diagnostics.last?.id {
                        Divider()
                    }
                }
            }
            .padding(14)
        }
        .scrollIndicators(.visible)
    }
}

private struct EditorDiagnosticBadge: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.system(size: 11, weight: .medium, design: .monospaced))
            .foregroundStyle(.secondary)
            .padding(.horizontal, 7)
            .padding(.vertical, 4)
            .background(
                Capsule(style: .continuous)
                    .fill(Color.primary.opacity(0.08))
            )
    }
}

private func diagnosticInlineSummary(for diagnostic: EditorSnapshotDiagnostic, extraCount: Int) -> String {
    if extraCount > 0 {
        return "\(diagnostic.message) +\(extraCount)"
    }
    return diagnostic.message
}

private func diagnosticSort(_ lhs: EditorSnapshotDiagnostic, _ rhs: EditorSnapshotDiagnostic) -> Bool {
    if lhs.severity.sortRank != rhs.severity.sortRank {
        return lhs.severity.sortRank > rhs.severity.sortRank
    }
    if lhs.startLine != rhs.startLine {
        return lhs.startLine < rhs.startLine
    }
    if lhs.startCharacter != rhs.startCharacter {
        return lhs.startCharacter < rhs.startCharacter
    }
    return lhs.index < rhs.index
}

private func diagnosticSeverityLabel(_ severity: EditorDiagnosticSeverity) -> String {
    switch severity {
    case .error:
        return "Error"
    case .warning:
        return "Warning"
    case .information:
        return "Info"
    case .hint:
        return "Hint"
    }
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
