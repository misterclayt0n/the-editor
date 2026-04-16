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
    let backgroundColor: EditorRGBA?
    let gutterBackgroundColor: EditorRGBA?
    let selectionColor: EditorRGBA?
    let viewportWidth: Int
    let viewportHeight: Int
    let contentOffsetX: Int
    let activePaneID: UInt
    let paneCount: Int
    let separatorCount: Int
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
    let cursorBlinkEnabled: Bool
    let cursorBlinkIntervalMs: Int
    let cursorBlinkDelayMs: Int
    let cursorBlinkGeneration: UInt64
    let scrollRow: Int
    let scrollCol: Int
    let documentLineCount: Int
    let softWrapEnabled: Bool
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

enum EditorStatusItemEmphasis: UInt8 {
    case normal = 0
    case muted = 1
    case strong = 2
}

struct EditorDocumentChrome: Hashable {
    let name: String
    let icon: String
    let relativePath: String?
    let absolutePath: String?
    let vcsText: String?
    let languageName: String?
    let encodingName: String?
    let lineEndingName: String?
    let isModified: Bool
    let isReadonly: Bool

    static let empty = EditorDocumentChrome(
        name: "Untitled",
        icon: "doc",
        relativePath: nil,
        absolutePath: nil,
        vcsText: nil,
        languageName: nil,
        encodingName: nil,
        lineEndingName: nil,
        isModified: false,
        isReadonly: false
    )
}

struct EditorStatusItem: Hashable, Identifiable {
    let icon: String?
    let text: String
    let emphasis: EditorStatusItemEmphasis

    var id: String {
        "\(icon ?? "")|\(text)|\(emphasis.rawValue)"
    }
}

struct EditorStatusBarState: Hashable {
    let leadingText: String?
    let items: [EditorStatusItem]
    let cursorText: String

    static let empty = EditorStatusBarState(
        leadingText: nil,
        items: [],
        cursorText: "1:1"
    )
}

struct EditorPendingKeyOutcome: Hashable, Identifiable {
    let pathDisplay: String
    let label: String
    let depth: Int
    let isImmediate: Bool

    var id: String { pathDisplay }
}

struct EditorPendingKeyState: Hashable {
    let scope: String?
    let pendingDisplay: String
    let immediateCount: Int
    let outcomes: [EditorPendingKeyOutcome]

    var outcomeCount: Int { outcomes.count }
}

struct EditorSnapshotPane: Hashable, Identifiable {
    let paneID: UInt
    let kind: EditorPaneKind
    let clientSurfaceID: UInt?
    let x: Int
    let y: Int
    let width: Int
    let height: Int
    let contentOffsetX: Int
    let scrollRow: Int
    let viewportRows: Int
    let documentLineCount: Int
    let isActive: Bool

    var id: UInt { paneID }
}

struct EditorSnapshotSeparator: Hashable, Identifiable {
    let splitID: UInt
    let axis: EditorSplitAxis
    let line: Int
    let spanStart: Int
    let spanEnd: Int

    var id: UInt { splitID }
}

struct EditorSnapshotLine: Hashable {
    let paneID: UInt
    let x: Int
    let row: Int
    let width: Int
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

enum EditorDocsRunKind: UInt8 {
    case body = 0
    case heading1 = 1
    case heading2 = 2
    case heading3 = 3
    case heading4 = 4
    case heading5 = 5
    case heading6 = 6
    case listMarker = 7
    case quoteMarker = 8
    case quoteText = 9
    case link = 10
    case inlineCode = 11
    case code = 12
    case activeParameter = 13
    case rule = 14
}

struct EditorDocsRun: Hashable {
    let text: String
    let style: EditorResolvedStyle
    let kind: EditorDocsRunKind
    let linkDestination: String?
}

enum EditorMarkdownBlockKind: UInt8 {
    case paragraph = 0
    case heading = 1
    case listItem = 2
    case quote = 3
    case codeFence = 4
    case rule = 5
    case blankLine = 6
}

struct EditorMarkdownBlock: Hashable {
    let kind: EditorMarkdownBlockKind
    let level: Int
    let listDepth: Int
    let runStart: Int
    let runCount: Int
    let language: String?
}

struct EditorRenderedMarkdown: Hashable {
    let runs: [EditorDocsRun]
    let blocks: [EditorMarkdownBlock]

    static let empty = EditorRenderedMarkdown(runs: [], blocks: [])
}

struct EditorDocsPanelState: Hashable {
    let isOpen: Bool
    let col: Int
    let row: Int
    let width: Int
    let height: Int
    let runs: [EditorDocsRun]

    static let empty = EditorDocsPanelState(
        isOpen: false,
        col: 0,
        row: 0,
        width: 0,
        height: 0,
        runs: []
    )
}

enum EditorDiagnosticSeverity: UInt8, Hashable {
    case error = 1
    case warning = 2
    case information = 3
    case hint = 4

    var symbolName: String {
        switch self {
        case .error:
            return "xmark.octagon.fill"
        case .warning:
            return "exclamationmark.triangle.fill"
        case .information:
            return "info.circle.fill"
        case .hint:
            return "lightbulb.fill"
        }
    }

    var statusIconName: String {
        switch self {
        case .error:
            return "diagnostic_error"
        case .warning:
            return "diagnostic_warning"
        case .information:
            return "diagnostic_info"
        case .hint:
            return "diagnostic_hint"
        }
    }

    var sortRank: Int {
        switch self {
        case .error:
            return 3
        case .warning:
            return 2
        case .information:
            return 1
        case .hint:
            return 0
        }
    }
}

struct EditorSnapshotDiagnostic: Hashable, Identifiable {
    let index: Int
    let startLine: Int
    let startCharacter: Int
    let endLine: Int
    let endCharacter: Int
    let severity: EditorDiagnosticSeverity
    let message: String
    let source: String?
    let code: String?

    var id: Int { index }
}

struct EditorSnapshotDiagnosticUnderline: Hashable {
    let row: Int
    let startCol: Int
    let endCol: Int
    let severity: EditorDiagnosticSeverity
}

struct EditorCompletionMenuItem: Hashable, Identifiable {
    let index: Int
    let title: String
    let subtitle: String?
    let leadingIcon: String?
    let leadingColor: EditorRGBA?

