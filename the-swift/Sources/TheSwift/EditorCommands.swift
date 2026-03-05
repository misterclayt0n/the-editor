import AppKit
import SwiftUI

enum EditorNamedCommand: String, CaseIterable {
    case openNativeTab = "native_new_tab"
    case closeSurface = "close_surface"
    case openFilePicker = "file_picker"
    case openCommandPalette = "command_palette"
    case toggleFileTree = "file_explorer"
    case openTerminal = "terminal_open"
    case closeTerminal = "terminal_close"
    case toggleLastTerminal = "terminal_toggle_last"
    case openGlobalTerminalSwitcher = "terminal_switcher_global"
    case toggleSurfaceOverview = "surface_overview"

    var title: String {
        switch self {
        case .openNativeTab:
            return "New Tab"
        case .closeSurface:
            return "Close Surface"
        case .openFilePicker:
            return "Open File Picker"
        case .openCommandPalette:
            return "Open Command Palette"
        case .toggleFileTree:
            return "Toggle File Tree"
        case .openTerminal:
            return "New Terminal"
        case .closeTerminal:
            return "Close Terminal"
        case .toggleLastTerminal:
            return "Toggle Last Terminal"
        case .openGlobalTerminalSwitcher:
            return "Global Terminal Switcher"
        case .toggleSurfaceOverview:
            return "Tab Overview"
        }
    }

    var keyEquivalent: KeyEquivalent {
        switch self {
        case .openNativeTab:
            return "t"
        case .closeSurface:
            return "w"
        case .openFilePicker:
            return "p"
        case .openCommandPalette:
            return "p"
        case .toggleFileTree:
            return "e"
        case .openTerminal:
            return "t"
        case .closeTerminal:
            return "t"
        case .toggleLastTerminal:
            return "`"
        case .openGlobalTerminalSwitcher:
            return "`"
        case .toggleSurfaceOverview:
            return "o"
        }
    }

    private var keyEquivalentString: String {
        switch self {
        case .openNativeTab:
            return "t"
        case .closeSurface:
            return "w"
        case .openFilePicker:
            return "p"
        case .openCommandPalette:
            return "p"
        case .toggleFileTree:
            return "e"
        case .openTerminal:
            return "t"
        case .closeTerminal:
            return "t"
        case .toggleLastTerminal:
            return "`"
        case .openGlobalTerminalSwitcher:
            return "`"
        case .toggleSurfaceOverview:
            return "o"
        }
    }

    var shortcutModifiers: EventModifiers {
        switch self {
        case .openNativeTab:
            return [.command]
        case .closeSurface:
            return [.command]
        case .openFilePicker:
            return [.command]
        case .openCommandPalette:
            return [.command, .shift]
        case .toggleFileTree:
            return [.command]
        case .openTerminal:
            return [.command, .option]
        case .closeTerminal:
            return [.command, .option, .shift]
        case .toggleLastTerminal:
            return [.command]
        case .openGlobalTerminalSwitcher:
            return [.command, .shift]
        case .toggleSurfaceOverview:
            return [.command, .shift]
        }
    }

    private var appKitModifiers: NSEvent.ModifierFlags {
        switch self {
        case .openNativeTab:
            return [.command]
        case .closeSurface:
            return [.command]
        case .openFilePicker:
            return [.command]
        case .openCommandPalette:
            return [.command, .shift]
        case .toggleFileTree:
            return [.command]
        case .openTerminal:
            return [.command, .option]
        case .closeTerminal:
            return [.command, .option, .shift]
        case .toggleLastTerminal:
            return [.command]
        case .openGlobalTerminalSwitcher:
            return [.command, .shift]
        case .toggleSurfaceOverview:
            return [.command, .shift]
        }
    }

    static func shouldDeferKeyEquivalentToApp(_ event: NSEvent) -> Bool {
        command(for: event) != nil || isNativeTabSelectionShortcut(event) || isAppQuitShortcut(event)
    }

