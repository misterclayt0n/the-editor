import Foundation
import TheEditorFFI
public class App: AppRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$App$_free(ptr)
        }
    }
}
extension App {
    public convenience init() {
        self.init(ptr: __swift_bridge__$App$new())
    }
}
extension App {
    class public func completion_docs_render_json<GenericToRustStr: ToRustStr>(_ markdown: GenericToRustStr, _ content_width: UInt, _ language_hint: GenericToRustStr) -> RustString {
        return language_hint.toRustStr({ language_hintAsRustStr in
            return markdown.toRustStr({ markdownAsRustStr in
            RustString(ptr: __swift_bridge__$App$completion_docs_render_json(markdownAsRustStr, content_width, language_hintAsRustStr))
        })
        })
    }

    class public func completion_popup_layout_json(_ area_width: UInt, _ area_height: UInt, _ cursor_x: Int64, _ cursor_y: Int64, _ list_width: UInt, _ list_height: UInt, _ docs_width: UInt, _ docs_height: UInt) -> RustString {
        RustString(ptr: __swift_bridge__$App$completion_popup_layout_json(area_width, area_height, cursor_x, cursor_y, list_width, list_height, docs_width, docs_height))
    }

    class public func signature_help_popup_layout_json(_ area_width: UInt, _ area_height: UInt, _ cursor_x: Int64, _ cursor_y: Int64, _ panel_width: UInt, _ panel_height: UInt) -> RustString {
        RustString(ptr: __swift_bridge__$App$signature_help_popup_layout_json(area_width, area_height, cursor_x, cursor_y, panel_width, panel_height))
    }
}
public class AppRefMut: AppRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
extension AppRefMut {
    public func create_editor<GenericToRustStr: ToRustStr>(_ text: GenericToRustStr, _ viewport: Rect, _ scroll: Position) -> EditorId {
        return text.toRustStr({ textAsRustStr in
            __swift_bridge__$App$create_editor(ptr, textAsRustStr, viewport.intoFfiRepr(), scroll.intoFfiRepr()).intoSwiftRepr()
        })
    }

    public func remove_editor(_ id: EditorId) -> Bool {
        __swift_bridge__$App$remove_editor(ptr, id.intoFfiRepr())
    }

    public func set_viewport(_ id: EditorId, _ viewport: Rect) -> Bool {
        __swift_bridge__$App$set_viewport(ptr, id.intoFfiRepr(), viewport.intoFfiRepr())
    }

    public func set_scroll(_ id: EditorId, _ scroll: Position) -> Bool {
        __swift_bridge__$App$set_scroll(ptr, id.intoFfiRepr(), scroll.intoFfiRepr())
    }

    public func set_file_path<GenericToRustStr: ToRustStr>(_ id: EditorId, _ path: GenericToRustStr) -> Bool {
        return path.toRustStr({ pathAsRustStr in
            __swift_bridge__$App$set_file_path(ptr, id.intoFfiRepr(), pathAsRustStr)
        })
    }

    public func set_active_cursor(_ id: EditorId, _ cursor_id: UInt64) -> Bool {
        __swift_bridge__$App$set_active_cursor(ptr, id.intoFfiRepr(), cursor_id)
    }

    public func clear_active_cursor(_ id: EditorId) -> Bool {
        __swift_bridge__$App$clear_active_cursor(ptr, id.intoFfiRepr())
    }

    public func split_separator_count(_ id: EditorId) -> UInt {
        __swift_bridge__$App$split_separator_count(ptr, id.intoFfiRepr())
    }

    public func split_separator_at(_ id: EditorId, _ index: UInt) -> SplitSeparator {
        SplitSeparator(ptr: __swift_bridge__$App$split_separator_at(ptr, id.intoFfiRepr(), index))
    }

    public func resize_split(_ id: EditorId, _ split_id: UInt64, _ x: UInt16, _ y: UInt16) -> Bool {
        __swift_bridge__$App$resize_split(ptr, id.intoFfiRepr(), split_id, x, y)
    }

    public func render_plan(_ id: EditorId) -> RenderPlan {
        RenderPlan(ptr: __swift_bridge__$App$render_plan(ptr, id.intoFfiRepr()))
    }

    public func frame_render_plan(_ id: EditorId) -> RenderFramePlan {
        RenderFramePlan(ptr: __swift_bridge__$App$frame_render_plan(ptr, id.intoFfiRepr()))
    }

    public func render_plan_with_styles(_ id: EditorId, _ styles: RenderStyles) -> RenderPlan {
        RenderPlan(ptr: __swift_bridge__$App$render_plan_with_styles(ptr, id.intoFfiRepr(), styles.intoFfiRepr()))
    }

    public func ui_tree_json(_ id: EditorId) -> RustString {
        RustString(ptr: __swift_bridge__$App$ui_tree_json(ptr, id.intoFfiRepr()))
    }

    public func message_snapshot_json(_ id: EditorId) -> RustString {
        RustString(ptr: __swift_bridge__$App$message_snapshot_json(ptr, id.intoFfiRepr()))
    }

    public func message_events_since_json(_ id: EditorId, _ seq: UInt64) -> RustString {
        RustString(ptr: __swift_bridge__$App$message_events_since_json(ptr, id.intoFfiRepr(), seq))
    }

    public func ui_event_json<GenericToRustStr: ToRustStr>(_ id: EditorId, _ event_json: GenericToRustStr) -> Bool {
        return event_json.toRustStr({ event_jsonAsRustStr in
            __swift_bridge__$App$ui_event_json(ptr, id.intoFfiRepr(), event_jsonAsRustStr)
        })
    }

    public func command_palette_is_open(_ id: EditorId) -> Bool {
        __swift_bridge__$App$command_palette_is_open(ptr, id.intoFfiRepr())
    }

    public func command_palette_query(_ id: EditorId) -> RustString {
        RustString(ptr: __swift_bridge__$App$command_palette_query(ptr, id.intoFfiRepr()))
    }

    public func command_palette_layout(_ id: EditorId) -> UInt8 {
        __swift_bridge__$App$command_palette_layout(ptr, id.intoFfiRepr())
    }

    public func command_palette_filtered_count(_ id: EditorId) -> UInt {
        __swift_bridge__$App$command_palette_filtered_count(ptr, id.intoFfiRepr())
    }

    public func command_palette_filtered_selected_index(_ id: EditorId) -> Int64 {
        __swift_bridge__$App$command_palette_filtered_selected_index(ptr, id.intoFfiRepr())
    }

    public func command_palette_filtered_title(_ id: EditorId, _ index: UInt) -> RustString {
        RustString(ptr: __swift_bridge__$App$command_palette_filtered_title(ptr, id.intoFfiRepr(), index))
    }

    public func command_palette_filtered_subtitle(_ id: EditorId, _ index: UInt) -> RustString {
        RustString(ptr: __swift_bridge__$App$command_palette_filtered_subtitle(ptr, id.intoFfiRepr(), index))
    }

    public func command_palette_filtered_description(_ id: EditorId, _ index: UInt) -> RustString {
        RustString(ptr: __swift_bridge__$App$command_palette_filtered_description(ptr, id.intoFfiRepr(), index))
    }

    public func command_palette_filtered_shortcut(_ id: EditorId, _ index: UInt) -> RustString {
        RustString(ptr: __swift_bridge__$App$command_palette_filtered_shortcut(ptr, id.intoFfiRepr(), index))
    }

    public func command_palette_filtered_badge(_ id: EditorId, _ index: UInt) -> RustString {
        RustString(ptr: __swift_bridge__$App$command_palette_filtered_badge(ptr, id.intoFfiRepr(), index))
    }

    public func command_palette_filtered_leading_icon(_ id: EditorId, _ index: UInt) -> RustString {
        RustString(ptr: __swift_bridge__$App$command_palette_filtered_leading_icon(ptr, id.intoFfiRepr(), index))
    }

