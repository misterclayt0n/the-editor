import AppKit
import SwiftUI

struct EditorInputPromptView: View {
    @ObservedObject var controller: EditorSurfaceController

    var body: some View {
        Group {
            if controller.inputPrompt.isOpen {
                EditorInputPromptPanel(
                    prompt: controller.inputPrompt,
                    backgroundColor: controller.chrome.backgroundColor,
                    onQueryChange: controller.setInputPromptQuery,
                    onSubmit: controller.submitInputPrompt,
                    onClose: controller.closeInputPrompt,
                    onNext: controller.stepInputPromptNext,
                    onPrevious: controller.stepInputPromptPrevious
                )
                .padding(8)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing)
                .transition(.opacity)
            }
        }
        .animation(.easeOut(duration: 0.12), value: controller.inputPrompt.isOpen)
    }
}

private struct EditorInputPromptPanel: View {
    let prompt: EditorInputPromptState
    let backgroundColor: NSColor
    let onQueryChange: (String) -> Void
    let onSubmit: () -> Void
    let onClose: () -> Void
    let onNext: () -> Void
    let onPrevious: () -> Void

    @State private var query: String = ""
    @State private var suppressQueryCallback = false
    @FocusState private var isFieldFocused: Bool

    private var promptColorScheme: ColorScheme {
        let bg = backgroundColor.usingColorSpace(.sRGB) ?? backgroundColor
        let luminance = (0.299 * bg.redComponent) + (0.587 * bg.greenComponent) + (0.114 * bg.blueComponent)
        return luminance >= 0.6 ? .light : .dark
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 6) {
                TextField(prompt.placeholder, text: $query)
                    .textFieldStyle(.plain)
                    .frame(width: prompt.canNavigate ? 220 : 260)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 7)
                    .background(
                        RoundedRectangle(cornerRadius: 6, style: .continuous)
                            .fill(Color.primary.opacity(0.08))
                    )
                    .focused($isFieldFocused)
                    .onSubmit(onSubmit)
                    .onExitCommand(perform: onClose)

                if prompt.canNavigate {
                    Button(action: onNext) {
                        Image(systemName: "chevron.up")
                    }
                    .buttonStyle(EditorInputPromptButtonStyle())

                    Button(action: onPrevious) {
                        Image(systemName: "chevron.down")
                    }
                    .buttonStyle(EditorInputPromptButtonStyle())
                }

                Button(action: onClose) {
                    Image(systemName: "xmark")
                }
                .buttonStyle(EditorInputPromptButtonStyle())
            }

            if let error = prompt.error, !error.isEmpty {
                Text(error)
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(.red)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(8)
        .background(panelBackground)
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .strokeBorder(Color.primary.opacity(0.08), lineWidth: 1)
        }
        .shadow(color: .black.opacity(0.18), radius: 8, y: 3)
        .onAppear {
            setLocalQuery(prompt.query)
            isFieldFocused = true
        }
        .onChange(of: prompt.query) { _, newValue in
            guard newValue != query else { return }
            setLocalQuery(newValue)
        }
        .onChange(of: query) { _, newValue in
            guard !suppressQueryCallback, newValue != prompt.query else { return }
            onQueryChange(newValue)
        }
        .task(id: prompt.title) {
            isFieldFocused = true
        }
        .accessibilityElement(children: .contain)
        .environment(\.colorScheme, promptColorScheme)
    }

    private var panelBackground: some View {
        RoundedRectangle(cornerRadius: 8, style: .continuous)
            .fill(Color(nsColor: backgroundColor))
    }

    private func setLocalQuery(_ newValue: String) {
        suppressQueryCallback = true
        query = newValue
        DispatchQueue.main.async {
            suppressQueryCallback = false
        }
    }
}

private struct EditorInputPromptButtonStyle: ButtonStyle {
    @State private var isHovered = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 12, weight: .semibold))
            .foregroundStyle(isHovered || configuration.isPressed ? .primary : .secondary)
            .frame(width: 22, height: 26)
            .background(
                RoundedRectangle(cornerRadius: 6, style: .continuous)
                    .fill(backgroundColor(isPressed: configuration.isPressed))
            )
            .onHover { hovering in
                isHovered = hovering
            }
    }

    private func backgroundColor(isPressed: Bool) -> Color {
        if isPressed {
            return Color.primary.opacity(0.16)
        }
        if isHovered {
            return Color.primary.opacity(0.08)
        }
        return .clear
    }
}
