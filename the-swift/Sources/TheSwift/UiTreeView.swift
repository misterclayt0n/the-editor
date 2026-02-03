import SwiftUI

struct UiOverlayHost: View {
    let tree: UiTreeSnapshot
    let commandPalette: CommandPaletteSnapshot
    let cellSize: CGSize
    let onSelectCommand: (Int) -> Void
    let onSubmitCommand: (Int) -> Void
    let onCloseCommandPalette: () -> Void
    let onQueryChange: (String) -> Void

    var body: some View {
        GeometryReader { proxy in
            ZStack {
                ForEach(Array(tree.overlays.enumerated()), id: \.offset) { _, node in
                    if case .panel(let panel) = node, panel.id == "command_palette" {
                        if commandPalette.isOpen {
                            CommandPaletteView(
                                snapshot: commandPalette,
                                onSelect: onSelectCommand,
                                onSubmit: onSubmitCommand,
                                onClose: onCloseCommandPalette,
                                onQueryChange: onQueryChange
                            )
                        }
                    } else {
                        UiNodeView(node: node, cellSize: cellSize, containerSize: proxy.size)
                    }
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
            Divider().background(Color.white.opacity(0.15))
        case .spacer(let spacer):
            Spacer().frame(height: CGFloat(spacer.size) * cellSize.height)
        case .tooltip(let tooltip):
            tooltipView(tooltip)
        case .statusBar(let status):
            statusBarView(status)
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

        UiNodeView(node: panel.child, cellSize: cellSize, containerSize: containerSize)
            .padding(EdgeInsets(
                top: CGFloat(padding.top) * cellSize.height,
                leading: CGFloat(padding.left) * cellSize.width,
                bottom: CGFloat(padding.bottom) * cellSize.height,
                trailing: CGFloat(padding.right) * cellSize.width
            ))
            .frame(minWidth: minWidth, idealWidth: nil, maxWidth: maxWidth,
                   minHeight: minHeight, idealHeight: nil, maxHeight: maxHeight,
                   alignment: .topLeading)
            .background(resolveColor(panel.style.bg, fallback: Color.black.opacity(0.7)))
            .overlay(
                RoundedRectangle(cornerRadius: radius(panel.style.radius))
                    .stroke(resolveColor(panel.style.border, fallback: Color.white.opacity(0.1)), lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: radius(panel.style.radius)))
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
            .foregroundColor(resolveColor(text.style.fg, fallback: Color.white))
            .font(font(for: text.style))
            .frame(maxWidth: .infinity, alignment: .leading)
    }

    @ViewBuilder
    private func listView(_ list: UiListSnapshot) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(Array(list.items.enumerated()), id: \.offset) { index, item in
                let isSelected = list.selected == index
                VStack(alignment: .leading, spacing: 2) {
                    Text(item.title)
                        .foregroundColor(isSelected ? resolveColor(list.style.fg, fallback: Color.white) : Color.white)
                        .font(.system(size: 14, weight: .semibold))
                    if let subtitle = item.subtitle, !subtitle.isEmpty {
                        Text(subtitle)
                            .foregroundColor(Color.gray)
                            .font(.system(size: 12))
                    } else if let description = item.description, !description.isEmpty {
                        Text(description)
                            .foregroundColor(Color.gray)
                            .font(.system(size: 12))
                    }
                }
                .padding(.vertical, 4)
                .padding(.horizontal, 8)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(isSelected ? resolveColor(list.style.bg, fallback: Color.blue.opacity(0.35)) : Color.clear)
                .clipShape(RoundedRectangle(cornerRadius: 6))
            }
        }
    }

    @ViewBuilder
    private func inputView(_ input: UiInputSnapshot) -> some View {
        let display = input.value.isEmpty ? (input.placeholder ?? "") : input.value
        Text(display)
            .foregroundColor(resolveColor(input.style.fg, fallback: Color.white.opacity(input.value.isEmpty ? 0.6 : 1.0)))
            .font(.system(size: 14, weight: .light))
            .frame(maxWidth: .infinity, alignment: .leading)
    }

    @ViewBuilder
    private func tooltipView(_ tooltip: UiTooltipSnapshot) -> some View {
        Text(tooltip.content)
            .padding(8)
            .background(resolveColor(tooltip.style.bg, fallback: Color.black.opacity(0.85)))
            .foregroundColor(resolveColor(tooltip.style.fg, fallback: Color.white))
            .clipShape(RoundedRectangle(cornerRadius: radius(tooltip.style.radius)))
    }

    @ViewBuilder
    private func statusBarView(_ status: UiStatusBarSnapshot) -> some View {
        HStack {
            Text(status.left)
            Spacer()
            Text(status.center)
            Spacer()
            Text(status.right)
        }
        .font(.system(size: 12))
        .foregroundColor(resolveColor(status.style.fg, fallback: Color.white.opacity(0.8)))
        .padding(4)
        .background(resolveColor(status.style.bg, fallback: Color.black.opacity(0.7)))
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

    private func resolveColor(_ uiColor: UiColorSnapshot?, fallback: Color) -> Color {
        guard let uiColor else { return fallback }
        switch uiColor {
        case .token(let token):
            return self.color(for: token)
        case .value(let value):
            return self.color(for: value)
        case .unknown:
            return fallback
        }
    }

    private func color(for token: UiColorTokenSnapshot) -> Color {
        switch token {
        case .text:
            return Color.white
        case .mutedText:
            return Color.gray
        case .panelBg:
            return Color.black.opacity(0.75)
        case .panelBorder:
            return Color.white.opacity(0.2)
        case .accent:
            return Color.blue
        case .selectedBg:
            return Color.blue.opacity(0.4)
        case .selectedText:
            return Color.white
        case .divider:
            return Color.white.opacity(0.2)
        case .placeholder:
            return Color.gray.opacity(0.8)
        }
    }

    private func color(for value: ColorSnapshot) -> Color {
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
        case .indexed:
            return Color.gray
        case .unknown:
            return Color.clear
        }
    }

    private func font(for style: UiStyleSnapshot) -> Font {
        switch style.emphasis {
        case .strong:
            return .system(size: 14, weight: .bold)
        case .muted:
            return .system(size: 13, weight: .light)
        case .normal:
            return .system(size: 14)
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
}
