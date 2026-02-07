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

    private enum CodingKeys: String, CodingKey {
        case display
        case isDir = "is_dir"
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        display = try container.decode(String.self, forKey: .display)
        isDir = (try? container.decode(Bool.self, forKey: .isDir)) ?? false
        // id will be set by the parent during array mapping
        id = 0
    }

    init(id: Int, display: String, isDir: Bool) {
        self.id = id
        self.display = display
        self.isDir = isDir
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
}

// MARK: - File type icons

fileprivate func fileIcon(for item: FilePickerItemSnapshot) -> (symbol: String, color: Color) {
    if item.isDir {
        return ("folder.fill", .secondary)
    }
    switch item.fileExtension {
    case "swift":
        return ("swift", .orange)
    case "rs":
        return ("gearshape.2.fill", Color(red: 0.72, green: 0.35, blue: 0.16))
    case "js", "jsx":
        return ("doc.text.fill", .yellow)
    case "ts", "tsx":
        return ("doc.text.fill", .blue)
    case "py":
        return ("doc.text.fill", Color(red: 0.2, green: 0.6, blue: 0.85))
    case "rb":
        return ("doc.text.fill", .red)
    case "go":
        return ("doc.text.fill", .cyan)
    case "md", "markdown":
        return ("doc.richtext.fill", .gray)
    case "json", "yaml", "yml", "toml":
        return ("gearshape.fill", .gray)
    case "html", "htm":
        return ("globe", .orange)
    case "css", "scss", "less":
        return ("paintbrush.fill", .pink)
    case "png", "jpg", "jpeg", "gif", "svg", "ico", "webp":
        return ("photo.fill", .green)
    case "pdf":
        return ("doc.fill", .red)
    case "txt", "log":
        return ("doc.text", .secondary)
    case "sh", "bash", "zsh", "fish":
        return ("terminal.fill", .green)
    case "c", "h":
        return ("doc.text.fill", Color(red: 0.3, green: 0.5, blue: 0.8))
    case "cpp", "cc", "hpp", "cxx":
        return ("doc.text.fill", Color(red: 0.3, green: 0.5, blue: 0.8))
    case "java", "kt", "kts":
        return ("doc.text.fill", .orange)
    case "xml":
        return ("chevron.left.forwardslash.chevron.right", .gray)
    case "lock":
        return ("lock.fill", .gray)
    default:
        return ("doc.fill", .secondary)
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
                fileRowContent(for: items[index])
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

    private func fileRowContent(for item: FilePickerItemSnapshot) -> some View {
        let (iconName, iconColor) = fileIcon(for: item)

        return HStack(spacing: 8) {
            Image(systemName: iconName)
                .foregroundStyle(iconColor)
                .font(.system(size: 14, weight: .medium))
                .frame(width: 18, alignment: .center)

            VStack(alignment: .leading, spacing: 2) {
                Text(item.filename)
                    .font(.system(size: 14, weight: item.isDir ? .semibold : .medium))
                    .foregroundColor(.primary)
                    .lineLimit(1)

                if !item.directory.isEmpty {
                    Text(item.directory)
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }

            Spacer()
        }
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
