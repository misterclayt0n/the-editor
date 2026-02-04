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
    let onSubmit: (Int) -> Void
    let onClose: () -> Void
    let onQueryChange: (String) -> Void

    @State private var hoveredIndex: Int? = nil
    @State private var query: String = ""
    @State private var selectedIndex: Int? = nil
    @FocusState private var isTextFieldFocused: Bool

    private let maxListHeight: CGFloat = 220
    private let paletteWidth: CGFloat = 520
    private let backgroundColor: Color = Color(nsColor: .windowBackgroundColor)

    var body: some View {
        if !snapshot.isOpen {
            EmptyView()
        } else {
            paletteContainer
                .onAppear {
                    query = snapshot.query
                    selectedIndex = snapshot.selectedIndex ?? (snapshot.items.isEmpty ? nil : 0)
                    DispatchQueue.main.async {
                        isTextFieldFocused = true
                    }
                }
                .onChange(of: snapshot.query) { newValue in
                    if newValue != query {
                        query = newValue
                    }
                    syncSelectionIfNeeded()
                }
                .onChange(of: snapshot.items.count) { _ in
                    syncSelectionIfNeeded()
                }
                .onChange(of: snapshot.isOpen) { isOpen in
                    if isOpen {
                        DispatchQueue.main.async {
                            isTextFieldFocused = true
                        }
                    } else {
                        isTextFieldFocused = false
                    }
                }
        }
    }

    private var paletteCard: some View {
        VStack(alignment: .leading, spacing: 0) {
            paletteHeader
            Divider()
            paletteList
        }
        .frame(maxWidth: paletteWidth)
        .background(
            ZStack {
                Rectangle()
                    .fill(.ultraThinMaterial)
                Rectangle()
                    .fill(backgroundColor)
                    .blendMode(.color)
            }
            .compositingGroup()
        )
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(Color(nsColor: .tertiaryLabelColor).opacity(0.75))
        )
        .shadow(radius: 28, x: 0, y: 12)
    }

    private var paletteContainer: some View {
        let layout = snapshot.layout == .custom ? .floating : snapshot.layout
        return Group {
            switch layout {
            case .bottom:
                VStack {
                    Spacer()
                    paletteCard
                }
                .padding(.horizontal, 24)
                .padding(.bottom, 24)
            case .top:
                VStack {
                    paletteCard
                    Spacer()
                }
                .padding(.horizontal, 24)
                .padding(.top, 24)
            case .floating, .custom:
                paletteCard
                    .padding()
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: alignment(for: layout))
    }

    private func alignment(for layout: CommandPaletteLayout) -> Alignment {
        switch layout {
        case .bottom:
            return .bottom
        case .top:
            return .top
        case .floating, .custom:
            return .center
        }
    }

    private var paletteHeader: some View {
        return ZStack(alignment: .leading) {
            paletteKeyboardShortcuts
            HStack(spacing: 8) {
                Rectangle()
                    .fill(Color.accentColor)
                    .frame(width: 2, height: 18)
                    .cornerRadius(1)
                TextField("Execute a command…", text: $query)
                    .font(.system(size: 18, weight: .light))
                    .textFieldStyle(.plain)
                    .focused($isTextFieldFocused)
                    .onSubmit {
                        if let selected = selectedIndex ?? snapshot.selectedIndex {
                            onSubmit(selected)
                        }
                    }
                    .onExitCommand {
                        onClose()
                    }
                    .onMoveCommand { direction in
                        moveSelection(direction)
                    }
                    .onChange(of: query) { newValue in
                        if newValue != snapshot.query {
                            onQueryChange(newValue)
                        }
                    }
                Spacer()
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    private var paletteKeyboardShortcuts: some View {
        Group {
            Button { moveSelection(.up) } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.upArrow, modifiers: [])
            Button { moveSelection(.down) } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.downArrow, modifiers: [])
            Button { moveSelection(.up) } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.init("p"), modifiers: [.control])
            Button { moveSelection(.down) } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.init("n"), modifiers: [.control])
        }
        .frame(width: 0, height: 0)
        .accessibilityHidden(true)
    }

    private var paletteList: some View {
        if snapshot.items.isEmpty {
            return AnyView(
                Text("No matches")
                    .foregroundStyle(.secondary)
                    .padding()
            )
        }

        return AnyView(
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 4) {
                        ForEach(snapshot.items) { item in
                            paletteRow(for: item)
                                .id(item.id)
                        }
                    }
                    .padding(10)
                }
                .frame(maxHeight: maxListHeight)
                .onChange(of: snapshot.selectedIndex) { _ in
                    guard let selected = snapshot.selectedIndex else { return }
                    proxy.scrollTo(selected, anchor: .center)
                }
            }
        )
    }

    private func paletteRow(for item: CommandPaletteItemSnapshot) -> some View {
        let isSelected = (selectedIndex ?? snapshot.selectedIndex) == item.id
        let isHovered = hoveredIndex == item.id

        return Button {
            selectedIndex = item.id
            onSelect(item.id)
            onSubmit(item.id)
        } label: {
            HStack(spacing: 8) {
                if let color = item.leadingColor {
                    Circle()
                        .fill(color)
                        .frame(width: 8, height: 8)
                }

                if let icon = item.leadingIcon, !icon.isEmpty {
                    Image(systemName: icon)
                        .foregroundStyle(item.emphasis ? Color.accentColor : .secondary)
                        .font(.system(size: 14, weight: .medium))
                }

                VStack(alignment: .leading, spacing: 2) {
                    Text(item.title)
                        .font(.system(size: 14, weight: item.emphasis ? .semibold : .medium))
                        .foregroundColor(.primary)

                    if let subtitle = item.subtitle, !subtitle.isEmpty {
                        Text(subtitle)
                            .font(.system(size: 12))
                            .foregroundStyle(.secondary)
                    } else if let description = item.description, !description.isEmpty {
                        Text(description)
                            .font(.system(size: 12))
                            .foregroundStyle(.secondary)
                    }
                }

                Spacer()

                if let badge = item.badge, !badge.isEmpty {
                    Text(badge)
                        .font(.system(size: 11, weight: .semibold))
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
            .padding(8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: 6)
                    .fill(isSelected ? Color.accentColor.opacity(0.2) : (isHovered ? Color.secondary.opacity(0.2) : Color.clear))
            )
        }
        .buttonStyle(.plain)
        .contentShape(Rectangle())
        .onHover { hovering in
            hoveredIndex = hovering ? item.id : nil
        }
        .help(item.description ?? "")
    }

    private func moveSelection(_ direction: MoveCommandDirection) {
        guard !snapshot.items.isEmpty else { return }

        let current = selectedIndex ?? (direction == .up ? snapshot.items.count : -1)
        let next: Int

        switch direction {
        case .up:
            next = current <= 0 ? (snapshot.items.count - 1) : (current - 1)
        case .down:
            next = current >= (snapshot.items.count - 1) ? 0 : (current + 1)
        default:
            return
        }

        selectedIndex = next
        onSelect(next)
    }

    private func syncSelectionIfNeeded() {
        if snapshot.items.isEmpty {
            selectedIndex = nil
            return
        }
        if let selectedIndex, selectedIndex < snapshot.items.count {
            return
        }
        if let snapshotIndex = snapshot.selectedIndex, snapshotIndex < snapshot.items.count {
            selectedIndex = snapshotIndex
        } else if !snapshot.query.isEmpty {
            selectedIndex = 0
        } else {
            selectedIndex = nil
        }
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
