import SwiftUI

// MARK: - Shared statusline parsing

private struct EditorToolbarSnapshotParts {
    let modeName: String
    let filename: String
    let isModified: Bool
    let leftIcon: (svg: Image?, symbol: String)?

    init(snapshot: StatuslineSnapshot) {
        modeName = String(snapshot.left.split(whereSeparator: { $0.isWhitespace }).first ?? "")
        filename = snapshot.left
            .split(whereSeparator: { $0.isWhitespace })
            .dropFirst()
            .map(String.init)
            .joined(separator: " ")
            .replacingOccurrences(of: "[+]", with: "")
            .replacingOccurrences(of: "[RO]", with: "")
            .trimmingCharacters(in: .whitespaces)
        isModified = snapshot.left.contains("[+]")
        if let icon = snapshot.leftIcon, !icon.isEmpty {
            leftIcon = (PickerIconLoader.image(named: icon), filePickerSymbol(for: icon) ?? "doc.fill")
        } else {
            leftIcon = nil
        }
    }
}

// MARK: - Toolbar Leading (left: mode-only badge)

struct EditorToolbarLeading: View {
    let snapshot: StatuslineSnapshot

    private var parts: EditorToolbarSnapshotParts { EditorToolbarSnapshotParts(snapshot: snapshot) }

    private var modeColor: Color {
        switch parts.modeName {
        case "NOR": return Color(nsColor: .tertiaryLabelColor)
        case "INS": return Color(nsColor: .systemBlue)
        case "SEL": return Color(nsColor: .systemPurple)
        case "CMD": return Color(nsColor: .secondaryLabelColor)
        case "COL": return Color(nsColor: .systemOrange)
        case "REM": return Color(nsColor: .systemRed)
        default:    return Color(nsColor: .tertiaryLabelColor)
        }
    }

    var body: some View {
        Text(parts.modeName)
            .font(FontLoader.uiFont(size: 11).weight(.semibold))
            .foregroundStyle(modeColor)
            .lineLimit(1)
            .padding(.horizontal, 8)
    }
}

// MARK: - Toolbar VCS (left: branch badge)

struct EditorToolbarVCS: View {
    let snapshot: StatuslineSnapshot

    static func text(from snapshot: StatuslineSnapshot) -> String? {
        let vcsRaw = snapshot.rightSegments
            .first {
                let text = $0.text.trimmingCharacters(in: .whitespacesAndNewlines)
                let emphasis = $0.style?.emphasis ?? .normal
                return emphasis == .muted && !text.isEmpty && !text.hasPrefix("lsp:")
            }?
            .text
            .trimmingCharacters(in: .whitespacesAndNewlines)

        guard let vcsRaw, !vcsRaw.isEmpty else {
            return nil
        }
        return normalizedVcsText(vcsRaw)
    }

    private static func normalizedVcsText(_ raw: String) -> String? {
        let tokens = raw.split(whereSeparator: { $0.isWhitespace })
        guard !tokens.isEmpty else { return nil }
        if tokens.count >= 2,
           let firstScalar = tokens[0].unicodeScalars.first,
           !firstScalar.isASCII {
            let value = tokens.dropFirst().joined(separator: " ")
            return value.isEmpty ? nil : value
        }
        return raw
    }

    var body: some View {
        if let text = Self.text(from: snapshot) {
            Label {
                Text(text)
                    .lineLimit(1)
            } icon: {
                Image(systemName: "arrow.triangle.branch")
            }
            .font(FontLoader.uiFont(size: 11).weight(.medium))
            .foregroundStyle(.secondary)
            .labelStyle(.titleAndIcon)
            .padding(.horizontal, 8)
        }
    }
}

// MARK: - Toolbar Title (buffer icon + filename outside mode badge)

struct EditorToolbarTitle: View {
    let snapshot: StatuslineSnapshot
    let fallbackTitle: String

    private var parts: EditorToolbarSnapshotParts { EditorToolbarSnapshotParts(snapshot: snapshot) }

    private var titleText: String {
        if !parts.filename.isEmpty {
            return parts.filename
        }
        return fallbackTitle
    }

    var body: some View {
        HStack(spacing: 6) {
            if let icon = parts.leftIcon {
                Group {
                    if let svg = icon.svg {
                        svg.renderingMode(.template)
                    } else {
                        Image(systemName: icon.symbol)
                            .symbolRenderingMode(.monochrome)
                    }
                }
                .foregroundStyle(.secondary)
                .font(FontLoader.uiFont(size: 12))
            }

            Text(titleText)
                .font(FontLoader.uiFont(size: 13).weight(.semibold))
                .foregroundStyle(.primary)
                .lineLimit(1)
                .truncationMode(.middle)

            if parts.isModified {
                Circle()
                    .fill(Color(nsColor: .tertiaryLabelColor))
                    .frame(width: 5, height: 5)
            }
        }
    }
}

// MARK: - Toolbar Trailing (right: LSP status + cursor position)

struct EditorToolbarTrailing: View {
    let snapshot: StatuslineSnapshot
    let pendingKeys: [String]

    private var filteredSegments: [(text: String, emphasis: UiEmphasisSnapshot)] {
        let pending = pendingKeys.isEmpty ? nil : pendingKeys.joined(separator: " ")
        return snapshot.rightSegments.compactMap { span in
            guard span.text != pending, !span.text.isEmpty else { return nil }
            return (text: span.text, emphasis: span.style?.emphasis ?? .normal)
        }
    }

    private var lspStatus: String? {
        filteredSegments.first { $0.text.hasPrefix("lsp:") && $0.emphasis == .muted }?.text
    }

    private var cursorPosition: String? {
        filteredSegments.last {
            $0.emphasis != .muted
                && $0.emphasis != .strong
                && $0.text.range(of: #"^\d+:\d+$"#, options: .regularExpression) != nil
        }?.text
    }

    private var cursorPick: String? {
        filteredSegments.first {
            $0.emphasis == .strong
                && ($0.text.lowercased().hasPrefix("collapse ") || $0.text.lowercased().hasPrefix("remove "))
        }?.text
    }

    var body: some View {
        HStack(spacing: 12) {
            if let pick = cursorPick {
                let isRemove = pick.lowercased().hasPrefix("remove ")
                let tint = isRemove ? Color(nsColor: .systemRed) : Color(nsColor: .systemOrange)
                Text(pick)
                    .font(FontLoader.uiFont(size: 11).weight(.semibold))
                    .foregroundStyle(tint)
                    .lineLimit(1)
            }

            if let lsp = lspStatus {
                Text(lsp)
                    .font(FontLoader.uiFont(size: 11))
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
            }

            if let pos = cursorPosition {
                Text(pos)
                    .font(FontLoader.uiFont(size: 11).monospacedDigit())
                    .foregroundStyle(.tertiary)
            }
        }
        .padding(.horizontal, 8)
    }
}
