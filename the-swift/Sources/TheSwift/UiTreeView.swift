import SwiftUI

struct UiOverlayHost: View {
    let tree: UiTreeSnapshot
    let cellSize: CGSize
    let onSelectCommand: (Int) -> Void
    let onSubmitCommand: (Int) -> Void
    let onCloseCommandPalette: () -> Void
    let onQueryChange: (String) -> Void

    var body: some View {
        GeometryReader { proxy in
            ZStack {
                let paletteSnapshot = tree.commandPaletteSnapshot()
                let statuslineSnapshot = tree.statuslineSnapshot()

                ForEach(Array(tree.overlays.enumerated()), id: \.offset) { _, node in
                    if case .panel(let panel) = node, panel.id == "command_palette" {
                        EmptyView()
                    } else if case .panel(let panel) = node, panel.id == "command_palette_help" {
                        EmptyView()
                    } else if case .panel(let panel) = node, panel.id == "statusline" {
                        EmptyView()
                    } else {
                        UiNodeView(node: node, cellSize: cellSize, containerSize: proxy.size)
                    }
                }

                if let statuslineSnapshot {
                    StatuslineView(snapshot: statuslineSnapshot, cellSize: cellSize)
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomLeading)
                }

                if let paletteSnapshot {
                    CommandPaletteView(
                        snapshot: paletteSnapshot,
                        onSelect: onSelectCommand,
                        onSubmit: onSubmitCommand,
                        onClose: onCloseCommandPalette,
                        onQueryChange: onQueryChange
                    )
                }
            }
        }
    }
}

struct UiNodeView: View {
    let node: UiNodeSnapshot
    let cellSize: CGSize
    let containerSize: CGSize

    var body: some View {
        switch node {
        case .panel(let panel):
            panelView(panel)
        case .container(let container):
            containerView(container)
        case .text(let text):
            textView(text)
        case .list(let list):
            listView(list)
        case .input(let input):
            inputView(input)
        case .divider:
            Divider()
        case .spacer(let spacer):
            Spacer().frame(height: CGFloat(spacer.size) * cellSize.height)
        case .tooltip(let tooltip):
            tooltipView(tooltip)
        case .statusBar(let status):
            statusBarView(status, cellSize: cellSize)
        case .unknown:
            EmptyView()
        }
    }

    @ViewBuilder
    private func panelView(_ panel: UiPanelSnapshot) -> some View {
        let alignment = alignment(for: panel.intent, align: panel.constraints.align)
        let minWidth = length(panel.constraints.minWidth, unit: cellSize.width)
        let maxWidth = length(panel.constraints.maxWidth, unit: cellSize.width)
        let minHeight = length(panel.constraints.minHeight, unit: cellSize.height)
        let maxHeight = length(panel.constraints.maxHeight, unit: cellSize.height)
        let padding = panel.constraints.padding

        let content = UiNodeView(node: panel.child, cellSize: cellSize, containerSize: containerSize)
            .padding(EdgeInsets(
                top: CGFloat(padding.top) * cellSize.height,
                leading: CGFloat(padding.left) * cellSize.width,
                bottom: CGFloat(padding.bottom) * cellSize.height,
                trailing: CGFloat(padding.right) * cellSize.width
            ))
            .frame(minWidth: minWidth, idealWidth: nil, maxWidth: maxWidth,
                   minHeight: minHeight, idealHeight: nil, maxHeight: maxHeight,
                   alignment: .topLeading)

        let cornerRadius = panelCornerRadius(panel)
        content
            .background(panelBackground(panel, radius: cornerRadius))
            .overlay(
                RoundedRectangle(cornerRadius: cornerRadius)
                    .stroke(panelBorderColor(panel), lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: cornerRadius))
            .shadow(color: Color.black.opacity(0.25), radius: 20, x: 0, y: 8)
            .frame(maxWidth: containerSize.width, maxHeight: containerSize.height, alignment: alignment)
    }