    static func command(for event: NSEvent) -> EditorNamedCommand? {
        guard event.type == .keyDown else { return nil }
        let relevantFlags = event.modifierFlags.intersection([.command, .shift, .option, .control])
        guard relevantFlags.contains(.command) else { return nil }
        let key = (event.charactersIgnoringModifiers ?? "").lowercased()
        return allCases.first { command in
            command.keyEquivalentString == key && command.appKitModifiers == relevantFlags
        }
    }

    private static func isNativeTabSelectionShortcut(_ event: NSEvent) -> Bool {
        guard event.type == .keyDown else { return false }
        let relevantFlags = event.modifierFlags.intersection([.command, .shift, .option, .control])
        guard relevantFlags == [.command] else { return false }
        guard let key = event.charactersIgnoringModifiers?.lowercased(), key.count == 1 else {
            return false
        }
        return key >= "1" && key <= "9"
    }

    private static func isAppQuitShortcut(_ event: NSEvent) -> Bool {
        guard event.type == .keyDown else { return false }
        let relevantFlags = event.modifierFlags.intersection([.command, .shift, .option, .control])
        guard relevantFlags == [.command] else { return false }
        return event.charactersIgnoringModifiers?.lowercased() == "q"
    }
}

struct EditorCommandExecutor {
    let executeNamedCommand: (EditorNamedCommand) -> Bool
    let selectNativeTabCommand: (Int) -> Bool

    @discardableResult
    func callAsFunction(_ command: EditorNamedCommand) -> Bool {
        executeNamedCommand(command)
    }

    @discardableResult
    func selectNativeTab(_ indexOneBased: Int) -> Bool {
        selectNativeTabCommand(indexOneBased)
    }
}

private struct EditorCommandExecutorFocusedKey: FocusedValueKey {
    typealias Value = EditorCommandExecutor
}

extension FocusedValues {
    var editorCommandExecutor: EditorCommandExecutor? {
        get { self[EditorCommandExecutorFocusedKey.self] }
        set { self[EditorCommandExecutorFocusedKey.self] = newValue }
    }
}

private final class WindowModelReference {
    weak var model: EditorModel?
    weak var window: NSWindow?

    init(window: NSWindow?, model: EditorModel?) {
        self.window = window
        self.model = model
    }
}

final class EditorCommandModelRegistry {
    static let shared = EditorCommandModelRegistry()

    private var modelsByWindow: [ObjectIdentifier: WindowModelReference] = [:]

    private init() {}

    func register(window: NSWindow?, model: EditorModel) {
        guard let window else { return }
        pruneDeadEntries()
        modelsByWindow[ObjectIdentifier(window)] = WindowModelReference(window: window, model: model)
    }

    func unregister(window: NSWindow?) {
        guard let window else { return }
        modelsByWindow.removeValue(forKey: ObjectIdentifier(window))
    }

    func fallbackExecutor() -> EditorCommandExecutor? {
        pruneDeadEntries()

        guard let anchorWindow = NSApp.keyWindow ?? NSApp.mainWindow else {
            return nil
        }

        if let model = model(for: anchorWindow) {
            return makeExecutor(for: model)
        }

        if let selectedWindow = anchorWindow.tabGroup?.selectedWindow,
           let model = model(for: selectedWindow) {
            return makeExecutor(for: model)
        }

        if let tabbedWindows = anchorWindow.tabbedWindows {
            for tabWindow in tabbedWindows {
                if let model = model(for: tabWindow) {
                    return makeExecutor(for: model)
                }
            }
        }

        return nil
    }

    func globalTerminalEntries(anchorWindow: NSWindow?) -> [GlobalTerminalSurfaceEntry] {
        let windowModels = orderedWindowModels(anchorWindow: anchorWindow)
        var entries: [GlobalTerminalSurfaceEntry] = []
        entries.reserveCapacity(windowModels.count * 2)

        for (window, model) in windowModels {
            entries.append(contentsOf: model.globalTerminalSurfaceEntries(isCurrentWindow: window === anchorWindow))
        }

        return entries
    }

    @discardableResult
    func focusTerminalSurface(runtimeId: UInt64, terminalId: UInt64) -> Bool {
        let windowModels = orderedWindowModels(anchorWindow: NSApp.keyWindow ?? NSApp.mainWindow)
        for (_, model) in windowModels where model.runtimeInstanceId == runtimeId {
            if model.focusTerminalSurface(terminalId: terminalId) {
                return true
            }
        }
        return false
    }