    var id: Int { index }
}

struct EditorCompletionMenuState: Hashable {
    let isOpen: Bool
    let col: Int
    let row: Int
    let width: Int
    let height: Int
    let selectedIndex: Int?
    let scrollOffset: Int
    let items: [EditorCompletionMenuItem]

    static let empty = EditorCompletionMenuState(
        isOpen: false,
        col: 0,
        row: 0,
        width: 0,
        height: 0,
        selectedIndex: nil,
        scrollOffset: 0,
        items: []
    )
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

enum EditorInputPromptKind: UInt8 {
    case search = 0
    case selectRegex = 1
    case splitSelection = 2
    case keepSelections = 3
    case removeSelections = 4
    case renameSymbol = 5
    case shellPipe = 6
    case shellPipeTo = 7
    case shellInsertOutput = 8
    case shellAppendOutput = 9
    case shellKeepPipe = 10
}

struct EditorInputPromptState: Hashable {
    let isOpen: Bool
    let kind: EditorInputPromptKind
    let title: String
    let placeholder: String
    let query: String
    let error: String?

    var canNavigate: Bool {
        kind == .search
    }

    static let empty = EditorInputPromptState(
        isOpen: false,
        kind: .search,
        title: "Search",
        placeholder: "Search",
        query: "",
        error: nil
    )
}

enum EditorFilePickerKind: UInt8 {
    case generic = 0
    case diagnostics = 1
    case symbols = 2
    case liveGrep = 3
    case vcsDiff = 4
}

enum EditorFilePickerRowKind: UInt8 {
    case generic = 0
    case diagnostics = 1
    case symbols = 2
    case liveGrepHeader = 3
    case liveGrepMatch = 4
    case vcsDiffHeader = 5
    case vcsDiffHunk = 6
}

enum EditorFilePickerPreviewKind: UInt8 {
    case empty = 0
    case source = 1
    case text = 2
    case message = 3
    case vcsDiff = 4
}

enum EditorFilePickerPreviewNavigationMode: UInt8 {
    case `static` = 0
    case scrollable = 1
    case anchored = 2
}

enum EditorFilePickerPreviewLineKind: UInt8 {
    case content = 0
    case truncatedAbove = 1
    case truncatedBelow = 2
    case sectionHeader = 3
    case info = 4
    case added = 5
    case removed = 6
    case modified = 7
}

enum EditorFilePickerPreviewSource: UInt8 {
    case none = 0
    case base = 1
    case worktree = 2
    case meta = 3
}

enum EditorFilePickerPreviewChangeKind: Int8 {
    case added = 0
    case removed = 1
    case modified = 2
}

enum EditorFilePickerSearchMode: UInt8, Hashable {
    case none = 0
    case plainText = 1
    case regex = 2
    case fuzzy = 3
}

enum EditorFilePickerStatusBannerKind: UInt8, Hashable {
    case info = 0
    case warning = 1
}

struct EditorFilePickerStatusBanner: Hashable {
    let kind: EditorFilePickerStatusBannerKind
    let text: String
}

private func parseFilePickerMatchRanges(_ raw: UnsafePointer<CChar>?) -> [Range<Int>] {
    guard let raw else { return [] }
    return String(cString: raw)
        .split(separator: ",")
        .compactMap { component in
            let parts = component.split(separator: ":", maxSplits: 1).map(String.init)
            guard parts.count == 2,
                  let start = Int(parts[0]),
                  let end = Int(parts[1]),
                  end > start
            else {
                return nil
            }
            return start..<end
        }
}

struct EditorFilePickerItem: Hashable, Identifiable {
    let stableID: UInt64
    let globalIndex: Int
    let rowKind: EditorFilePickerRowKind
    let selectable: Bool
    let isDirectory: Bool
    let icon: String
    let primary: String
    let secondary: String?
    let tertiary: String?
    let quaternary: String?
    let primaryMatchRanges: [Range<Int>]
    let secondaryMatchRanges: [Range<Int>]
    let line: Int
    let column: Int
    let depth: Int

    var id: UInt64 { stableID }
}

struct EditorFilePickerPreviewSegment: Hashable {
    let text: String
    let style: EditorResolvedStyle
    let isMatch: Bool
    let changeKind: EditorFilePickerPreviewChangeKind?
}

struct EditorFilePickerPreviewLine: Hashable, Identifiable {
    let virtualRow: Int
    let kind: EditorFilePickerPreviewLineKind
    let source: EditorFilePickerPreviewSource
    let lineNumber: Int?
    let focused: Bool
    let marker: String?
    let segments: [EditorFilePickerPreviewSegment]

    var id: Int { virtualRow }
}

struct EditorFilePickerState: Hashable {
    let isOpen: Bool
    let kind: EditorFilePickerKind
    let selectedIndex: Int?
    let matchedCount: Int
    let visibleItemStart: Int
    let title: String
    let query: String
    let showPreview: Bool
    let isLoading: Bool
    let error: String?
    let statusBanner: EditorFilePickerStatusBanner?
    let searchMode: EditorFilePickerSearchMode
    let previewPath: String?
    let previewNavigationMode: EditorFilePickerPreviewNavigationMode
    let previewKind: EditorFilePickerPreviewKind
    let previewTotalRows: Int
    let previewOffset: Int
    let previewWindowStart: Int
    let items: [EditorFilePickerItem]
    let previewLines: [EditorFilePickerPreviewLine]

    static let empty = EditorFilePickerState(
        isOpen: false,
        kind: .generic,
        selectedIndex: nil,
        matchedCount: 0,
        visibleItemStart: 0,
        title: "File Picker",
        query: "",
        showPreview: true,
        isLoading: false,
        error: nil,
        statusBanner: nil,
        searchMode: .none,
        previewPath: nil,
        previewNavigationMode: .static,
        previewKind: .empty,
        previewTotalRows: 0,
        previewOffset: 0,
        previewWindowStart: 0,
        items: [],
        previewLines: []
    )
}

enum EditorFileTreeVcsKind: UInt8, Hashable {
    case conflict = 1
    case deleted = 2
    case modified = 3
    case renamed = 4
    case untracked = 5
}

struct EditorBufferTabRow: Hashable, Identifiable {
    let bufferID: UInt
    let title: String
    let directoryHint: String?
    let filePath: String?
    let iconName: String
    let isActive: Bool
    let isModified: Bool
    let vcsKind: EditorFileTreeVcsKind?
    let diagnosticSeverity: EditorDiagnosticSeverity?

