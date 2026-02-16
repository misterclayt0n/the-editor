import Foundation
import SwiftUI
import class TheEditorFFIBridge.App

struct SignatureHelpPopupView: View {
    let snapshot: SignatureHelpSnapshot
    let cursorOrigin: CGPoint
    let cellSize: CGSize
    let containerSize: CGSize
    let languageHint: String

    private let maxWidth: CGFloat = 480
    private let maxHeight: CGFloat = 300
    private let cornerRadius: CGFloat = 6

    var body: some View {
        let placement = computePlacement()
        let hasDocs = contentHasDocs

        VStack(alignment: .leading, spacing: 0) {
            signatureView(width: placement.width)

            if hasDocs {
                Divider()
                    .opacity(0.4)

                docsView(width: placement.width, maxHeight: placement.height - signatureHeight)
            }
        }
        .frame(width: placement.width)
        .fixedSize(horizontal: false, vertical: true)
        .frame(maxHeight: placement.height)
        .glassBackground(cornerRadius: cornerRadius)
        .offset(x: placement.x, y: placement.y)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .onDisappear {
            DispatchQueue.main.async {
                KeyCaptureFocusBridge.shared.reclaimActive()
            }
        }
    }

    // MARK: - Content parsing

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

    // MARK: - Subviews

    private let signatureHeight: CGFloat = 36

    @ViewBuilder
    private func signatureView(width: CGFloat) -> some View {
        CompletionDocsTextView(
            docs: signatureMarkdown,
            width: width,
            height: signatureHeight,
            languageHint: languageHint
        )
        .frame(width: width, height: signatureHeight)
        .clipped()
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
                languageHint: languageHint
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
            height: pixelsForRows(shared.panel.height)
        )
    }
}