    public func command_palette_filtered_leading_color(_ id: EditorId, _ index: UInt) -> Color {
        __swift_bridge__$App$command_palette_filtered_leading_color(ptr, id.intoFfiRepr(), index).intoSwiftRepr()
    }

    public func command_palette_filtered_symbol_count(_ id: EditorId, _ index: UInt) -> UInt {
        __swift_bridge__$App$command_palette_filtered_symbol_count(ptr, id.intoFfiRepr(), index)
    }

    public func command_palette_filtered_symbol(_ id: EditorId, _ index: UInt, _ symbol_index: UInt) -> RustString {
        RustString(ptr: __swift_bridge__$App$command_palette_filtered_symbol(ptr, id.intoFfiRepr(), index, symbol_index))
    }

    public func command_palette_select_filtered(_ id: EditorId, _ index: UInt) -> Bool {
        __swift_bridge__$App$command_palette_select_filtered(ptr, id.intoFfiRepr(), index)
    }

    public func command_palette_submit_filtered(_ id: EditorId, _ index: UInt) -> Bool {
        __swift_bridge__$App$command_palette_submit_filtered(ptr, id.intoFfiRepr(), index)
    }

    public func command_palette_close(_ id: EditorId) -> Bool {
        __swift_bridge__$App$command_palette_close(ptr, id.intoFfiRepr())
    }

    public func command_palette_set_query<GenericToRustStr: ToRustStr>(_ id: EditorId, _ query: GenericToRustStr) -> Bool {
        return query.toRustStr({ queryAsRustStr in
            __swift_bridge__$App$command_palette_set_query(ptr, id.intoFfiRepr(), queryAsRustStr)
        })
    }

    public func search_prompt_set_query<GenericToRustStr: ToRustStr>(_ id: EditorId, _ query: GenericToRustStr) -> Bool {
        return query.toRustStr({ queryAsRustStr in
            __swift_bridge__$App$search_prompt_set_query(ptr, id.intoFfiRepr(), queryAsRustStr)
        })
    }

    public func search_prompt_close(_ id: EditorId) -> Bool {
        __swift_bridge__$App$search_prompt_close(ptr, id.intoFfiRepr())
    }

    public func search_prompt_submit(_ id: EditorId) -> Bool {
        __swift_bridge__$App$search_prompt_submit(ptr, id.intoFfiRepr())
    }

    public func file_picker_set_query<GenericToRustStr: ToRustStr>(_ id: EditorId, _ query: GenericToRustStr) -> Bool {
        return query.toRustStr({ queryAsRustStr in
            __swift_bridge__$App$file_picker_set_query(ptr, id.intoFfiRepr(), queryAsRustStr)
        })
    }

    public func file_picker_submit(_ id: EditorId, _ index: UInt) -> Bool {
        __swift_bridge__$App$file_picker_submit(ptr, id.intoFfiRepr(), index)
    }

    public func file_picker_close(_ id: EditorId) -> Bool {
        __swift_bridge__$App$file_picker_close(ptr, id.intoFfiRepr())
    }

    public func file_picker_select_index(_ id: EditorId, _ index: UInt) -> Bool {
        __swift_bridge__$App$file_picker_select_index(ptr, id.intoFfiRepr(), index)
    }

    public func file_picker_snapshot(_ id: EditorId, _ max_items: UInt) -> FilePickerSnapshotData {
        FilePickerSnapshotData(ptr: __swift_bridge__$App$file_picker_snapshot(ptr, id.intoFfiRepr(), max_items))
    }

    public func file_picker_preview(_ id: EditorId) -> PreviewData {
        PreviewData(ptr: __swift_bridge__$App$file_picker_preview(ptr, id.intoFfiRepr()))
    }

    public func poll_background(_ id: EditorId) -> Bool {
        __swift_bridge__$App$poll_background(ptr, id.intoFfiRepr())
    }

    public func take_should_quit() -> Bool {
        __swift_bridge__$App$take_should_quit(ptr)
    }

    public func handle_key(_ id: EditorId, _ event: KeyEvent) -> Bool {
        __swift_bridge__$App$handle_key(ptr, id.intoFfiRepr(), event.intoFfiRepr())
    }

    public func ensure_cursor_visible(_ id: EditorId) -> Bool {
        __swift_bridge__$App$ensure_cursor_visible(ptr, id.intoFfiRepr())
    }

    public func insert<GenericToRustStr: ToRustStr>(_ id: EditorId, _ text: GenericToRustStr) -> Bool {
        return text.toRustStr({ textAsRustStr in
            __swift_bridge__$App$insert(ptr, id.intoFfiRepr(), textAsRustStr)
        })
    }

    public func delete_backward(_ id: EditorId) -> Bool {
        __swift_bridge__$App$delete_backward(ptr, id.intoFfiRepr())
    }

    public func delete_forward(_ id: EditorId) -> Bool {
        __swift_bridge__$App$delete_forward(ptr, id.intoFfiRepr())
    }

    public func move_left(_ id: EditorId) {
        __swift_bridge__$App$move_left(ptr, id.intoFfiRepr())
    }

    public func move_right(_ id: EditorId) {
        __swift_bridge__$App$move_right(ptr, id.intoFfiRepr())
    }

    public func move_up(_ id: EditorId) {
        __swift_bridge__$App$move_up(ptr, id.intoFfiRepr())
    }

    public func move_down(_ id: EditorId) {
        __swift_bridge__$App$move_down(ptr, id.intoFfiRepr())
    }

    public func add_cursor_above(_ id: EditorId) -> Bool {
        __swift_bridge__$App$add_cursor_above(ptr, id.intoFfiRepr())
    }

    public func add_cursor_below(_ id: EditorId) -> Bool {
        __swift_bridge__$App$add_cursor_below(ptr, id.intoFfiRepr())
    }

    public func collapse_to_cursor(_ id: EditorId, _ cursor_id: UInt64) -> Bool {
        __swift_bridge__$App$collapse_to_cursor(ptr, id.intoFfiRepr(), cursor_id)
    }

    public func collapse_to_first(_ id: EditorId) -> Bool {
        __swift_bridge__$App$collapse_to_first(ptr, id.intoFfiRepr())
    }
}
public class AppRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension AppRef {
    public func cursor_ids(_ id: EditorId) -> RustVec<UInt64> {
        RustVec(ptr: __swift_bridge__$App$cursor_ids(ptr, id.intoFfiRepr()))
    }

    public func text(_ id: EditorId) -> RustString {
        RustString(ptr: __swift_bridge__$App$text(ptr, id.intoFfiRepr()))
    }

    public func pending_keys_json(_ id: EditorId) -> RustString {
        RustString(ptr: __swift_bridge__$App$pending_keys_json(ptr, id.intoFfiRepr()))
    }

    public func pending_key_hints_json(_ id: EditorId) -> RustString {
        RustString(ptr: __swift_bridge__$App$pending_key_hints_json(ptr, id.intoFfiRepr()))
    }

    public func mode(_ id: EditorId) -> UInt8 {
        __swift_bridge__$App$mode(ptr, id.intoFfiRepr())
    }