    @ViewBuilder
    private func containerView(_ container: UiContainerSnapshot) -> some View {
        switch container.layout {
        case .stack(let axis, let gap):
            if axis == .vertical {
                VStack(alignment: .leading, spacing: CGFloat(gap) * cellSize.height) {
                    ForEach(Array(container.children.enumerated()), id: \.offset) { _, child in
                        UiNodeView(node: child, cellSize: cellSize, containerSize: containerSize)
                    }
                }
            } else {
                HStack(alignment: .center, spacing: CGFloat(gap) * cellSize.width) {
                    ForEach(Array(container.children.enumerated()), id: \.offset) { _, child in
                        UiNodeView(node: child, cellSize: cellSize, containerSize: containerSize)
                    }
                }
            }
        case .split(let axis, _):
            if axis == .vertical {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(container.children.enumerated()), id: \.offset) { _, child in
                        UiNodeView(node: child, cellSize: cellSize, containerSize: containerSize)
                    }
                }
            } else {
                HStack(alignment: .center, spacing: 0) {
                    ForEach(Array(container.children.enumerated()), id: \.offset) { _, child in
                        UiNodeView(node: child, cellSize: cellSize, containerSize: containerSize)
                    }
                }
            }
        case .unknown:
            EmptyView()
        }
    }

    @ViewBuilder
    private func textView(_ text: UiTextSnapshot) -> some View {
        Text(text.content)
            .foregroundColor(resolveColor(text.style.fg, fallback: nativePrimary))
            .font(font(for: text.style))
            .lineLimit(text.maxLines.map { Int($0) })
            .truncationMode(.tail)
            .frame(maxWidth: .infinity, alignment: .leading)
    }

    @ViewBuilder
    private func listView(_ list: UiListSnapshot) -> some View {
        let items = list.maxVisible.map { max(1, $0) }.map { Array(list.items.prefix($0)) } ?? list.items
        let baseText = resolveColor(list.style.fg, fallback: nativePrimary)
        let selectedText = resolveColor(list.style.border, fallback: baseText)
        let selectedBg = resolveColor(list.style.accent, fallback: Color.accentColor).opacity(0.2)
        VStack(alignment: .leading, spacing: 4) {
            ForEach(Array(items.enumerated()), id: \.offset) { index, item in
                let isSelected = list.selected == index
                listRow(
                    item,
                    isSelected: isSelected,
                    baseText: baseText,
                    secondaryText: baseText.opacity(0.7),
                    selectedText: selectedText,
                    selectedBg: selectedBg
                )
            }
        }
    }

    @ViewBuilder
    private func inputView(_ input: UiInputSnapshot) -> some View {
        let display = input.value.isEmpty ? (input.placeholder ?? "") : input.value
        let baseText = resolveColor(input.style.fg, fallback: nativePrimary)
        let placeholderText = resolveColor(input.style.accent, fallback: nativeSecondary)
        Text(display)
            .foregroundColor(input.value.isEmpty ? placeholderText : baseText)
            .font(.system(size: 14, weight: .regular, design: .rounded))
            .padding(.vertical, 6)
            .padding(.horizontal, 8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: 6)
                    .fill(Color(nsColor: .controlBackgroundColor).opacity(0.45))
            )
    }

    @ViewBuilder
    private func tooltipView(_ tooltip: UiTooltipSnapshot) -> some View {
        Text(tooltip.content)
            .padding(10)
            .foregroundColor(resolveColor(tooltip.style.fg, fallback: nativePrimary))
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(.ultraThinMaterial)
                    .overlay(
                        RoundedRectangle(cornerRadius: 8)
                            .fill(tooltipTint(tooltip))
                            .blendMode(.color)
                    )
                    .compositingGroup()
            )
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(Color(nsColor: .separatorColor).opacity(0.6), lineWidth: 1)
            )
            .shadow(color: Color.black.opacity(0.2), radius: 16, x: 0, y: 6)
    }

    @ViewBuilder
    private func statusBarView(_ status: UiStatusBarSnapshot, cellSize: CGSize) -> some View {
        HStack {
            Text(status.left)
                .lineLimit(1)
                .truncationMode(.tail)
            Spacer()
            if !status.center.isEmpty {
                Text(status.center)
                    .lineLimit(1)
                    .truncationMode(.tail)
                Spacer()
            }
            Text(status.right)
                .lineLimit(1)
                .truncationMode(.tail)
        }
        .font(.system(size: 12, weight: .medium, design: .rounded))
        .tracking(-0.2)
        .foregroundColor(resolveColor(status.style.fg, fallback: nativePrimary))
        .frame(maxWidth: .infinity, minHeight: cellSize.height, maxHeight: cellSize.height)
    }

    private func length(_ value: UInt16?, unit: CGFloat) -> CGFloat? {
        guard let value else { return nil }
        return CGFloat(value) * unit
    }

    private func alignment(for intent: LayoutIntentSnapshot, align: UiAlignPairSnapshot) -> Alignment {
        switch intent {
        case .bottom:
            return .bottom
        case .top:
            return .top
        case .sidebarLeft:
            return .leading
        case .sidebarRight:
            return .trailing
        case .fullscreen:
            return .center
        case .floating, .custom, .unknown:
            return alignment(from: align)
        }
    }

    private func alignment(from align: UiAlignPairSnapshot) -> Alignment {
        switch (align.horizontal, align.vertical) {
        case (.start, .start):
            return .topLeading
        case (.center, .start):
            return .top
        case (.end, .start):
            return .topTrailing
        case (.start, .center):
            return .leading
        case (.center, .center):
            return .center
        case (.end, .center):
            return .trailing
        case (.start, .end):
            return .bottomLeading
        case (.center, .end):
            return .bottom
        case (.end, .end):
            return .bottomTrailing
        case (.stretch, .stretch):
            return .center
        default:
            return .center
        }
    }

    private func font(for style: UiStyleSnapshot) -> Font {
        switch style.emphasis {
        case .strong:
            return .system(size: 14, weight: .bold, design: .rounded)
        case .muted:
            return .system(size: 13, weight: .regular, design: .rounded)
        case .normal:
            return .system(size: 14, weight: .regular, design: .rounded)
        }
    }

    private func radius(_ radius: UiRadiusSnapshot) -> CGFloat {
        switch radius {
        case .none:
            return 0
        case .small:
            return 4
        case .medium:
            return 8
        case .large:
            return 12
        }
    }

    private var nativePrimary: Color {
        Color(nsColor: .labelColor)
    }

    private var nativeSecondary: Color {
        Color(nsColor: .secondaryLabelColor)
    }

    private func panelCornerRadius(_ panel: UiPanelSnapshot) -> CGFloat {
        let base = radius(panel.style.radius)
        return base == 0 ? 10 : base
    }

    private func panelTint(_ panel: UiPanelSnapshot) -> Color {
        uiColorToColor(panel.style.bg) ?? Color(nsColor: .windowBackgroundColor)
    }

    private func panelBorderColor(_ panel: UiPanelSnapshot) -> Color {
        uiColorToColor(panel.style.border) ?? Color(nsColor: .separatorColor).opacity(0.65)
    }

    @ViewBuilder
    private func panelBackground(_ panel: UiPanelSnapshot, radius: CGFloat) -> some View {
        ZStack {
            RoundedRectangle(cornerRadius: radius)
                .fill(.ultraThinMaterial)
            RoundedRectangle(cornerRadius: radius)
                .fill(panelTint(panel))
                .blendMode(.color)
        }
        .compositingGroup()
    }

    private func tooltipTint(_ tooltip: UiTooltipSnapshot) -> Color {
        uiColorToColor(tooltip.style.bg) ?? Color(nsColor: .windowBackgroundColor)
    }

    @ViewBuilder
    private func listRow(
        _ item: UiListItemSnapshot,
        isSelected: Bool,
        baseText: Color,
        secondaryText: Color,
        selectedText: Color,
        selectedBg: Color
    ) -> some View {
        let titleColor = isSelected ? selectedText : baseText
        let detailColor = isSelected ? selectedText.opacity(0.75) : secondaryText
        HStack(spacing: 8) {
            if let color = uiColorToColor(item.leadingColor) {
                Circle()
                    .fill(color)
                    .frame(width: 8, height: 8)
            }

            if let icon = item.leadingIcon, !icon.isEmpty {
                Image(systemName: icon)
                    .foregroundStyle(item.emphasis ? Color.accentColor : detailColor)
                    .font(.system(size: 14, weight: .medium))
            }

            VStack(alignment: .leading, spacing: 2) {
                Text(item.title)
                    .foregroundColor(titleColor)
                    .font(.system(size: 14, weight: item.emphasis ? .semibold : .medium, design: .rounded))
                if let subtitle = item.subtitle, !subtitle.isEmpty {
                    Text(subtitle)
                        .foregroundColor(detailColor)
                        .font(.system(size: 12, design: .rounded))
                } else if let description = item.description, !description.isEmpty {
                    Text(description)
                        .foregroundColor(detailColor)
                        .font(.system(size: 12, design: .rounded))
                }
            }

            Spacer()

            if let badge = item.badge, !badge.isEmpty {
                Text(badge)
                    .font(.system(size: 11, weight: .semibold, design: .rounded))
                    .padding(.horizontal, 7)
                    .padding(.vertical, 3)
                    .background(
                        Capsule()
                            .fill(Color.accentColor.opacity(0.15))
                    )
                    .foregroundStyle(Color.accentColor)
            }

            if !item.symbols.isEmpty {
                UiShortcutSymbolsView(symbols: item.symbols)
                    .foregroundStyle(detailColor)
            } else if let shortcut = item.shortcut, !shortcut.isEmpty {
                UiShortcutSymbolsView(symbols: [shortcut])
                    .foregroundStyle(detailColor)
            }
        }
        .padding(8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 6)
                .fill(isSelected ? selectedBg : Color.clear)
        )
    }
}

