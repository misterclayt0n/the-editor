import AppKit
import SwiftUI

enum EditorNamedCommand: String, CaseIterable {
    case openNativeTab = "native_new_tab"
    case openFilePicker = "file_picker"
    case openCommandPalette = "command_palette"
    case toggleFileTree = "file_explorer"
    case openTerminal = "terminal_open"
    case closeTerminal = "terminal_close"

    var title: String {
        switch self {
        case .openNativeTab:
            return "New Tab"
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
        }
    }

    var keyEquivalent: KeyEquivalent {
        switch self {
        case .openNativeTab:
            return "t"
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
        }
    }

    private var keyEquivalentString: String {
        switch self {
        case .openNativeTab:
            return "t"
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
        }
    }

    var shortcutModifiers: EventModifiers {
        switch self {
        case .openNativeTab:
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
        }
    }

    private var appKitModifiers: NSEvent.ModifierFlags {
        switch self {
        case .openNativeTab:
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
        }
    }

    static func shouldDeferKeyEquivalentToApp(_ event: NSEvent) -> Bool {
        command(for: event) != nil || isNativeTabSelectionShortcut(event) || isAppQuitShortcut(event)
    }

    private static func command(for event: NSEvent) -> EditorNamedCommand? {
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

struct EditorAppCommands: Commands {
    @FocusedValue(\.editorCommandExecutor) private var editorCommandExecutor

    var body: some Commands {
        CommandGroup(replacing: .printItem) {
            commandButton(.openFilePicker)
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
        }
    }

    @ViewBuilder
    private func commandButton(_ command: EditorNamedCommand) -> some View {
        Button(command.title) {
            _ = editorCommandExecutor?(command)
        }
        .keyboardShortcut(command.keyEquivalent, modifiers: command.shortcutModifiers)
        .disabled(editorCommandExecutor == nil)
    }

    @ViewBuilder
    private func tabSelectionButton(_ index: Int) -> some View {
        Button("Select Tab \(index)") {
            _ = editorCommandExecutor?.selectNativeTab(index)
        }
        .keyboardShortcut(
            KeyEquivalent(Character(String(index))),
            modifiers: [.command]
        )
        .disabled(editorCommandExecutor == nil)
    }
}
