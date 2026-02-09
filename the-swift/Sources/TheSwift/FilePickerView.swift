import SwiftUI

// MARK: - Data model

struct FilePickerSnapshot: Decodable {
    let active: Bool
    let query: String?
    let matchedCount: Int?
    let totalCount: Int?
    let scanning: Bool?
    let root: String?
    let items: [FilePickerItemSnapshot]?

    private enum CodingKeys: String, CodingKey {
        case active
        case query
        case matchedCount = "matched_count"
        case totalCount = "total_count"
        case scanning
        case root
        case items
    }
}

struct FilePickerItemSnapshot: Decodable, Identifiable {
    let id: Int
    let display: String
    let isDir: Bool
    let icon: String?
    let matchIndices: [Int]

    private enum CodingKeys: String, CodingKey {
        case display
        case isDir = "is_dir"
        case icon
        case matchIndices = "match_indices"
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        display = try container.decode(String.self, forKey: .display)
        isDir = (try? container.decode(Bool.self, forKey: .isDir)) ?? false
        icon = try? container.decode(String.self, forKey: .icon)
        matchIndices = (try? container.decode([Int].self, forKey: .matchIndices)) ?? []
        // id will be set by the parent during array mapping
        id = 0
    }

    init(id: Int, display: String, isDir: Bool, icon: String?, matchIndices: [Int]) {
        self.id = id
        self.display = display
        self.isDir = isDir
        self.icon = icon
        self.matchIndices = matchIndices
    }

    var filename: String {
        (display as NSString).lastPathComponent
    }

    var directory: String {
        let parent = (display as NSString).deletingLastPathComponent
        return parent.isEmpty ? "" : parent
    }

    var fileExtension: String {
        (filename as NSString).pathExtension.lowercased()
    }

    var filenameMatchIndices: [Int] {
        let filenameStart = max(0, display.count - filename.count)
        return matchIndices.compactMap { index in
            let local = index - filenameStart
            return (0..<filename.count).contains(local) ? local : nil
        }
    }

    var directoryMatchIndices: [Int] {
        let count = directory.count
        guard count > 0 else { return [] }
        return matchIndices.compactMap { index in
            (0..<count).contains(index) ? index : nil
        }
    }
}

// MARK: - File type icons

fileprivate func iconToken(for item: FilePickerItemSnapshot) -> String {
    if item.isDir {
        return "folder"
    }
    if let icon = item.icon, !icon.isEmpty {
        return icon
    }
    return "file_generic"
}

fileprivate func fileIcon(for item: FilePickerItemSnapshot) -> (svg: Image?, symbol: String) {
    let token = iconToken(for: item)
    let svg = PickerIconLoader.image(named: token)
    let symbol = filePickerSymbol(for: token) ?? fallbackFileIcon(forExtension: item.fileExtension)
    return (svg, symbol)
}

fileprivate func filePickerSymbol(for icon: String) -> String? {
    switch icon {
    case "archive":
        return "archivebox.fill"
    case "book", "file_markdown":
        return "doc.richtext.fill"
    case "c", "cpp", "go", "html", "java", "javascript", "json", "kotlin", "python", "sass", "swift", "typescript":
        return "chevron.left.forwardslash.chevron.right"
    case "css":
        return "paintbrush.fill"
    case "database":
        return "cylinder.fill"
    case "docker":
        return "shippingbox.fill"
    case "file_doc":
        return "doc.fill"
    case "file_git":
        return "point.topleft.down.curvedto.point.bottomright.up"
    case "file_lock", "lock":
        return "lock.fill"
    case "file_rust", "rust":
        return "r.square.fill"
    case "file_toml", "toml", "settings":
        return "doc.text.fill"
    case "image":
        return "photo.fill"
    case "nix":
        return "chevron.left.forwardslash.chevron.right"
    case "terminal", "tool_hammer":
        return "terminal.fill"
    case "folder", "folder_open", "folder_search":
        return "folder.fill"
    case "file_generic":
        return "doc.fill"
    default:
        return nil
    }
}

fileprivate func fallbackFileIcon(forExtension ext: String) -> String {
    switch ext {
    case "swift":
        return "chevron.left.forwardslash.chevron.right"
    case "rs":
        return "r.square.fill"
    case "js", "jsx":
        return "chevron.left.forwardslash.chevron.right"
    case "ts", "tsx":
        return "chevron.left.forwardslash.chevron.right"
    case "py":
        return "chevron.left.forwardslash.chevron.right"
    case "rb":
        return "chevron.left.forwardslash.chevron.right"
    case "go":
        return "chevron.left.forwardslash.chevron.right"
    case "md", "markdown":
        return "doc.richtext.fill"
    case "json", "yaml", "yml", "toml":
        return "doc.text.fill"
    case "html", "htm":
        return "chevron.left.forwardslash.chevron.right"
    case "css", "scss", "less":
        return "paintbrush.fill"
    case "png", "jpg", "jpeg", "gif", "svg", "ico", "webp":
        return "photo.fill"
    case "pdf":
        return "doc.fill"
    case "txt", "log":
        return "doc.text"
    case "sh", "bash", "zsh", "fish":
        return "terminal.fill"
    case "c", "h":
        return "chevron.left.forwardslash.chevron.right"
    case "cpp", "cc", "hpp", "cxx":
        return "chevron.left.forwardslash.chevron.right"
    case "java", "kt", "kts":
        return "chevron.left.forwardslash.chevron.right"
    case "xml":
        return "chevron.left.forwardslash.chevron.right"
    case "lock":
        return "lock.fill"
    default:
        return "doc.fill"
    }
}

