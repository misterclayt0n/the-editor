import SwiftUI

struct GlobalTerminalSurfaceEntry: Identifiable, Equatable {
    let runtimeId: UInt64
    let terminalId: UInt64
    let paneId: UInt64?
    let title: String
    let subtitle: String
    let windowTitle: String
    let isActive: Bool
    let isAttached: Bool
    let isInCurrentWindow: Bool

    var id: String {
        "\(runtimeId):\(terminalId)"
    }

    var searchText: String {
        "\(title) \(subtitle) \(windowTitle) t\(terminalId) p\(paneId ?? 0) \(isAttached ? "attached" : "detached")".lowercased()
    }
}

struct GlobalTerminalSwitcherSnapshot: Equatable {
    let isOpen: Bool
    let sessionId: UUID
    let items: [GlobalTerminalSurfaceEntry]

    static let closed = GlobalTerminalSwitcherSnapshot(
        isOpen: false,
        sessionId: UUID(),
        items: []
    )

    static func opened(items: [GlobalTerminalSurfaceEntry]) -> GlobalTerminalSwitcherSnapshot {
        GlobalTerminalSwitcherSnapshot(isOpen: true, sessionId: UUID(), items: items)
    }
}

struct GlobalTerminalSwitcherView: View {
    let snapshot: GlobalTerminalSwitcherSnapshot
    let onSubmit: (GlobalTerminalSurfaceEntry) -> Void
    let onClose: () -> Void

    @State private var query: String = ""
    @State private var selectedIndex: Int? = nil
    @State private var activeSessionId: UUID? = nil

    private var filteredItems: [GlobalTerminalSurfaceEntry] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !trimmed.isEmpty else {
            return snapshot.items
        }

        return snapshot.items.filter { item in
            item.searchText.contains(trimmed)
        }
    }

    private var summaryText: String {
        let count = snapshot.items.count
        return count == 1 ? "1 terminal" : "\(count) terminals"
    }

    var body: some View {
        if snapshot.isOpen {
            FilePickerBackdrop()
            PickerPanel(
                width: 760,
                maxListHeight: 360,
                placeholder: "Jump to terminal…",
                fontSize: 16,
                layout: .center,
                pageSize: 10,
                showTabNavigation: false,
                showPageNavigation: true,
                showCtrlCClose: false,
                autoSelectFirstItem: true,
                itemCount: filteredItems.count,
                externalQuery: query,
                externalSelectedIndex: selectedIndex,
                onQueryChange: { value in
                    query = value
                    let maxIndex = filteredItems.count - 1
                    if let selectedIndex, selectedIndex > maxIndex {
                        self.selectedIndex = maxIndex >= 0 ? maxIndex : nil
                    }
                },
                onSubmit: { index in
                    guard !filteredItems.isEmpty else {
                        onClose()
                        return
                    }
                    let resolvedIndex = index ?? selectedIndex ?? 0
                    guard filteredItems.indices.contains(resolvedIndex) else {
                        return
                    }
                    onSubmit(filteredItems[resolvedIndex])
                },
                onClose: onClose,
                onSelectionChange: { index in
                    selectedIndex = index
                },
                leadingHeader: {
                    Image(systemName: "terminal")
                        .foregroundStyle(.secondary)
                        .font(.system(size: 16))
                },
                trailingHeader: {
                    Text(summaryText)
                        .font(FontLoader.uiFont(size: 12))
                        .foregroundStyle(.secondary)
                },
                itemContent: { index, _, _ in
                    row(for: filteredItems[index])
                },
                emptyContent: {
                    Text("No terminal sessions found")
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity)
                        .padding()
                }
            )
            .onAppear {
                resetPickerStateIfNeeded()
            }
            .onChange(of: snapshot.sessionId) { _ in
                resetPickerStateIfNeeded()
            }
        }
    }

    @ViewBuilder
    private func row(for item: GlobalTerminalSurfaceEntry) -> some View {
        HStack(spacing: 10) {
            Image(systemName: "terminal.fill")
                .foregroundStyle(item.isActive ? Color.accentColor : .secondary)
                .font(.system(size: 12))

            VStack(alignment: .leading, spacing: 2) {
                Text(item.title)
                    .font(FontLoader.uiFont(size: 14).weight(.medium))
                    .foregroundColor(.primary)

                Text(item.subtitle)
                    .font(FontLoader.uiFont(size: 12))
                    .foregroundStyle(.secondary)
            }

            Spacer(minLength: 8)

            if item.isInCurrentWindow {
                badge(text: "HERE", tint: Color.accentColor)
            }
            if item.isActive {
                badge(text: "ACTIVE", tint: Color.green)
            }
            if !item.isAttached {
                badge(text: "DETACHED", tint: Color.orange)
            }

            Text(item.windowTitle)
                .font(FontLoader.uiFont(size: 11).weight(.semibold))
                .foregroundStyle(.secondary)
                .lineLimit(1)
        }
    }

    @ViewBuilder
    private func badge(text: String, tint: Color) -> some View {
        Text(text)
            .font(FontLoader.uiFont(size: 10).weight(.semibold))
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(Capsule().fill(tint.opacity(0.16)))
            .foregroundStyle(tint)
    }

    private func resetPickerStateIfNeeded() {
        guard activeSessionId != snapshot.sessionId else {
            return
        }
        activeSessionId = snapshot.sessionId
        query = ""
        selectedIndex = nil
    }
}
