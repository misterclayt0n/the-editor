import AppKit
import Foundation
import TheEditorFFI

struct EditorRGBA: Hashable {
    let r: UInt8
    let g: UInt8
    let b: UInt8
    let a: UInt8

    var color: NSColor {
        NSColor(
            calibratedRed: CGFloat(r) / 255,
            green: CGFloat(g) / 255,
            blue: CGFloat(b) / 255,
            alpha: CGFloat(a) / 255
        )
    }
}

struct EditorResolvedStyle: Hashable {
    let fg: EditorRGBA?
    let bg: EditorRGBA?
    let underlineColor: EditorRGBA?
    let addModifiers: UInt16
    let removeModifiers: UInt16
    let underlineStyle: UInt8

    var foregroundColor: NSColor {
        fg?.color ?? .textColor
    }

    var backgroundColor: NSColor? {
        bg?.color
    }
}

enum EditorMode: UInt8 {
    case normal = 0
    case insert = 1
    case select = 2
    case command = 3
}

enum EditorDamageReason: UInt8 {
    case none = 0
    case full = 1
    case layout = 2
    case text = 3
    case decoration = 4
    case cursor = 5
    case scroll = 6
    case theme = 7
    case paneStructure = 8
}

enum EditorCursorKind: UInt8 {
    case block = 0
    case bar = 1
    case underline = 2
    case hollow = 3
    case hidden = 4
}

enum EditorSelectionKind: UInt8 {
    case primary = 0
    case match = 1
    case hover = 2
}

enum EditorOverlayKind: UInt8 {
    case rect = 0
    case text = 1
}

enum EditorOverlayRectKind: UInt8 {
    case panel = 0
    case divider = 1
    case highlight = 2
    case backdrop = 3
}

struct EditorSnapshotInfo {
    let surfaceWidthPx: Int
    let surfaceHeightPx: Int
    let surfaceMetrics: EditorSurfaceMetrics
    let viewportWidth: Int
    let viewportHeight: Int
    let contentOffsetX: Int
    let damageStartRow: Int
    let damageEndRow: Int
    let damageIsFull: Bool
    let damageReason: EditorDamageReason
    let mode: EditorMode
    let layoutGeneration: UInt64
    let textGeneration: UInt64
    let decorationGeneration: UInt64
    let cursorGeneration: UInt64
    let scrollGeneration: UInt64
    let themeGeneration: UInt64
    let cursorBlinkGeneration: UInt64
    let scrollRow: Int
    let scrollCol: Int
    let documentLineCount: Int
}

struct EditorSnapshotSpan: Hashable {
    let col: Int
    let cols: Int
    let text: String
    let isVirtual: Bool
    let style: EditorResolvedStyle
}

struct EditorSnapshotTextCell: Hashable {
    let row: Int
    let col: Int
    let cols: Int
    let text: String
    let isVirtual: Bool
    let style: EditorResolvedStyle
}

struct EditorSnapshotLine: Hashable {
    let row: Int
    let docLine: Int?
    let firstVisualLine: Bool
    let spans: [EditorSnapshotSpan]
    let textCells: [EditorSnapshotTextCell]
}

struct EditorSnapshotCursor: Hashable {
    let row: Int
    let col: Int
    let kind: EditorCursorKind
    let style: EditorResolvedStyle
}

struct EditorSnapshotSelection: Hashable {
    let x: Int
    let y: Int
    let width: Int
    let height: Int
    let kind: EditorSelectionKind
    let style: EditorResolvedStyle
}

struct EditorSnapshotOverlay: Hashable {
    let kind: EditorOverlayKind
    let rectKind: EditorOverlayRectKind?
    let x: Int
    let y: Int
    let width: Int
    let height: Int
    let radius: Int
    let row: Int
    let col: Int
    let text: String?
    let style: EditorResolvedStyle
}

struct EditorCommandPaletteItem: Hashable {
    let title: String
    let subtitle: String?
    let description: String?
    let badge: String?
    let leadingIcon: String?
    let leadingColor: EditorRGBA?
    let emphasis: Bool
}

struct EditorCommandPaletteState: Hashable {
    let isOpen: Bool
    let selectedIndex: Int?
    let query: String
    let placeholder: String
    let items: [EditorCommandPaletteItem]