fileprivate struct UiShortcutSymbolsView: View {
    let symbols: [String]

    var body: some View {
        HStack(spacing: 1) {
            ForEach(symbols, id: \.self) { symbol in
                Text(symbol)
                    .frame(minWidth: 13)
            }
        }
        .font(.system(size: 11, weight: .medium, design: .rounded))
    }
}

struct StatuslineSnapshot {
    let left: String
    let center: String
    let right: String
    let style: UiStyleSnapshot
    let panelStyle: UiStyleSnapshot
}

struct StatuslineView: View {
    let snapshot: StatuslineSnapshot
    let cellSize: CGSize

    private var statuslineHeight: CGFloat { 22 }

    private var modeName: String {
        snapshot.left.components(separatedBy: " ").first ?? ""
    }

    private var rawFilename: String {
        let parts = snapshot.left.components(separatedBy: " ")
        return parts.dropFirst().joined(separator: " ")
    }

    private var isModified: Bool {
        rawFilename.contains("[+]")
    }

    private var filename: String {
        rawFilename
            .replacingOccurrences(of: " [+]", with: "")
            .replacingOccurrences(of: "[+]", with: "")
    }

    private var modeColor: Color {
        switch modeName.uppercased() {
        case "NORMAL", "N":
            return Color(nsColor: .tertiaryLabelColor)
        case "INSERT", "I":
            return Color(nsColor: .secondaryLabelColor)
        case "VISUAL", "V", "VISUAL LINE", "VISUAL BLOCK":
            return Color(nsColor: .labelColor)
        case "COMMAND", "C", ":":
            return Color(nsColor: .secondaryLabelColor)
        default:
            return Color(nsColor: .tertiaryLabelColor)
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            Rectangle()
                .fill(Color(nsColor: .separatorColor))
                .frame(height: 0.5)
                .opacity(0.25)

            HStack(spacing: 0) {
                Text(modeName)
                    .font(.system(size: 11, weight: .medium))
                    .foregroundColor(modeColor)
                    .frame(minWidth: 48, alignment: .leading)

                if !filename.isEmpty {
                    Text(filename)
                        .font(.system(size: 12))
                        .foregroundStyle(.primary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }

                if isModified {
                    Circle()
                        .fill(Color(nsColor: .tertiaryLabelColor))
                        .frame(width: 6, height: 6)
                        .padding(.leading, 6)
                }

                Spacer(minLength: 8)

                if !snapshot.center.isEmpty {
                    Text(snapshot.center)
                        .font(.system(size: 11))
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)

                    Spacer(minLength: 8)
                }

                Text(snapshot.right)
                    .font(.system(size: 11).monospacedDigit())
                    .foregroundStyle(.secondary)
            }
            .padding(.horizontal, 12)
            .frame(height: statuslineHeight - 0.5)
        }
        .frame(height: statuslineHeight)
    }
}


