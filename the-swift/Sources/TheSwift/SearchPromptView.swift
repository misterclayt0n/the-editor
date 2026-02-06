import SwiftUI

struct SearchPromptLayout {
    var corner: SearchPromptView.Corner = .topRight
}

struct SearchPromptSnapshot {
    let isOpen: Bool
    let query: String
    let error: String?

    static let closed = SearchPromptSnapshot(
        isOpen: false,
        query: "",
        error: nil
    )
}

struct SearchPromptView: View {
    let snapshot: SearchPromptSnapshot
    @Binding var layout: SearchPromptLayout
    let onQueryChange: (String) -> Void
    let onSearchPrev: () -> Void
    let onSearchNext: () -> Void
    let onClose: () -> Void
    let onSubmit: () -> Void

    @State private var query: String = ""
    @State private var dragOffset: CGSize = .zero
    @State private var barSize: CGSize = .zero
    @FocusState private var isSearchFieldFocused: Bool

    private let padding: CGFloat = 8

    var body: some View {
        GeometryReader { geo in
            searchBar
                .background(
                    GeometryReader { barGeo in
                        Color.clear.onAppear {
                            barSize = barGeo.size
                        }
                    }
                )
                .padding(padding)
                .offset(dragOffset)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: layout.corner.alignment)
                .gesture(
                    DragGesture()
                        .onChanged { value in
                            dragOffset = value.translation
                        }
                        .onEnded { value in
                            let center = centerPosition(for: layout.corner, in: geo.size)
                            let newCenter = CGPoint(
                                x: center.x + value.translation.width,
                                y: center.y + value.translation.height
                            )
                            let newCorner = closestCorner(to: newCenter, in: geo.size)
                            withAnimation(.easeOut(duration: 0.2)) {
                                layout.corner = newCorner
                                dragOffset = .zero
                            }
                        }
                )
        }
    }

    private var searchBar: some View {
        HStack(spacing: 4) {
            TextField("Search", text: $query)
                .textFieldStyle(.plain)
                .frame(width: 180)
                .padding(.leading, 8)
                .padding(.trailing, 8)
                .padding(.vertical, 6)
                .background(Color.primary.opacity(0.1))
                .cornerRadius(6)
                .focused($isSearchFieldFocused)
                .overlay(alignment: .trailing) {
                    if let error = snapshot.error, !error.isEmpty {
                        Image(systemName: "exclamationmark.triangle")
                            .font(.caption)
                            .foregroundColor(.red.opacity(0.8))
                            .padding(.trailing, 8)
                    }
                }
                .onExitCommand {
                    onClose()
                }
                .onSubmit {
                    onSubmit()
                }
                .onChange(of: query) { newValue in
                    if newValue != snapshot.query {
                        onQueryChange(newValue)
                    }
                }

            Button(action: onSearchPrev) {
                Image(systemName: "chevron.up")
            }
            .buttonStyle(SearchBarButtonStyle())

            Button(action: onSearchNext) {
                Image(systemName: "chevron.down")
            }
            .buttonStyle(SearchBarButtonStyle())

            Button(action: onClose) {
                Image(systemName: "xmark")
            }
            .buttonStyle(SearchBarButtonStyle())
        }
        .padding(8)
        .background(.background)
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .shadow(radius: 4)
        .onAppear {
            query = snapshot.query
            DispatchQueue.main.async {
                isSearchFieldFocused = true
            }
        }
        .onChange(of: snapshot.query) { newValue in
            if newValue != query {
                query = newValue
            }
        }
        .onChange(of: snapshot.isOpen) { isOpen in
            if isOpen {
                DispatchQueue.main.async {
                    isSearchFieldFocused = true
                }
            }
        }
    }

    private func centerPosition(for corner: Corner, in containerSize: CGSize) -> CGPoint {
        let halfWidth = barSize.width / 2 + padding
        let halfHeight = barSize.height / 2 + padding

        switch corner {
        case .topLeft:
            return CGPoint(x: halfWidth, y: halfHeight)
        case .topRight:
            return CGPoint(x: containerSize.width - halfWidth, y: halfHeight)
        case .bottomLeft:
            return CGPoint(x: halfWidth, y: containerSize.height - halfHeight)
        case .bottomRight:
            return CGPoint(x: containerSize.width - halfWidth, y: containerSize.height - halfHeight)
        }
    }

    private func closestCorner(to point: CGPoint, in containerSize: CGSize) -> Corner {
        let midX = containerSize.width / 2
        let midY = containerSize.height / 2

        if point.x < midX {
            return point.y < midY ? .topLeft : .bottomLeft
        } else {
            return point.y < midY ? .topRight : .bottomRight
        }
    }

    enum Corner {
        case topLeft, topRight, bottomLeft, bottomRight

        var alignment: Alignment {
            switch self {
            case .topLeft: return .topLeading
            case .topRight: return .topTrailing
            case .bottomLeft: return .bottomLeading
            case .bottomRight: return .bottomTrailing
            }
        }
    }
}

fileprivate struct SearchBarButtonStyle: ButtonStyle {
    @State private var isHovered = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundStyle(isHovered || configuration.isPressed ? .primary : .secondary)
            .padding(.horizontal, 2)
            .frame(height: 26)
            .background(
                RoundedRectangle(cornerRadius: 6)
                    .fill(backgroundColor(isPressed: configuration.isPressed))
            )
            .onHover { hovering in
                isHovered = hovering
            }
    }

    private func backgroundColor(isPressed: Bool) -> Color {
        if isPressed {
            return Color.primary.opacity(0.2)
        } else if isHovered {
            return Color.primary.opacity(0.1)
        } else {
            return Color.clear
        }
    }
}