    static let empty = EditorCommandPaletteState(
        isOpen: false,
        selectedIndex: nil,
        query: "",
        placeholder: "Execute a command…",
        items: []
    )
}

struct EditorSnapshot {
    let info: EditorSnapshotInfo
    let lines: [EditorSnapshotLine]
    let cursors: [EditorSnapshotCursor]
    let selections: [EditorSnapshotSelection]
    let overlays: [EditorSnapshotOverlay]
    let commandPalette: EditorCommandPaletteState
}

enum EditorFFIBridge {
    static func createHandle(initialPath: String?) -> OpaquePointer? {
        if let initialPath {
            return initialPath.withCString { the_editor_new($0) }
        }
        return the_editor_new(nil)
    }

    static func destroyHandle(_ handle: OpaquePointer?) {
        guard let handle else { return }
        the_editor_free(handle)
    }

    @discardableResult
    static func configureSurface(_ handle: OpaquePointer?, configuration: EditorSurfaceConfiguration) -> Bool {
        guard let handle else { return false }
        let metrics = the_editor_surface_metrics_t(
            backing_scale: Float(configuration.metrics.backingScale),
            cell_width_px: UInt16(clamping: configuration.metrics.cellWidthPx),
            cell_height_px: UInt16(clamping: configuration.metrics.cellHeightPx),
            cell_baseline_px: UInt16(clamping: configuration.metrics.cellBaselinePx),
            underline_position_px: UInt16(clamping: configuration.metrics.underlinePositionPx),
            underline_thickness_px: UInt16(clamping: configuration.metrics.underlineThicknessPx),
            cursor_thickness_px: UInt16(clamping: configuration.metrics.cursorThicknessPx)
        )
        let config = the_editor_surface_config_t(
            width_px: UInt32(clamping: configuration.widthPx),
            height_px: UInt32(clamping: configuration.heightPx),
            metrics: metrics
        )
        return the_editor_configure_surface(handle, config)
    }

    static func setViewport(_ handle: OpaquePointer?, cols: UInt16, rows: UInt16) {
        guard let handle else { return }
        the_editor_set_viewport(handle, cols, rows)
    }

    @discardableResult
    static func setScrollRow(_ handle: OpaquePointer?, row: UInt32) -> Bool {
        guard let handle else { return false }
        return the_editor_set_scroll_row(handle, row)
    }

    @discardableResult
    static func setScrollCol(_ handle: OpaquePointer?, col: UInt32) -> Bool {
        guard let handle else { return false }
        return the_editor_set_scroll_col(handle, col)
    }

    @discardableResult
    static func sendKey(_ handle: OpaquePointer?, event: the_editor_key_event_t) -> Bool {
        guard let handle else { return false }
        return the_editor_handle_key(handle, event)
    }

    @discardableResult
    static func toggleCommandPalette(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_toggle_command_palette(handle)
    }

    @discardableResult
    static func closeCommandPalette(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_close_command_palette(handle)
    }

    @discardableResult
    static func setCommandPaletteQuery(_ handle: OpaquePointer?, query: String) -> Bool {
        guard let handle else { return false }
        return query.withCString { the_editor_command_palette_set_query(handle, $0) }
    }

    @discardableResult
    static func selectNextCommandPaletteItem(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_command_palette_select_next(handle)
    }

    @discardableResult
    static func selectPreviousCommandPaletteItem(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_command_palette_select_previous(handle)
    }

    @discardableResult
    static func selectCommandPaletteVisibleIndex(_ handle: OpaquePointer?, index: Int) -> Bool {
        guard let handle else { return false }
        return the_editor_command_palette_select_visible_index(handle, UInt(index))
    }

    @discardableResult
    static func submitCommandPalette(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_command_palette_submit(handle)
    }

    @discardableResult
    static func insertText(_ handle: OpaquePointer?, text: String) -> Bool {
        guard let handle else { return false }
        return text.withCString { the_editor_insert_text(handle, $0) }
    }