    private func model(for window: NSWindow) -> EditorModel? {
        modelsByWindow[ObjectIdentifier(window)]?.model
    }

    private func makeExecutor(for model: EditorModel) -> EditorCommandExecutor {
        EditorCommandExecutor(
            executeNamedCommand: { [weak model] command in
                model?.executeNamedCommand(command) ?? false
            },
            selectNativeTabCommand: { [weak model] indexOneBased in
                model?.selectNativeWindowTab(indexOneBased: indexOneBased) ?? false
            }
        )
    }

    private func orderedWindowModels(anchorWindow: NSWindow?) -> [(window: NSWindow, model: EditorModel)] {
        pruneDeadEntries()
        let pairs: [(window: NSWindow, model: EditorModel)] = modelsByWindow.values.compactMap { reference in
            guard let window = reference.window,
                  let model = reference.model else {
                return nil
            }
            return (window: window, model: model)
        }

        return pairs.sorted(by: { lhs, rhs in
            let lhsScore = windowPriority(lhs.window, anchorWindow: anchorWindow)
            let rhsScore = windowPriority(rhs.window, anchorWindow: anchorWindow)
            if lhsScore != rhsScore {
                return lhsScore > rhsScore
            }
            return lhs.window.windowNumber > rhs.window.windowNumber
        })
    }

    private func windowPriority(_ window: NSWindow, anchorWindow: NSWindow?) -> Int {
        if let anchorWindow, window === anchorWindow {
            return 100
        }
        if let anchorWindow, anchorWindow.tabGroup?.selectedWindow === window {
            return 90
        }
        if window.isKeyWindow {
            return 80
        }
        if window.isMainWindow {
            return 70
        }
        if window.isVisible {
            return 60
        }
        return 10
    }

    private func pruneDeadEntries() {
        modelsByWindow = modelsByWindow.filter { $0.value.model != nil && $0.value.window != nil }
    }
}

struct EditorAppCommands: Commands {
    @FocusedValue(\.editorCommandExecutor) private var editorCommandExecutor

    private var resolvedEditorCommandExecutor: EditorCommandExecutor? {
        editorCommandExecutor ?? EditorCommandModelRegistry.shared.fallbackExecutor()
    }

    var body: some Commands {
        CommandGroup(replacing: .printItem) {
            commandButton(.openFilePicker)
        }

        CommandGroup(replacing: .saveItem) {
            commandButton(.closeSurface)
        }

        CommandGroup(before: .pasteboard) {
            commandButton(.toggleFileTree)
            Divider()
        }

        CommandMenu("Editor") {
            commandButton(.openNativeTab)
            commandButton(.openCommandPalette)
        }

        CommandMenu("Tabs") {
            tabSelectionButton(1)
            tabSelectionButton(2)
            tabSelectionButton(3)
            tabSelectionButton(4)
            tabSelectionButton(5)
            tabSelectionButton(6)
            tabSelectionButton(7)
            tabSelectionButton(8)
            tabSelectionButton(9)
        }

        CommandMenu("Terminal") {
            commandButton(.openTerminal)
            commandButton(.closeTerminal)
            Divider()
            commandButton(.toggleLastTerminal)
            commandButton(.openGlobalTerminalSwitcher)
            commandButton(.toggleSurfaceOverview)
        }
    }

    @ViewBuilder
    private func commandButton(_ command: EditorNamedCommand) -> some View {
        Button(command.title) {
            _ = resolvedEditorCommandExecutor?(command)
        }
        .keyboardShortcut(command.keyEquivalent, modifiers: command.shortcutModifiers)
        .disabled(resolvedEditorCommandExecutor == nil)
    }

    @ViewBuilder
    private func tabSelectionButton(_ index: Int) -> some View {
        Button("Select Tab \(index)") {
            _ = resolvedEditorCommandExecutor?.selectNativeTab(index)
        }
        .keyboardShortcut(
            KeyEquivalent(Character(String(index))),
            modifiers: [.command]
        )
        .disabled(resolvedEditorCommandExecutor == nil)
    }
}
