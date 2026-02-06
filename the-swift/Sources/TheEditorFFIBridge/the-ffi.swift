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

    public func render_plan(_ id: EditorId) -> RenderPlan {
        RenderPlan(ptr: __swift_bridge__$App$render_plan(ptr, id.intoFfiRepr()))
    }

    public func render_plan_with_styles(_ id: EditorId, _ styles: RenderStyles) -> RenderPlan {
        RenderPlan(ptr: __swift_bridge__$App$render_plan_with_styles(ptr, id.intoFfiRepr(), styles.intoFfiRepr()))
    }

    public func ui_tree_json(_ id: EditorId) -> RustString {
        RustString(ptr: __swift_bridge__$App$ui_tree_json(ptr, id.intoFfiRepr()))
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

    public func file_picker_snapshot_json(_ id: EditorId, _ max_items: UInt) -> RustString {
        RustString(ptr: __swift_bridge__$App$file_picker_snapshot_json(ptr, id.intoFfiRepr(), max_items))
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

    public func mode(_ id: EditorId) -> UInt8 {
        __swift_bridge__$App$mode(ptr, id.intoFfiRepr())
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

    public func collapse_to_primary() {
        __swift_bridge__$Document$collapse_to_primary(ptr)
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

    public func primary_cursor() -> UInt {
        __swift_bridge__$Document$primary_cursor(ptr)
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

    public init(selection: Style,cursor: Style,active_cursor: Style) {
        self.selection = selection
        self.cursor = cursor
        self.active_cursor = active_cursor
    }

    @inline(__always)
    func intoFfiRepr() -> __swift_bridge__$RenderStyles {
        { let val = self; return __swift_bridge__$RenderStyles(selection: val.selection.intoFfiRepr(), cursor: val.cursor.intoFfiRepr(), active_cursor: val.active_cursor.intoFfiRepr()); }()
    }
}
extension __swift_bridge__$RenderStyles {
    @inline(__always)
    func intoSwiftRepr() -> RenderStyles {
        { let val = self; return RenderStyles(selection: val.selection.intoSwiftRepr(), cursor: val.cursor.intoSwiftRepr(), active_cursor: val.active_cursor.intoSwiftRepr()); }()
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



