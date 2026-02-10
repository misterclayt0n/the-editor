import SwiftUI

struct KeySequenceIndicator: View {
    let keys: [String]

    var body: some View {
        ZStack {
            if !keys.isEmpty {
                indicatorContent
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .animation(.spring(response: 0.3, dampingFraction: 0.8), value: keys.isEmpty)
        .animation(.spring(response: 0.3, dampingFraction: 0.8), value: keys.count)
    }

    private var indicatorContent: some View {
        HStack(alignment: .center, spacing: 8) {
            Image(systemName: "keyboard.badge.ellipsis")
                .font(.system(size: 13))
                .foregroundStyle(.secondary)

            HStack(alignment: .center, spacing: 4) {
                ForEach(Array(keys.enumerated()), id: \.offset) { _, key in
                    KeyCapView(key)
                }
                PendingIndicator()
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background {
            Capsule()
                .fill(.regularMaterial)
                .overlay {
                    Capsule()
                        .strokeBorder(Color.primary.opacity(0.15), lineWidth: 1)
                }
                .shadow(color: .black.opacity(0.2), radius: 8, y: 2)
        }
    }
}

struct KeyCapView: View {
    let text: String

    init(_ text: String) {
        self.text = text
    }

    var body: some View {
        Text(verbatim: text)
            .font(.system(size: 12, weight: .medium, design: .rounded))
            .padding(.horizontal, 5)
            .padding(.vertical, 2)
            .background(
                RoundedRectangle(cornerRadius: 4)
                    .fill(Color(NSColor.controlBackgroundColor))
                    .shadow(color: .black.opacity(0.12), radius: 0.5, y: 0.5)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 4)
                    .strokeBorder(Color.primary.opacity(0.15), lineWidth: 0.5)
            )
    }
}

struct PendingIndicator: View {
    @State private var animationPhase: Double = 0

    var body: some View {
        TimelineView(.animation) { context in
            HStack(spacing: 2) {
                ForEach(0..<3, id: \.self) { index in
                    Circle()
                        .fill(Color.secondary)
                        .frame(width: 4, height: 4)
                        .opacity(dotOpacity(for: index))
                }
            }
            .onChange(of: context.date.timeIntervalSinceReferenceDate) { newValue in
                animationPhase = newValue
            }
        }
    }

    private func dotOpacity(for index: Int) -> Double {
        let offset = Double(index) / 3.0
        let wave = sin((animationPhase + offset) * .pi * 2)
        return 0.3 + 0.7 * ((wave + 1) / 2)
    }
}
