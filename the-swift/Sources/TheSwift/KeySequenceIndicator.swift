import SwiftUI

struct PendingKeyHintsSnapshot: Decodable {
    let pending: [String]
    let scope: String?
    let options: [PendingKeyHintOption]
}

struct PendingKeyHintOption: Decodable, Identifiable {
    let key: String
    let label: String
    let kind: String

    var id: String {
        "\(key)|\(label)|\(kind)"
    }

    var isPrefix: Bool {
        kind == "prefix"
    }
}

struct KeySequenceIndicator: View {
    let keys: [String]
    let hints: PendingKeyHintsSnapshot?

    @State private var isShowingPopover = false
    @State private var hoverWorkItem: DispatchWorkItem?

    private var displayKeys: [String] {
        if let hints, !hints.pending.isEmpty {
            return hints.pending
        }
        return keys
    }

    private var options: [PendingKeyHintOption] {
        hints?.options ?? []
    }

    private var canShowPopover: Bool {
        !options.isEmpty
    }

    var body: some View {
        ZStack {
            if !displayKeys.isEmpty {
                indicatorContent
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .animation(.spring(response: 0.3, dampingFraction: 0.8), value: displayKeys.isEmpty)
        .animation(.spring(response: 0.3, dampingFraction: 0.8), value: displayKeys.count)
        .onChange(of: displayKeys.count) { count in
            if count == 0 {
                dismissPopover()
            }
        }
        .onChange(of: canShowPopover) { canShow in
            if !canShow {
                dismissPopover()
            }
        }
    }

    private var indicatorContent: some View {
        HStack(alignment: .center, spacing: 8) {
            Image(systemName: "keyboard.badge.ellipsis")
                .font(.system(size: 13))
                .foregroundStyle(.secondary)

            HStack(alignment: .center, spacing: 4) {
                ForEach(Array(displayKeys.enumerated()), id: \.offset) { _, key in
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
        .contentShape(Capsule())
        .onTapGesture {
            guard canShowPopover else { return }
            isShowingPopover.toggle()
        }
        .onHover { hovering in
            handleHover(hovering)
        }
        .popover(isPresented: $isShowingPopover, arrowEdge: .bottom) {
            popoverContent
        }
    }

    private var popoverContent: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let scope = hints?.scope, !scope.isEmpty {
                Label(scope, systemImage: "keyboard.badge.ellipsis")
                    .font(.headline)
            } else {
                Label("Available keys", systemImage: "keyboard.badge.ellipsis")
                    .font(.headline)
            }

            if !options.isEmpty {
                ScrollView {
                    VStack(alignment: .leading, spacing: 6) {
                        ForEach(options) { option in
                            HStack(alignment: .center, spacing: 8) {
                                KeyCapView(option.key)

                                Text(option.label)
                                    .font(.system(size: 13))
                                    .foregroundStyle(.primary)
                                    .lineLimit(1)

                                if option.isPrefix {
                                    Text("prefix")
                                        .font(.system(size: 11, weight: .medium))
                                        .foregroundStyle(.secondary)
                                        .padding(.horizontal, 6)
                                        .padding(.vertical, 2)
                                        .background(
                                            Capsule()
                                                .fill(Color.secondary.opacity(0.12))
                                        )
                                }

                                Spacer(minLength: 0)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }
                }
                .frame(maxHeight: 260)
            }
        }
        .padding()
        .frame(minWidth: 320, maxWidth: 420)
    }

    private func handleHover(_ hovering: Bool) {
        hoverWorkItem?.cancel()

        if hovering {
            guard canShowPopover else { return }
            let task = DispatchWorkItem {
                isShowingPopover = true
            }
            hoverWorkItem = task
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.2, execute: task)
        } else {
            isShowingPopover = false
        }
    }

    private func dismissPopover() {
        hoverWorkItem?.cancel()
        hoverWorkItem = nil
        isShowingPopover = false
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
