import AppKit
import Foundation
import SwiftUI
import TheEditorFFIBridge

final class KeyCaptureFocusBridge {
    static let shared = KeyCaptureFocusBridge()

    weak var keyCaptureView: NSView?

    private init() {}

    func register(_ view: NSView) {
        keyCaptureView = view
    }

    func keyResponder(in window: NSWindow?) -> NSResponder? {
        guard let view = keyCaptureView,
              view.window === window else {
            return nil
        }
        return view
    }

    func reclaim(in window: NSWindow?) {
        guard let view = keyCaptureView,
              view.window === window else {
            return
        }
        window?.makeFirstResponder(view)
    }

    func reclaimActive() {
        guard let view = keyCaptureView,
              let window = view.window else {
            return
        }
        window.makeFirstResponder(view)
    }
}

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
        KeyCaptureFocusBridge.shared.register(view)
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
        KeyCaptureFocusBridge.shared.register(nsView)
    }
}

struct MouseBridgeEvent {
    let kind: UInt8
    let button: UInt8
    let logicalCol: UInt16
    let logicalRow: UInt16
    let modifiers: UInt8
    let clickCount: UInt8
    let surfaceId: UInt64

    var packed: UInt64 {
        UInt64(kind)
            | (UInt64(button) << 8)
            | (UInt64(modifiers) << 16)
            | (UInt64(clickCount) << 24)
    }
}

struct ScrollCaptureView: NSViewRepresentable {
    struct SeparatorHandle: Equatable {
        let splitId: UInt64
        let axis: UInt8
        let linePx: CGFloat
        let spanStartPx: CGFloat
        let spanEndPx: CGFloat
    }

    struct PaneHandle: Equatable {
        let paneId: UInt64
        let rect: CGRect
        let contentOffsetXPx: CGFloat
    }

    final class ScrollCaptureNSView: NSView {
        override var isFlipped: Bool { true }

        var onScroll: ((CGFloat, CGFloat, Bool) -> Void)?
        var onPointer: ((MouseBridgeEvent) -> Void)?
        var onSplitResize: ((UInt64, CGPoint) -> Void)?
        var separators: [SeparatorHandle] = [] {
            didSet {
                needsDisplay = true
                window?.invalidateCursorRects(for: self)
            }
        }
        var panes: [PaneHandle] = []
        var cellSize: CGSize = .init(width: 1, height: 1)
        private var trackingArea: NSTrackingArea?
        private var activeSeparator: SeparatorHandle?
        private var activeEditorPane: PaneHandle?
        private var lastDragSignature: (paneId: UInt64, row: UInt16, col: UInt16)?
        private let hitTolerance: CGFloat = 4.0

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            window?.acceptsMouseMovedEvents = true
        }

        override func updateTrackingAreas() {
            super.updateTrackingAreas()
            if let trackingArea {
                removeTrackingArea(trackingArea)
            }
            let options: NSTrackingArea.Options = [.activeInKeyWindow, .inVisibleRect, .mouseMoved, .cursorUpdate]
            let area = NSTrackingArea(rect: .zero, options: options, owner: self, userInfo: nil)
            addTrackingArea(area)
            trackingArea = area
        }

        override func resetCursorRects() {
            super.resetCursorRects()
            for separator in separators {
                let rect = cursorRect(for: separator).intersection(bounds)
                guard !rect.isEmpty else { continue }
                addCursorRect(rect, cursor: cursor(for: separator))
            }
            addCursorRect(bounds, cursor: .arrow)
        }

        override func cursorUpdate(with event: NSEvent) {
            let point = convert(event.locationInWindow, from: nil)
            updateCursor(at: point)
        }

        override func mouseMoved(with event: NSEvent) {
            let point = convert(event.locationInWindow, from: nil)
            updateCursor(at: point)
            super.mouseMoved(with: event)
        }

        override func mouseDown(with event: NSEvent) {
            let point = convert(event.locationInWindow, from: nil)
            KeyCaptureFocusBridge.shared.reclaim(in: window)
            if let separator = hitSeparator(at: point) {
                activeSeparator = separator
                activeEditorPane = nil
                lastDragSignature = nil
                cursor(for: separator).set()
                onSplitResize?(separator.splitId, CGPoint(x: point.x, y: point.y))
                return
            }

            let pane = hitPane(at: point)
            activeEditorPane = pane
            lastDragSignature = nil
            if let pointer = makePointerEvent(kind: 0, button: 1, point: point, event: event, pane: pane) {
                onPointer?(pointer)
                return
            }
            super.mouseDown(with: event)
        }

        override func mouseDragged(with event: NSEvent) {
            if let separator = activeSeparator {
                let point = convert(event.locationInWindow, from: nil)
                cursor(for: separator).set()
                onSplitResize?(separator.splitId, CGPoint(x: point.x, y: point.y))
                return
            }

            guard let pane = activeEditorPane else {
                super.mouseDragged(with: event)
                return
            }
            let point = clampToPane(convert(event.locationInWindow, from: nil), pane: pane)
            guard let pointer = makePointerEvent(kind: 1, button: 1, point: point, event: event, pane: pane) else {
                return
            }
            let signature = (pointer.surfaceId, pointer.logicalRow, pointer.logicalCol)
            if let lastDragSignature,
               lastDragSignature.0 == signature.0,
               lastDragSignature.1 == signature.1,
               lastDragSignature.2 == signature.2 {
                return
            }
            lastDragSignature = signature
            onPointer?(pointer)
        }

