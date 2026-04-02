import AppKit
import Foundation
import SwiftUI
import TheEditorFFI

struct RustEditorRepresentable: NSViewRepresentable {
    let initialPath: String?

    func makeNSView(context: Context) -> RustEditorView {
        RustEditorView(initialPath: initialPath)
    }

    func updateNSView(_ nsView: RustEditorView, context: Context) {}
}

private struct EditorHandle: @unchecked Sendable {
    let raw: OpaquePointer
}

@MainActor
final class RustEditorView: NSView {
    private var handle: EditorHandle?
    private var snapshot: EditorSnapshot?

    private let font = NSFont.monospacedSystemFont(ofSize: 14, weight: .regular)
    private lazy var textAttributes: [NSAttributedString.Key: Any] = [
        .font: font,
        .foregroundColor: NSColor.textColor,
    ]
    private lazy var gutterAttributes: [NSAttributedString.Key: Any] = [
        .font: font,
        .foregroundColor: NSColor.secondaryLabelColor,
    ]
    private lazy var selectedTextAttributes: [NSAttributedString.Key: Any] = [
        .font: font,
        .foregroundColor: NSColor.selectedTextColor,
    ]

    private lazy var cellSize: CGSize = {
        let sample = "W" as NSString
        let size = sample.size(withAttributes: [.font: font])
        return CGSize(width: ceil(size.width), height: ceil(size.height + 4))
    }()

    override var acceptsFirstResponder: Bool { true }
    override var isFlipped: Bool { true }

    init(initialPath: String?) {
        if let initialPath {
            handle = initialPath.withCString { ptr in
                the_editor_new(ptr).map(EditorHandle.init(raw:))
            }
        } else {
            handle = the_editor_new(nil).map(EditorHandle.init(raw:))
        }
        super.init(frame: .zero)
        wantsLayer = true
        layer?.backgroundColor = NSColor.textBackgroundColor.cgColor
        refreshViewport()
        refreshSnapshot()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        if let handle {
            the_editor_free(handle.raw)
        }
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        window?.makeFirstResponder(self)
    }

    override func layout() {
        super.layout()
        refreshViewport()
        refreshSnapshot()
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        super.mouseDown(with: event)
    }

    override func scrollWheel(with event: NSEvent) {
        guard let handle else { return }
        let raw = event.hasPreciseScrollingDeltas ? event.scrollingDeltaY / cellSize.height : event.scrollingDeltaY
        let delta = Int32((-raw).rounded())
        if delta != 0 {
            _ = the_editor_scroll_lines(handle.raw, delta)
            refreshSnapshot()
        }
    }

    override func keyDown(with event: NSEvent) {
        guard let handle else { return }
        guard let keyEvent = translate(event: event) else {
            super.keyDown(with: event)
            return
        }
        _ = the_editor_handle_key(handle.raw, keyEvent)
        refreshSnapshot()
    }

    override func draw(_ dirtyRect: NSRect) {
        NSColor.textBackgroundColor.setFill()
        dirtyRect.fill()

        guard let snapshot else { return }

        let gutterWidth = CGFloat(snapshot.contentOffsetX) * cellSize.width
        if gutterWidth > 0 {
            NSColor.controlBackgroundColor.setFill()
            NSRect(x: 0, y: 0, width: gutterWidth, height: bounds.height).fill()
        }

        for selection in snapshot.selections {
            draw(selection: selection, contentOffsetX: snapshot.contentOffsetX)
        }

        for line in snapshot.lines {
            draw(line: line, contentOffsetX: snapshot.contentOffsetX)
        }

        for cursor in snapshot.cursors {
            draw(cursor: cursor, contentOffsetX: snapshot.contentOffsetX)
        }
    }

    private func draw(line: EditorSnapshotLine, contentOffsetX: UInt16) {
        let y = CGFloat(line.row) * cellSize.height
        let baselineY = y + 2
        let gutterWidth = CGFloat(contentOffsetX) * cellSize.width

        if !line.gutter.isEmpty {
            (line.gutter as NSString).draw(
                at: NSPoint(x: 4, y: baselineY),
                withAttributes: gutterAttributes
            )
        }

        for span in line.spans {
            (span.text as NSString).draw(
                at: NSPoint(x: gutterWidth + CGFloat(span.col) * cellSize.width, y: baselineY),
                withAttributes: textAttributes
            )
        }
    }

