import Foundation
import SwiftUI
import class TheEditorFFIBridge.App

struct SignatureHelpPopupView: View {
    let snapshot: SignatureHelpSnapshot
    let cursorOrigin: CGPoint
    let theme: PopupChromeTheme
    let cellSize: CGSize
    let containerSize: CGSize
    let languageHint: String

    private let maxWidth: CGFloat = 480
    private let maxHeight: CGFloat = 300
    private let cornerRadius: CGFloat = 6
    private let headerHorizontalPadding: CGFloat = 14
    private let headerVerticalPadding: CGFloat = 12
    private let headerInnerSpacing: CGFloat = 10
    private let headerContainerInset: CGFloat = 8

    var body: some View {
        let placement = computePlacement()
        let hasDocs = contentHasDocs

        VStack(alignment: .leading, spacing: 0) {
            signatureView(width: placement.width)

            if hasDocs {
                Divider()
                    .background(theme.panelBorderColor.opacity(0.4))
                    .padding(.horizontal, headerContainerInset)

                docsView(width: placement.width, maxHeight: placement.height - placement.signatureHeight)
            }
        }
        .frame(width: placement.width)
        .fixedSize(horizontal: false, vertical: true)
        .frame(maxHeight: placement.height)
        .popupBackground(theme: theme, cornerRadius: cornerRadius)
        .offset(x: placement.x, y: placement.y)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .onDisappear {
            DispatchQueue.main.async {
                KeyCaptureFocusBridge.shared.reclaimActive()
            }
        }
    }

    // MARK: - Content parsing

    private struct SignatureSegment: Equatable {
        let text: String
        let isActive: Bool
    }

    private struct SignaturePresentation {
        let counter: String?
        let segments: [SignatureSegment]
        let plainText: String
    }

    /// The signature line content is always a code block. The markdown from Rust looks like:
    /// ```
    /// (1/2)
    ///
    /// ` ` `
    /// add(<<active>>int x<</active>>, int y) -> int
    /// ` ` `
    ///
    /// ---
    ///
    /// Documentation text here...
    /// ```
    ///
    /// We separate the full content from any trailing documentation after the `---` separator.

    private var contentHasDocs: Bool {
        let lines = snapshot.docsText.split(separator: "\n", omittingEmptySubsequences: false)
        return lines.contains { $0.trimmingCharacters(in: .whitespaces) == "---" }
    }

    private var signatureMarkdown: String {
        let lines = snapshot.docsText.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        var result: [String] = []
        for line in lines {
            if line.trimmingCharacters(in: .whitespaces) == "---" {
                break
            }
            result.append(line)
        }
        // Trim trailing empty lines
        while result.last?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty == true {
            result.removeLast()
        }
        return result.joined(separator: "\n")
    }

    private var docsMarkdown: String {
        let lines = snapshot.docsText.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        guard let separatorIndex = lines.firstIndex(where: { $0.trimmingCharacters(in: .whitespaces) == "---" }) else {
            return ""
        }
        let remaining = lines.suffix(from: lines.index(after: separatorIndex))
        let trimmed = remaining.drop(while: { $0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty })
        return trimmed.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var signaturePresentation: SignaturePresentation {
        let lines = signatureMarkdown
            .split(separator: "\n", omittingEmptySubsequences: false)
            .map(String.init)

        var index = 0
        while index < lines.count, lines[index].trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            index += 1
        }

        var counter: String?
        if index < lines.count, isSignatureCounter(lines[index]) {
            counter = lines[index].trimmingCharacters(in: .whitespacesAndNewlines)
            index += 1
        }

        while index < lines.count, lines[index].trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            index += 1
        }

        let signature: String
        if index < lines.count, lines[index].trimmingCharacters(in: .whitespaces).hasPrefix("```") {
            index += 1
            var codeLines: [String] = []
            while index < lines.count, !lines[index].trimmingCharacters(in: .whitespaces).hasPrefix("```") {
                codeLines.append(lines[index])
                index += 1
            }
            signature = codeLines.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
        } else {
            signature = lines.suffix(from: index).joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
        }

        let segments = parseSignatureSegments(from: signature)
        let plainText = segments.map(\.text).joined()

