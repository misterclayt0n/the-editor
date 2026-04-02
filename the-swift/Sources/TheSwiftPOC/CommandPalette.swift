import AppKit
import SwiftUI

struct CommandOption: Identifiable, Hashable {
    let id = UUID()
    let title: String
    let subtitle: String?
    let description: String?
    let symbols: [String]?
    let leadingIcon: String?
    let leadingColor: Color?
    let badge: String?
    let emphasis: Bool
    let action: () -> Void

    init(
        title: String,
        subtitle: String? = nil,
        description: String? = nil,
        symbols: [String]? = nil,
        leadingIcon: String? = nil,
        leadingColor: Color? = nil,
        badge: String? = nil,
        emphasis: Bool = false,
        action: @escaping () -> Void
    ) {
        self.title = title
        self.subtitle = subtitle
        self.description = description
        self.symbols = symbols
        self.leadingIcon = leadingIcon
        self.leadingColor = leadingColor
        self.badge = badge
        self.emphasis = emphasis
        self.action = action
    }

    static func == (lhs: CommandOption, rhs: CommandOption) -> Bool {
        lhs.id == rhs.id
    }

    func hash(into hasher: inout Hasher) {
        hasher.combine(id)
    }
}

struct CommandPaletteView: View {
    @Binding var isPresented: Bool
    let query: String
    var placeholder: String = "Execute a command…"
    var backgroundColor: Color = Color(nsColor: .windowBackgroundColor)
    var options: [CommandOption]
    var selectedIndex: UInt?
    var onQueryChange: (String) -> Void
    var onMove: (MoveCommandDirection) -> Void
    var onSubmit: () -> Void
    @State private var hoveredOptionID: UUID?
    @State private var localQuery: String = ""
    @State private var suppressQueryCallback = false

    var body: some View {
        let scheme: ColorScheme = NSColor(backgroundColor).isLightColor ? .light : .dark

        VStack(alignment: .leading, spacing: 0) {
            CommandPaletteQuery(query: $localQuery, placeholder: placeholder) { event in
                switch event {
                case .exit:
                    isPresented = false

                case .submit:
                    onSubmit()

                case .move(.up):
                    if !options.isEmpty {
                        onMove(.up)
                    }

                case .move(.down):
                    if !options.isEmpty {
                        onMove(.down)
                    }

                case .move:
                    break
                }
            }
            .onChange(of: localQuery) { _, newValue in
                guard !suppressQueryCallback else { return }
                onQueryChange(newValue)
            }
            .onChange(of: query) { _, newValue in
                guard localQuery != newValue else { return }
                suppressQueryCallback = true
                localQuery = newValue
                DispatchQueue.main.async {
                    suppressQueryCallback = false
                }
            }
            .onAppear {
                localQuery = query
            }

            Divider()

            CommandTable(
                options: options,
                selectedIndex: selectedIndex,
                hoveredOptionID: $hoveredOptionID
            ) { option in
                option.action()
            }
        }
        .frame(maxWidth: 500)
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
        .clipShape(RoundedRectangle(cornerRadius: 10))
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(Color(nsColor: .tertiaryLabelColor).opacity(0.75))
        )
        .shadow(radius: 32, x: 0, y: 12)
        .padding()
        .environment(\.colorScheme, scheme)
    }
}

private struct CommandPaletteQuery: View {
    @Binding var query: String
    let placeholder: String
    var onEvent: ((KeyboardEvent) -> Void)?
    @FocusState private var isTextFieldFocused: Bool

    init(
        query: Binding<String>,
        placeholder: String,
        onEvent: ((KeyboardEvent) -> Void)? = nil
    ) {
        _query = query
        self.placeholder = placeholder
        self.onEvent = onEvent
    }

    enum KeyboardEvent {
        case exit
        case submit
        case move(MoveCommandDirection)
    }

