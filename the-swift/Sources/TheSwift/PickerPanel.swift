import SwiftUI

enum PickerPanelLayout {
    case center, top, bottom
}

struct PickerPanel<
    LeadingHeader: View,
    TrailingHeader: View,
    ItemContent: View,
    EmptyContent: View
>: View {
    // Configuration
    let width: CGFloat
    let maxListHeight: CGFloat
    let placeholder: String
    let fontSize: CGFloat
    let layout: PickerPanelLayout
    let pageSize: Int
    let showTabNavigation: Bool
    let showPageNavigation: Bool
    let showCtrlCClose: Bool

    // Data
    let itemCount: Int
    let externalQuery: String
    let externalSelectedIndex: Int?

    // Callbacks
    let onQueryChange: (String) -> Void
    let onSubmit: (Int) -> Void
    let onClose: () -> Void
    let onSelectionChange: ((Int) -> Void)?

    // Content
    @ViewBuilder let leadingHeader: () -> LeadingHeader
    @ViewBuilder let trailingHeader: () -> TrailingHeader
    let itemContent: (_ index: Int, _ isSelected: Bool, _ isHovered: Bool) -> ItemContent
    @ViewBuilder let emptyContent: () -> EmptyContent

    // Internal state
    @State private var query: String = ""
    @State private var selectedIndex: Int? = nil
    @State private var hoveredIndex: Int? = nil
    @FocusState private var isTextFieldFocused: Bool

    private let backgroundColor: Color = Color(nsColor: .windowBackgroundColor)

    var body: some View {
        panelContainer
            .onAppear {
                query = externalQuery
                selectedIndex = clampedIndex(externalSelectedIndex ?? 0)
                DispatchQueue.main.async {
                    isTextFieldFocused = true
                }
            }
            .onChange(of: externalQuery) { newValue in
                if newValue != query {
                    query = newValue
                }
                syncSelection()
            }
            .onChange(of: itemCount) { _ in
                syncSelection()
            }
    }

    // MARK: - Layout

    private var panelContainer: some View {
        Group {
            switch layout {
            case .bottom:
                VStack {
                    Spacer()
                    panelCard
                }
                .padding(.horizontal, 24)
                .padding(.bottom, 24)
            case .top:
                VStack {
                    panelCard
                    Spacer()
                }
                .padding(.horizontal, 24)
                .padding(.top, 24)
            case .center:
                panelCard
                    .padding()
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: layoutAlignment)
    }

    private var layoutAlignment: Alignment {
        switch layout {
        case .bottom: return .bottom
        case .top: return .top
        case .center: return .center
        }
    }

    // MARK: - Glass card

    private var panelCard: some View {
        VStack(alignment: .leading, spacing: 0) {
            panelHeader
            Divider()
            panelList
        }
        .frame(maxWidth: width)
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
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(Color(nsColor: .tertiaryLabelColor).opacity(0.75))
        )
        .shadow(radius: 28, x: 0, y: 12)
    }

    // MARK: - Header

    private var panelHeader: some View {
        ZStack(alignment: .leading) {
            keyboardShortcuts
            HStack(spacing: 8) {
                leadingHeader()

                TextField(placeholder, text: $query)
                    .font(.system(size: fontSize, weight: .light))
                    .textFieldStyle(.plain)
                    .focused($isTextFieldFocused)
                    .onSubmit {
                        if let selected = clampedIndex(selectedIndex) {
                            onSubmit(selected)
                        }
                    }
                    .onExitCommand {
                        onClose()
                    }
                    .onChange(of: query) { newValue in
                        if newValue != externalQuery {
                            onQueryChange(newValue)
                        }
                    }

                Spacer()

                trailingHeader()
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    // MARK: - Keyboard shortcuts

    private var keyboardShortcuts: some View {
        Group {
            // Arrow keys (always)
            Button { moveSelection(-1) } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.upArrow, modifiers: [])
            Button { moveSelection(1) } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.downArrow, modifiers: [])

            // Ctrl+P / Ctrl+N (always)
            Button { moveSelection(-1) } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.init("p"), modifiers: [.control])
            Button { moveSelection(1) } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.init("n"), modifiers: [.control])

            // Tab / Shift+Tab (opt-in)
            if showTabNavigation {
                Button { moveSelection(1) } label: { Color.clear }
                    .buttonStyle(.plain)
                    .keyboardShortcut(.tab, modifiers: [])
                Button { moveSelection(-1) } label: { Color.clear }
                    .buttonStyle(.plain)
                    .keyboardShortcut(.tab, modifiers: [.shift])
            }

            // Page navigation: Ctrl+U / Ctrl+D (opt-in)
            if showPageNavigation {
                Button { moveSelection(-pageSize) } label: { Color.clear }
                    .buttonStyle(.plain)
                    .keyboardShortcut(.init("u"), modifiers: [.control])
                Button { moveSelection(pageSize) } label: { Color.clear }
                    .buttonStyle(.plain)
                    .keyboardShortcut(.init("d"), modifiers: [.control])
            }

            // Ctrl+C to close (opt-in)
            if showCtrlCClose {
                Button { onClose() } label: { Color.clear }
                    .buttonStyle(.plain)
                    .keyboardShortcut(.init("c"), modifiers: [.control])
            }
        }
        .frame(width: 0, height: 0)
        .accessibilityHidden(true)
    }

    // MARK: - List

    @ViewBuilder
    private var panelList: some View {
        if itemCount == 0 {
            emptyContent()
        } else {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 4) {
                        ForEach(0..<itemCount, id: \.self) { index in
                            rowContainer(index: index)
                                .id(index)
                        }
                    }
                    .padding(10)
                }
                .frame(maxHeight: maxListHeight)
                .onChange(of: selectedIndex) { newIndex in
                    guard let index = newIndex else { return }
                    scrollSelectionIntoView(index: index, proxy: proxy)
                }
                .onAppear {
                    guard let index = selectedIndex else { return }
                    scrollSelectionIntoView(index: index, proxy: proxy)
                }
            }
        }
    }

    private func rowContainer(index: Int) -> some View {
        let isSelected = selectedIndex == index
        let isHovered = hoveredIndex == index

        return Button {
            selectedIndex = index
            onSelectionChange?(index)
            onSubmit(index)
        } label: {
            itemContent(index, isSelected, isHovered)
                .padding(8)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(
                    RoundedRectangle(cornerRadius: 6)
                        .fill(isSelected ? Color.accentColor.opacity(0.2) : (isHovered ? Color.secondary.opacity(0.15) : Color.clear))
                )
        }
        .buttonStyle(.plain)
        .contentShape(Rectangle())
        .onHover { hovering in
            hoveredIndex = hovering ? index : nil
        }
    }

    // MARK: - Selection logic

    private func moveSelection(_ delta: Int) {
        guard itemCount > 0 else { return }

        let current = selectedIndex ?? (delta < 0 ? itemCount - 1 : 0)
        let next = max(0, min(current + delta, itemCount - 1))

        selectedIndex = next
        onSelectionChange?(next)
    }

    private func syncSelection() {
        if itemCount == 0 {
            selectedIndex = nil
            return
        }
        if let idx = clampedIndex(selectedIndex) {
            selectedIndex = idx
            return
        }
        if let ext = clampedIndex(externalSelectedIndex) {
            selectedIndex = ext
        } else if !query.isEmpty {
            selectedIndex = 0
        } else {
            selectedIndex = nil
        }
    }

    private func clampedIndex(_ index: Int?) -> Int? {
        guard itemCount > 0 else { return nil }
        guard let index else { return nil }
        return max(0, min(index, itemCount - 1))
    }

    private func scrollSelectionIntoView(index: Int, proxy: ScrollViewProxy) {
        var transaction = Transaction()
        transaction.animation = nil
        withTransaction(transaction) {
            // nil anchor keeps native "only scroll when needed" behavior.
            proxy.scrollTo(index, anchor: nil)
        }
    }
}