    var id: UInt { bufferID }
}

struct EditorBufferTabsState: Hashable {
    let isVisible: Bool
    let activeIndex: Int?
    let activeBufferID: UInt?
    let tabs: [EditorBufferTabRow]

    static let empty = EditorBufferTabsState(
        isVisible: false,
        activeIndex: nil,
        activeBufferID: nil,
        tabs: []
    )
}

enum EditorOpenItemKind: UInt8, Hashable {
    case buffer = 0
    case terminal = 1
    case agent = 2
}

struct EditorPaneOpenItemRow: Hashable, Identifiable {
    let paneID: UInt
    let kind: EditorOpenItemKind
    let itemID: UInt
    let bufferID: UInt?
    let clientSurfaceID: UInt?
    let title: String
    let subtitle: String?
    let filePath: String?
    let iconName: String
    let isActive: Bool
    let isModified: Bool
    let vcsKind: EditorFileTreeVcsKind?
    let diagnosticSeverity: EditorDiagnosticSeverity?

    var id: String { "\(paneID):\(kind.rawValue):\(itemID)" }
}

struct EditorPaneOpenItemGroup: Hashable, Identifiable {
    let paneID: UInt
    let isActivePane: Bool
    let activeIndex: Int?
    let items: [EditorPaneOpenItemRow]

    var id: UInt { paneID }
}

struct EditorPaneOpenItemsState: Hashable {
    let isVisible: Bool
    let groups: [EditorPaneOpenItemGroup]

    var totalItemCount: Int {
        groups.reduce(0) { $0 + $1.items.count }
    }

    static let empty = EditorPaneOpenItemsState(
        isVisible: false,
        groups: []
    )
}

struct EditorFileTreeRow: Hashable, Identifiable {
    let path: String
    let displayName: String
    let iconName: String
    let iconGlyph: String
    let depth: Int
    let hasChildren: Bool
    let isDirectory: Bool
    let isExpanded: Bool
    let isCurrentFile: Bool
    let isSelected: Bool
    let vcsKind: EditorFileTreeVcsKind?
    let diagnosticSeverity: EditorDiagnosticSeverity?

    var id: String { path }
}

struct EditorFileTreeState: Hashable {
    let isVisible: Bool
    let paneID: UInt?
    let root: String?
    let selectedIndex: Int?
    let scrollOffset: Int
    let rows: [EditorFileTreeRow]

    static let empty = EditorFileTreeState(
        isVisible: false,
        paneID: nil,
        root: nil,
        selectedIndex: nil,
        scrollOffset: 0,
        rows: []
    )
}

enum EditorPaneKind: UInt8 {
    case editorBuffer = 0
    case clientSurface = 1
    case agent = 2
}

enum EditorSplitAxis: UInt8 {
    case horizontal = 0
    case vertical = 1
}

struct EditorSnapshot {
    let info: EditorSnapshotInfo
    let document: EditorDocumentChrome
    let statusBar: EditorStatusBarState
    let pendingKeys: EditorPendingKeyState?
    let panes: [EditorSnapshotPane]
    let separators: [EditorSnapshotSeparator]
    let lines: [EditorSnapshotLine]
    let cursors: [EditorSnapshotCursor]
    let selections: [EditorSnapshotSelection]
    let overlays: [EditorSnapshotOverlay]
    let diagnostics: [EditorSnapshotDiagnostic]
    let diagnosticUnderlines: [EditorSnapshotDiagnosticUnderline]
    let commandPalette: EditorCommandPaletteState
    let completionMenu: EditorCompletionMenuState
    let inputPrompt: EditorInputPromptState
    let hoverDocs: EditorDocsPanelState
    let completionDocs: EditorDocsPanelState
    let signatureHelp: EditorDocsPanelState
    let filePicker: EditorFilePickerState
    let bufferTabs: EditorBufferTabsState
    let openItems: EditorPaneOpenItemsState
    let fileTree: EditorFileTreeState
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
    static func setActivePane(_ handle: OpaquePointer?, paneID: UInt) -> Bool {
        guard let handle else { return false }
        return the_editor_set_active_pane(handle, paneID)
    }

    @discardableResult
    static func resizeSplit(_ handle: OpaquePointer?, splitID: UInt, x: Int, y: Int) -> Bool {
        guard let handle else { return false }
        return the_editor_resize_split(handle, splitID, UInt16(clamping: x), UInt16(clamping: y))
    }

    @discardableResult
    static func splitActivePaneVertical(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_split_active_pane_vertical(handle)
    }

    @discardableResult
    static func splitActivePaneHorizontal(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_split_active_pane_horizontal(handle)
    }

    @discardableResult
    static func closeActivePaneItem(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_close_active_pane_item(handle)
    }

    @discardableResult
    static func clickBufferPosition(
        _ handle: OpaquePointer?,
        paneID: UInt,
        logicalCol: Int,
        logicalRow: Int,
        modifiers: UInt8,
        clickCount: Int
    ) -> Bool {
        guard let handle else { return false }
        return the_editor_click_buffer_position(
            handle,
            paneID,
            UInt16(clamping: logicalCol),
            UInt16(clamping: logicalRow),
            modifiers,
            UInt8(clamping: clickCount)
        )
    }

