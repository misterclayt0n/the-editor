import SwiftUI

struct BufferTabItemSnapshot: Identifiable, Decodable, Equatable {
    let bufferIndex: Int
    let title: String
    let modified: Bool
    let isActive: Bool
    let filePath: String?
    let directoryHint: String?

    var id: Int { bufferIndex }
}

struct BufferTabsSnapshot: Decodable, Equatable {
    let visible: Bool
    let activeTab: Int?
    let activeBufferIndex: Int?
    let tabs: [BufferTabItemSnapshot]
}

struct BufferTabBarView: View {
    let snapshot: BufferTabsSnapshot
    let onSelect: (Int) -> Void

    @State private var hoveredTab: Int? = nil

    var body: some View {
        ZStack(alignment: .bottom) {
            LinearGradient(
                colors: [
                    SwiftUI.Color(red: 0.09, green: 0.10, blue: 0.13),
                    SwiftUI.Color(red: 0.07, green: 0.08, blue: 0.10)
                ],
                startPoint: .top,
                endPoint: .bottom
            )
            Rectangle()
                .fill(SwiftUI.Color.white.opacity(0.08))
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
        let baseText = tab.isActive ? SwiftUI.Color.white : SwiftUI.Color.white.opacity(0.78)
        let bg = tab.isActive
            ? SwiftUI.Color(red: 0.14, green: 0.24, blue: 0.43)
            : (isHovered ? SwiftUI.Color.white.opacity(0.08) : SwiftUI.Color.clear)

        Button {
            onSelect(tab.bufferIndex)
        } label: {
            HStack(spacing: 6) {
                if tab.modified {
                    Circle()
                        .fill(SwiftUI.Color(red: 0.99, green: 0.72, blue: 0.25))
                        .frame(width: 6, height: 6)
                }

                Text(tab.title)
                    .lineLimit(1)
                    .truncationMode(.tail)

                if let directoryHint = tab.directoryHint, !directoryHint.isEmpty {
                    Text(directoryHint)
                        .lineLimit(1)
                        .foregroundStyle(baseText.opacity(0.58))
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
                    .stroke(SwiftUI.Color.white.opacity(tab.isActive ? 0.14 : 0.06), lineWidth: 1)
            )
            .contentShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
        }
        .buttonStyle(.plain)
        .onHover { inside in
            hoveredTab = inside ? tab.bufferIndex : (hoveredTab == tab.bufferIndex ? nil : hoveredTab)
        }
    }
}