extension UiTreeSnapshot {
    func commandPaletteSnapshot() -> CommandPaletteSnapshot? {
        guard let panel = commandPalettePanel() else {
            return nil
        }

        let input = findInput(in: panel.child, id: "command_palette_input")
        let list = findList(in: panel.child, id: "command_palette_list")

        var query = input?.value ?? ""
        if query.hasPrefix(":") {
            query.removeFirst()
        }

        let items = list?.items ?? []
        let paletteItems = items.enumerated().map { index, item in
            CommandPaletteItemSnapshot(
                id: index,
                title: item.title,
                subtitle: item.subtitle,
                description: item.description,
                shortcut: item.shortcut,
                badge: item.badge,
                leadingIcon: item.leadingIcon,
                leadingColor: uiColorToColor(item.leadingColor),
                symbols: item.symbols,
                emphasis: item.emphasis
            )
        }

        return CommandPaletteSnapshot(
            isOpen: true,
            query: query,
            selectedIndex: list?.selected,
            items: paletteItems,
            layout: CommandPaletteLayout.from(intent: panel.intent)
        )
    }

    func statuslineSnapshot() -> StatuslineSnapshot? {
        let panel = findPanel(in: root, id: "statusline") ?? overlays.compactMap { node in
            if case .panel(let panel) = node, panel.id == "statusline" { return panel }
            return nil
        }.first

        guard let panel else {
            return nil
        }

        guard let status = findStatusBar(in: panel.child) else {
            return nil
        }

        return StatuslineSnapshot(
            left: status.left,
            center: status.center,
            right: status.right,
            style: status.style,
            panelStyle: panel.style
        )
    }

