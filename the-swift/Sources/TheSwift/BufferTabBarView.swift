import SwiftUI

struct BufferTabItemSnapshot: Identifiable, Decodable, Equatable {
    let bufferId: UInt64
    let bufferIndex: Int
    let title: String
    let modified: Bool
    let isActive: Bool
    let filePath: String?
    let directoryHint: String?

    var id: UInt64 { bufferId }
}

struct BufferTabsSnapshot: Decodable, Equatable {
    let visible: Bool
    let activeTab: Int?
    let activeBufferIndex: Int?
    let tabs: [BufferTabItemSnapshot]
}

struct BufferTabBarTheme {
    let barBackground: SwiftUI.Color
    let barBorder: SwiftUI.Color
    let tabActiveBackground: SwiftUI.Color
    let tabActiveForeground: SwiftUI.Color
    let tabInactiveBackground: SwiftUI.Color
    let tabInactiveForeground: SwiftUI.Color
    let tabHoverBackground: SwiftUI.Color
    let tabStroke: SwiftUI.Color
    let tabStrokeActive: SwiftUI.Color
    let modifiedIndicator: SwiftUI.Color
    let directoryText: SwiftUI.Color

    static let fallback = BufferTabBarTheme(
        barBackground: SwiftUI.Color(red: 0.09, green: 0.10, blue: 0.13),
        barBorder: SwiftUI.Color.white.opacity(0.08),
        tabActiveBackground: SwiftUI.Color(red: 0.14, green: 0.24, blue: 0.43),
        tabActiveForeground: .white,
        tabInactiveBackground: .clear,
        tabInactiveForeground: SwiftUI.Color.white.opacity(0.78),
        tabHoverBackground: SwiftUI.Color.white.opacity(0.08),
        tabStroke: SwiftUI.Color.white.opacity(0.06),
        tabStrokeActive: SwiftUI.Color.white.opacity(0.14),
        modifiedIndicator: SwiftUI.Color(red: 0.99, green: 0.72, blue: 0.25),
        directoryText: SwiftUI.Color.white.opacity(0.48)
    )
}

struct BufferTabBarView: View {
    let snapshot: BufferTabsSnapshot
    let theme: BufferTabBarTheme
    let onSelect: (Int) -> Void

    @State private var hoveredTab: Int? = nil

    var body: some View {
        ZStack(alignment: .bottom) {
            theme.barBackground
            Rectangle()
                .fill(theme.barBorder)
                .frame(height: 1)
        }
        .overlay {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    ForEach(snapshot.tabs) { tab in
                        tabButton(tab)
                    }
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 3)
            }
        }
    }

    @ViewBuilder
    private func tabButton(_ tab: BufferTabItemSnapshot) -> some View {
        let isHovered = hoveredTab == tab.bufferIndex
        let baseText = tab.isActive ? theme.tabActiveForeground : theme.tabInactiveForeground
        let bg = tab.isActive
            ? theme.tabActiveBackground
            : (isHovered ? theme.tabHoverBackground : theme.tabInactiveBackground)

        Button {
            onSelect(tab.bufferIndex)
        } label: {
            HStack(spacing: 6) {
                if tab.modified {
                    Circle()
                        .fill(theme.modifiedIndicator)
                        .frame(width: 6, height: 6)
                }

                Text(tab.title)
                    .lineLimit(1)
                    .truncationMode(.tail)

                if let directoryHint = tab.directoryHint, !directoryHint.isEmpty {
                    Text(directoryHint)
                        .lineLimit(1)
                        .foregroundStyle(theme.directoryText)
                }
            }
            .font(.system(size: 12, weight: tab.isActive ? .semibold : .medium, design: .rounded))
            .foregroundStyle(baseText)
            .padding(.horizontal, 10)
            .padding(.vertical, 5)
            .background(
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(bg)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .stroke(tab.isActive ? theme.tabStrokeActive : theme.tabStroke, lineWidth: 1)
            )
            .contentShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
        }
        .buttonStyle(.plain)
        .onHover { inside in
            hoveredTab = inside ? tab.bufferIndex : (hoveredTab == tab.bufferIndex ? nil : hoveredTab)
        }
    }
}