    @discardableResult
    static func dragBufferSelection(
        _ handle: OpaquePointer?,
        paneID: UInt,
        dragOriginCol: Int,
        dragOriginRow: Int,
        logicalCol: Int,
        logicalRow: Int,
        modifiers: UInt8,
        clickCount: Int
    ) -> Bool {
        guard let handle else { return false }
        return the_editor_drag_buffer_selection(
            handle,
            paneID,
            UInt16(clamping: dragOriginCol),
            UInt16(clamping: dragOriginRow),
            UInt16(clamping: logicalCol),
            UInt16(clamping: logicalRow),
            modifiers,
            UInt8(clamping: clickCount)
        )
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
    static func closeCompletionMenu(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_close_completion_menu(handle)
    }

    @discardableResult
    static func selectCompletionMenuIndex(_ handle: OpaquePointer?, index: Int) -> Bool {
        guard let handle else { return false }
        return the_editor_completion_menu_select_index(handle, UInt(index))
    }

    @discardableResult
    static func setCompletionMenuScroll(_ handle: OpaquePointer?, offset: Int) -> Bool {
        guard let handle else { return false }
        return the_editor_set_completion_menu_scroll(handle, UInt(offset))
    }

    @discardableResult
    static func submitCompletionMenu(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_completion_menu_submit(handle)
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
    static func pollBackgroundTasks(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_poll_background_tasks(handle)
    }

    @discardableResult
    static func openSearchPrompt(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_open_search_prompt(handle)
    }

    @discardableResult
    static func closeInputPrompt(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_close_input_prompt(handle)
    }

    @discardableResult
    static func setInputPromptQuery(_ handle: OpaquePointer?, query: String) -> Bool {
        guard let handle else { return false }
        return query.withCString { the_editor_input_prompt_set_query(handle, $0) }
    }

    @discardableResult
    static func submitInputPrompt(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_input_prompt_submit(handle)
    }

    @discardableResult
    static func stepNextInputPrompt(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_input_prompt_step_next(handle)
    }

    @discardableResult
    static func stepPreviousInputPrompt(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_input_prompt_step_previous(handle)
    }

    @discardableResult
    static func closeDocsPanels(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_close_docs_panels(handle)
    }

    @discardableResult
    static func configureFilePicker(_ handle: OpaquePointer?, listVisibleRows: Int, previewVisibleRows: Int) -> Bool {
        guard let handle else { return false }
        return the_editor_configure_file_picker(handle, UInt(listVisibleRows), UInt(previewVisibleRows))
    }

    @discardableResult
    static func closeFilePicker(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_close_file_picker(handle)
    }

    @discardableResult
    static func setFilePickerQuery(_ handle: OpaquePointer?, query: String) -> Bool {
        guard let handle else { return false }
        return query.withCString { the_editor_file_picker_set_query(handle, $0) }
    }

    @discardableResult
    static func cycleFilePickerSearchMode(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_file_picker_cycle_search_mode(handle)
    }

    @discardableResult
    static func selectNextFilePickerItem(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_file_picker_select_next(handle)
    }

    @discardableResult
    static func selectPreviousFilePickerItem(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_file_picker_select_previous(handle)
    }

    @discardableResult
    static func setFilePickerListOffset(_ handle: OpaquePointer?, offset: Int) -> Bool {
        guard let handle else { return false }
        return the_editor_file_picker_set_list_offset(handle, UInt(offset))
    }

    @discardableResult
    static func setFilePickerPreviewOffset(_ handle: OpaquePointer?, offset: Int, visibleRows: Int) -> Bool {
        guard let handle else { return false }
        return the_editor_file_picker_set_preview_offset(handle, UInt(offset), UInt(visibleRows))
    }

    @discardableResult
    static func activateBufferTab(_ handle: OpaquePointer?, bufferID: UInt) -> Bool {
        guard let handle else { return false }
        return the_editor_activate_buffer_tab(handle, bufferID)
    }

    @discardableResult
    static func closeBufferTab(_ handle: OpaquePointer?, bufferID: UInt) -> Bool {
        guard let handle else { return false }
        return the_editor_close_buffer_tab(handle, bufferID)
    }

    @discardableResult
    static func activateOpenItem(_ handle: OpaquePointer?, paneID: UInt, kind: EditorOpenItemKind, itemID: UInt) -> Bool {
        guard let handle else { return false }
        return the_editor_activate_open_item(handle, paneID, kind.rawValue, itemID)
    }

    @discardableResult
    static func closeOpenItem(_ handle: OpaquePointer?, paneID: UInt, kind: EditorOpenItemKind, itemID: UInt) -> Bool {
        guard let handle else { return false }
        return the_editor_close_open_item(handle, paneID, kind.rawValue, itemID)
    }

    @discardableResult
    static func setEmbeddedTerminalEnabled(_ handle: OpaquePointer?, enabled: Bool) -> Bool {
        guard let handle else { return false }
        return the_editor_set_embedded_terminal_enabled(handle, enabled)
    }

    static func takeQuitRequested(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_take_quit_requested(handle)
    }

    @discardableResult
    static func openTerminalInActivePane(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_open_terminal_in_active_pane(handle)
    }

    @discardableResult
    static func openAgentInActivePane(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_open_agent_in_active_pane(handle)
    }

    @discardableResult
    static func closeTerminalInActivePane(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_close_terminal_in_active_pane(handle)
    }

    @discardableResult
    static func selectFileTreeIndex(_ handle: OpaquePointer?, index: Int) -> Bool {
        guard let handle else { return false }
        return the_editor_file_tree_select_index(handle, UInt(max(index, 0)))
    }

    @discardableResult
    static func clickFileTreeIndex(_ handle: OpaquePointer?, index: Int) -> Bool {
        guard let handle else { return false }
        return the_editor_file_tree_click_index(handle, UInt(max(index, 0)))
    }

    @discardableResult
    static func activateFileTreeIndex(_ handle: OpaquePointer?, index: Int) -> Bool {
        guard let handle else { return false }
        return the_editor_file_tree_activate_index(handle, UInt(max(index, 0)))
    }

    @discardableResult
    static func setFileTreeVisibleRows(_ handle: OpaquePointer?, visibleRows: Int) -> Bool {
        guard let handle else { return false }
        return the_editor_file_tree_set_visible_rows(handle, UInt(max(visibleRows, 1)))
    }

    @discardableResult
    static func setFileTreeScrollOffset(_ handle: OpaquePointer?, scrollOffset: Int) -> Bool {
        guard let handle else { return false }
        return the_editor_file_tree_set_scroll_offset(handle, UInt(max(scrollOffset, 0)))
    }

    @discardableResult
    static func setFileTreeActive(_ handle: OpaquePointer?, active: Bool) -> Bool {
        guard let handle else { return false }
        return the_editor_file_tree_set_active(handle, active)
    }

    @discardableResult
    static func toggleFileTree(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_toggle_file_tree(handle)
    }

    @discardableResult
    static func selectFilePickerIndex(_ handle: OpaquePointer?, index: Int) -> Bool {
        guard let handle else { return false }
        return the_editor_file_picker_select_index(handle, UInt(index))
    }

    @discardableResult
    static func submitFilePicker(_ handle: OpaquePointer?) -> Bool {
        guard let handle else { return false }
        return the_editor_file_picker_submit(handle)
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

    static func renderMarkdown(_ handle: OpaquePointer?, markdown: String) -> EditorRenderedMarkdown {
        guard let handle else { return .empty }
        return markdown.withCString { rawMarkdown in
            guard let render = the_editor_render_markdown(handle, rawMarkdown) else {
                return .empty
            }
            defer { the_editor_markdown_render_free(render) }

            let runCount = Int(the_editor_markdown_render_run_count(render))
            let runs = (0..<runCount).map { runIndex in
                let runValue = the_editor_markdown_render_run_at(render, UInt(runIndex))
                return EditorDocsRun(
                    text: runValue.text.map { String(cString: $0) } ?? "",
                    style: Self.style(from: runValue.style),
                    kind: EditorDocsRunKind(rawValue: runValue.kind) ?? .body,
                    linkDestination: runValue.link_destination.map { String(cString: $0) }
                )
            }

            let blockCount = Int(the_editor_markdown_render_block_count(render))
            let blocks = (0..<blockCount).map { blockIndex in
                let blockValue = the_editor_markdown_render_block_at(render, UInt(blockIndex))
                return EditorMarkdownBlock(
                    kind: EditorMarkdownBlockKind(rawValue: blockValue.kind) ?? .blankLine,
                    level: Int(blockValue.level),
                    listDepth: Int(blockValue.list_depth),
                    runStart: Int(blockValue.run_start),
                    runCount: Int(blockValue.run_count),
                    language: blockValue.language.map { String(cString: $0) }
                )
            }

            return EditorRenderedMarkdown(runs: runs, blocks: blocks)
        }
    }

    static func makeSnapshot(_ handle: OpaquePointer?) -> EditorSnapshot? {
        guard let handle else { return nil }
        let createStarted = CFAbsoluteTimeGetCurrent()
        guard let rawSnapshot = the_editor_snapshot_create(handle) else {
            return nil
        }
        let createMs = (CFAbsoluteTimeGetCurrent() - createStarted) * 1000
        defer { the_editor_snapshot_free(rawSnapshot) }

        let decodeStarted = CFAbsoluteTimeGetCurrent()
        let infoValue = the_editor_snapshot_info(rawSnapshot)
        let info = EditorSnapshotInfo(
            surfaceWidthPx: Int(infoValue.surface_width_px),
            surfaceHeightPx: Int(infoValue.surface_height_px),
            surfaceMetrics: surfaceMetrics(from: infoValue.surface_metrics),
            backgroundColor: rgba(from: infoValue.background_color),
            gutterBackgroundColor: rgba(from: infoValue.gutter_background_color),
            selectionColor: rgba(from: infoValue.selection_color),
            viewportWidth: Int(infoValue.viewport_width),
            viewportHeight: Int(infoValue.viewport_height),
            contentOffsetX: Int(infoValue.content_offset_x),
            activePaneID: UInt(infoValue.active_pane_id),
            paneCount: Int(infoValue.pane_count),
            separatorCount: Int(infoValue.separator_count),
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
            cursorBlinkEnabled: infoValue.cursor_blink_enabled,
            cursorBlinkIntervalMs: Int(infoValue.cursor_blink_interval_ms),
            cursorBlinkDelayMs: Int(infoValue.cursor_blink_delay_ms),
            cursorBlinkGeneration: infoValue.cursor_blink_generation,
            scrollRow: Int(infoValue.scroll_row),
            scrollCol: Int(infoValue.scroll_col),
            documentLineCount: Int(infoValue.document_line_count),
            softWrapEnabled: infoValue.soft_wrap_enabled
        )

        let panes: [EditorSnapshotPane] = (0..<info.paneCount).map { paneIndex in
            let paneValue = the_editor_snapshot_pane_at(rawSnapshot, UInt(paneIndex))
            return EditorSnapshotPane(
                paneID: UInt(paneValue.pane_id),
                kind: EditorPaneKind(rawValue: paneValue.kind) ?? .editorBuffer,
                clientSurfaceID: paneValue.client_surface_id == 0 ? nil : UInt(paneValue.client_surface_id),
                x: Int(paneValue.x),
                y: Int(paneValue.y),
                width: Int(paneValue.width),
                height: Int(paneValue.height),
                contentOffsetX: Int(paneValue.content_offset_x),
                scrollRow: Int(paneValue.scroll_row),
                viewportRows: Int(paneValue.viewport_rows),
                documentLineCount: Int(paneValue.document_line_count),
                isActive: paneValue.is_active
            )
        }

        let separators: [EditorSnapshotSeparator] = (0..<info.separatorCount).map { separatorIndex in
            let separatorValue = the_editor_snapshot_separator_at(rawSnapshot, UInt(separatorIndex))
            return EditorSnapshotSeparator(
                splitID: UInt(separatorValue.split_id),
                axis: EditorSplitAxis(rawValue: separatorValue.axis) ?? .horizontal,
                line: Int(separatorValue.line),
                spanStart: Int(separatorValue.span_start),
                spanEnd: Int(separatorValue.span_end)
            )
        }

        let documentValue = the_editor_snapshot_document(rawSnapshot)
        let document = EditorDocumentChrome(
            name: documentValue.name.map { String(cString: $0) } ?? "Untitled",
            icon: documentValue.icon.map { String(cString: $0) } ?? "doc",
            relativePath: documentValue.relative_path.map { String(cString: $0) },
            absolutePath: documentValue.absolute_path.map { String(cString: $0) },
            vcsText: documentValue.vcs_text.map { String(cString: $0) },
            languageName: documentValue.language_name.map { String(cString: $0) },
            encodingName: documentValue.encoding_name.map { String(cString: $0) },
            lineEndingName: documentValue.line_ending_name.map { String(cString: $0) },
            isModified: documentValue.is_modified,
            isReadonly: documentValue.is_readonly
        )

        let statusValue = the_editor_snapshot_status(rawSnapshot)
        let statusBar = EditorStatusBarState(
            leadingText: statusValue.leading_text.map { String(cString: $0) },
            items: (0..<Int(statusValue.item_count)).map { itemIndex in
                let itemValue = the_editor_snapshot_status_item_at(rawSnapshot, UInt(itemIndex))
                return EditorStatusItem(
                    icon: itemValue.icon.map { String(cString: $0) },
                    text: itemValue.text.map { String(cString: $0) } ?? "",
                    emphasis: EditorStatusItemEmphasis(rawValue: itemValue.emphasis) ?? .normal
                )
            },
            cursorText: statusValue.cursor_text.map { String(cString: $0) } ?? "1:1"
        )

        let pendingKeysValue = the_editor_snapshot_pending_keys(rawSnapshot)
        let pendingKeyOutcomes: [EditorPendingKeyOutcome] = (0..<Int(pendingKeysValue.outcome_count)).map { outcomeIndex in
            let outcomeValue = the_editor_snapshot_pending_key_outcome_at(rawSnapshot, UInt(outcomeIndex))
            return EditorPendingKeyOutcome(
                pathDisplay: outcomeValue.path_display.map { String(cString: $0) } ?? "",
                label: outcomeValue.label.map { String(cString: $0) } ?? "",
                depth: Int(outcomeValue.depth),
                isImmediate: outcomeValue.immediate
            )
        }
        let pendingKeys: EditorPendingKeyState? = pendingKeysValue.visible ? EditorPendingKeyState(
            scope: pendingKeysValue.scope.map { String(cString: $0) },
            pendingDisplay: pendingKeysValue.pending_display.map { String(cString: $0) } ?? "",
            immediateCount: Int(pendingKeysValue.immediate_count),
            outcomes: pendingKeyOutcomes
        ) : nil

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

        let completionMenuValue = the_editor_snapshot_completion_menu(rawSnapshot)
        let completionMenuItems: [EditorCompletionMenuItem] = (0..<Int(completionMenuValue.item_count)).map { itemIndex in
            let itemValue = the_editor_snapshot_completion_menu_item_at(rawSnapshot, UInt(itemIndex))
            return EditorCompletionMenuItem(
                index: itemIndex,
                title: itemValue.title.map { String(cString: $0) } ?? "",
                subtitle: itemValue.subtitle.map { String(cString: $0) },
                leadingIcon: itemValue.leading_icon.map { String(cString: $0) },
                leadingColor: rgba(from: itemValue.leading_color)
            )
        }
        let completionMenu = EditorCompletionMenuState(
            isOpen: completionMenuValue.is_open,
            col: Int(completionMenuValue.col),
            row: Int(completionMenuValue.row),
            width: Int(completionMenuValue.width),
            height: Int(completionMenuValue.height),
            selectedIndex: completionMenuValue.selected_index >= 0 ? Int(completionMenuValue.selected_index) : nil,
            scrollOffset: Int(completionMenuValue.scroll_offset),
            items: completionMenuItems
        )

        let inputPromptValue = the_editor_snapshot_input_prompt(rawSnapshot)
        let inputPrompt = EditorInputPromptState(
            isOpen: inputPromptValue.is_open,
            kind: EditorInputPromptKind(rawValue: inputPromptValue.kind) ?? .search,
            title: inputPromptValue.title.map { String(cString: $0) } ?? "Search",
            placeholder: inputPromptValue.placeholder.map { String(cString: $0) } ?? "Search",
            query: inputPromptValue.query.map { String(cString: $0) } ?? "",
            error: inputPromptValue.error.map { String(cString: $0) }
        )

        let hoverDocsValue = the_editor_snapshot_hover_docs_panel(rawSnapshot)
        let hoverDocsRuns: [EditorDocsRun] = (0..<Int(hoverDocsValue.run_count)).map { runIndex in
            let runValue = the_editor_snapshot_hover_docs_run_at(rawSnapshot, UInt(runIndex))
            return EditorDocsRun(
                text: runValue.text.map { String(cString: $0) } ?? "",
                style: style(from: runValue.style),
                kind: EditorDocsRunKind(rawValue: runValue.kind) ?? .body,
                linkDestination: runValue.link_destination.map { String(cString: $0) }
            )
        }
        let hoverDocs = EditorDocsPanelState(
            isOpen: hoverDocsValue.is_open,
            col: Int(hoverDocsValue.col),
            row: Int(hoverDocsValue.row),
            width: Int(hoverDocsValue.width),
            height: Int(hoverDocsValue.height),
            runs: hoverDocsRuns
        )

        let completionDocsValue = the_editor_snapshot_completion_docs_panel(rawSnapshot)
        let completionDocsRuns: [EditorDocsRun] = (0..<Int(completionDocsValue.run_count)).map { runIndex in
            let runValue = the_editor_snapshot_completion_docs_run_at(rawSnapshot, UInt(runIndex))
            return EditorDocsRun(
                text: runValue.text.map { String(cString: $0) } ?? "",
                style: style(from: runValue.style),
                kind: EditorDocsRunKind(rawValue: runValue.kind) ?? .body,
                linkDestination: runValue.link_destination.map { String(cString: $0) }
            )
        }
        let completionDocs = EditorDocsPanelState(
            isOpen: completionDocsValue.is_open,
            col: Int(completionDocsValue.col),
            row: Int(completionDocsValue.row),
            width: Int(completionDocsValue.width),
            height: Int(completionDocsValue.height),
            runs: completionDocsRuns
        )

        let signatureHelpValue = the_editor_snapshot_signature_help_panel(rawSnapshot)
        let signatureHelpRuns: [EditorDocsRun] = (0..<Int(signatureHelpValue.run_count)).map { runIndex in
            let runValue = the_editor_snapshot_signature_help_run_at(rawSnapshot, UInt(runIndex))
            return EditorDocsRun(
                text: runValue.text.map { String(cString: $0) } ?? "",
                style: style(from: runValue.style),
                kind: EditorDocsRunKind(rawValue: runValue.kind) ?? .body,
                linkDestination: runValue.link_destination.map { String(cString: $0) }
            )
        }
        let signatureHelp = EditorDocsPanelState(
            isOpen: signatureHelpValue.is_open,
            col: Int(signatureHelpValue.col),
            row: Int(signatureHelpValue.row),
            width: Int(signatureHelpValue.width),
            height: Int(signatureHelpValue.height),
            runs: signatureHelpRuns
        )

        let diagnosticCount = Int(the_editor_snapshot_diagnostic_count(rawSnapshot))
        let diagnostics: [EditorSnapshotDiagnostic] = (0..<diagnosticCount).map { diagnosticIndex in
            let diagnosticValue = the_editor_snapshot_diagnostic_at(rawSnapshot, UInt(diagnosticIndex))
            return EditorSnapshotDiagnostic(
                index: diagnosticIndex,
                startLine: Int(diagnosticValue.start_line),
                startCharacter: Int(diagnosticValue.start_character),
                endLine: Int(diagnosticValue.end_line),
                endCharacter: Int(diagnosticValue.end_character),
                severity: EditorDiagnosticSeverity(rawValue: diagnosticValue.severity) ?? .warning,
                message: diagnosticValue.message.map { String(cString: $0) } ?? "",
                source: diagnosticValue.source.map { String(cString: $0) },
                code: diagnosticValue.code.map { String(cString: $0) }
            )
        }
        let diagnosticUnderlineCount = Int(the_editor_snapshot_diagnostic_underline_count(rawSnapshot))
        let diagnosticUnderlines: [EditorSnapshotDiagnosticUnderline] = (0..<diagnosticUnderlineCount).map { underlineIndex in
            let underlineValue = the_editor_snapshot_diagnostic_underline_at(rawSnapshot, UInt(underlineIndex))
            return EditorSnapshotDiagnosticUnderline(
                row: Int(underlineValue.row),
                startCol: Int(underlineValue.start_col),
                endCol: Int(underlineValue.end_col),
                severity: EditorDiagnosticSeverity(rawValue: underlineValue.severity) ?? .information
            )
        }

        let filePickerValue = the_editor_snapshot_file_picker(rawSnapshot)
        let filePickerItems: [EditorFilePickerItem] = (0..<Int(filePickerValue.visible_item_count)).map { itemIndex in
            let itemValue = the_editor_snapshot_file_picker_item_at(rawSnapshot, UInt(itemIndex))
            return EditorFilePickerItem(
                stableID: itemValue.stable_id,
                globalIndex: Int(itemValue.global_index),
                rowKind: EditorFilePickerRowKind(rawValue: itemValue.row_kind) ?? .generic,
                selectable: itemValue.selectable,
                isDirectory: itemValue.is_dir,
                icon: itemValue.icon.map { String(cString: $0) } ?? "",
                primary: itemValue.primary.map { String(cString: $0) } ?? "",
                secondary: itemValue.secondary.map { String(cString: $0) },
                tertiary: itemValue.tertiary.map { String(cString: $0) },
                quaternary: itemValue.quaternary.map { String(cString: $0) },
                primaryMatchRanges: parseFilePickerMatchRanges(itemValue.primary_match_ranges),
                secondaryMatchRanges: parseFilePickerMatchRanges(itemValue.secondary_match_ranges),
                line: Int(itemValue.line),
                column: Int(itemValue.column),
                depth: Int(itemValue.depth)
            )
        }
        let filePickerPreviewLines: [EditorFilePickerPreviewLine] = (0..<Int(filePickerValue.preview_window_count)).map { lineIndex in
            let lineValue = the_editor_snapshot_file_picker_preview_line_at(rawSnapshot, UInt(lineIndex))
            let segments: [EditorFilePickerPreviewSegment] = (0..<Int(lineValue.segment_count)).map { segmentIndex in
                let segmentValue = the_editor_snapshot_file_picker_preview_segment_at(rawSnapshot, UInt(lineIndex), UInt(segmentIndex))
                return EditorFilePickerPreviewSegment(
                    text: segmentValue.text.map { String(cString: $0) } ?? "",
                    style: style(from: segmentValue.style),
                    isMatch: segmentValue.is_match,
                    changeKind: EditorFilePickerPreviewChangeKind(rawValue: segmentValue.change_kind)
                )
            }
            return EditorFilePickerPreviewLine(
                virtualRow: Int(lineValue.virtual_row),
                kind: EditorFilePickerPreviewLineKind(rawValue: lineValue.kind) ?? .content,
                source: EditorFilePickerPreviewSource(rawValue: lineValue.source) ?? .none,
                lineNumber: lineValue.line_number >= 0 ? Int(lineValue.line_number) : nil,
                focused: lineValue.focused,
                marker: lineValue.marker.map { String(cString: $0) },
                segments: segments
            )
        }
        let filePicker = EditorFilePickerState(
            isOpen: filePickerValue.is_open,
            kind: EditorFilePickerKind(rawValue: filePickerValue.kind) ?? .generic,
            selectedIndex: filePickerValue.selected_index >= 0 ? Int(filePickerValue.selected_index) : nil,
            matchedCount: Int(filePickerValue.matched_count),
            visibleItemStart: Int(filePickerValue.visible_item_start),
            title: filePickerValue.title.map { String(cString: $0) } ?? "File Picker",
            query: filePickerValue.query.map { String(cString: $0) } ?? "",
            showPreview: filePickerValue.show_preview,
            isLoading: filePickerValue.loading,
            error: filePickerValue.error.map { String(cString: $0) },
            statusBanner: filePickerValue.status_banner.map {
                EditorFilePickerStatusBanner(
                    kind: EditorFilePickerStatusBannerKind(rawValue: filePickerValue.status_banner_kind) ?? .info,
                    text: String(cString: $0)
                )
            },
            searchMode: EditorFilePickerSearchMode(rawValue: filePickerValue.search_mode) ?? .none,
            previewPath: filePickerValue.preview_path.map { String(cString: $0) },
            previewNavigationMode: EditorFilePickerPreviewNavigationMode(rawValue: filePickerValue.preview_navigation_mode) ?? .static,
            previewKind: EditorFilePickerPreviewKind(rawValue: filePickerValue.preview_kind) ?? .empty,
            previewTotalRows: Int(filePickerValue.preview_total_rows),
            previewOffset: Int(filePickerValue.preview_offset),
            previewWindowStart: Int(filePickerValue.preview_window_start),
            items: filePickerItems,
            previewLines: filePickerPreviewLines
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
                paneID: UInt(lineValue.pane_id),
                x: Int(lineValue.x),
                row: Int(lineValue.row),
                width: Int(lineValue.width),
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

        let bufferTabsValue = the_editor_snapshot_buffer_tabs(rawSnapshot)
        let bufferTabRows: [EditorBufferTabRow] = (0..<Int(bufferTabsValue.row_count)).map { index in
            let rowValue = the_editor_snapshot_buffer_tab_at(rawSnapshot, UInt(index))
            return EditorBufferTabRow(
                bufferID: UInt(rowValue.buffer_id),
                title: rowValue.title.map { String(cString: $0) } ?? "",
                directoryHint: rowValue.directory_hint.map { String(cString: $0) },
                filePath: rowValue.file_path.map { String(cString: $0) },
                iconName: rowValue.icon_name.map { String(cString: $0) } ?? "doc",
                isActive: rowValue.is_active,
                isModified: rowValue.is_modified,
                vcsKind: rowValue.vcs_kind == 0 ? nil : EditorFileTreeVcsKind(rawValue: rowValue.vcs_kind),
                diagnosticSeverity: rowValue.diagnostic_severity == 0
                    ? nil
                    : EditorDiagnosticSeverity(rawValue: rowValue.diagnostic_severity)
            )
        }
        let bufferTabs = EditorBufferTabsState(
            isVisible: bufferTabsValue.visible,
            activeIndex: bufferTabsValue.active_index >= 0 ? Int(bufferTabsValue.active_index) : nil,
            activeBufferID: bufferTabsValue.active_buffer_id == 0 ? nil : UInt(bufferTabsValue.active_buffer_id),
            tabs: bufferTabRows
        )

        let openItemsValue = the_editor_snapshot_open_items(rawSnapshot)
        let openItemGroups: [EditorPaneOpenItemGroup] = (0..<Int(openItemsValue.group_count)).map { groupIndex in
            let groupValue = the_editor_snapshot_open_item_group_at(rawSnapshot, UInt(groupIndex))
            let paneID = UInt(groupValue.pane_id)
            let items: [EditorPaneOpenItemRow] = (0..<Int(groupValue.item_count)).map { itemIndex in
                let itemValue = the_editor_snapshot_open_item_at(rawSnapshot, UInt(groupIndex), UInt(itemIndex))
                let kind = EditorOpenItemKind(rawValue: itemValue.kind) ?? .buffer
                return EditorPaneOpenItemRow(
                    paneID: paneID,
                    kind: kind,
                    itemID: UInt(itemValue.item_id),
                    bufferID: itemValue.buffer_id == 0 ? nil : UInt(itemValue.buffer_id),
                    clientSurfaceID: itemValue.client_surface_id == 0 ? nil : UInt(itemValue.client_surface_id),
                    title: itemValue.title.map { String(cString: $0) } ?? "",
                    subtitle: itemValue.subtitle.map { String(cString: $0) },
                    filePath: itemValue.file_path.map { String(cString: $0) },
                    iconName: itemValue.icon_name.map { String(cString: $0) } ?? (kind == .terminal ? "terminal" : "doc"),
                    isActive: itemValue.is_active,
                    isModified: itemValue.is_modified,
                    vcsKind: itemValue.vcs_kind == 0 ? nil : EditorFileTreeVcsKind(rawValue: itemValue.vcs_kind),
                    diagnosticSeverity: itemValue.diagnostic_severity == 0
                        ? nil
                        : EditorDiagnosticSeverity(rawValue: itemValue.diagnostic_severity)
                )
            }
            return EditorPaneOpenItemGroup(
                paneID: paneID,
                isActivePane: groupValue.is_active_pane,
                activeIndex: groupValue.active_index >= 0 ? Int(groupValue.active_index) : nil,
                items: items
            )
        }
        let openItems = EditorPaneOpenItemsState(
            isVisible: openItemsValue.visible,
            groups: openItemGroups
        )

        let fileTreeValue = the_editor_snapshot_file_tree(rawSnapshot)
        let fileTreeRows: [EditorFileTreeRow] = (0..<Int(fileTreeValue.row_count)).map { index in
            let rowValue = the_editor_snapshot_file_tree_row_at(rawSnapshot, UInt(index))
            return EditorFileTreeRow(
                path: rowValue.path.map { String(cString: $0) } ?? "",
                displayName: rowValue.display_name.map { String(cString: $0) } ?? "",
                iconName: rowValue.icon_name.map { String(cString: $0) } ?? "",
                iconGlyph: rowValue.icon_glyph.map { String(cString: $0) } ?? "",
                depth: Int(rowValue.depth),
                hasChildren: rowValue.has_children,
                isDirectory: rowValue.is_dir,
                isExpanded: rowValue.is_expanded,
                isCurrentFile: rowValue.is_current_file,
                isSelected: rowValue.is_selected,
                vcsKind: rowValue.vcs_kind == 0 ? nil : EditorFileTreeVcsKind(rawValue: rowValue.vcs_kind),
                diagnosticSeverity: rowValue.diagnostic_severity == 0
                    ? nil
                    : EditorDiagnosticSeverity(rawValue: rowValue.diagnostic_severity)
            )
        }
        let fileTree = EditorFileTreeState(
            isVisible: fileTreeValue.visible,
            paneID: fileTreeValue.pane_id == 0 ? nil : UInt(fileTreeValue.pane_id),
            root: fileTreeValue.root.map { String(cString: $0) },
            selectedIndex: fileTreeValue.selected_index >= 0 ? Int(fileTreeValue.selected_index) : nil,
            scrollOffset: Int(fileTreeValue.scroll_offset),
            rows: fileTreeRows
        )

        let snapshot = EditorSnapshot(
            info: info,
            document: document,
            statusBar: statusBar,
            pendingKeys: pendingKeys,
            panes: panes,
            separators: separators,
            lines: lines,
            cursors: cursors,
            selections: selections,
            overlays: overlays,
            diagnostics: diagnostics,
            diagnosticUnderlines: diagnosticUnderlines,
            commandPalette: commandPalette,
            completionMenu: completionMenu,
            inputPrompt: inputPrompt,
            hoverDocs: hoverDocs,
            completionDocs: completionDocs,
            signatureHelp: signatureHelp,
            filePicker: filePicker,
            bufferTabs: bufferTabs,
            openItems: openItems,
            fileTree: fileTree
        )
        let decodeMs = (CFAbsoluteTimeGetCurrent() - decodeStarted) * 1000
        let spanCount = lines.reduce(into: 0) { $0 += $1.spans.count }
        let textCellCount = lines.reduce(into: 0) { $0 += $1.textCells.count }
        themePerfLog(
            "snapshot_decode themeGen=\(info.themeGeneration) createMs=\(String(format: "%.2f", createMs)) decodeMs=\(String(format: "%.2f", decodeMs)) lines=\(lines.count) spans=\(spanCount) cells=\(textCellCount) diagnostics=\(diagnostics.count) underlines=\(diagnosticUnderlines.count) paletteItems=\(commandPalette.items.count)"
        )
        return snapshot
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
