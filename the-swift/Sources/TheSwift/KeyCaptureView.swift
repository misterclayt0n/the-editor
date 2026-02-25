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
        private var dragPointerModifiers: UInt8 = 0
        private var dragPointerClickCount: UInt8 = 1
        private var dragRawPoint: NSPoint?
        private var dragAutoScrollTimer: Timer?
        private let hitTolerance: CGFloat = 4.0
        private let dragAutoScrollInterval: TimeInterval = 1.0 / 30.0

        deinit {
            stopDragAutoScroll(clearPoint: true)
        }

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
                dragPointerModifiers = 0
                dragPointerClickCount = 1
                stopDragAutoScroll(clearPoint: true)
                cursor(for: separator).set()
                onSplitResize?(separator.splitId, CGPoint(x: point.x, y: point.y))
                return
            }

            let pane = hitPane(at: point)
            activeEditorPane = pane
            lastDragSignature = nil
            dragPointerModifiers = pointerModifierBits(from: event.modifierFlags)
            dragPointerClickCount = UInt8(clamping: event.clickCount)
            dragRawPoint = point
            stopDragAutoScroll(clearPoint: false)
            if let pointer = makePointerEvent(kind: 0, button: 1, point: point, event: event, pane: pane) {
                onPointer?(pointer)
                return
            }
            super.mouseDown(with: event)
        }

        override func mouseDragged(with event: NSEvent) {
            if let separator = activeSeparator {
                let point = convert(event.locationInWindow, from: nil)
                stopDragAutoScroll(clearPoint: true)
                cursor(for: separator).set()
                onSplitResize?(separator.splitId, CGPoint(x: point.x, y: point.y))
                return
            }

            guard let pane = activeEditorPane else {
                stopDragAutoScroll(clearPoint: true)
                super.mouseDragged(with: event)
                return
            }
            let rawPoint = convert(event.locationInWindow, from: nil)
            dragPointerModifiers = pointerModifierBits(from: event.modifierFlags)
            dragPointerClickCount = UInt8(clamping: event.clickCount)
            dragRawPoint = rawPoint
            updateDragAutoScrollState(for: rawPoint, pane: pane)
            let point = clampToPane(rawPoint, pane: pane)
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
            stopDragAutoScroll(clearPoint: true)
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

        private func updateDragAutoScrollState(for point: NSPoint, pane: PaneHandle) {
            guard verticalDragAutoScrollDelta(for: point, pane: pane) != 0 else {
                stopDragAutoScroll(clearPoint: false)
                return
            }
            ensureDragAutoScrollTimer()
        }

        private func ensureDragAutoScrollTimer() {
            guard dragAutoScrollTimer == nil else { return }
            let timer = Timer(timeInterval: dragAutoScrollInterval, repeats: true) { [weak self] _ in
                self?.handleDragAutoScrollTick()
            }
            RunLoop.main.add(timer, forMode: .common)
            dragAutoScrollTimer = timer
        }

        private func stopDragAutoScroll(clearPoint: Bool) {
            dragAutoScrollTimer?.invalidate()
            dragAutoScrollTimer = nil
            if clearPoint {
                dragRawPoint = nil
            }
        }

        private func handleDragAutoScrollTick() {
            guard let pane = currentActiveDragPane(),
                  let rawPoint = dragRawPoint else {
                stopDragAutoScroll(clearPoint: true)
                return
            }

            let verticalDelta = verticalDragAutoScrollDelta(for: rawPoint, pane: pane)
            guard verticalDelta != 0 else {
                stopDragAutoScroll(clearPoint: false)
                return
            }

            onScroll?(0, CGFloat(verticalDelta), false)

            let clamped = clampToPane(rawPoint, pane: pane)
            guard let pointer = makePointerEvent(
                kind: 1,
                button: 1,
                point: clamped,
                modifiers: dragPointerModifiers,
                clickCount: dragPointerClickCount,
                pane: pane
            ) else {
                return
            }
            let signature = (pointer.surfaceId, pointer.logicalRow, pointer.logicalCol)
            lastDragSignature = signature
            onPointer?(pointer)
        }

        private func currentActiveDragPane() -> PaneHandle? {
            guard let activeEditorPane else { return nil }
            return panes.first(where: { $0.paneId == activeEditorPane.paneId }) ?? activeEditorPane
        }

        private func verticalDragAutoScrollDelta(for point: NSPoint, pane: PaneHandle) -> Int {
            let rect = pane.rect
            guard !rect.isEmpty else { return 0 }

            let edgeThreshold = max(12, cellSize.height * 1.5)
            let maxRowsPerTick = 4
            let rowHeight = max(1, cellSize.height)

            let topBand = rect.minY + edgeThreshold
            if point.y < topBand {
                let distance = topBand - point.y
                let rows = min(maxRowsPerTick, max(1, Int(ceil(distance / rowHeight))))
                return rows
            }

            let bottomBand = rect.maxY - edgeThreshold
            if point.y > bottomBand {
                let distance = point.y - bottomBand
                let rows = min(maxRowsPerTick, max(1, Int(ceil(distance / rowHeight))))
                return -rows
            }

            return 0
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
            makePointerEvent(
                kind: kind,
                button: button,
                point: point,
                modifiers: pointerModifierBits(from: event.modifierFlags),
                clickCount: UInt8(clamping: event.clickCount),
                pane: pane
            )
        }

        private func makePointerEvent(
            kind: UInt8,
            button: UInt8,
            point: NSPoint,
            modifiers: UInt8,
            clickCount: UInt8,
            pane: PaneHandle?
        ) -> MouseBridgeEvent? {
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
                    clickCount: clickCount,
                    surfaceId: pane.paneId
                )
            }

            return MouseBridgeEvent(
                kind: kind,
                button: button,
                logicalCol: UInt16.max,
                logicalRow: UInt16.max,
                modifiers: modifiers,
                clickCount: clickCount,
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
