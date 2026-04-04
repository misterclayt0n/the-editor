import AppKit
import SwiftUI

private let completionPanelEdgePadding: CGFloat = 8
private let completionRowHeight: CGFloat = 24
private let completionHorizontalPadding: CGFloat = 10

struct EditorCompletionMenuView: View {
    @ObservedObject var controller: EditorSurfaceController

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .topLeading) {
                if let scene = controller.scene, controller.completionMenu.isOpen {
                    let frame = completionFrame(scene: scene, state: controller.completionMenu)
                    EditorPopoverPanel(frame: frame, backgroundColor: controller.chrome.backgroundColor) {
                        EditorCompletionListPanel(
                            controller: controller,
                            completion: controller.completionMenu,
                            frameWidth: frame.width
                        )
                    }
                    .zIndex(3)

                    if controller.completionDocs.isOpen {
                        EditorDocsPanelOverlay(
                            kind: .completionDocs,
                            panel: controller.completionDocs,
                            scene: scene,
                            backgroundColor: controller.chrome.backgroundColor,
                            onEscape: controller.closeCompletionMenu
                        )
                        .zIndex(4)
                    }
                }
            }
            .frame(width: geometry.size.width, height: geometry.size.height, alignment: .topLeading)
        }
        .allowsHitTesting(true)
    }

    private func completionFrame(scene: EditorRenderScene, state: EditorCompletionMenuState) -> CGRect {
        let metrics = scene.info.surfaceMetrics
        let viewportSize = CGSize(
            width: CGFloat(scene.info.viewportWidth) * metrics.cellSizePoints.width,
            height: CGFloat(scene.info.viewportHeight) * metrics.cellSizePoints.height
        )
        let baseOrigin = CGPoint(
            x: CGFloat(state.col) * metrics.cellSizePoints.width,
            y: CGFloat(state.row) * metrics.cellSizePoints.height
        )
        let exportedSize = CGSize(
            width: CGFloat(state.width) * metrics.cellSizePoints.width,
            height: CGFloat(state.height) * metrics.cellSizePoints.height
        )
        let fittedWidth = min(max(contentWidth(for: state), 260), min(max(exportedSize.width, 260), 460))
        let fittedHeight = min(
            max(CGFloat(min(state.items.count, 10)) * completionRowHeight + 8, exportedSize.height),
            max(viewportSize.height - completionPanelEdgePadding * 2, completionRowHeight)
        )
        let x = min(max(baseOrigin.x, completionPanelEdgePadding), max(viewportSize.width - fittedWidth - completionPanelEdgePadding, completionPanelEdgePadding))
        let y = min(max(baseOrigin.y, completionPanelEdgePadding), max(viewportSize.height - fittedHeight - completionPanelEdgePadding, completionPanelEdgePadding))
        return CGRect(x: x, y: y, width: fittedWidth, height: fittedHeight)
    }

    private func contentWidth(for state: EditorCompletionMenuState) -> CGFloat {
        let titleFont = NSFont.systemFont(ofSize: 12, weight: .medium)
        let subtitleFont = NSFont.systemFont(ofSize: 11, weight: .regular)
        let iconWidth: CGFloat = state.items.contains(where: { $0.leadingIcon != nil }) ? 18 : 0
        let widest = state.items.reduce(CGFloat.zero) { partial, item in
            let titleWidth = (item.title as NSString).size(withAttributes: [.font: titleFont]).width
            let subtitleWidth = item.subtitle.map { ($0 as NSString).size(withAttributes: [.font: subtitleFont]).width } ?? 0
            return max(partial, titleWidth + subtitleWidth + iconWidth + completionHorizontalPadding * 2 + 16)
        }
        return widest
    }
}

private struct EditorCompletionListPanel: View {
    @ObservedObject var controller: EditorSurfaceController
    let completion: EditorCompletionMenuState
    let frameWidth: CGFloat

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView(.vertical) {
                LazyVStack(spacing: 0) {
                    ForEach(completion.items) { item in
                        EditorCompletionRow(
                            item: item,
                            isSelected: completion.selectedIndex == item.index,
                            onSelect: { controller.selectCompletionMenuIndex(item.index) },
                            onSubmit: {
                                controller.selectCompletionMenuIndex(item.index)
                                controller.submitCompletionMenu()
                            }
                        )
                        .id(item.index)
                    }
                }
                .padding(.vertical, 4)
            }
            .onAppear {
                syncScroll(into: proxy)
            }
            .onChange(of: completion.scrollOffset) { _, _ in
                syncScroll(into: proxy)
            }
        }
        .frame(width: frameWidth)
    }

    private func syncScroll(into proxy: ScrollViewProxy) {
        guard !completion.items.isEmpty else { return }
        let target = min(max(completion.scrollOffset, 0), completion.items.count - 1)
        withAnimation(.easeInOut(duration: 0.12)) {
            proxy.scrollTo(target, anchor: .top)
        }
    }
}

private struct EditorCompletionRow: View {
    let item: EditorCompletionMenuItem
    let isSelected: Bool
    let onSelect: () -> Void
    let onSubmit: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            if let icon = item.leadingIcon {
                Text(icon)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(Color(nsColor: item.leadingColor?.color ?? .secondaryLabelColor))
                    .frame(width: 14, alignment: .center)
            }

            Text(item.title)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(isSelected ? .primary : .primary)
                .lineLimit(1)

            if let subtitle = item.subtitle, !subtitle.isEmpty {
                Text(subtitle)
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }

            Spacer(minLength: 0)
        }
        .padding(.horizontal, completionHorizontalPadding)
        .frame(maxWidth: .infinity, minHeight: completionRowHeight, alignment: .leading)
        .background(selectionBackground)
        .contentShape(Rectangle())
        .onHover { isHovering in
            if isHovering {
                onSelect()
            }
        }
        .onTapGesture(count: 2, perform: onSubmit)
        .onTapGesture(perform: onSelect)
    }

    @ViewBuilder
    private var selectionBackground: some View {
        if isSelected {
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .fill(Color.accentColor.opacity(0.18))
                .padding(.horizontal, 4)
                .padding(.vertical, 1)
        } else {
            Color.clear
        }
    }
}