    var body: some View {
        ZStack {
            Group {
                Button { onEvent?(.move(.up)) } label: { Color.clear }
                    .buttonStyle(.plain)
                    .keyboardShortcut(.upArrow, modifiers: [])
                Button { onEvent?(.move(.down)) } label: { Color.clear }
                    .buttonStyle(.plain)
                    .keyboardShortcut(.downArrow, modifiers: [])

                Button { onEvent?(.move(.up)) } label: { Color.clear }
                    .buttonStyle(.plain)
                    .keyboardShortcut(.init("p"), modifiers: [.control])
                Button { onEvent?(.move(.down)) } label: { Color.clear }
                    .buttonStyle(.plain)
                    .keyboardShortcut(.init("n"), modifiers: [.control])
            }
            .frame(width: 0, height: 0)
            .accessibilityHidden(true)

            TextField(placeholder, text: $query)
                .padding()
                .font(.system(size: 20, weight: .light))
                .frame(height: 48)
                .textFieldStyle(.plain)
                .focused($isTextFieldFocused)
                .onExitCommand { onEvent?(.exit) }
                .onMoveCommand { onEvent?(.move($0)) }
                .onSubmit { onEvent?(.submit) }
                .onAppear {
                    DispatchQueue.main.async {
                        isTextFieldFocused = true
                    }
                }
        }
    }
}

private struct CommandTable: View {
    var options: [CommandOption]
    var selectedIndex: UInt?
    @Binding var hoveredOptionID: UUID?
    var action: (CommandOption) -> Void

    var body: some View {
        if options.isEmpty {
            Text("No matches")
                .foregroundStyle(.secondary)
                .padding()
        } else {
            ScrollViewReader { proxy in
                ScrollView {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(Array(options.enumerated()), id: \.1.id) { index, option in
                            CommandRow(
                                option: option,
                                isSelected: {
                                    if let selectedIndex {
                                        let selected = Int(selectedIndex)
                                        return selected == index ||
                                            (selected >= options.count && index == options.count - 1)
                                    } else {
                                        return false
                                    }
                                }(),
                                hoveredID: $hoveredOptionID
                            ) {
                                action(option)
                            }
                        }
                    }
                    .padding(10)
                }
                .frame(maxHeight: 200)
                .onChange(of: selectedIndex) { _, _ in
                    guard let selectedIndex, Int(selectedIndex) < options.count else { return }
                    proxy.scrollTo(options[Int(selectedIndex)].id)
                }
            }
        }
    }
}

private struct CommandRow: View {
    let option: CommandOption
    var isSelected: Bool
    @Binding var hoveredID: UUID?
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                if let color = option.leadingColor {
                    Circle()
                        .fill(color)
                        .frame(width: 8, height: 8)
                }

                if let icon = option.leadingIcon {
                    Image(systemName: icon)
                        .foregroundStyle(option.emphasis ? Color.accentColor : .secondary)
                        .font(.system(size: 14, weight: .medium))
                }

                VStack(alignment: .leading, spacing: 2) {
                    Text(option.title)
                        .fontWeight(option.emphasis ? .medium : .regular)

                    if let subtitle = option.subtitle {
                        Text(subtitle)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                Spacer()

                if let badge = option.badge, !badge.isEmpty {
                    Text(badge)
                        .font(.caption2.weight(.medium))
                        .padding(.horizontal, 7)
                        .padding(.vertical, 3)
                        .background(
                            Capsule().fill(Color.accentColor.opacity(0.15))
                        )
                        .foregroundStyle(Color.accentColor)
                }

                if let symbols = option.symbols {
                    ShortcutSymbolsView(symbols: symbols)
                        .foregroundStyle(.secondary)
                }
            }
            .padding(8)
            .contentShape(Rectangle())
            .background(
                isSelected
                    ? Color.accentColor.opacity(0.2)
                    : (hoveredID == option.id
                        ? Color.secondary.opacity(0.2)
                        : Color.clear)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 5)
                    .strokeBorder(Color.accentColor.opacity(option.emphasis && !isSelected ? 0.3 : 0), lineWidth: 1.5)
            )
            .cornerRadius(5)
        }
        .help(option.description ?? "")
        .buttonStyle(.plain)
        .onHover { hovering in
            hoveredID = hovering ? option.id : nil
        }
    }
}

private struct ShortcutSymbolsView: View {
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

private extension NSColor {
    var isLightColor: Bool {
        guard let color = usingColorSpace(.sRGB) else { return false }
        let luminance = (0.299 * color.redComponent) + (0.587 * color.greenComponent) + (0.114 * color.blueComponent)
        return luminance >= 0.6
    }
}