    private func draw(selection: EditorSnapshotSelection, contentOffsetX: UInt16) {
        let gutterWidth = CGFloat(contentOffsetX) * cellSize.width
        let rect = NSRect(
            x: gutterWidth + CGFloat(selection.x) * cellSize.width,
            y: CGFloat(selection.y) * cellSize.height,
            width: max(CGFloat(selection.width) * cellSize.width, 2),
            height: max(CGFloat(selection.height) * cellSize.height, cellSize.height)
        )
        NSColor.selectedTextBackgroundColor.withAlphaComponent(0.35).setFill()
        rect.fill()
    }

    private func draw(cursor: EditorSnapshotCursor, contentOffsetX: UInt16) {
        let x = CGFloat(contentOffsetX) * cellSize.width + CGFloat(cursor.col) * cellSize.width
        let y = CGFloat(cursor.row) * cellSize.height
        let rect: NSRect
        switch cursor.kind {
        case "bar":
            rect = NSRect(x: x, y: y, width: 2, height: cellSize.height)
        case "underline":
            rect = NSRect(x: x, y: y + cellSize.height - 2, width: cellSize.width, height: 2)
        case "hidden":
            return
        default:
            rect = NSRect(x: x, y: y, width: max(cellSize.width, 2), height: cellSize.height)
        }
        NSColor.textColor.setFill()
        rect.fill()
    }

    private func refreshViewport() {
        guard let handle else { return }
        let cols = UInt16(max(Int(bounds.width / cellSize.width), 1))
        let rows = UInt16(max(Int(bounds.height / cellSize.height), 1))
        the_editor_set_viewport(handle.raw, cols, rows)
    }

    private func refreshSnapshot() {
        guard let handle else { return }
        guard let raw = the_editor_snapshot_json(handle.raw) else { return }
        defer { the_editor_string_free(raw) }
        let json = String(cString: raw)
        guard let data = json.data(using: String.Encoding.utf8) else { return }
        snapshot = try? JSONDecoder().decode(EditorSnapshot.self, from: data)
        needsDisplay = true
    }

    private func translate(event: NSEvent) -> the_editor_key_event_t? {
        var keyEvent = the_editor_key_event_t(kind: THE_EDITOR_KEY_OTHER.rawValue, codepoint: 0, modifiers: modifierBits(for: event))

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
            if let scalar = event.characters?.unicodeScalars.first {
                keyEvent.kind = THE_EDITOR_KEY_CHAR.rawValue
                keyEvent.codepoint = scalar.value
            } else {
                return nil
            }
        }

        return keyEvent
    }

    private func modifierBits(for event: NSEvent) -> UInt8 {
        var bits: UInt8 = 0
        if event.modifierFlags.contains(.control) {
            bits |= UInt8(THE_EDITOR_MODIFIER_CTRL)
        }
        if event.modifierFlags.contains(.option) {
            bits |= UInt8(THE_EDITOR_MODIFIER_ALT)
        }
        if event.modifierFlags.contains(.shift) {
            bits |= UInt8(THE_EDITOR_MODIFIER_SHIFT)
        }
        return bits
    }
}

private struct EditorSnapshot: Decodable {
    let displayName: String
    let filePath: String?
    let mode: String
    let viewportWidth: UInt16
    let viewportHeight: UInt16
    let scrollRow: Int
    let scrollCol: Int
    let contentOffsetX: UInt16
    let lineCount: Int
    let message: String?
    let damageReason: String
    let lines: [EditorSnapshotLine]
    let cursors: [EditorSnapshotCursor]
    let selections: [EditorSnapshotSelection]
}

private struct EditorSnapshotLine: Decodable {
    let row: UInt16
    let docLine: Int?
    let gutter: String
    let spans: [EditorSnapshotSpan]
}

private struct EditorSnapshotSpan: Decodable {
    let col: UInt16
    let cols: UInt16
    let text: String
    let isVirtual: Bool
}

private struct EditorSnapshotCursor: Decodable {
    let row: Int
    let col: Int
    let kind: String
}

private struct EditorSnapshotSelection: Decodable {
    let x: UInt16
    let y: UInt16
    let width: UInt16
    let height: UInt16
}