        return SignaturePresentation(
            counter: counter,
            segments: segments.isEmpty ? [SignatureSegment(text: signature, isActive: false)] : segments,
            plainText: plainText.isEmpty ? signature : plainText
        )
    }

    // MARK: - Subviews

    @ViewBuilder
    private func signatureView(width: CGFloat) -> some View {
        let presentation = signaturePresentation

        HStack(alignment: .top, spacing: headerInnerSpacing) {
            if let counter = presentation.counter {
                Text(counter)
                    .font(FontLoader.uiFont(size: 10.5, weight: .semibold))
                    .foregroundStyle(theme.secondaryTextColor.opacity(0.96))
                    .padding(.horizontal, 8)
                    .padding(.vertical, 5)
                    .background(
                        Capsule()
                            .fill(theme.hoveredBackgroundColor.opacity(0.88))
                    )
                    .overlay(
                        Capsule()
                            .stroke(theme.panelBorderColor.opacity(0.38), lineWidth: 0.5)
                    )
            }

            signatureText(for: presentation.segments)
                .font(FontLoader.bufferFont(size: 13))
                .lineSpacing(2)
                .frame(maxWidth: .infinity, alignment: .leading)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.horizontal, headerHorizontalPadding)
        .padding(.vertical, headerVerticalPadding)
        .frame(width: width - (headerContainerInset * 2), alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(theme.hoveredBackgroundColor.opacity(0.18))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(theme.panelBorderColor.opacity(0.28), lineWidth: 0.5)
        )
        .padding(headerContainerInset)
        .frame(width: width, alignment: .leading)
    }

    @ViewBuilder
    private func docsView(width: CGFloat, maxHeight: CGFloat) -> some View {
        let text = docsMarkdown
        if !text.isEmpty {
            let height = min(max(40, estimatedDocsHeight(for: text, width: width)), max(40, maxHeight))
            CompletionDocsTextView(
                docs: text,
                width: width,
                height: height,
                languageHint: languageHint,
                theme: theme.docsTheme
            )
            .frame(width: width, height: height)
        }
    }

    // MARK: - Sizing

    private func estimatedSignatureWidth() -> CGFloat {
        // Strip the markers and code fences to measure the actual signature text.
        let cleaned = signatureMarkdown
            .replacingOccurrences(of: "```", with: "")
            .replacingOccurrences(of: "<<the-editor-active-param>>", with: "")
            .replacingOccurrences(of: "<</the-editor-active-param>>", with: "")
        let longestLine = cleaned
            .split(separator: "\n", omittingEmptySubsequences: true)
            .map { $0.trimmingCharacters(in: .whitespaces).count }
            .max() ?? 20
        // ~7.5px per monospaced character at code font size + padding
        return CGFloat(longestLine) * 7.5 + 32
    }

    private func estimatedDocsHeight(for docs: String, width: CGFloat) -> CGFloat {
        let columns = max(20, Int((width - 24) / 6.8))
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
        return CGFloat(min(12, max(1, wrappedLines))) * 18 + 16
    }

    // MARK: - Placement

    private struct Placement {
        let x: CGFloat
        let y: CGFloat
        let width: CGFloat
        let height: CGFloat
        let signatureHeight: CGFloat
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
        max(1, Int(ceil(width / max(1, cellSize.width))))
    }

    private func cellsForHeight(_ height: CGFloat) -> Int {
        max(1, Int(ceil(height / max(1, cellSize.height))))
    }

    private func pixelsForCols(_ cols: Int) -> CGFloat {
        CGFloat(max(0, cols)) * max(1, cellSize.width)
    }

    private func pixelsForRows(_ rows: Int) -> CGFloat {
        CGFloat(max(0, rows)) * max(1, cellSize.height)
    }

    private func computePlacement() -> Placement {
        let areaWidth = max(1, containerSize.width)
        let areaHeight = max(1, containerSize.height)
        let areaCols = max(1, Int(floor(areaWidth / max(1, cellSize.width))))
        let areaRows = max(1, Int(floor(areaHeight / max(1, cellSize.height))))
        let cursorCol = Int(floor(cursorOrigin.x / max(1, cellSize.width)))
        let cursorRow = Int(floor(cursorOrigin.y / max(1, cellSize.height)))

        // Width: fit the signature, clamped to max.
        let sigWidth = estimatedSignatureWidth()
        let panelWidth = min(max(160, sigWidth), min(maxWidth, areaWidth))

        // Height: signature + optional docs.
        let signatureHeight = estimatedSignatureHeight(for: panelWidth)
        var panelHeight = signatureHeight
        if contentHasDocs {
            let docsText = docsMarkdown
            if !docsText.isEmpty {
                panelHeight += 1 // divider
                panelHeight += estimatedDocsHeight(for: docsText, width: panelWidth)
            }
        }
        panelHeight = min(panelHeight, min(maxHeight, areaHeight))

        let widthCells = min(areaCols, cellsForWidth(panelWidth))
        let heightCells = min(areaRows, cellsForHeight(panelHeight))

        let layoutJSON = App.signature_help_popup_layout_json(
            UInt(areaCols),
            UInt(areaRows),
            Int64(cursorCol),
            Int64(cursorRow),
            UInt(widthCells),
            UInt(heightCells)
        ).toString()

        let placementData = Data(layoutJSON.utf8)
        let shared = (try? JSONDecoder().decode(SharedPlacement.self, from: placementData))
            ?? SharedPlacement(
                panel: SharedRect(x: 0, y: 0, width: widthCells, height: heightCells)
            )

        return Placement(
            x: pixelsForCols(shared.panel.x),
            y: pixelsForRows(shared.panel.y),
            width: pixelsForCols(shared.panel.width),
            height: pixelsForRows(shared.panel.height),
            signatureHeight: signatureHeight
        )
    }

    private func estimatedSignatureHeight(for width: CGFloat) -> CGFloat {
        let presentation = signaturePresentation
        let badgeWidth = presentation.counter == nil ? 0 : 54
        let contentWidth = max(
            120,
            width
                - (headerContainerInset * 2)
                - (headerHorizontalPadding * 2)
                - CGFloat(badgeWidth)
                - (presentation.counter == nil ? 0 : headerInnerSpacing)
        )
        let columns = max(18, Int(floor(contentWidth / 7.4)))
        let lineCount = max(1, Int(ceil(Double(max(1, presentation.plainText.count)) / Double(columns))))
        let contentHeight = CGFloat(lineCount) * 18 + (headerVerticalPadding * 2)
        return max(48, contentHeight + (headerContainerInset * 2))
    }

    private func isSignatureCounter(_ line: String) -> Bool {
        let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.first == "(", trimmed.last == ")" else {
            return false
        }

        let body = trimmed.dropFirst().dropLast()
        let parts = body.split(separator: "/", omittingEmptySubsequences: false)
        guard parts.count == 2 else {
            return false
        }

        return parts.allSatisfy { !$0.isEmpty && $0.allSatisfy(\.isNumber) }
    }

    private func parseSignatureSegments(from raw: String) -> [SignatureSegment] {
        let startMarker = "<<the-editor-active-param>>"
        let endMarker = "<</the-editor-active-param>>"

        guard !raw.isEmpty else {
            return []
        }

        var segments: [SignatureSegment] = []
        var remainder = raw[...]
        var active = false

        while !remainder.isEmpty {
            let marker = active ? endMarker : startMarker
            if let range = remainder.range(of: marker) {
                let text = String(remainder[..<range.lowerBound])
                if !text.isEmpty {
                    appendSegment(text, isActive: active, to: &segments)
                }
                remainder = remainder[range.upperBound...]
                active.toggle()
            } else {
                let text = String(remainder)
                if !text.isEmpty {
                    appendSegment(text, isActive: active, to: &segments)
                }
                break
            }
        }

        return segments
    }

    private func appendSegment(_ text: String, isActive: Bool, to segments: inout [SignatureSegment]) {
        guard !text.isEmpty else {
            return
        }
        if let last = segments.last, last.isActive == isActive {
            segments[segments.count - 1] = SignatureSegment(text: last.text + text, isActive: isActive)
        } else {
            segments.append(SignatureSegment(text: text, isActive: isActive))
        }
    }

    private func signatureText(for segments: [SignatureSegment]) -> Text {
        segments.reduce(Text("")) { partial, segment in
            partial + signatureTextSegment(segment)
        }
    }

    private func signatureTextSegment(_ segment: SignatureSegment) -> Text {
        let text = Text(verbatim: segment.text)
        if segment.isActive {
            return text
                .fontWeight(.semibold)
                .foregroundColor(theme.accentColor.opacity(0.96))
                .underline(true, color: theme.accentColor.opacity(0.55))
        }
        return text.foregroundColor(theme.primaryTextColor.opacity(0.94))
    }
}