    static func primarySelectionUTF16Range(_ handle: OpaquePointer?) -> NSRange {
        guard let handle else { return NSRange(location: 0, length: 0) }
        return NSRange(
            location: Int(the_editor_primary_selection_utf16_location(handle)),
            length: Int(the_editor_primary_selection_utf16_length(handle))
        )
    }

    static func primarySelectionText(_ handle: OpaquePointer?) -> String {
        guard let handle, let raw = the_editor_primary_selection_text(handle) else {
            return ""
        }
        defer { the_editor_string_free(raw) }
        return String(cString: raw)
    }

    static func makeSnapshot(_ handle: OpaquePointer?) -> EditorSnapshot? {
        guard let handle, let rawSnapshot = the_editor_snapshot_create(handle) else {
            return nil
        }
        defer { the_editor_snapshot_free(rawSnapshot) }

        let infoValue = the_editor_snapshot_info(rawSnapshot)
        let info = EditorSnapshotInfo(
            surfaceWidthPx: Int(infoValue.surface_width_px),
            surfaceHeightPx: Int(infoValue.surface_height_px),
            surfaceMetrics: surfaceMetrics(from: infoValue.surface_metrics),
            viewportWidth: Int(infoValue.viewport_width),
            viewportHeight: Int(infoValue.viewport_height),
            contentOffsetX: Int(infoValue.content_offset_x),
            damageStartRow: Int(infoValue.damage_start_row),
            damageEndRow: Int(infoValue.damage_end_row),
            damageIsFull: infoValue.damage_is_full,
            damageReason: EditorDamageReason(rawValue: infoValue.damage_reason) ?? .none,
            mode: EditorMode(rawValue: infoValue.mode) ?? .normal,
            layoutGeneration: infoValue.layout_generation,
            textGeneration: infoValue.text_generation,
            decorationGeneration: infoValue.decoration_generation,
            cursorGeneration: infoValue.cursor_generation,
            scrollGeneration: infoValue.scroll_generation,
            themeGeneration: infoValue.theme_generation,
            cursorBlinkGeneration: infoValue.cursor_blink_generation,
            scrollRow: Int(infoValue.scroll_row),
            scrollCol: Int(infoValue.scroll_col),
            documentLineCount: Int(infoValue.document_line_count)
        )

        let paletteValue = the_editor_snapshot_command_palette(rawSnapshot)
        let commandPalette = EditorCommandPaletteState(
            isOpen: paletteValue.is_open,
            selectedIndex: paletteValue.selected_index >= 0 ? Int(paletteValue.selected_index) : nil,
            query: paletteValue.query.map { String(cString: $0) } ?? "",
            placeholder: paletteValue.placeholder.map { String(cString: $0) } ?? "Execute a command…",
            items: (0..<Int(paletteValue.item_count)).map { itemIndex in
                let itemValue = the_editor_snapshot_command_palette_item_at(rawSnapshot, UInt(itemIndex))
                return EditorCommandPaletteItem(
                    title: itemValue.title.map { String(cString: $0) } ?? "",
                    subtitle: itemValue.subtitle.map { String(cString: $0) },
                    description: itemValue.description.map { String(cString: $0) },
                    badge: itemValue.badge.map { String(cString: $0) },
                    leadingIcon: itemValue.leading_icon.map { String(cString: $0) },
                    leadingColor: rgba(from: itemValue.leading_color),
                    emphasis: itemValue.emphasis
                )
            }
        )

        let lines: [EditorSnapshotLine] = (0..<Int(infoValue.line_count)).map { lineIndex in
            let lineValue = the_editor_snapshot_line_at(rawSnapshot, UInt(lineIndex))
            let spans: [EditorSnapshotSpan] = (0..<Int(lineValue.span_count)).map { spanIndex in
                let spanValue = the_editor_snapshot_span_at(rawSnapshot, UInt(lineIndex), UInt(spanIndex))
                return EditorSnapshotSpan(
                    col: Int(spanValue.col),
                    cols: Int(spanValue.cols),
                    text: spanValue.text.map { String(cString: $0) } ?? "",
                    isVirtual: spanValue.is_virtual,
                    style: style(from: spanValue.style)
                )
            }
            let textCells: [EditorSnapshotTextCell] = (0..<Int(lineValue.text_cell_count)).map { cellIndex in
                let cellValue = the_editor_snapshot_text_cell_at(rawSnapshot, UInt(lineIndex), UInt(cellIndex))
                return EditorSnapshotTextCell(
                    row: Int(cellValue.row),
                    col: Int(cellValue.col),
                    cols: Int(cellValue.cols),
                    text: cellValue.text.map { String(cString: $0) } ?? "",
                    isVirtual: cellValue.is_virtual,
                    style: style(from: cellValue.style)
                )
            }
            return EditorSnapshotLine(
                row: Int(lineValue.row),
                docLine: lineValue.doc_line >= 0 ? Int(lineValue.doc_line) : nil,
                firstVisualLine: lineValue.first_visual_line,
                spans: spans,
                textCells: textCells
            )
        }

        let cursors: [EditorSnapshotCursor] = (0..<Int(infoValue.cursor_count)).map { index in
            let cursorValue = the_editor_snapshot_cursor_at(rawSnapshot, UInt(index))
            return EditorSnapshotCursor(
                row: Int(cursorValue.row),
                col: Int(cursorValue.col),
                kind: EditorCursorKind(rawValue: cursorValue.kind) ?? .bar,
                style: style(from: cursorValue.style)
            )
        }

        let selections: [EditorSnapshotSelection] = (0..<Int(infoValue.selection_count)).map { index in
            let selectionValue = the_editor_snapshot_selection_at(rawSnapshot, UInt(index))
            return EditorSnapshotSelection(
                x: Int(selectionValue.x),
                y: Int(selectionValue.y),
                width: Int(selectionValue.width),
                height: Int(selectionValue.height),
                kind: EditorSelectionKind(rawValue: selectionValue.kind) ?? .primary,
                style: style(from: selectionValue.style)
            )
        }

        let overlays: [EditorSnapshotOverlay] = (0..<Int(infoValue.overlay_count)).map { index in
            let overlayValue = the_editor_snapshot_overlay_at(rawSnapshot, UInt(index))
            return EditorSnapshotOverlay(
                kind: EditorOverlayKind(rawValue: overlayValue.kind) ?? .rect,
                rectKind: EditorOverlayKind(rawValue: overlayValue.kind) == .rect
                    ? (EditorOverlayRectKind(rawValue: overlayValue.rect_kind) ?? .panel)
                    : nil,
                x: Int(overlayValue.x),
                y: Int(overlayValue.y),
                width: Int(overlayValue.width),
                height: Int(overlayValue.height),
                radius: Int(overlayValue.radius),
                row: Int(overlayValue.row),
                col: Int(overlayValue.col),
                text: overlayValue.text.map { String(cString: $0) },
                style: style(from: overlayValue.style)
            )
        }

        return EditorSnapshot(
            info: info,
            lines: lines,
            cursors: cursors,
            selections: selections,
            overlays: overlays,
            commandPalette: commandPalette
        )
    }

    private static func style(from style: the_editor_style_t) -> EditorResolvedStyle {
        EditorResolvedStyle(
            fg: rgba(from: style.fg),
            bg: rgba(from: style.bg),
            underlineColor: rgba(from: style.underline_color),
            addModifiers: style.add_modifiers,
            removeModifiers: style.remove_modifiers,
            underlineStyle: style.underline_style
        )
    }

    private static func surfaceMetrics(from metrics: the_editor_surface_metrics_t) -> EditorSurfaceMetrics {
        EditorSurfaceMetrics(
            backingScale: CGFloat(metrics.backing_scale),
            cellWidthPx: Int(metrics.cell_width_px),
            cellHeightPx: Int(metrics.cell_height_px),
            cellBaselinePx: Int(metrics.cell_baseline_px),
            underlinePositionPx: Int(metrics.underline_position_px),
            underlineThicknessPx: Int(metrics.underline_thickness_px),
            cursorThicknessPx: Int(metrics.cursor_thickness_px)
        )
    }

    private static func rgba(from rgba: the_editor_rgba_t) -> EditorRGBA? {
        guard rgba.present else { return nil }
        return EditorRGBA(r: rgba.r, g: rgba.g, b: rgba.b, a: rgba.a)
    }
}
