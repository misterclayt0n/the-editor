import SwiftUI

struct CommandPaletteItemSnapshot: Identifiable {
    let id: Int
    let title: String
    let subtitle: String?
    let description: String?
    let shortcut: String?
    let badge: String?
    let leadingIcon: String?
    let leadingColor: Color?
    let symbols: [String]
    let emphasis: Bool
}

enum CommandPaletteLayout: Int {
    case floating = 0
    case bottom = 1
    case top = 2
    case custom = 3

    static func from(rawValue: UInt8) -> CommandPaletteLayout {
        CommandPaletteLayout(rawValue: Int(rawValue)) ?? .floating
    }

    var pickerLayout: PickerPanelLayout {
        switch self {
        case .bottom: return .bottom
        case .top: return .top
        case .floating, .custom: return .center
        }
    }
}

struct CommandPaletteSnapshot {
    let isOpen: Bool
    let query: String
    let selectedIndex: Int?
    let items: [CommandPaletteItemSnapshot]
    let layout: CommandPaletteLayout

    static let closed = CommandPaletteSnapshot(
        isOpen: false,
        query: "",
        selectedIndex: nil,
        items: [],
        layout: .floating
    )
}

struct CommandPaletteView: View {
    let snapshot: CommandPaletteSnapshot
    let onSelect: (Int) -> Void
    let onSubmit: (Int?) -> Void
    let onClose: () -> Void
    let onQueryChange: (String) -> Void

    var body: some View {
        if !snapshot.isOpen {
            EmptyView()
        } else {
            let layout = snapshot.layout == .custom ? .floating : snapshot.layout
            PickerPanel(
                width: 520,
                maxListHeight: 220,
                placeholder: "Execute a command…",
                fontSize: 18,
                layout: layout.pickerLayout,
                pageSize: 12,
                showTabNavigation: false,
                showPageNavigation: false,
                showCtrlCClose: false,
                autoSelectFirstItem: false,
                itemCount: snapshot.items.count,
                externalQuery: snapshot.query,
                externalSelectedIndex: snapshot.selectedIndex,
                onQueryChange: onQueryChange,
                onSubmit: onSubmit,
                onClose: onClose,
                onSelectionChange: onSelect,
                leadingHeader: {
                    Rectangle()
                        .fill(Color.accentColor)
                        .frame(width: 2, height: 18)
                        .cornerRadius(1)
                },
                trailingHeader: { EmptyView() },
                itemContent: { index, isSelected, isHovered in
                    paletteRowContent(for: snapshot.items[index])
                },
                emptyContent: {
                    Text("No matches")
                        .foregroundStyle(.secondary)
                        .padding()
                }
            )
        }
    }

    private func paletteRowContent(for item: CommandPaletteItemSnapshot) -> some View {
        HStack(spacing: 8) {
            if let color = item.leadingColor {
                Circle()
                    .fill(color)
                    .frame(width: 8, height: 8)
            }

            if let icon = item.leadingIcon, !icon.isEmpty {
                Image(systemName: icon)
                    .foregroundStyle(item.emphasis ? Color.accentColor : .secondary)
                    .font(FontLoader.uiFont(size: 14).weight(.medium))
            }

            VStack(alignment: .leading, spacing: 2) {
                Text(item.title)
                    .font(FontLoader.uiFont(size: 14).weight(item.emphasis ? .semibold : .medium))
                    .foregroundColor(.primary)

                if let subtitle = item.subtitle, !subtitle.isEmpty {
                    Text(subtitle)
                        .font(FontLoader.uiFont(size: 12))
                        .foregroundStyle(.secondary)
                } else if let description = item.description, !description.isEmpty {
                    Text(description)
                        .font(FontLoader.uiFont(size: 12))
                        .foregroundStyle(.secondary)
                }
            }

            Spacer()

            if let badge = item.badge, !badge.isEmpty {
                Text(badge)
                    .font(FontLoader.uiFont(size: 11).weight(.semibold))
                    .padding(.horizontal, 7)
                    .padding(.vertical, 3)
                    .background(
                        Capsule().fill(Color.accentColor.opacity(0.15))
                    )
                    .foregroundStyle(Color.accentColor)
            }

            if !item.symbols.isEmpty {
                ShortcutSymbolsView(symbols: item.symbols)
                    .foregroundStyle(.secondary)
            } else if let shortcut = item.shortcut, !shortcut.isEmpty {
                ShortcutSymbolsView(symbols: [shortcut])
                    .foregroundStyle(.secondary)
            }
        }
        .help(item.description ?? "")
    }
}

fileprivate struct ShortcutSymbolsView: View {
    let symbols: [String]

    var body: some View {
        HStack(spacing: 1) {
            ForEach(symbols, id: \.self) { symbol in
                Text(symbol)
                    .frame(minWidth: 13)
            }
        }
    }
}
