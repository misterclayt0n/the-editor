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
    let autoSelectFirstItem: Bool
    var showBackground: Bool = true

    // Data
    let itemCount: Int
    let externalQuery: String
    let externalSelectedIndex: Int?

    // Callbacks
    let onQueryChange: (String) -> Void
    let onSubmit: (Int?) -> Void
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
            .background(
                PickerKeyInterceptor(
                    onMoveSelection: { delta in moveSelection(delta) },
                    onClose: showCtrlCClose ? { onClose() } : nil,
                    onTextInput: { chars in
                        query.append(chars)
                        isTextFieldFocused = true
                    },
                    onBackspace: {
                        if !query.isEmpty { query.removeLast() }
                        isTextFieldFocused = true
                    },
                    isTextFieldFocused: isTextFieldFocused,
                    pageSize: pageSize,
                    showTabNavigation: showTabNavigation,
                    showPageNavigation: showPageNavigation
                )
                .frame(width: 0, height: 0)
            )
            .onAppear {
                query = externalQuery
                selectedIndex = initialSelection()
                DispatchQueue.main.async {
                    isTextFieldFocused = true
                }
                if let sel = selectedIndex {
                    onSelectionChange?(sel)
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
            .onChange(of: selectedIndex) { newValue in
                normalizeSelection(newValue)
            }
    }

    // MARK: - Layout

    private var panelContainer: some View {
        Group {
            if !showBackground {
                panelCard
            } else {
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
        }
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
        let content = VStack(alignment: .leading, spacing: 0) {
            panelHeader
            Divider()
            panelList
        }
        .frame(maxWidth: width)

        return Group {
            if showBackground {
                content
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
            } else {
                content
            }
        }
    }

    // MARK: - Header

    private var panelHeader: some View {
        HStack(spacing: 8) {
            leadingHeader()

            TextField(placeholder, text: $query)
                .font(FontLoader.uiFont(size: fontSize).weight(.light))
                .textFieldStyle(.plain)
                .focused($isTextFieldFocused)
                .onSubmit {
                    onSubmit(clampedIndex(selectedIndex))
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
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    // MARK: - List

    @ViewBuilder
    private var panelList: some View {
        if itemCount == 0 {
            emptyContent()
                .frame(maxHeight: maxListHeight)
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

        let len = itemCount
        let next: Int
        if let current = clampedIndex(selectedIndex) {
            let raw = current + delta
            next = ((raw % len) + len) % len
        } else {
            next = delta >= 0 ? 0 : (len - 1)
        }

        selectedIndex = next
        onSelectionChange?(next)
    }

    private func syncSelection() {
        if itemCount == 0 {
            selectedIndex = nil
            return
        }
        let prev = selectedIndex
        if !autoSelectFirstItem {
            selectedIndex = clampedIndex(externalSelectedIndex)
        } else {
            selectedIndex = clampedIndex(selectedIndex)
                ?? clampedIndex(externalSelectedIndex)
                ?? (autoSelectFirstItem ? 0 : nil)
        }
        if selectedIndex != prev, let sel = selectedIndex {
            onSelectionChange?(sel)
        }
    }

    private func clampedIndex(_ index: Int?) -> Int? {
        guard itemCount > 0 else { return nil }
        guard let index else { return nil }
        return max(0, min(index, itemCount - 1))
    }

    private func initialSelection() -> Int? {
        guard itemCount > 0 else { return nil }
        return clampedIndex(externalSelectedIndex) ?? (autoSelectFirstItem ? 0 : nil)
    }

    private func normalizeSelection(_ newValue: Int?) {
        guard itemCount > 0 else {
            if selectedIndex != nil {
                selectedIndex = nil
            }
            return
        }

        let normalized = clampedIndex(newValue) ?? (autoSelectFirstItem ? 0 : nil)
        if selectedIndex != normalized {
            selectedIndex = normalized
        }
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

// MARK: - NSEvent-based key interceptor

/// Intercepts navigation keys (arrows, Ctrl+P/N, Tab, Ctrl+U/D, Ctrl+C) at the
/// NSEvent level, bypassing SwiftUI's keyboard shortcut system which can miss
/// events when a TextField has focus.
///
/// Also intercepts printable characters and backspace so that typing always
/// goes to the search field, even if another view (e.g. preview panel) has
/// stolen keyboard focus.
private struct PickerKeyInterceptor: NSViewRepresentable {
    let onMoveSelection: (Int) -> Void
    let onClose: (() -> Void)?
    let onTextInput: ((String) -> Void)?
    let onBackspace: (() -> Void)?
    let isTextFieldFocused: Bool
    let pageSize: Int
    let showTabNavigation: Bool
    let showPageNavigation: Bool

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        let coordinator = context.coordinator
        coordinator.isActive = true
        coordinator.monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            guard coordinator.isActive else { return event }
            return coordinator.handleKey(event)
        }
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        let c = context.coordinator
        c.onMoveSelection = onMoveSelection
        c.onClose = onClose
        c.onTextInput = onTextInput
        c.onBackspace = onBackspace
        c.isTextFieldFocused = isTextFieldFocused
        c.pageSize = pageSize
        c.showTabNavigation = showTabNavigation
        c.showPageNavigation = showPageNavigation
    }

    static func dismantleNSView(_ nsView: NSView, coordinator: Coordinator) {
        coordinator.isActive = false
        if let monitor = coordinator.monitor {
            NSEvent.removeMonitor(monitor)
            coordinator.monitor = nil
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(
            onMoveSelection: onMoveSelection,
            onClose: onClose,
            onTextInput: onTextInput,
            onBackspace: onBackspace,
            isTextFieldFocused: isTextFieldFocused,
            pageSize: pageSize,
            showTabNavigation: showTabNavigation,
            showPageNavigation: showPageNavigation
        )
    }

    class Coordinator {
        var onMoveSelection: (Int) -> Void
        var onClose: (() -> Void)?
        var onTextInput: ((String) -> Void)?
        var onBackspace: (() -> Void)?
        var isTextFieldFocused: Bool
        var pageSize: Int
        var showTabNavigation: Bool
        var showPageNavigation: Bool
        var monitor: Any?
        var isActive: Bool = false

        init(
            onMoveSelection: @escaping (Int) -> Void,
            onClose: (() -> Void)?,
            onTextInput: ((String) -> Void)?,
            onBackspace: (() -> Void)?,
            isTextFieldFocused: Bool,
            pageSize: Int,
            showTabNavigation: Bool,
            showPageNavigation: Bool
        ) {
            self.onMoveSelection = onMoveSelection
            self.onClose = onClose
            self.onTextInput = onTextInput
            self.onBackspace = onBackspace
            self.isTextFieldFocused = isTextFieldFocused
            self.pageSize = pageSize
            self.showTabNavigation = showTabNavigation
            self.showPageNavigation = showPageNavigation
        }

        deinit {
            if let monitor { NSEvent.removeMonitor(monitor) }
        }

        func handleKey(_ event: NSEvent) -> NSEvent? {
            let keyCode = event.keyCode
            let mods = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            let importantMods = mods.intersection([.command, .option, .control, .shift])
            let chars = event.charactersIgnoringModifiers ?? ""

            // Up arrow (no modifiers)
            if keyCode == 126 && importantMods.isEmpty {
                onMoveSelection(-1)
                return nil
            }

            // Down arrow (no modifiers)
            if keyCode == 125 && importantMods.isEmpty {
                onMoveSelection(1)
                return nil
            }

            // Ctrl+P / Ctrl+N
            if importantMods == [.control] {
                if chars == "p" { onMoveSelection(-1); return nil }
                if chars == "n" { onMoveSelection(1); return nil }
            }

            // Tab / Shift+Tab
            if showTabNavigation && keyCode == 48 {
                if importantMods.isEmpty { onMoveSelection(1); return nil }
                if importantMods == [.shift] { onMoveSelection(-1); return nil }
            }

            // Ctrl+U / Ctrl+D (page navigation)
            if showPageNavigation && importantMods == [.control] {
                if chars == "u" { onMoveSelection(-pageSize); return nil }
                if chars == "d" { onMoveSelection(pageSize); return nil }
            }

            // Ctrl+C (close)
            if let onClose, importantMods == [.control] && chars == "c" {
                onClose()
                return nil
            }

            // When the TextField has focus, let it handle text input natively
            // (including selection-aware backspace, Cmd+A, etc.)
            if isTextFieldFocused {
                return event
            }

            // Below: TextField does NOT have focus (e.g. preview panel stole it).
            // Intercept text input and forward to the query.

            // Let Cmd-key combos pass through (Cmd+C for copy, Cmd+V for paste, etc.)
            if importantMods.contains(.command) {
                return event
            }

            // Backspace — forward to query
            if keyCode == 51 && importantMods.isEmpty {
                onBackspace?()
                return nil
            }

            // Printable characters (no control/option modifiers) — forward to query.
            if !importantMods.contains(.control) && !importantMods.contains(.option) {
                if let typed = event.characters, !typed.isEmpty {
                    let scalar = typed.unicodeScalars.first!
                    // Only forward actual printable characters (not function keys, etc.)
                    if scalar.value >= 0x20 && scalar.value < 0xF700 {
                        onTextInput?(typed)
                        return nil
                    }
                }
            }

            return event
        }
    }
}