    var hasCommandPalettePanel: Bool {
        return commandPalettePanel() != nil
    }

    private func commandPalettePanel() -> UiPanelSnapshot? {
        if let panel = findPanel(in: root, id: "command_palette") {
            return panel
        }
        for node in overlays {
            if let panel = findPanel(in: node, id: "command_palette") {
                return panel
            }
        }
        return nil
    }

    private func findInput(in node: UiNodeSnapshot, id: String) -> UiInputSnapshot? {
        switch node {
        case .input(let input):
            return input.id == id ? input : nil
        case .container(let container):
            for child in container.children {
                if let found = findInput(in: child, id: id) {
                    return found
                }
            }
            return nil
        case .panel(let panel):
            return findInput(in: panel.child, id: id)
        default:
            return nil
        }
    }

    private func findList(in node: UiNodeSnapshot, id: String) -> UiListSnapshot? {
        switch node {
        case .list(let list):
            return list.id == id ? list : nil
        case .container(let container):
            for child in container.children {
                if let found = findList(in: child, id: id) {
                    return found
                }
            }
            return nil
        case .panel(let panel):
            return findList(in: panel.child, id: id)
        default:
            return nil
        }
    }

    private func findPanel(in node: UiNodeSnapshot, id: String) -> UiPanelSnapshot? {
        switch node {
        case .panel(let panel):
            if panel.id == id {
                return panel
            }
            return findPanel(in: panel.child, id: id)
        case .container(let container):
            for child in container.children {
                if let found = findPanel(in: child, id: id) {
                    return found
                }
            }
            return nil
        default:
            return nil
        }
    }

