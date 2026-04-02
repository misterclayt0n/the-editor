import AppKit
import Foundation
import MetalKit
import TheEditorFFI

@MainActor
final class EditorSurfaceView: NSView, @preconcurrency NSTextInputClient {
    weak var controller: EditorSurfaceController?

    private let renderer: MetalEditorRenderer
    private let font: NSFont
    private let fontMetrics: EditorFontMetrics
    private let fallbackCellSize: CGSize

    var cellSize: CGSize {
        controller?.scene?.info.surfaceMetrics.cellSizePoints ?? fallbackCellSize
    }
    private var markedText = NSMutableAttributedString()
    private var pendingScrollRows: CGFloat = 0

    override var acceptsFirstResponder: Bool { true }
    override var isFlipped: Bool { true }

    init?(controller: EditorSurfaceController) {
        self.controller = controller
        self.font = NSFont.monospacedSystemFont(ofSize: 14, weight: .regular)
        self.fontMetrics = EditorFontMetrics(font: font)
        self.fallbackCellSize = fontMetrics.cellSize
        guard let renderer = MetalEditorRenderer(fontMetrics: fontMetrics, scaleProvider: {
            NSScreen.main?.backingScaleFactor ?? 2
        }) else {
            return nil
        }
        self.renderer = renderer
        super.init(frame: .zero)
        wantsLayer = true
        addSubview(renderer.view)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func layout() {
        super.layout()
        renderer.view.frame = bounds
        let backingBounds = convertToBacking(bounds)
        renderer.view.drawableSize = backingBounds.size
        synchronizeSurfaceConfiguration()
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        synchronizeSurfaceConfiguration()
    }

    private func synchronizeSurfaceConfiguration() {
        guard let controller else { return }
        controller.configureSurface(
            size: bounds.size,
            backingScale: window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 1,
            fontMetrics: fontMetrics
        )
    }

    func update(scene: EditorRenderScene) {
        renderer.update(scene: scene)
        window?.invalidateCursorRects(for: self)
    }

    override func resetCursorRects() {
        discardCursorRects()
        let gutterWidth: CGFloat
        if let scene = controller?.scene {
            gutterWidth = CGFloat(scene.info.contentOffsetX) * scene.info.surfaceMetrics.cellSizePoints.width
        } else {
            gutterWidth = 0
        }
        if gutterWidth > 0 {
            addCursorRect(NSRect(x: 0, y: 0, width: gutterWidth, height: bounds.height), cursor: .arrow)
        }
        let textRect = NSRect(x: gutterWidth, y: 0, width: max(bounds.width - gutterWidth, 0), height: bounds.height)
        addCursorRect(textRect, cursor: .iBeam)
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        super.mouseDown(with: event)
    }

    override func scrollWheel(with event: NSEvent) {
        guard let controller else {
            super.scrollWheel(with: event)
            return
        }

        let cellHeight = max(cellSize.height, 1)
        let contentDeltaY = (event.isDirectionInvertedFromDevice ? -1 : 1) * event.scrollingDeltaY
        let deltaY = event.hasPreciseScrollingDeltas ? contentDeltaY * 2 : contentDeltaY
        let rowDelta: Int
        if event.hasPreciseScrollingDeltas {
            pendingScrollRows += deltaY / cellHeight
            rowDelta = Int(pendingScrollRows.rounded(.towardZero))
            pendingScrollRows -= CGFloat(rowDelta)
        } else {
            rowDelta = Int(deltaY.rounded(.towardZero))
            pendingScrollRows = 0
        }

        guard rowDelta != 0 else { return }
        controller.scrollRows(by: rowDelta)
    }

    override func keyDown(with event: NSEvent) {
        guard let controller else {
            super.keyDown(with: event)
            return
        }

        if controller.currentMode == .insert {
            if let special = translateSpecialEvent(event) {
                if special.kind == THE_EDITOR_KEY_ESCAPE.rawValue {
                    cancelMarkedTextComposition()
                }
                controller.handleKey(special)
                return
            }
            interpretKeyEvents([event])
            return
        }

        guard let keyEvent = translateRawEvent(event) else {
            super.keyDown(with: event)
            return
        }
        controller.handleKey(keyEvent)
    }

    private func translateSpecialEvent(_ event: NSEvent) -> the_editor_key_event_t? {
        switch Int(event.keyCode) {
        case 53, 36, 76, 51, 48, 117, 114, 115, 119, 116, 121, 123, 124, 125, 126, 122, 120, 99, 118, 96, 97, 98, 100, 101, 109, 103, 111:
            return translateRawEvent(event)
        default:
            return nil
        }
    }

    private func translateRawEvent(_ event: NSEvent) -> the_editor_key_event_t? {
        var keyEvent = the_editor_key_event_t(kind: THE_EDITOR_KEY_OTHER.rawValue, codepoint: 0, modifiers: modifierBits(for: event, includeShift: true))
        switch Int(event.keyCode) {
        case 53: keyEvent.kind = THE_EDITOR_KEY_ESCAPE.rawValue
        case 36: keyEvent.kind = THE_EDITOR_KEY_ENTER.rawValue
        case 76: keyEvent.kind = THE_EDITOR_KEY_NUMPAD_ENTER.rawValue
        case 51: keyEvent.kind = THE_EDITOR_KEY_BACKSPACE.rawValue
        case 48: keyEvent.kind = THE_EDITOR_KEY_TAB.rawValue
        case 117: keyEvent.kind = THE_EDITOR_KEY_DELETE.rawValue
        case 114: keyEvent.kind = THE_EDITOR_KEY_INSERT.rawValue
        case 115: keyEvent.kind = THE_EDITOR_KEY_HOME.rawValue
        case 119: keyEvent.kind = THE_EDITOR_KEY_END.rawValue
        case 116: keyEvent.kind = THE_EDITOR_KEY_PAGE_UP.rawValue
        case 121: keyEvent.kind = THE_EDITOR_KEY_PAGE_DOWN.rawValue
        case 123: keyEvent.kind = THE_EDITOR_KEY_LEFT.rawValue
        case 124: keyEvent.kind = THE_EDITOR_KEY_RIGHT.rawValue
        case 125: keyEvent.kind = THE_EDITOR_KEY_DOWN.rawValue
        case 126: keyEvent.kind = THE_EDITOR_KEY_UP.rawValue
        case 122: keyEvent.kind = THE_EDITOR_KEY_F1.rawValue
        case 120: keyEvent.kind = THE_EDITOR_KEY_F2.rawValue
        case 99: keyEvent.kind = THE_EDITOR_KEY_F3.rawValue
        case 118: keyEvent.kind = THE_EDITOR_KEY_F4.rawValue
        case 96: keyEvent.kind = THE_EDITOR_KEY_F5.rawValue
        case 97: keyEvent.kind = THE_EDITOR_KEY_F6.rawValue
        case 98: keyEvent.kind = THE_EDITOR_KEY_F7.rawValue
        case 100: keyEvent.kind = THE_EDITOR_KEY_F8.rawValue
        case 101: keyEvent.kind = THE_EDITOR_KEY_F9.rawValue
        case 109: keyEvent.kind = THE_EDITOR_KEY_F10.rawValue
        case 103: keyEvent.kind = THE_EDITOR_KEY_F11.rawValue
        case 111: keyEvent.kind = THE_EDITOR_KEY_F12.rawValue
        default:
            guard let scalar = event.characters?.unicodeScalars.first else { return nil }
            keyEvent.kind = THE_EDITOR_KEY_CHAR.rawValue
            keyEvent.codepoint = scalar.value
            keyEvent.modifiers = modifierBits(for: event, includeShift: false)
        }
        return keyEvent
    }

    private func modifierBits(for event: NSEvent, includeShift: Bool) -> UInt8 {
        var bits: UInt8 = 0
        if event.modifierFlags.contains(.control) {
            bits |= UInt8(THE_EDITOR_MODIFIER_CTRL)
        }
        if event.modifierFlags.contains(.option) {
            bits |= UInt8(THE_EDITOR_MODIFIER_ALT)
        }
        if includeShift && event.modifierFlags.contains(.shift) {
            bits |= UInt8(THE_EDITOR_MODIFIER_SHIFT)
        }
        return bits
    }

    // MARK: NSTextInputClient

    func hasMarkedText() -> Bool {
        markedText.length > 0
    }

    func markedRange() -> NSRange {
        guard markedText.length > 0 else { return NSRange(location: NSNotFound, length: 0) }
        return NSRange(location: 0, length: markedText.length)
    }

    func selectedRange() -> NSRange {
        controller?.primarySelectionUTF16Range() ?? NSRange(location: 0, length: 0)
    }

    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        switch string {
        case let value as NSAttributedString:
            markedText = NSMutableAttributedString(attributedString: value)
        case let value as String:
            markedText = NSMutableAttributedString(string: value)
        default:
            markedText = NSMutableAttributedString()
        }
        controller?.updateMarkedText(markedText.string)
    }

