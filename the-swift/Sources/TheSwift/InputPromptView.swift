import SwiftUI

struct InputPromptView: View {
    let snapshot: InputPromptSnapshot
    let onQueryChange: (String) -> Void
    let onClose: () -> Void
    let onSubmit: () -> Void

    @State private var query: String = ""
    @FocusState private var isFieldFocused: Bool

    var body: some View {
        VStack(spacing: 0) {
            promptBar
            Spacer()
        }
        .padding(.top, 48)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
    }

    private var promptBar: some View {
        HStack(spacing: 8) {
            Text(snapshot.label)
                .font(FontLoader.editorFont(size: 11).weight(.semibold))
                .foregroundStyle(.secondary)
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .background(
                    Capsule().fill(Color.primary.opacity(0.08))
                )

            TextField("", text: $query)
                .textFieldStyle(.plain)
                .font(FontLoader.editorFont(size: 14))
                .frame(minWidth: 180)
                .focused($isFieldFocused)
                .onExitCommand { onClose() }
                .onSubmit { onSubmit() }
                .onChange(of: query) { newValue in
                    if newValue != snapshot.query {
                        onQueryChange(newValue)
                    }
                }

            if let error = snapshot.error, !error.isEmpty {
                Image(systemName: "exclamationmark.triangle")
                    .font(.system(size: 12))
                    .foregroundColor(.red.opacity(0.8))
                    .help(error)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .glassBackground(cornerRadius: 8)
        .frame(maxWidth: 320)
        .onAppear {
            query = snapshot.query
            DispatchQueue.main.async {
                isFieldFocused = true
            }
        }
        .onChange(of: snapshot.query) { newValue in
            if newValue != query { query = newValue }
        }
        .onChange(of: snapshot.isOpen) { isOpen in
            if isOpen {
                DispatchQueue.main.async { isFieldFocused = true }
            }
        }
    }
}