// MARK: - Backdrop

struct FilePickerBackdrop: View {
    var body: some View {
        Color.black.opacity(0.15)
            .ignoresSafeArea()
    }
}

// MARK: - Main view

struct FilePickerView: View {
    let snapshot: FilePickerSnapshot
    let onQueryChange: (String) -> Void
    let onSubmit: (Int) -> Void
    let onClose: () -> Void

    private var items: [FilePickerItemSnapshot] {
        snapshot.items ?? []
    }

    private var matchedCount: Int {
        snapshot.matchedCount ?? 0
    }

    private var totalCount: Int {
        snapshot.totalCount ?? 0
    }

    private var isScanning: Bool {
        snapshot.scanning ?? false
    }

    var body: some View {
        PickerPanel(
            width: 600,
            maxListHeight: 380,
            placeholder: "Open file…",
            fontSize: 16,
            layout: .center,
            pageSize: 12,
            showTabNavigation: true,
            showPageNavigation: true,
            showCtrlCClose: true,
            itemCount: items.count,
            externalQuery: snapshot.query ?? "",
            externalSelectedIndex: nil,
            onQueryChange: onQueryChange,
            onSubmit: onSubmit,
            onClose: onClose,
            onSelectionChange: nil,
            leadingHeader: {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(.secondary)
            },
            trailingHeader: {
                statusText
            },
            itemContent: { index, isSelected, isHovered in
                fileRowContent(for: items[index], isSelected: isSelected)
            },
            emptyContent: {
                VStack(spacing: 8) {
                    Image(systemName: "doc.questionmark")
                        .font(.system(size: 24))
                        .foregroundStyle(.tertiary)
                    Text("No matching files")
                        .font(.system(size: 14))
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, minHeight: 80)
                .padding()
            }
        )
    }

    // MARK: - Status text

    @ViewBuilder
    private var statusText: some View {
        if items.isEmpty && !(snapshot.query ?? "").isEmpty && !isScanning {
            Text("No matches")
                .font(.system(size: 12))
                .foregroundStyle(.tertiary)
        } else if isScanning && matchedCount == 0 {
            ScanningText()
        } else {
            HStack(spacing: 6) {
                if matchedCount > 0 && matchedCount < totalCount {
                    Text("\(matchedCount) of \(totalCount)")
                        .font(.system(size: 12))
                        .foregroundStyle(.tertiary)
                } else if totalCount > 0 {
                    Text("\(totalCount) files")
                        .font(.system(size: 12))
                        .foregroundStyle(.tertiary)
                }
                if isScanning {
                    ScanningText()
                }
            }
        }
    }

    // MARK: - Row content

    private func fileRowContent(for item: FilePickerItemSnapshot, isSelected: Bool) -> some View {
        let icon = fileIcon(for: item)

        return HStack(spacing: 8) {
            Group {
                if let svg = icon.svg {
                    svg
                        .renderingMode(.template)
                } else {
                    Image(systemName: icon.symbol)
                        .symbolRenderingMode(.monochrome)
                }
            }
            .foregroundStyle(.secondary)
            .font(.system(size: 14, weight: .medium))
            .frame(width: 18, alignment: .center)

            VStack(alignment: .leading, spacing: 2) {
                highlightedText(
                    item.filename,
                    matchIndices: item.filenameMatchIndices,
                    baseColor: .primary,
                    highlightColor: .accentColor,
                    fontSize: 14,
                    baseWeight: item.isDir ? .semibold : .medium
                )
                    .lineLimit(1)

                if !item.directory.isEmpty {
                    highlightedText(
                        item.directory,
                        matchIndices: item.directoryMatchIndices,
                        baseColor: .secondary,
                        highlightColor: isSelected ? .primary : .accentColor,
                        fontSize: 12,
                        baseWeight: .regular
                    )
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }

            Spacer()
        }
    }

    private func highlightedText(
        _ text: String,
        matchIndices: [Int],
        baseColor: Color,
        highlightColor: Color,
        fontSize: CGFloat,
        baseWeight: Font.Weight
    ) -> Text {
        guard !text.isEmpty else { return Text("") }
        let indexSet = Set(matchIndices)

        func segmentText(_ segment: String, matched: Bool) -> Text {
            Text(segment)
                .font(.system(size: fontSize, weight: matched ? .bold : baseWeight))
                .foregroundColor(matched ? highlightColor : baseColor)
        }

        var result = Text("")
        var current = ""
        var currentMatched: Bool? = nil

        for (index, ch) in text.enumerated() {
            let matched = indexSet.contains(index)
            if currentMatched == nil {
                currentMatched = matched
            }
            if currentMatched != matched {
                result = result + segmentText(current, matched: currentMatched ?? false)
                current = ""
                currentMatched = matched
            }
            current.append(ch)
        }

        if !current.isEmpty {
            result = result + segmentText(current, matched: currentMatched ?? false)
        }

        return result
    }
}

// MARK: - Scanning indicator

fileprivate struct ScanningText: View {
    @State private var opacity: Double = 1.0

    var body: some View {
        Text("Scanning…")
            .font(.system(size: 12))
            .foregroundStyle(.tertiary)
            .opacity(opacity)
            .onAppear {
                withAnimation(.easeInOut(duration: 0.8).repeatForever(autoreverses: true)) {
                    opacity = 0.4
                }
            }
    }
}
