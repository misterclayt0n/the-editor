import AppKit
import Foundation
import SwiftUI
import TheEditorFFIBridge

struct KeyCaptureView: NSViewRepresentable {
    final class KeyCaptureNSView: NSView, NSTextInputClient {
        var onKey: ((KeyEvent) -> Void)?
        var onText: ((String, NSEvent.ModifierFlags) -> Void)?
        var modeProvider: (() -> EditorMode)?
        var onScroll: ((CGFloat, CGFloat, Bool) -> Void)?

        private var lastModifiers: NSEvent.ModifierFlags = []
        private var keyTextAccumulator: [String]? = nil
        private var markedText: NSMutableAttributedString = NSMutableAttributedString()

        override var acceptsFirstResponder: Bool { true }

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            window?.makeFirstResponder(self)
        }

        override func flagsChanged(with event: NSEvent) {
            lastModifiers = event.modifierFlags
        }

        override func keyDown(with event: NSEvent) {
            lastModifiers = event.modifierFlags

            if let keyEvent = KeyEventMapper.mapSpecial(event: event) {
                onKey?(keyEvent)
                return
            }

            let mode = modeProvider?() ?? .normal
            let hasCtrl = event.modifierFlags.contains(.control)
            let hasAlt = event.modifierFlags.contains(.option)

            if hasCtrl || (hasAlt && !mode.isTextInput) {
                if let keyEvent = KeyEventMapper.mapModified(event: event) {
                    onKey?(keyEvent)
                }
                return
            }

            keyTextAccumulator = []
            let markedTextBefore = markedText.length > 0
            interpretKeyEvents([event])

            if let acc = keyTextAccumulator, !acc.isEmpty {
                for text in acc {
                    onText?(text, lastModifiers)
                }
                keyTextAccumulator = nil
                return
            }

            keyTextAccumulator = nil

            if markedText.length > 0 {
                return
            }

            if markedTextBefore {
                return
            }

            if let chars = event.characters, !chars.isEmpty {
                onText?(chars, lastModifiers)
                return
            }

            if hasAlt, mode.isTextInput {
                if let keyEvent = KeyEventMapper.mapModified(event: event) {
                    onKey?(keyEvent)
                }
            }
        }

        func insertText(_ insertString: Any, replacementRange: NSRange) {
            let text: String
            if let string = insertString as? String {
                text = string
            } else if let attributed = insertString as? NSAttributedString {
                text = attributed.string
            } else {
                return
            }

            unmarkText()

            if var acc = keyTextAccumulator {
                acc.append(text)
                keyTextAccumulator = acc
                return
            }

            onText?(text, lastModifiers)
        }

        override func insertText(_ insertString: Any) {
            insertText(insertString, replacementRange: NSRange(location: NSNotFound, length: 0))
        }

        func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
            switch string {
            case let value as NSAttributedString:
                markedText = NSMutableAttributedString(attributedString: value)
            case let value as String:
                markedText = NSMutableAttributedString(string: value)
            default:
                return
            }
        }

        func unmarkText() {
            if markedText.length > 0 {
                markedText.mutableString.setString("")
            }
        }

        func hasMarkedText() -> Bool {
            markedText.length > 0
        }

        func markedRange() -> NSRange {
            guard markedText.length > 0 else { return NSRange() }
            return NSRange(location: 0, length: markedText.length)
        }

        func selectedRange() -> NSRange {
            NSRange(location: 0, length: 0)
        }

        func validAttributesForMarkedText() -> [NSAttributedString.Key] {
            []
        }

        func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? {
            return nil
        }

        func characterIndex(for point: NSPoint) -> Int {
            return 0
        }

        func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
            return NSRect.zero
        }

        override func doCommand(by selector: Selector) {
            // Intentionally no-op to avoid system beep.
        }

        override func scrollWheel(with event: NSEvent) {
            onScroll?(event.scrollingDeltaX, event.scrollingDeltaY, event.hasPreciseScrollingDeltas)
        }
    }

    let onKey: (KeyEvent) -> Void
    let onText: (String, NSEvent.ModifierFlags) -> Void
    let onScroll: (CGFloat, CGFloat, Bool) -> Void
    let modeProvider: () -> EditorMode

    func makeNSView(context: Context) -> KeyCaptureNSView {
        let view = KeyCaptureNSView(frame: .zero)
        view.onKey = onKey
        view.onText = onText
        view.onScroll = onScroll
        view.modeProvider = modeProvider
        DispatchQueue.main.async {
            view.window?.makeFirstResponder(view)
        }
        return view
    }

    func updateNSView(_ nsView: KeyCaptureNSView, context: Context) {
        nsView.onKey = onKey
        nsView.onText = onText
        nsView.onScroll = onScroll
        nsView.modeProvider = modeProvider
    }
}

struct ScrollCaptureView: NSViewRepresentable {
    final class ScrollCaptureNSView: NSView {
        var onScroll: ((CGFloat, CGFloat, Bool) -> Void)?

        override func scrollWheel(with event: NSEvent) {
            onScroll?(event.scrollingDeltaX, event.scrollingDeltaY, event.hasPreciseScrollingDeltas)
        }
    }

    let onScroll: (CGFloat, CGFloat, Bool) -> Void

    func makeNSView(context: Context) -> ScrollCaptureNSView {
        let view = ScrollCaptureNSView(frame: .zero)
        view.onScroll = onScroll
        return view
    }

    func updateNSView(_ nsView: ScrollCaptureNSView, context: Context) {
        nsView.onScroll = onScroll
    }
}