        override func mouseUp(with event: NSEvent) {
            let point = convert(event.locationInWindow, from: nil)
            if let pane = activeEditorPane {
                let clamped = clampToPane(point, pane: pane)
                if let pointer = makePointerEvent(kind: 2, button: 1, point: clamped, event: event, pane: pane) {
                    onPointer?(pointer)
                }
            }
            activeSeparator = nil
            activeEditorPane = nil
            lastDragSignature = nil
            updateCursor(at: point)
            super.mouseUp(with: event)
        }

        override func scrollWheel(with event: NSEvent) {
            onScroll?(event.scrollingDeltaX, event.scrollingDeltaY, event.hasPreciseScrollingDeltas)
        }

        private func updateCursor(at point: NSPoint) {
            if let separator = hitSeparator(at: point) {
                cursor(for: separator).set()
            } else {
                NSCursor.arrow.set()
            }
        }

        private func hitSeparator(at point: NSPoint) -> SeparatorHandle? {
            var best: (SeparatorHandle, CGFloat)?
            for separator in separators {
                let distance: CGFloat
                let inSpan: Bool
                if separator.axis == 0 {
                    inSpan = point.y >= separator.spanStartPx - hitTolerance
                        && point.y <= separator.spanEndPx + hitTolerance
                    distance = abs(point.x - separator.linePx)
                } else {
                    inSpan = point.x >= separator.spanStartPx - hitTolerance
                        && point.x <= separator.spanEndPx + hitTolerance
                    distance = abs(point.y - separator.linePx)
                }
                guard inSpan, distance <= hitTolerance else { continue }
                if let current = best {
                    if distance < current.1 {
                        best = (separator, distance)
                    }
                } else {
                    best = (separator, distance)
                }
            }
            return best?.0
        }

        private func hitPane(at point: NSPoint) -> PaneHandle? {
            panes.first(where: { $0.rect.contains(point) })
        }

        private func clampToPane(_ point: NSPoint, pane: PaneHandle) -> NSPoint {
            let rect = pane.rect
            guard !rect.isEmpty else { return point }
            let maxX = max(rect.minX, rect.maxX - 1)
            let maxY = max(rect.minY, rect.maxY - 1)
            return NSPoint(
                x: min(max(point.x, rect.minX), maxX),
                y: min(max(point.y, rect.minY), maxY)
            )
        }

        private func makePointerEvent(
            kind: UInt8,
            button: UInt8,
            point: NSPoint,
            event: NSEvent,
            pane: PaneHandle?
        ) -> MouseBridgeEvent? {
            let modifiers = pointerModifierBits(from: event.modifierFlags)

            if let pane {
                let localX = point.x - pane.rect.minX
                let localY = point.y - pane.rect.minY
                let row = max(0, Int(floor(localY / max(1, cellSize.height))))
                let textX = localX - pane.contentOffsetXPx
                let col = max(0, Int(floor(textX / max(1, cellSize.width))))
                return MouseBridgeEvent(
                    kind: kind,
                    button: button,
                    logicalCol: UInt16(clamping: col),
                    logicalRow: UInt16(clamping: row),
                    modifiers: modifiers,
                    clickCount: UInt8(clamping: event.clickCount),
                    surfaceId: pane.paneId
                )
            }

            return MouseBridgeEvent(
                kind: kind,
                button: button,
                logicalCol: UInt16.max,
                logicalRow: UInt16.max,
                modifiers: modifiers,
                clickCount: UInt8(clamping: event.clickCount),
                surfaceId: 0
            )
        }

        private func pointerModifierBits(from flags: NSEvent.ModifierFlags) -> UInt8 {
            var bits: UInt8 = 0
            if flags.contains(.control) {
                bits |= 0b0000_0001
            }
            if flags.contains(.option) {
                bits |= 0b0000_0010
            }
            if flags.contains(.shift) {
                bits |= 0b0000_0100
            }
            return bits
        }

        private func cursorRect(for separator: SeparatorHandle) -> CGRect {
            if separator.axis == 0 {
                return CGRect(
                    x: separator.linePx - hitTolerance,
                    y: separator.spanStartPx,
                    width: hitTolerance * 2,
                    height: max(0, separator.spanEndPx - separator.spanStartPx)
                )
            }
            return CGRect(
                x: separator.spanStartPx,
                y: separator.linePx - hitTolerance,
                width: max(0, separator.spanEndPx - separator.spanStartPx),
                height: hitTolerance * 2
            )
        }

        private func cursor(for separator: SeparatorHandle) -> NSCursor {
            separator.axis == 0 ? .resizeLeftRight : .resizeUpDown
        }
    }

    let onScroll: (CGFloat, CGFloat, Bool) -> Void
    let onPointer: (MouseBridgeEvent) -> Void
    let separators: [SeparatorHandle]
    let panes: [PaneHandle]
    let cellSize: CGSize
    let onSplitResize: (UInt64, CGPoint) -> Void

    func makeNSView(context: Context) -> ScrollCaptureNSView {
        let view = ScrollCaptureNSView(frame: .zero)
        view.onScroll = onScroll
        view.onPointer = onPointer
        view.separators = separators
        view.panes = panes
        view.cellSize = cellSize
        view.onSplitResize = onSplitResize
        return view
    }

    func updateNSView(_ nsView: ScrollCaptureNSView, context: Context) {
        nsView.onScroll = onScroll
        nsView.onPointer = onPointer
        nsView.separators = separators
        nsView.panes = panes
        nsView.cellSize = cellSize
        nsView.onSplitResize = onSplitResize
    }
}