    public func theme_highlight_style(_ highlight: UInt32) -> Style {
        __swift_bridge__$App$theme_highlight_style(ptr, highlight).intoSwiftRepr()
    }
}
extension App: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_App$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_App$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: App) {
        __swift_bridge__$Vec_App$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_App$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (App(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<AppRef> {
        let pointer = __swift_bridge__$Vec_App$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return AppRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<AppRefMut> {
        let pointer = __swift_bridge__$Vec_App$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return AppRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<AppRef> {
        UnsafePointer<AppRef>(OpaquePointer(__swift_bridge__$Vec_App$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_App$len(vecPtr)
    }
}


public class Document: DocumentRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$Document$_free(ptr)
        }
    }
}
extension Document {
    public convenience init() {
        self.init(ptr: __swift_bridge__$Document$new())
    }
}
extension Document {
    class public func from_text<GenericToRustStr: ToRustStr>(_ text: GenericToRustStr) -> Document {
        return text.toRustStr({ textAsRustStr in
            Document(ptr: __swift_bridge__$Document$from_text(textAsRustStr))
        })
    }
}
public class DocumentRefMut: DocumentRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
extension DocumentRefMut {
    public func insert<GenericToRustStr: ToRustStr>(_ text: GenericToRustStr) -> Bool {
        return text.toRustStr({ textAsRustStr in
            __swift_bridge__$Document$insert(ptr, textAsRustStr)
        })
    }

    public func delete_backward() -> Bool {
        __swift_bridge__$Document$delete_backward(ptr)
    }

    public func delete_forward() -> Bool {
        __swift_bridge__$Document$delete_forward(ptr)
    }

    public func move_left() {
        __swift_bridge__$Document$move_left(ptr)
    }

    public func move_right() {
        __swift_bridge__$Document$move_right(ptr)
    }

    public func move_up() {
        __swift_bridge__$Document$move_up(ptr)
    }

    public func move_down() {
        __swift_bridge__$Document$move_down(ptr)
    }

    public func add_cursor_above() -> Bool {
        __swift_bridge__$Document$add_cursor_above(ptr)
    }

    public func add_cursor_below() -> Bool {
        __swift_bridge__$Document$add_cursor_below(ptr)
    }

    public func collapse_to_first() {
        __swift_bridge__$Document$collapse_to_first(ptr)
    }

    public func commit() -> Bool {
        __swift_bridge__$Document$commit(ptr)
    }

    public func undo() -> Bool {
        __swift_bridge__$Document$undo(ptr)
    }

    public func redo() -> Bool {
        __swift_bridge__$Document$redo(ptr)
    }
}
public class DocumentRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension DocumentRef {
    public func text() -> RustString {
        RustString(ptr: __swift_bridge__$Document$text(ptr))
    }

    public func len_chars() -> UInt {
        __swift_bridge__$Document$len_chars(ptr)
    }

    public func len_lines() -> UInt {
        __swift_bridge__$Document$len_lines(ptr)
    }

    public func is_empty() -> Bool {
        __swift_bridge__$Document$is_empty(ptr)
    }

    public func version() -> UInt64 {
        __swift_bridge__$Document$version(ptr)
    }

    public func is_modified() -> Bool {
        __swift_bridge__$Document$is_modified(ptr)
    }

    public func first_cursor() -> UInt {
        __swift_bridge__$Document$first_cursor(ptr)
    }

    public func cursor_count() -> UInt {
        __swift_bridge__$Document$cursor_count(ptr)
    }

    public func all_cursors() -> RustVec<UInt> {
        RustVec(ptr: __swift_bridge__$Document$all_cursors(ptr))
    }

    public func char_to_line(_ char_idx: UInt) -> UInt {
        __swift_bridge__$Document$char_to_line(ptr, char_idx)
    }

    public func line_to_char(_ line_idx: UInt) -> UInt {
        __swift_bridge__$Document$line_to_char(ptr, line_idx)
    }
}
extension Document: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_Document$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_Document$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: Document) {
        __swift_bridge__$Vec_Document$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_Document$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (Document(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<DocumentRef> {
        let pointer = __swift_bridge__$Vec_Document$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return DocumentRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<DocumentRefMut> {
        let pointer = __swift_bridge__$Vec_Document$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return DocumentRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<DocumentRef> {
        UnsafePointer<DocumentRef>(OpaquePointer(__swift_bridge__$Vec_Document$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_Document$len(vecPtr)
    }
}

public struct KeyEvent {
    public var kind: UInt8
    public var codepoint: UInt32
    public var modifiers: UInt8

    public init(kind: UInt8,codepoint: UInt32,modifiers: UInt8) {
        self.kind = kind
        self.codepoint = codepoint
        self.modifiers = modifiers
    }

    @inline(__always)
    func intoFfiRepr() -> __swift_bridge__$KeyEvent {
        { let val = self; return __swift_bridge__$KeyEvent(kind: val.kind, codepoint: val.codepoint, modifiers: val.modifiers); }()
    }
}
extension __swift_bridge__$KeyEvent {
    @inline(__always)
    func intoSwiftRepr() -> KeyEvent {
        { let val = self; return KeyEvent(kind: val.kind, codepoint: val.codepoint, modifiers: val.modifiers); }()
    }
}
extension __swift_bridge__$Option$KeyEvent {
    @inline(__always)
    func intoSwiftRepr() -> Optional<KeyEvent> {
        if self.is_some {
            return self.val.intoSwiftRepr()
        } else {
            return nil
        }
    }

    @inline(__always)
    static func fromSwiftRepr(_ val: Optional<KeyEvent>) -> __swift_bridge__$Option$KeyEvent {
        if let v = val {
            return __swift_bridge__$Option$KeyEvent(is_some: true, val: v.intoFfiRepr())
        } else {
            return __swift_bridge__$Option$KeyEvent(is_some: false, val: __swift_bridge__$KeyEvent())
        }
    }
}
public struct EditorId {
    public var value: UInt64

    public init(value: UInt64) {
        self.value = value
    }

    @inline(__always)
    func intoFfiRepr() -> __swift_bridge__$EditorId {
        { let val = self; return __swift_bridge__$EditorId(value: val.value); }()
    }
}
extension __swift_bridge__$EditorId {
    @inline(__always)
    func intoSwiftRepr() -> EditorId {
        { let val = self; return EditorId(value: val.value); }()
    }
}
extension __swift_bridge__$Option$EditorId {
    @inline(__always)
    func intoSwiftRepr() -> Optional<EditorId> {
        if self.is_some {
            return self.val.intoSwiftRepr()
        } else {
            return nil
        }
    }

    @inline(__always)
    static func fromSwiftRepr(_ val: Optional<EditorId>) -> __swift_bridge__$Option$EditorId {
        if let v = val {
            return __swift_bridge__$Option$EditorId(is_some: true, val: v.intoFfiRepr())
        } else {
            return __swift_bridge__$Option$EditorId(is_some: false, val: __swift_bridge__$EditorId())
        }
    }
}
public struct Rect {
    public var x: UInt16
    public var y: UInt16
    public var width: UInt16
    public var height: UInt16

    public init(x: UInt16,y: UInt16,width: UInt16,height: UInt16) {
        self.x = x
        self.y = y
        self.width = width
        self.height = height
    }

    @inline(__always)
    func intoFfiRepr() -> __swift_bridge__$Rect {
        { let val = self; return __swift_bridge__$Rect(x: val.x, y: val.y, width: val.width, height: val.height); }()
    }
}
extension __swift_bridge__$Rect {
    @inline(__always)
    func intoSwiftRepr() -> Rect {
        { let val = self; return Rect(x: val.x, y: val.y, width: val.width, height: val.height); }()
    }
}
extension __swift_bridge__$Option$Rect {
    @inline(__always)
    func intoSwiftRepr() -> Optional<Rect> {
        if self.is_some {
            return self.val.intoSwiftRepr()
        } else {
            return nil
        }
    }

    @inline(__always)
    static func fromSwiftRepr(_ val: Optional<Rect>) -> __swift_bridge__$Option$Rect {
        if let v = val {
            return __swift_bridge__$Option$Rect(is_some: true, val: v.intoFfiRepr())
        } else {
            return __swift_bridge__$Option$Rect(is_some: false, val: __swift_bridge__$Rect())
        }
    }
}
public struct Position {
    public var row: UInt64
    public var col: UInt64

    public init(row: UInt64,col: UInt64) {
        self.row = row
        self.col = col
    }

    @inline(__always)
    func intoFfiRepr() -> __swift_bridge__$Position {
        { let val = self; return __swift_bridge__$Position(row: val.row, col: val.col); }()
    }
}
extension __swift_bridge__$Position {
    @inline(__always)
    func intoSwiftRepr() -> Position {
        { let val = self; return Position(row: val.row, col: val.col); }()
    }
}
extension __swift_bridge__$Option$Position {
    @inline(__always)
    func intoSwiftRepr() -> Optional<Position> {
        if self.is_some {
            return self.val.intoSwiftRepr()
        } else {
            return nil
        }
    }

    @inline(__always)
    static func fromSwiftRepr(_ val: Optional<Position>) -> __swift_bridge__$Option$Position {
        if let v = val {
            return __swift_bridge__$Option$Position(is_some: true, val: v.intoFfiRepr())
        } else {
            return __swift_bridge__$Option$Position(is_some: false, val: __swift_bridge__$Position())
        }
    }
}
public struct Color {
    public var kind: UInt8
    public var value: UInt32

    public init(kind: UInt8,value: UInt32) {
        self.kind = kind
        self.value = value
    }

    @inline(__always)
    func intoFfiRepr() -> __swift_bridge__$Color {
        { let val = self; return __swift_bridge__$Color(kind: val.kind, value: val.value); }()
    }
}
extension __swift_bridge__$Color {
    @inline(__always)
    func intoSwiftRepr() -> Color {
        { let val = self; return Color(kind: val.kind, value: val.value); }()
    }
}
extension __swift_bridge__$Option$Color {
    @inline(__always)
    func intoSwiftRepr() -> Optional<Color> {
        if self.is_some {
            return self.val.intoSwiftRepr()
        } else {
            return nil
        }
    }

    @inline(__always)
    static func fromSwiftRepr(_ val: Optional<Color>) -> __swift_bridge__$Option$Color {
        if let v = val {
            return __swift_bridge__$Option$Color(is_some: true, val: v.intoFfiRepr())
        } else {
            return __swift_bridge__$Option$Color(is_some: false, val: __swift_bridge__$Color())
        }
    }
}
public struct Style {
    public var has_fg: Bool
    public var fg: Color
    public var has_bg: Bool
    public var bg: Color
    public var has_underline_color: Bool
    public var underline_color: Color
    public var underline_style: UInt8
    public var add_modifier: UInt16
    public var sub_modifier: UInt16

    public init(has_fg: Bool,fg: Color,has_bg: Bool,bg: Color,has_underline_color: Bool,underline_color: Color,underline_style: UInt8,add_modifier: UInt16,sub_modifier: UInt16) {
        self.has_fg = has_fg
        self.fg = fg
        self.has_bg = has_bg
        self.bg = bg
        self.has_underline_color = has_underline_color
        self.underline_color = underline_color
        self.underline_style = underline_style
        self.add_modifier = add_modifier
        self.sub_modifier = sub_modifier
    }

    @inline(__always)
    func intoFfiRepr() -> __swift_bridge__$Style {
        { let val = self; return __swift_bridge__$Style(has_fg: val.has_fg, fg: val.fg.intoFfiRepr(), has_bg: val.has_bg, bg: val.bg.intoFfiRepr(), has_underline_color: val.has_underline_color, underline_color: val.underline_color.intoFfiRepr(), underline_style: val.underline_style, add_modifier: val.add_modifier, sub_modifier: val.sub_modifier); }()
    }
}
extension __swift_bridge__$Style {
    @inline(__always)
    func intoSwiftRepr() -> Style {
        { let val = self; return Style(has_fg: val.has_fg, fg: val.fg.intoSwiftRepr(), has_bg: val.has_bg, bg: val.bg.intoSwiftRepr(), has_underline_color: val.has_underline_color, underline_color: val.underline_color.intoSwiftRepr(), underline_style: val.underline_style, add_modifier: val.add_modifier, sub_modifier: val.sub_modifier); }()
    }
}
extension __swift_bridge__$Option$Style {
    @inline(__always)
    func intoSwiftRepr() -> Optional<Style> {
        if self.is_some {
            return self.val.intoSwiftRepr()
        } else {
            return nil
        }
    }

    @inline(__always)
    static func fromSwiftRepr(_ val: Optional<Style>) -> __swift_bridge__$Option$Style {
        if let v = val {
            return __swift_bridge__$Option$Style(is_some: true, val: v.intoFfiRepr())
        } else {
            return __swift_bridge__$Option$Style(is_some: false, val: __swift_bridge__$Style())
        }
    }
}
public struct RenderStyles {
    public var selection: Style
    public var cursor: Style
    public var active_cursor: Style
    public var gutter: Style
    public var gutter_active: Style

    public init(selection: Style,cursor: Style,active_cursor: Style,gutter: Style,gutter_active: Style) {
        self.selection = selection
        self.cursor = cursor
        self.active_cursor = active_cursor
        self.gutter = gutter
        self.gutter_active = gutter_active
    }

    @inline(__always)
    func intoFfiRepr() -> __swift_bridge__$RenderStyles {
        { let val = self; return __swift_bridge__$RenderStyles(selection: val.selection.intoFfiRepr(), cursor: val.cursor.intoFfiRepr(), active_cursor: val.active_cursor.intoFfiRepr(), gutter: val.gutter.intoFfiRepr(), gutter_active: val.gutter_active.intoFfiRepr()); }()
    }
}
extension __swift_bridge__$RenderStyles {
    @inline(__always)
    func intoSwiftRepr() -> RenderStyles {
        { let val = self; return RenderStyles(selection: val.selection.intoSwiftRepr(), cursor: val.cursor.intoSwiftRepr(), active_cursor: val.active_cursor.intoSwiftRepr(), gutter: val.gutter.intoSwiftRepr(), gutter_active: val.gutter_active.intoSwiftRepr()); }()
    }
}
extension __swift_bridge__$Option$RenderStyles {
    @inline(__always)
    func intoSwiftRepr() -> Optional<RenderStyles> {
        if self.is_some {
            return self.val.intoSwiftRepr()
        } else {
            return nil
        }
    }

    @inline(__always)
    static func fromSwiftRepr(_ val: Optional<RenderStyles>) -> __swift_bridge__$Option$RenderStyles {
        if let v = val {
            return __swift_bridge__$Option$RenderStyles(is_some: true, val: v.intoFfiRepr())
        } else {
            return __swift_bridge__$Option$RenderStyles(is_some: false, val: __swift_bridge__$RenderStyles())
        }
    }
}

public class RenderSpan: RenderSpanRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$RenderSpan$_free(ptr)
        }
    }
}
public class RenderSpanRefMut: RenderSpanRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class RenderSpanRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension RenderSpanRef {
    public func col() -> UInt16 {
        __swift_bridge__$RenderSpan$col(ptr)
    }

    public func cols() -> UInt16 {
        __swift_bridge__$RenderSpan$cols(ptr)
    }

    public func text() -> RustString {
        RustString(ptr: __swift_bridge__$RenderSpan$text(ptr))
    }

    public func has_highlight() -> Bool {
        __swift_bridge__$RenderSpan$has_highlight(ptr)
    }

    public func highlight() -> UInt32 {
        __swift_bridge__$RenderSpan$highlight(ptr)
    }

    public func is_virtual() -> Bool {
        __swift_bridge__$RenderSpan$is_virtual(ptr)
    }
}
extension RenderSpan: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_RenderSpan$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_RenderSpan$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: RenderSpan) {
        __swift_bridge__$Vec_RenderSpan$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_RenderSpan$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (RenderSpan(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderSpanRef> {
        let pointer = __swift_bridge__$Vec_RenderSpan$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderSpanRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderSpanRefMut> {
        let pointer = __swift_bridge__$Vec_RenderSpan$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderSpanRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<RenderSpanRef> {
        UnsafePointer<RenderSpanRef>(OpaquePointer(__swift_bridge__$Vec_RenderSpan$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_RenderSpan$len(vecPtr)
    }
}


public class RenderLine: RenderLineRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$RenderLine$_free(ptr)
        }
    }
}
public class RenderLineRefMut: RenderLineRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class RenderLineRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension RenderLineRef {
    public func row() -> UInt16 {
        __swift_bridge__$RenderLine$row(ptr)
    }

    public func span_count() -> UInt {
        __swift_bridge__$RenderLine$span_count(ptr)
    }

    public func span_at(_ index: UInt) -> RenderSpan {
        RenderSpan(ptr: __swift_bridge__$RenderLine$span_at(ptr, index))
    }
}
extension RenderLine: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_RenderLine$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_RenderLine$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: RenderLine) {
        __swift_bridge__$Vec_RenderLine$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_RenderLine$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (RenderLine(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderLineRef> {
        let pointer = __swift_bridge__$Vec_RenderLine$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderLineRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderLineRefMut> {
        let pointer = __swift_bridge__$Vec_RenderLine$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderLineRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<RenderLineRef> {
        UnsafePointer<RenderLineRef>(OpaquePointer(__swift_bridge__$Vec_RenderLine$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_RenderLine$len(vecPtr)
    }
}


public class RenderGutterSpan: RenderGutterSpanRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$RenderGutterSpan$_free(ptr)
        }
    }
}
public class RenderGutterSpanRefMut: RenderGutterSpanRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class RenderGutterSpanRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension RenderGutterSpanRef {
    public func col() -> UInt16 {
        __swift_bridge__$RenderGutterSpan$col(ptr)
    }

    public func text() -> RustString {
        RustString(ptr: __swift_bridge__$RenderGutterSpan$text(ptr))
    }

    public func style() -> Style {
        __swift_bridge__$RenderGutterSpan$style(ptr).intoSwiftRepr()
    }
}
extension RenderGutterSpan: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_RenderGutterSpan$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_RenderGutterSpan$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: RenderGutterSpan) {
        __swift_bridge__$Vec_RenderGutterSpan$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_RenderGutterSpan$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (RenderGutterSpan(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderGutterSpanRef> {
        let pointer = __swift_bridge__$Vec_RenderGutterSpan$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderGutterSpanRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderGutterSpanRefMut> {
        let pointer = __swift_bridge__$Vec_RenderGutterSpan$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderGutterSpanRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<RenderGutterSpanRef> {
        UnsafePointer<RenderGutterSpanRef>(OpaquePointer(__swift_bridge__$Vec_RenderGutterSpan$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_RenderGutterSpan$len(vecPtr)
    }
}


public class RenderGutterLine: RenderGutterLineRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$RenderGutterLine$_free(ptr)
        }
    }
}
public class RenderGutterLineRefMut: RenderGutterLineRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class RenderGutterLineRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension RenderGutterLineRef {
    public func row() -> UInt16 {
        __swift_bridge__$RenderGutterLine$row(ptr)
    }

    public func span_count() -> UInt {
        __swift_bridge__$RenderGutterLine$span_count(ptr)
    }

    public func span_at(_ index: UInt) -> RenderGutterSpan {
        RenderGutterSpan(ptr: __swift_bridge__$RenderGutterLine$span_at(ptr, index))
    }
}
extension RenderGutterLine: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_RenderGutterLine$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_RenderGutterLine$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: RenderGutterLine) {
        __swift_bridge__$Vec_RenderGutterLine$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_RenderGutterLine$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (RenderGutterLine(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderGutterLineRef> {
        let pointer = __swift_bridge__$Vec_RenderGutterLine$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderGutterLineRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderGutterLineRefMut> {
        let pointer = __swift_bridge__$Vec_RenderGutterLine$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderGutterLineRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<RenderGutterLineRef> {
        UnsafePointer<RenderGutterLineRef>(OpaquePointer(__swift_bridge__$Vec_RenderGutterLine$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_RenderGutterLine$len(vecPtr)
    }
}


public class RenderCursor: RenderCursorRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$RenderCursor$_free(ptr)
        }
    }
}
public class RenderCursorRefMut: RenderCursorRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class RenderCursorRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension RenderCursorRef {
    public func id() -> UInt64 {
        __swift_bridge__$RenderCursor$id(ptr)
    }

    public func pos() -> Position {
        __swift_bridge__$RenderCursor$pos(ptr).intoSwiftRepr()
    }

    public func kind() -> UInt8 {
        __swift_bridge__$RenderCursor$kind(ptr)
    }

    public func style() -> Style {
        __swift_bridge__$RenderCursor$style(ptr).intoSwiftRepr()
    }
}
extension RenderCursor: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_RenderCursor$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_RenderCursor$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: RenderCursor) {
        __swift_bridge__$Vec_RenderCursor$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_RenderCursor$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (RenderCursor(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderCursorRef> {
        let pointer = __swift_bridge__$Vec_RenderCursor$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderCursorRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderCursorRefMut> {
        let pointer = __swift_bridge__$Vec_RenderCursor$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderCursorRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<RenderCursorRef> {
        UnsafePointer<RenderCursorRef>(OpaquePointer(__swift_bridge__$Vec_RenderCursor$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_RenderCursor$len(vecPtr)
    }
}


public class RenderSelection: RenderSelectionRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$RenderSelection$_free(ptr)
        }
    }
}
public class RenderSelectionRefMut: RenderSelectionRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class RenderSelectionRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension RenderSelectionRef {
    public func rect() -> Rect {
        __swift_bridge__$RenderSelection$rect(ptr).intoSwiftRepr()
    }

    public func style() -> Style {
        __swift_bridge__$RenderSelection$style(ptr).intoSwiftRepr()
    }
}
extension RenderSelection: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_RenderSelection$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_RenderSelection$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: RenderSelection) {
        __swift_bridge__$Vec_RenderSelection$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_RenderSelection$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (RenderSelection(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderSelectionRef> {
        let pointer = __swift_bridge__$Vec_RenderSelection$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderSelectionRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderSelectionRefMut> {
        let pointer = __swift_bridge__$Vec_RenderSelection$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderSelectionRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<RenderSelectionRef> {
        UnsafePointer<RenderSelectionRef>(OpaquePointer(__swift_bridge__$Vec_RenderSelection$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_RenderSelection$len(vecPtr)
    }
}


public class RenderOverlayNode: RenderOverlayNodeRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$RenderOverlayNode$_free(ptr)
        }
    }
}
public class RenderOverlayNodeRefMut: RenderOverlayNodeRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class RenderOverlayNodeRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension RenderOverlayNodeRef {
    public func kind() -> UInt8 {
        __swift_bridge__$RenderOverlayNode$kind(ptr)
    }

    public func rect_kind() -> UInt8 {
        __swift_bridge__$RenderOverlayNode$rect_kind(ptr)
    }

    public func rect() -> Rect {
        __swift_bridge__$RenderOverlayNode$rect(ptr).intoSwiftRepr()
    }

    public func radius() -> UInt16 {
        __swift_bridge__$RenderOverlayNode$radius(ptr)
    }

    public func pos() -> Position {
        __swift_bridge__$RenderOverlayNode$pos(ptr).intoSwiftRepr()
    }

    public func text() -> RustString {
        RustString(ptr: __swift_bridge__$RenderOverlayNode$text(ptr))
    }

    public func style() -> Style {
        __swift_bridge__$RenderOverlayNode$style(ptr).intoSwiftRepr()
    }
}
extension RenderOverlayNode: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_RenderOverlayNode$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_RenderOverlayNode$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: RenderOverlayNode) {
        __swift_bridge__$Vec_RenderOverlayNode$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_RenderOverlayNode$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (RenderOverlayNode(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderOverlayNodeRef> {
        let pointer = __swift_bridge__$Vec_RenderOverlayNode$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderOverlayNodeRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderOverlayNodeRefMut> {
        let pointer = __swift_bridge__$Vec_RenderOverlayNode$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderOverlayNodeRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<RenderOverlayNodeRef> {
        UnsafePointer<RenderOverlayNodeRef>(OpaquePointer(__swift_bridge__$Vec_RenderOverlayNode$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_RenderOverlayNode$len(vecPtr)
    }
}


public class RenderDiagnosticUnderline: RenderDiagnosticUnderlineRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$RenderDiagnosticUnderline$_free(ptr)
        }
    }
}
public class RenderDiagnosticUnderlineRefMut: RenderDiagnosticUnderlineRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class RenderDiagnosticUnderlineRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension RenderDiagnosticUnderlineRef {
    public func row() -> UInt16 {
        __swift_bridge__$RenderDiagnosticUnderline$row(ptr)
    }

    public func start_col() -> UInt16 {
        __swift_bridge__$RenderDiagnosticUnderline$start_col(ptr)
    }

    public func end_col() -> UInt16 {
        __swift_bridge__$RenderDiagnosticUnderline$end_col(ptr)
    }

    public func severity() -> UInt8 {
        __swift_bridge__$RenderDiagnosticUnderline$severity(ptr)
    }
}
extension RenderDiagnosticUnderline: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_RenderDiagnosticUnderline$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_RenderDiagnosticUnderline$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: RenderDiagnosticUnderline) {
        __swift_bridge__$Vec_RenderDiagnosticUnderline$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_RenderDiagnosticUnderline$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (RenderDiagnosticUnderline(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderDiagnosticUnderlineRef> {
        let pointer = __swift_bridge__$Vec_RenderDiagnosticUnderline$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderDiagnosticUnderlineRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderDiagnosticUnderlineRefMut> {
        let pointer = __swift_bridge__$Vec_RenderDiagnosticUnderline$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderDiagnosticUnderlineRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<RenderDiagnosticUnderlineRef> {
        UnsafePointer<RenderDiagnosticUnderlineRef>(OpaquePointer(__swift_bridge__$Vec_RenderDiagnosticUnderline$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_RenderDiagnosticUnderline$len(vecPtr)
    }
}


public class RenderEolDiagnostic: RenderEolDiagnosticRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$RenderEolDiagnostic$_free(ptr)
        }
    }
}
public class RenderEolDiagnosticRefMut: RenderEolDiagnosticRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class RenderEolDiagnosticRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension RenderEolDiagnosticRef {
    public func row() -> UInt16 {
        __swift_bridge__$RenderEolDiagnostic$row(ptr)
    }

    public func col() -> UInt16 {
        __swift_bridge__$RenderEolDiagnostic$col(ptr)
    }

    public func message() -> RustString {
        RustString(ptr: __swift_bridge__$RenderEolDiagnostic$message(ptr))
    }

    public func severity() -> UInt8 {
        __swift_bridge__$RenderEolDiagnostic$severity(ptr)
    }
}
extension RenderEolDiagnostic: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_RenderEolDiagnostic$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_RenderEolDiagnostic$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: RenderEolDiagnostic) {
        __swift_bridge__$Vec_RenderEolDiagnostic$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_RenderEolDiagnostic$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (RenderEolDiagnostic(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderEolDiagnosticRef> {
        let pointer = __swift_bridge__$Vec_RenderEolDiagnostic$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderEolDiagnosticRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderEolDiagnosticRefMut> {
        let pointer = __swift_bridge__$Vec_RenderEolDiagnostic$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderEolDiagnosticRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<RenderEolDiagnosticRef> {
        UnsafePointer<RenderEolDiagnosticRef>(OpaquePointer(__swift_bridge__$Vec_RenderEolDiagnostic$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_RenderEolDiagnostic$len(vecPtr)
    }
}


public class RenderInlineDiagnosticLine: RenderInlineDiagnosticLineRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$RenderInlineDiagnosticLine$_free(ptr)
        }
    }
}
public class RenderInlineDiagnosticLineRefMut: RenderInlineDiagnosticLineRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class RenderInlineDiagnosticLineRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension RenderInlineDiagnosticLineRef {
    public func row() -> UInt16 {
        __swift_bridge__$RenderInlineDiagnosticLine$row(ptr)
    }

    public func col() -> UInt16 {
        __swift_bridge__$RenderInlineDiagnosticLine$col(ptr)
    }

    public func text() -> RustString {
        RustString(ptr: __swift_bridge__$RenderInlineDiagnosticLine$text(ptr))
    }

    public func severity() -> UInt8 {
        __swift_bridge__$RenderInlineDiagnosticLine$severity(ptr)
    }
}
extension RenderInlineDiagnosticLine: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_RenderInlineDiagnosticLine$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_RenderInlineDiagnosticLine$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: RenderInlineDiagnosticLine) {
        __swift_bridge__$Vec_RenderInlineDiagnosticLine$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_RenderInlineDiagnosticLine$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (RenderInlineDiagnosticLine(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderInlineDiagnosticLineRef> {
        let pointer = __swift_bridge__$Vec_RenderInlineDiagnosticLine$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderInlineDiagnosticLineRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderInlineDiagnosticLineRefMut> {
        let pointer = __swift_bridge__$Vec_RenderInlineDiagnosticLine$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderInlineDiagnosticLineRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<RenderInlineDiagnosticLineRef> {
        UnsafePointer<RenderInlineDiagnosticLineRef>(OpaquePointer(__swift_bridge__$Vec_RenderInlineDiagnosticLine$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_RenderInlineDiagnosticLine$len(vecPtr)
    }
}


public class RenderPlan: RenderPlanRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$RenderPlan$_free(ptr)
        }
    }
}
public class RenderPlanRefMut: RenderPlanRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class RenderPlanRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension RenderPlanRef {
    public func viewport() -> Rect {
        __swift_bridge__$RenderPlan$viewport(ptr).intoSwiftRepr()
    }

    public func scroll() -> Position {
        __swift_bridge__$RenderPlan$scroll(ptr).intoSwiftRepr()
    }

    public func content_offset_x() -> UInt16 {
        __swift_bridge__$RenderPlan$content_offset_x(ptr)
    }

    public func gutter_line_count() -> UInt {
        __swift_bridge__$RenderPlan$gutter_line_count(ptr)
    }

    public func gutter_line_at(_ index: UInt) -> RenderGutterLine {
        RenderGutterLine(ptr: __swift_bridge__$RenderPlan$gutter_line_at(ptr, index))
    }

    public func line_count() -> UInt {
        __swift_bridge__$RenderPlan$line_count(ptr)
    }

    public func line_at(_ index: UInt) -> RenderLine {
        RenderLine(ptr: __swift_bridge__$RenderPlan$line_at(ptr, index))
    }

    public func cursor_count() -> UInt {
        __swift_bridge__$RenderPlan$cursor_count(ptr)
    }

    public func cursor_at(_ index: UInt) -> RenderCursor {
        RenderCursor(ptr: __swift_bridge__$RenderPlan$cursor_at(ptr, index))
    }

    public func selection_count() -> UInt {
        __swift_bridge__$RenderPlan$selection_count(ptr)
    }

    public func selection_at(_ index: UInt) -> RenderSelection {
        RenderSelection(ptr: __swift_bridge__$RenderPlan$selection_at(ptr, index))
    }

    public func overlay_count() -> UInt {
        __swift_bridge__$RenderPlan$overlay_count(ptr)
    }

    public func overlay_at(_ index: UInt) -> RenderOverlayNode {
        RenderOverlayNode(ptr: __swift_bridge__$RenderPlan$overlay_at(ptr, index))
    }

    public func inline_diagnostic_line_count() -> UInt {
        __swift_bridge__$RenderPlan$inline_diagnostic_line_count(ptr)
    }

    public func inline_diagnostic_line_at(_ index: UInt) -> RenderInlineDiagnosticLine {
        RenderInlineDiagnosticLine(ptr: __swift_bridge__$RenderPlan$inline_diagnostic_line_at(ptr, index))
    }

    public func eol_diagnostic_count() -> UInt {
        __swift_bridge__$RenderPlan$eol_diagnostic_count(ptr)
    }

    public func eol_diagnostic_at(_ index: UInt) -> RenderEolDiagnostic {
        RenderEolDiagnostic(ptr: __swift_bridge__$RenderPlan$eol_diagnostic_at(ptr, index))
    }

    public func diagnostic_underline_count() -> UInt {
        __swift_bridge__$RenderPlan$diagnostic_underline_count(ptr)
    }

    public func diagnostic_underline_at(_ index: UInt) -> RenderDiagnosticUnderline {
        RenderDiagnosticUnderline(ptr: __swift_bridge__$RenderPlan$diagnostic_underline_at(ptr, index))
    }
}
extension RenderPlan: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_RenderPlan$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_RenderPlan$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: RenderPlan) {
        __swift_bridge__$Vec_RenderPlan$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_RenderPlan$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (RenderPlan(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderPlanRef> {
        let pointer = __swift_bridge__$Vec_RenderPlan$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderPlanRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderPlanRefMut> {
        let pointer = __swift_bridge__$Vec_RenderPlan$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderPlanRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<RenderPlanRef> {
        UnsafePointer<RenderPlanRef>(OpaquePointer(__swift_bridge__$Vec_RenderPlan$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_RenderPlan$len(vecPtr)
    }
}


public class RenderFramePane: RenderFramePaneRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$RenderFramePane$_free(ptr)
        }
    }
}
public class RenderFramePaneRefMut: RenderFramePaneRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class RenderFramePaneRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension RenderFramePaneRef {
    public func pane_id() -> UInt64 {
        __swift_bridge__$RenderFramePane$pane_id(ptr)
    }

    public func rect() -> Rect {
        __swift_bridge__$RenderFramePane$rect(ptr).intoSwiftRepr()
    }

    public func is_active() -> Bool {
        __swift_bridge__$RenderFramePane$is_active(ptr)
    }

    public func plan() -> RenderPlan {
        RenderPlan(ptr: __swift_bridge__$RenderFramePane$plan(ptr))
    }
}
extension RenderFramePane: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_RenderFramePane$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_RenderFramePane$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: RenderFramePane) {
        __swift_bridge__$Vec_RenderFramePane$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_RenderFramePane$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (RenderFramePane(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderFramePaneRef> {
        let pointer = __swift_bridge__$Vec_RenderFramePane$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderFramePaneRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderFramePaneRefMut> {
        let pointer = __swift_bridge__$Vec_RenderFramePane$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderFramePaneRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<RenderFramePaneRef> {
        UnsafePointer<RenderFramePaneRef>(OpaquePointer(__swift_bridge__$Vec_RenderFramePane$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_RenderFramePane$len(vecPtr)
    }
}


public class RenderFramePlan: RenderFramePlanRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$RenderFramePlan$_free(ptr)
        }
    }
}
public class RenderFramePlanRefMut: RenderFramePlanRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class RenderFramePlanRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension RenderFramePlanRef {
    public func active_pane_id() -> UInt64 {
        __swift_bridge__$RenderFramePlan$active_pane_id(ptr)
    }

    public func pane_count() -> UInt {
        __swift_bridge__$RenderFramePlan$pane_count(ptr)
    }

    public func pane_at(_ index: UInt) -> RenderFramePane {
        RenderFramePane(ptr: __swift_bridge__$RenderFramePlan$pane_at(ptr, index))
    }

    public func active_plan() -> RenderPlan {
        RenderPlan(ptr: __swift_bridge__$RenderFramePlan$active_plan(ptr))
    }
}
extension RenderFramePlan: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_RenderFramePlan$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_RenderFramePlan$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: RenderFramePlan) {
        __swift_bridge__$Vec_RenderFramePlan$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_RenderFramePlan$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (RenderFramePlan(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderFramePlanRef> {
        let pointer = __swift_bridge__$Vec_RenderFramePlan$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderFramePlanRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<RenderFramePlanRefMut> {
        let pointer = __swift_bridge__$Vec_RenderFramePlan$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return RenderFramePlanRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<RenderFramePlanRef> {
        UnsafePointer<RenderFramePlanRef>(OpaquePointer(__swift_bridge__$Vec_RenderFramePlan$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_RenderFramePlan$len(vecPtr)
    }
}


public class SplitSeparator: SplitSeparatorRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$SplitSeparator$_free(ptr)
        }
    }
}
public class SplitSeparatorRefMut: SplitSeparatorRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class SplitSeparatorRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension SplitSeparatorRef {
    public func split_id() -> UInt64 {
        __swift_bridge__$SplitSeparator$split_id(ptr)
    }

    public func axis() -> UInt8 {
        __swift_bridge__$SplitSeparator$axis(ptr)
    }

    public func line() -> UInt16 {
        __swift_bridge__$SplitSeparator$line(ptr)
    }

    public func span_start() -> UInt16 {
        __swift_bridge__$SplitSeparator$span_start(ptr)
    }

    public func span_end() -> UInt16 {
        __swift_bridge__$SplitSeparator$span_end(ptr)
    }
}
extension SplitSeparator: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_SplitSeparator$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_SplitSeparator$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: SplitSeparator) {
        __swift_bridge__$Vec_SplitSeparator$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_SplitSeparator$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (SplitSeparator(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<SplitSeparatorRef> {
        let pointer = __swift_bridge__$Vec_SplitSeparator$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return SplitSeparatorRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<SplitSeparatorRefMut> {
        let pointer = __swift_bridge__$Vec_SplitSeparator$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return SplitSeparatorRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<SplitSeparatorRef> {
        UnsafePointer<SplitSeparatorRef>(OpaquePointer(__swift_bridge__$Vec_SplitSeparator$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_SplitSeparator$len(vecPtr)
    }
}


public class FilePickerSnapshotData: FilePickerSnapshotDataRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$FilePickerSnapshotData$_free(ptr)
        }
    }
}
public class FilePickerSnapshotDataRefMut: FilePickerSnapshotDataRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class FilePickerSnapshotDataRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension FilePickerSnapshotDataRef {
    public func active() -> Bool {
        __swift_bridge__$FilePickerSnapshotData$active(ptr)
    }

    public func query() -> RustString {
        RustString(ptr: __swift_bridge__$FilePickerSnapshotData$query(ptr))
    }

    public func matched_count() -> UInt {
        __swift_bridge__$FilePickerSnapshotData$matched_count(ptr)
    }

    public func total_count() -> UInt {
        __swift_bridge__$FilePickerSnapshotData$total_count(ptr)
    }

    public func scanning() -> Bool {
        __swift_bridge__$FilePickerSnapshotData$scanning(ptr)
    }

    public func root() -> RustString {
        RustString(ptr: __swift_bridge__$FilePickerSnapshotData$root(ptr))
    }

    public func item_count() -> UInt {
        __swift_bridge__$FilePickerSnapshotData$item_count(ptr)
    }

    public func item_at(_ index: UInt) -> FilePickerItemFFI {
        FilePickerItemFFI(ptr: __swift_bridge__$FilePickerSnapshotData$item_at(ptr, index))
    }
}
extension FilePickerSnapshotData: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_FilePickerSnapshotData$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_FilePickerSnapshotData$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: FilePickerSnapshotData) {
        __swift_bridge__$Vec_FilePickerSnapshotData$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_FilePickerSnapshotData$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (FilePickerSnapshotData(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<FilePickerSnapshotDataRef> {
        let pointer = __swift_bridge__$Vec_FilePickerSnapshotData$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return FilePickerSnapshotDataRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<FilePickerSnapshotDataRefMut> {
        let pointer = __swift_bridge__$Vec_FilePickerSnapshotData$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return FilePickerSnapshotDataRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<FilePickerSnapshotDataRef> {
        UnsafePointer<FilePickerSnapshotDataRef>(OpaquePointer(__swift_bridge__$Vec_FilePickerSnapshotData$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_FilePickerSnapshotData$len(vecPtr)
    }
}


public class FilePickerItemFFI: FilePickerItemFFIRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$FilePickerItemFFI$_free(ptr)
        }
    }
}
public class FilePickerItemFFIRefMut: FilePickerItemFFIRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class FilePickerItemFFIRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension FilePickerItemFFIRef {
    public func display() -> RustString {
        RustString(ptr: __swift_bridge__$FilePickerItemFFI$display(ptr))
    }

    public func is_dir() -> Bool {
        __swift_bridge__$FilePickerItemFFI$is_dir(ptr)
    }

    public func icon() -> RustString {
        RustString(ptr: __swift_bridge__$FilePickerItemFFI$icon(ptr))
    }

    public func match_index_count() -> UInt {
        __swift_bridge__$FilePickerItemFFI$match_index_count(ptr)
    }

    public func match_index_at(_ index: UInt) -> UInt32 {
        __swift_bridge__$FilePickerItemFFI$match_index_at(ptr, index)
    }
}
extension FilePickerItemFFI: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_FilePickerItemFFI$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_FilePickerItemFFI$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: FilePickerItemFFI) {
        __swift_bridge__$Vec_FilePickerItemFFI$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_FilePickerItemFFI$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (FilePickerItemFFI(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<FilePickerItemFFIRef> {
        let pointer = __swift_bridge__$Vec_FilePickerItemFFI$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return FilePickerItemFFIRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<FilePickerItemFFIRefMut> {
        let pointer = __swift_bridge__$Vec_FilePickerItemFFI$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return FilePickerItemFFIRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<FilePickerItemFFIRef> {
        UnsafePointer<FilePickerItemFFIRef>(OpaquePointer(__swift_bridge__$Vec_FilePickerItemFFI$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_FilePickerItemFFI$len(vecPtr)
    }
}


public class PreviewData: PreviewDataRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$PreviewData$_free(ptr)
        }
    }
}
public class PreviewDataRefMut: PreviewDataRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class PreviewDataRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension PreviewDataRef {
    public func kind() -> UInt8 {
        __swift_bridge__$PreviewData$kind(ptr)
    }

    public func path() -> RustString {
        RustString(ptr: __swift_bridge__$PreviewData$path(ptr))
    }

    public func text() -> RustString {
        RustString(ptr: __swift_bridge__$PreviewData$text(ptr))
    }

    public func loading() -> Bool {
        __swift_bridge__$PreviewData$loading(ptr)
    }

    public func truncated() -> Bool {
        __swift_bridge__$PreviewData$truncated(ptr)
    }

    public func total_lines() -> UInt {
        __swift_bridge__$PreviewData$total_lines(ptr)
    }

    public func show() -> Bool {
        __swift_bridge__$PreviewData$show(ptr)
    }

    public func line_count() -> UInt {
        __swift_bridge__$PreviewData$line_count(ptr)
    }

    public func line_at(_ index: UInt) -> PreviewLine {
        PreviewLine(ptr: __swift_bridge__$PreviewData$line_at(ptr, index))
    }
}
extension PreviewData: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_PreviewData$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_PreviewData$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: PreviewData) {
        __swift_bridge__$Vec_PreviewData$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_PreviewData$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (PreviewData(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<PreviewDataRef> {
        let pointer = __swift_bridge__$Vec_PreviewData$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return PreviewDataRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<PreviewDataRefMut> {
        let pointer = __swift_bridge__$Vec_PreviewData$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return PreviewDataRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<PreviewDataRef> {
        UnsafePointer<PreviewDataRef>(OpaquePointer(__swift_bridge__$Vec_PreviewData$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_PreviewData$len(vecPtr)
    }
}


public class PreviewLine: PreviewLineRefMut {
    var isOwned: Bool = true

    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }

    deinit {
        if isOwned {
            __swift_bridge__$PreviewLine$_free(ptr)
        }
    }
}
public class PreviewLineRefMut: PreviewLineRef {
    public override init(ptr: UnsafeMutableRawPointer) {
        super.init(ptr: ptr)
    }
}
public class PreviewLineRef {
    var ptr: UnsafeMutableRawPointer

    public init(ptr: UnsafeMutableRawPointer) {
        self.ptr = ptr
    }
}
extension PreviewLineRef {
    public func text() -> RustString {
        RustString(ptr: __swift_bridge__$PreviewLine$text(ptr))
    }

    public func span_count() -> UInt {
        __swift_bridge__$PreviewLine$span_count(ptr)
    }

    public func span_char_start(_ index: UInt) -> UInt32 {
        __swift_bridge__$PreviewLine$span_char_start(ptr, index)
    }

    public func span_char_end(_ index: UInt) -> UInt32 {
        __swift_bridge__$PreviewLine$span_char_end(ptr, index)
    }

    public func span_highlight(_ index: UInt) -> UInt32 {
        __swift_bridge__$PreviewLine$span_highlight(ptr, index)
    }
}
extension PreviewLine: Vectorizable {
    public static func vecOfSelfNew() -> UnsafeMutableRawPointer {
        __swift_bridge__$Vec_PreviewLine$new()
    }

    public static func vecOfSelfFree(vecPtr: UnsafeMutableRawPointer) {
        __swift_bridge__$Vec_PreviewLine$drop(vecPtr)
    }

    public static func vecOfSelfPush(vecPtr: UnsafeMutableRawPointer, value: PreviewLine) {
        __swift_bridge__$Vec_PreviewLine$push(vecPtr, {value.isOwned = false; return value.ptr;}())
    }

    public static func vecOfSelfPop(vecPtr: UnsafeMutableRawPointer) -> Optional<Self> {
        let pointer = __swift_bridge__$Vec_PreviewLine$pop(vecPtr)
        if pointer == nil {
            return nil
        } else {
            return (PreviewLine(ptr: pointer!) as! Self)
        }
    }

    public static func vecOfSelfGet(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<PreviewLineRef> {
        let pointer = __swift_bridge__$Vec_PreviewLine$get(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return PreviewLineRef(ptr: pointer!)
        }
    }

    public static func vecOfSelfGetMut(vecPtr: UnsafeMutableRawPointer, index: UInt) -> Optional<PreviewLineRefMut> {
        let pointer = __swift_bridge__$Vec_PreviewLine$get_mut(vecPtr, index)
        if pointer == nil {
            return nil
        } else {
            return PreviewLineRefMut(ptr: pointer!)
        }
    }

    public static func vecOfSelfAsPtr(vecPtr: UnsafeMutableRawPointer) -> UnsafePointer<PreviewLineRef> {
        UnsafePointer<PreviewLineRef>(OpaquePointer(__swift_bridge__$Vec_PreviewLine$as_ptr(vecPtr)))
    }

    public static func vecOfSelfLen(vecPtr: UnsafeMutableRawPointer) -> UInt {
        __swift_bridge__$Vec_PreviewLine$len(vecPtr)
    }
}