    func unmarkText() {
        if markedText.length > 0 {
            markedText.mutableString.setString("")
            controller?.clearMarkedText()
        }
    }

    private func cancelMarkedTextComposition() {
        inputContext?.discardMarkedText()
        unmarkText()
    }

    func validAttributesForMarkedText() -> [NSAttributedString.Key] {
        []
    }

    func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? {
        let text = controller?.primarySelectionText() ?? ""
        actualRange?.pointee = NSRange(location: 0, length: text.utf16.count)
        return NSAttributedString(string: text)
    }

    func characterIndex(for point: NSPoint) -> Int {
        selectedRange().location
    }

    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
        actualRange?.pointee = range
        guard let scene = controller?.scene, let cursor = scene.primaryCursor else {
            return convert(NSRect(x: 0, y: 0, width: 0, height: cellSize.height), to: nil)
        }
        let metrics = scene.info.surfaceMetrics
        let cellSize = metrics.cellSizePoints
        let local = NSRect(
            x: CGFloat(cursor.col) * cellSize.width,
            y: CGFloat(cursor.row) * cellSize.height,
            width: cellSize.width,
            height: cellSize.height
        )
        let windowRect = convert(local, to: nil)
        return window?.convertToScreen(windowRect) ?? windowRect
    }

    func insertText(_ string: Any, replacementRange: NSRange) {
        let text: String
        switch string {
        case let value as NSAttributedString:
            text = value.string
        case let value as String:
            text = value
        default:
            return
        }
        unmarkText()
        controller?.insertText(text)
    }

    override func doCommand(by selector: Selector) {
        guard let controller else { return }
        let event: the_editor_key_event_t?
        switch selector {
        case #selector(moveLeft(_:)):
            event = the_editor_key_event_t(kind: THE_EDITOR_KEY_LEFT.rawValue, codepoint: 0, modifiers: 0)
        case #selector(moveRight(_:)):
            event = the_editor_key_event_t(kind: THE_EDITOR_KEY_RIGHT.rawValue, codepoint: 0, modifiers: 0)
        case #selector(moveUp(_:)):
            event = the_editor_key_event_t(kind: THE_EDITOR_KEY_UP.rawValue, codepoint: 0, modifiers: 0)
        case #selector(moveDown(_:)):
            event = the_editor_key_event_t(kind: THE_EDITOR_KEY_DOWN.rawValue, codepoint: 0, modifiers: 0)
        case #selector(deleteBackward(_:)):
            event = the_editor_key_event_t(kind: THE_EDITOR_KEY_BACKSPACE.rawValue, codepoint: 0, modifiers: 0)
        case #selector(insertNewline(_:)):
            event = the_editor_key_event_t(kind: THE_EDITOR_KEY_ENTER.rawValue, codepoint: 0, modifiers: 0)
        case #selector(insertTab(_:)):
            event = the_editor_key_event_t(kind: THE_EDITOR_KEY_TAB.rawValue, codepoint: 0, modifiers: 0)
        case #selector(cancelOperation(_:)):
            cancelMarkedTextComposition()
            event = the_editor_key_event_t(kind: THE_EDITOR_KEY_ESCAPE.rawValue, codepoint: 0, modifiers: 0)
        default:
            event = nil
        }

        if let event {
            controller.handleKey(event)
        }
    }
}