    private func findStatusBar(in node: UiNodeSnapshot) -> UiStatusBarSnapshot? {
        switch node {
        case .statusBar(let status):
            return status
        case .panel(let panel):
            return findStatusBar(in: panel.child)
        case .container(let container):
            for child in container.children {
                if let status = findStatusBar(in: child) {
                    return status
                }
            }
            return nil
        default:
            return nil
        }
    }

}

extension CommandPaletteLayout {
    static func from(intent: LayoutIntentSnapshot) -> CommandPaletteLayout {
        switch intent {
        case .bottom:
            return .bottom
        case .top:
            return .top
        case .floating:
            return .floating
        case .custom:
            return .custom
        case .sidebarLeft, .sidebarRight, .fullscreen, .unknown:
            return .floating
        }
    }
}

fileprivate func uiColorToColor(_ uiColor: UiColorSnapshot?) -> Color? {
    guard let uiColor else { return nil }
    switch uiColor {
    case .value(let value):
        return color(for: value)
    case .token:
        return nil
    case .unknown:
        return nil
    }
}

fileprivate func resolveColor(_ uiColor: UiColorSnapshot?, fallback: Color) -> Color {
    guard let uiColor else { return fallback }
    switch uiColor {
    case .value(let value):
        return color(for: value)
    case .token:
        return fallback
    case .unknown:
        return fallback
    }
}

fileprivate func color(for value: ColorSnapshot) -> Color {
    switch value {
    case .reset:
        return Color.clear
    case .black:
        return Color.black
    case .red:
        return Color.red
    case .green:
        return Color.green
    case .yellow:
        return Color.yellow
    case .blue:
        return Color.blue
    case .magenta:
        return Color.purple
    case .cyan:
        return Color.cyan
    case .gray:
        return Color.gray
    case .lightRed:
        return Color.red.opacity(0.8)
    case .lightGreen:
        return Color.green.opacity(0.8)
    case .lightYellow:
        return Color.yellow.opacity(0.8)
    case .lightBlue:
        return Color.blue.opacity(0.8)
    case .lightMagenta:
        return Color.purple.opacity(0.8)
    case .lightCyan:
        return Color.cyan.opacity(0.8)
    case .lightGray:
        return Color.gray.opacity(0.8)
    case .white:
        return Color.white
    case .rgb(let r, let g, let b):
        return Color(red: Double(r) / 255.0, green: Double(g) / 255.0, blue: Double(b) / 255.0)
    case .indexed(let index):
        return xterm256Color(index: Int(index)) ?? Color.gray
    case .unknown:
        return Color.clear
    }
}

fileprivate func xterm256Color(index: Int) -> Color? {
    if index < 0 {
        return nil
    }

    if index < 16 {
        let palette: [Color] = [
            .black, .red, .green, .yellow, .blue, .purple, .cyan, .gray,
            .red.opacity(0.8), .green.opacity(0.8), .yellow.opacity(0.8),
            .blue.opacity(0.8), .purple.opacity(0.8), .cyan.opacity(0.8),
            .gray.opacity(0.9), .white
        ]
        return palette[index]
    }

    if index >= 232 {
        let level = Double(index - 232) / 23.0
        return Color(white: level)
    }

    let idx = index - 16
    let r = idx / 36
    let g = (idx % 36) / 6
    let b = idx % 6
    func component(_ value: Int) -> Double {
        let levels: [Double] = [0.0, 0.37, 0.58, 0.74, 0.87, 1.0]
        return levels[min(max(value, 0), levels.count - 1)]
    }

    return Color(red: component(r), green: component(g), blue: component(b))
}
