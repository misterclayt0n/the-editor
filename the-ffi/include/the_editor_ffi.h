#ifndef THE_EDITOR_FFI_H
#define THE_EDITOR_FFI_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct the_editor_handle_t the_editor_handle_t;
typedef struct the_editor_snapshot_t the_editor_snapshot_t;

typedef enum the_editor_key_kind_t {
  THE_EDITOR_KEY_CHAR = 0,
  THE_EDITOR_KEY_ENTER = 1,
  THE_EDITOR_KEY_NUMPAD_ENTER = 2,
  THE_EDITOR_KEY_ESCAPE = 3,
  THE_EDITOR_KEY_BACKSPACE = 4,
  THE_EDITOR_KEY_TAB = 5,
  THE_EDITOR_KEY_DELETE = 6,
  THE_EDITOR_KEY_INSERT = 7,
  THE_EDITOR_KEY_HOME = 8,
  THE_EDITOR_KEY_END = 9,
  THE_EDITOR_KEY_PAGE_UP = 10,
  THE_EDITOR_KEY_PAGE_DOWN = 11,
  THE_EDITOR_KEY_LEFT = 12,
  THE_EDITOR_KEY_RIGHT = 13,
  THE_EDITOR_KEY_UP = 14,
  THE_EDITOR_KEY_DOWN = 15,
  THE_EDITOR_KEY_F1 = 16,
  THE_EDITOR_KEY_F2 = 17,
  THE_EDITOR_KEY_F3 = 18,
  THE_EDITOR_KEY_F4 = 19,
  THE_EDITOR_KEY_F5 = 20,
  THE_EDITOR_KEY_F6 = 21,
  THE_EDITOR_KEY_F7 = 22,
  THE_EDITOR_KEY_F8 = 23,
  THE_EDITOR_KEY_F9 = 24,
  THE_EDITOR_KEY_F10 = 25,
  THE_EDITOR_KEY_F11 = 26,
  THE_EDITOR_KEY_F12 = 27,
  THE_EDITOR_KEY_OTHER = 28,
} the_editor_key_kind_t;

typedef enum the_editor_mode_t {
  THE_EDITOR_MODE_NORMAL = 0,
  THE_EDITOR_MODE_INSERT = 1,
  THE_EDITOR_MODE_SELECT = 2,
  THE_EDITOR_MODE_COMMAND = 3,
} the_editor_mode_t;

typedef enum the_editor_damage_reason_t {
  THE_EDITOR_DAMAGE_NONE = 0,
  THE_EDITOR_DAMAGE_FULL = 1,
  THE_EDITOR_DAMAGE_LAYOUT = 2,
  THE_EDITOR_DAMAGE_TEXT = 3,
  THE_EDITOR_DAMAGE_DECORATION = 4,
  THE_EDITOR_DAMAGE_CURSOR = 5,
  THE_EDITOR_DAMAGE_SCROLL = 6,
  THE_EDITOR_DAMAGE_THEME = 7,
  THE_EDITOR_DAMAGE_PANE_STRUCTURE = 8,
} the_editor_damage_reason_t;

typedef enum the_editor_cursor_kind_t {
  THE_EDITOR_CURSOR_BLOCK = 0,
  THE_EDITOR_CURSOR_BAR = 1,
  THE_EDITOR_CURSOR_UNDERLINE = 2,
  THE_EDITOR_CURSOR_HOLLOW = 3,
  THE_EDITOR_CURSOR_HIDDEN = 4,
} the_editor_cursor_kind_t;

typedef enum the_editor_selection_kind_t {
  THE_EDITOR_SELECTION_PRIMARY = 0,
  THE_EDITOR_SELECTION_MATCH = 1,
  THE_EDITOR_SELECTION_HOVER = 2,
} the_editor_selection_kind_t;

typedef enum the_editor_overlay_kind_t {
  THE_EDITOR_OVERLAY_RECT = 0,
  THE_EDITOR_OVERLAY_TEXT = 1,
} the_editor_overlay_kind_t;

typedef enum the_editor_overlay_rect_kind_t {
  THE_EDITOR_OVERLAY_RECT_PANEL = 0,
  THE_EDITOR_OVERLAY_RECT_DIVIDER = 1,
  THE_EDITOR_OVERLAY_RECT_HIGHLIGHT = 2,
  THE_EDITOR_OVERLAY_RECT_BACKDROP = 3,
} the_editor_overlay_rect_kind_t;

typedef enum the_editor_underline_style_t {
  THE_EDITOR_UNDERLINE_NONE = 0,
  THE_EDITOR_UNDERLINE_LINE = 1,
  THE_EDITOR_UNDERLINE_CURL = 2,
  THE_EDITOR_UNDERLINE_DOTTED = 3,
  THE_EDITOR_UNDERLINE_DASHED = 4,
  THE_EDITOR_UNDERLINE_DOUBLE_LINE = 5,
} the_editor_underline_style_t;

enum {
  THE_EDITOR_MODIFIER_CTRL = 1 << 0,
  THE_EDITOR_MODIFIER_ALT = 1 << 1,
  THE_EDITOR_MODIFIER_SHIFT = 1 << 2,
};

enum {
  THE_EDITOR_STYLE_MODIFIER_BOLD = 1 << 0,
  THE_EDITOR_STYLE_MODIFIER_DIM = 1 << 1,
  THE_EDITOR_STYLE_MODIFIER_ITALIC = 1 << 2,
  THE_EDITOR_STYLE_MODIFIER_SLOW_BLINK = 1 << 3,
  THE_EDITOR_STYLE_MODIFIER_RAPID_BLINK = 1 << 4,
  THE_EDITOR_STYLE_MODIFIER_REVERSED = 1 << 5,
  THE_EDITOR_STYLE_MODIFIER_HIDDEN = 1 << 6,
  THE_EDITOR_STYLE_MODIFIER_CROSSED_OUT = 1 << 7,
};

typedef struct the_editor_key_event_t {
  uint32_t kind;
  uint32_t codepoint;
  uint8_t modifiers;
} the_editor_key_event_t;

typedef struct the_editor_rgba_t {
  bool present;
  uint8_t r;
  uint8_t g;
  uint8_t b;
  uint8_t a;
} the_editor_rgba_t;

typedef struct the_editor_style_t {
  struct the_editor_rgba_t fg;
  struct the_editor_rgba_t bg;
  struct the_editor_rgba_t underline_color;
  uint16_t add_modifiers;
  uint16_t remove_modifiers;
  uint8_t underline_style;
} the_editor_style_t;

typedef struct the_editor_surface_metrics_t {
  float backing_scale;
  uint16_t cell_width_px;
  uint16_t cell_height_px;
  uint16_t cell_baseline_px;
  uint16_t underline_position_px;
  uint16_t underline_thickness_px;
  uint16_t cursor_thickness_px;
} the_editor_surface_metrics_t;

typedef struct the_editor_surface_config_t {
  uint32_t width_px;
  uint32_t height_px;
  struct the_editor_surface_metrics_t metrics;
} the_editor_surface_config_t;

typedef struct the_editor_snapshot_info_t {
  uint32_t surface_width_px;
  uint32_t surface_height_px;
  struct the_editor_surface_metrics_t surface_metrics;
  struct the_editor_rgba_t background_color;
  struct the_editor_rgba_t gutter_background_color;
  struct the_editor_rgba_t selection_color;
  uint16_t viewport_width;
  uint16_t viewport_height;
  uint16_t content_offset_x;
  uintptr_t active_pane_id;
  uintptr_t pane_count;
  uintptr_t separator_count;
  uint16_t damage_start_row;
  uint16_t damage_end_row;
  bool damage_is_full;
  uint8_t damage_reason;
  uint8_t mode;
  uint64_t layout_generation;
  uint64_t text_generation;
  uint64_t decoration_generation;
  uint64_t cursor_generation;
  uint64_t scroll_generation;
  uint64_t theme_generation;
  bool cursor_blink_enabled;
  uint16_t cursor_blink_interval_ms;
  uint16_t cursor_blink_delay_ms;
  uint64_t cursor_blink_generation;
  uint32_t scroll_row;
  uint32_t scroll_col;
  uint32_t document_line_count;
  uintptr_t line_count;
  uintptr_t cursor_count;
  uintptr_t selection_count;
  uintptr_t overlay_count;
} the_editor_snapshot_info_t;

typedef struct the_editor_snapshot_pane_t {
  uintptr_t pane_id;
  uint8_t kind;
  uint16_t x;
  uint16_t y;
  uint16_t width;
  uint16_t height;
  uint16_t content_offset_x;
  uint32_t scroll_row;
  uint16_t viewport_rows;
  uint32_t document_line_count;
  bool is_active;
} the_editor_snapshot_pane_t;

typedef struct the_editor_snapshot_separator_t {
  uintptr_t split_id;
  uint8_t axis;
  uint16_t line;
  uint16_t span_start;
  uint16_t span_end;
} the_editor_snapshot_separator_t;

typedef struct the_editor_snapshot_line_t {
  uintptr_t pane_id;
  uint16_t x;
  uint16_t row;
  uint16_t width;
  int32_t doc_line;
  bool first_visual_line;
  uintptr_t span_count;
  uintptr_t text_cell_count;
} the_editor_snapshot_line_t;

typedef struct the_editor_snapshot_span_t {
  uint16_t col;
  uint16_t cols;
  const char *text;
  bool is_virtual;
  struct the_editor_style_t style;
} the_editor_snapshot_span_t;

typedef struct the_editor_snapshot_text_cell_t {
  uint16_t row;
  uint16_t col;
  uint16_t cols;
  const char *text;
  bool is_virtual;
  struct the_editor_style_t style;
} the_editor_snapshot_text_cell_t;

typedef struct the_editor_snapshot_document_t {
  const char *name;
  const char *icon;
  const char *relative_path;
  const char *absolute_path;
  const char *vcs_text;
  const char *language_name;
  const char *encoding_name;
  const char *line_ending_name;
  bool is_modified;
  bool is_readonly;
} the_editor_snapshot_document_t;

typedef struct the_editor_snapshot_status_t {
  const char *leading_text;
  uintptr_t item_count;
  const char *cursor_text;
} the_editor_snapshot_status_t;

typedef struct the_editor_snapshot_status_item_t {
  const char *icon;
  const char *text;
  uint8_t emphasis;
} the_editor_snapshot_status_item_t;

typedef struct the_editor_snapshot_pending_keys_t {
  bool visible;
  const char *scope;
  const char *pending_display;
  uintptr_t immediate_count;
  uintptr_t outcome_count;
} the_editor_snapshot_pending_keys_t;

typedef struct the_editor_snapshot_pending_key_outcome_t {
  const char *path_display;
  const char *label;
  uint16_t depth;
  bool immediate;
} the_editor_snapshot_pending_key_outcome_t;

typedef struct the_editor_snapshot_command_palette_t {
  bool is_open;
  int32_t selected_index;
  uintptr_t item_count;
  const char *query;
  const char *placeholder;
} the_editor_snapshot_command_palette_t;

typedef struct the_editor_snapshot_command_palette_item_t {
  const char *title;
  const char *subtitle;
  const char *description;
  const char *badge;
  const char *leading_icon;
  struct the_editor_rgba_t leading_color;
  bool emphasis;
} the_editor_snapshot_command_palette_item_t;

typedef struct the_editor_snapshot_completion_menu_t {
  bool is_open;
  uint16_t col;
  uint16_t row;
  uint16_t width;
  uint16_t height;
  int32_t selected_index;
  uintptr_t item_count;
  uintptr_t scroll_offset;
} the_editor_snapshot_completion_menu_t;

typedef struct the_editor_snapshot_completion_menu_item_t {
  const char *title;
  const char *subtitle;
  const char *leading_icon;
  struct the_editor_rgba_t leading_color;
} the_editor_snapshot_completion_menu_item_t;

typedef enum the_editor_input_prompt_kind_t {
  THE_EDITOR_INPUT_PROMPT_SEARCH = 0,
  THE_EDITOR_INPUT_PROMPT_SELECT_REGEX = 1,
  THE_EDITOR_INPUT_PROMPT_SPLIT_SELECTION = 2,
  THE_EDITOR_INPUT_PROMPT_KEEP_SELECTIONS = 3,
  THE_EDITOR_INPUT_PROMPT_REMOVE_SELECTIONS = 4,
  THE_EDITOR_INPUT_PROMPT_RENAME_SYMBOL = 5,
  THE_EDITOR_INPUT_PROMPT_SHELL_PIPE = 6,
  THE_EDITOR_INPUT_PROMPT_SHELL_PIPE_TO = 7,
  THE_EDITOR_INPUT_PROMPT_SHELL_INSERT_OUTPUT = 8,
  THE_EDITOR_INPUT_PROMPT_SHELL_APPEND_OUTPUT = 9,
  THE_EDITOR_INPUT_PROMPT_SHELL_KEEP_PIPE = 10,
} the_editor_input_prompt_kind_t;

typedef struct the_editor_snapshot_input_prompt_t {
  bool is_open;
  uint8_t kind;
  const char *title;
  const char *placeholder;
  const char *query;
  const char *error;
} the_editor_snapshot_input_prompt_t;

typedef struct the_editor_snapshot_docs_panel_t {
  bool is_open;
  uint16_t col;
  uint16_t row;
  uint16_t width;
  uint16_t height;
  uintptr_t run_count;
} the_editor_snapshot_docs_panel_t;

typedef struct the_editor_snapshot_docs_run_t {
  const char *text;
  struct the_editor_style_t style;
  uint8_t kind;
} the_editor_snapshot_docs_run_t;

typedef struct the_editor_snapshot_diagnostic_t {
  uint32_t start_line;
  uint32_t start_character;
  uint32_t end_line;
  uint32_t end_character;
  uint8_t severity;
  const char *message;
  const char *source;
  const char *code;
} the_editor_snapshot_diagnostic_t;

typedef struct the_editor_snapshot_diagnostic_underline_t {
  uint16_t row;
  uint16_t start_col;
  uint16_t end_col;
  uint8_t severity;
} the_editor_snapshot_diagnostic_underline_t;

typedef struct the_editor_snapshot_file_picker_t {
  bool is_open;
  uint8_t kind;
  int32_t selected_index;
  uintptr_t matched_count;
  uintptr_t visible_item_start;
  uintptr_t visible_item_count;
  const char *title;
  const char *query;
  bool show_preview;
  bool loading;
  const char *error;
  const char *preview_path;
  uint8_t preview_navigation_mode;
  uint8_t preview_kind;
  uintptr_t preview_total_rows;
  uintptr_t preview_offset;
  uintptr_t preview_window_start;
  uintptr_t preview_window_count;
} the_editor_snapshot_file_picker_t;

typedef struct the_editor_snapshot_file_picker_item_t {
  uint64_t stable_id;
  uintptr_t global_index;
  uint8_t row_kind;
  bool selectable;
  bool is_dir;
  const char *icon;
  const char *primary;
  const char *secondary;
  const char *tertiary;
  const char *quaternary;
  uint32_t line;
  uint32_t column;
  uint16_t depth;
} the_editor_snapshot_file_picker_item_t;

typedef struct the_editor_snapshot_file_picker_preview_line_t {
  uintptr_t virtual_row;
  uint8_t kind;
  uint8_t source;
  int32_t line_number;
  bool focused;
  const char *marker;
  uintptr_t segment_count;
} the_editor_snapshot_file_picker_preview_line_t;

typedef struct the_editor_snapshot_file_tree_t {
  bool visible;
  uintptr_t pane_id;
  const char *root;
  int32_t selected_index;
  uintptr_t scroll_offset;
  uintptr_t row_count;
} the_editor_snapshot_file_tree_t;

typedef struct the_editor_snapshot_file_tree_row_t {
  const char *path;
  const char *display_name;
  const char *icon_name;
  const char *icon_glyph;
  uint16_t depth;
  bool has_children;
  bool is_dir;
  bool is_expanded;
  bool is_current_file;
  bool is_selected;
  uint8_t vcs_kind;
  uint8_t diagnostic_severity;
} the_editor_snapshot_file_tree_row_t;

typedef struct the_editor_snapshot_buffer_tabs_t {
  bool visible;
  int32_t active_index;
  uintptr_t active_buffer_id;
  uintptr_t row_count;
} the_editor_snapshot_buffer_tabs_t;

typedef struct the_editor_snapshot_buffer_tab_t {
  uintptr_t buffer_id;
  const char *title;
  const char *directory_hint;
  const char *file_path;
  const char *icon_name;
  bool is_active;
  bool is_modified;
  uint8_t vcs_kind;
  uint8_t diagnostic_severity;
} the_editor_snapshot_buffer_tab_t;

typedef struct the_editor_snapshot_file_picker_preview_segment_t {
  const char *text;
  struct the_editor_style_t style;
  bool is_match;
  int8_t change_kind;
} the_editor_snapshot_file_picker_preview_segment_t;

typedef struct the_editor_snapshot_cursor_t {
  uint32_t row;
  uint32_t col;
  uint8_t kind;
  struct the_editor_style_t style;
} the_editor_snapshot_cursor_t;

typedef struct the_editor_snapshot_selection_t {
  uint16_t x;
  uint16_t y;
  uint16_t width;
  uint16_t height;
  uint8_t kind;
  struct the_editor_style_t style;
} the_editor_snapshot_selection_t;

typedef struct the_editor_snapshot_overlay_t {
  uint8_t kind;
  uint8_t rect_kind;
  uint16_t x;
  uint16_t y;
  uint16_t width;
  uint16_t height;
  uint16_t radius;
  uint32_t row;
  uint32_t col;
  const char *text;
  struct the_editor_style_t style;
} the_editor_snapshot_overlay_t;

the_editor_handle_t *the_editor_new(const char *path);
void the_editor_free(the_editor_handle_t *handle);

bool the_editor_open(the_editor_handle_t *handle, const char *path);
bool the_editor_configure_surface(the_editor_handle_t *handle, struct the_editor_surface_config_t config);
void the_editor_set_viewport(the_editor_handle_t *handle, uint16_t cols, uint16_t rows);
bool the_editor_set_scroll_row(the_editor_handle_t *handle, uint32_t row);
bool the_editor_set_scroll_col(the_editor_handle_t *handle, uint32_t col);
bool the_editor_set_active_pane(the_editor_handle_t *handle, uintptr_t pane_id);
bool the_editor_resize_split(the_editor_handle_t *handle, uintptr_t split_id, uint16_t x, uint16_t y);
bool the_editor_click_buffer_position(the_editor_handle_t *handle, uintptr_t pane_id, uint16_t logical_col, uint16_t logical_row, uint8_t modifiers, uint8_t click_count);
bool the_editor_drag_buffer_selection(the_editor_handle_t *handle, uintptr_t pane_id, uint16_t drag_origin_col, uint16_t drag_origin_row, uint16_t logical_col, uint16_t logical_row, uint8_t modifiers, uint8_t click_count);
bool the_editor_handle_key(the_editor_handle_t *handle, the_editor_key_event_t event);
bool the_editor_toggle_command_palette(the_editor_handle_t *handle);
bool the_editor_close_command_palette(the_editor_handle_t *handle);
bool the_editor_command_palette_set_query(the_editor_handle_t *handle, const char *query);
bool the_editor_command_palette_select_next(the_editor_handle_t *handle);
bool the_editor_command_palette_select_previous(the_editor_handle_t *handle);
bool the_editor_command_palette_select_visible_index(the_editor_handle_t *handle, uintptr_t visible_index);
bool the_editor_command_palette_submit(the_editor_handle_t *handle);
bool the_editor_close_completion_menu(the_editor_handle_t *handle);
bool the_editor_completion_menu_select_index(the_editor_handle_t *handle, uintptr_t index);
bool the_editor_set_completion_menu_scroll(the_editor_handle_t *handle, uintptr_t offset);
bool the_editor_completion_menu_submit(the_editor_handle_t *handle);
bool the_editor_poll_background_tasks(the_editor_handle_t *handle);
bool the_editor_open_search_prompt(the_editor_handle_t *handle);
bool the_editor_close_input_prompt(the_editor_handle_t *handle);
bool the_editor_input_prompt_set_query(the_editor_handle_t *handle, const char *query);
bool the_editor_input_prompt_submit(the_editor_handle_t *handle);
bool the_editor_input_prompt_step_next(the_editor_handle_t *handle);
bool the_editor_input_prompt_step_previous(the_editor_handle_t *handle);
bool the_editor_close_docs_panels(the_editor_handle_t *handle);
bool the_editor_configure_file_picker(the_editor_handle_t *handle, uintptr_t list_visible_rows, uintptr_t preview_visible_rows);
bool the_editor_close_file_picker(the_editor_handle_t *handle);
bool the_editor_file_picker_set_query(the_editor_handle_t *handle, const char *query);
bool the_editor_file_picker_select_next(the_editor_handle_t *handle);
bool the_editor_file_picker_select_previous(the_editor_handle_t *handle);
bool the_editor_file_picker_set_list_offset(the_editor_handle_t *handle, uintptr_t offset);
bool the_editor_file_picker_set_preview_offset(the_editor_handle_t *handle, uintptr_t offset, uintptr_t visible_rows);
bool the_editor_file_picker_select_index(the_editor_handle_t *handle, uintptr_t index);
bool the_editor_file_picker_submit(the_editor_handle_t *handle);
bool the_editor_activate_buffer_tab(the_editor_handle_t *handle, uintptr_t buffer_id);
bool the_editor_close_buffer_tab(the_editor_handle_t *handle, uintptr_t buffer_id);
bool the_editor_file_tree_select_index(the_editor_handle_t *handle, uintptr_t index);
bool the_editor_file_tree_click_index(the_editor_handle_t *handle, uintptr_t index);
bool the_editor_file_tree_activate_index(the_editor_handle_t *handle, uintptr_t index);
bool the_editor_file_tree_set_visible_rows(the_editor_handle_t *handle, uintptr_t visible_rows);
bool the_editor_file_tree_set_scroll_offset(the_editor_handle_t *handle, uintptr_t scroll_offset);
bool the_editor_file_tree_set_active(the_editor_handle_t *handle, bool active);
bool the_editor_toggle_file_tree(the_editor_handle_t *handle);
bool the_editor_insert_text(the_editor_handle_t *handle, const char *text);
uint32_t the_editor_primary_selection_utf16_location(the_editor_handle_t *handle);
uint32_t the_editor_primary_selection_utf16_length(the_editor_handle_t *handle);
char *the_editor_primary_selection_text(the_editor_handle_t *handle);

the_editor_snapshot_t *the_editor_snapshot_create(the_editor_handle_t *handle);
void the_editor_snapshot_free(the_editor_snapshot_t *snapshot);
struct the_editor_snapshot_info_t the_editor_snapshot_info(const the_editor_snapshot_t *snapshot);
struct the_editor_snapshot_pane_t the_editor_snapshot_pane_at(const the_editor_snapshot_t *snapshot, uintptr_t pane_index);
struct the_editor_snapshot_separator_t the_editor_snapshot_separator_at(const the_editor_snapshot_t *snapshot, uintptr_t separator_index);
struct the_editor_snapshot_document_t the_editor_snapshot_document(const the_editor_snapshot_t *snapshot);
struct the_editor_snapshot_status_t the_editor_snapshot_status(const the_editor_snapshot_t *snapshot);
struct the_editor_snapshot_status_item_t the_editor_snapshot_status_item_at(const the_editor_snapshot_t *snapshot, uintptr_t item_index);
struct the_editor_snapshot_pending_keys_t the_editor_snapshot_pending_keys(const the_editor_snapshot_t *snapshot);
struct the_editor_snapshot_pending_key_outcome_t the_editor_snapshot_pending_key_outcome_at(const the_editor_snapshot_t *snapshot, uintptr_t outcome_index);
struct the_editor_snapshot_command_palette_t the_editor_snapshot_command_palette(const the_editor_snapshot_t *snapshot);
struct the_editor_snapshot_command_palette_item_t the_editor_snapshot_command_palette_item_at(const the_editor_snapshot_t *snapshot, uintptr_t item_index);
struct the_editor_snapshot_completion_menu_t the_editor_snapshot_completion_menu(const the_editor_snapshot_t *snapshot);
struct the_editor_snapshot_completion_menu_item_t the_editor_snapshot_completion_menu_item_at(const the_editor_snapshot_t *snapshot, uintptr_t item_index);
struct the_editor_snapshot_input_prompt_t the_editor_snapshot_input_prompt(const the_editor_snapshot_t *snapshot);
struct the_editor_snapshot_docs_panel_t the_editor_snapshot_hover_docs_panel(const the_editor_snapshot_t *snapshot);
struct the_editor_snapshot_docs_run_t the_editor_snapshot_hover_docs_run_at(const the_editor_snapshot_t *snapshot, uintptr_t run_index);
struct the_editor_snapshot_docs_panel_t the_editor_snapshot_completion_docs_panel(const the_editor_snapshot_t *snapshot);
struct the_editor_snapshot_docs_run_t the_editor_snapshot_completion_docs_run_at(const the_editor_snapshot_t *snapshot, uintptr_t run_index);
struct the_editor_snapshot_docs_panel_t the_editor_snapshot_signature_help_panel(const the_editor_snapshot_t *snapshot);
struct the_editor_snapshot_docs_run_t the_editor_snapshot_signature_help_run_at(const the_editor_snapshot_t *snapshot, uintptr_t run_index);
uintptr_t the_editor_snapshot_diagnostic_count(const the_editor_snapshot_t *snapshot);
struct the_editor_snapshot_diagnostic_t the_editor_snapshot_diagnostic_at(const the_editor_snapshot_t *snapshot, uintptr_t diagnostic_index);
uintptr_t the_editor_snapshot_diagnostic_underline_count(const the_editor_snapshot_t *snapshot);
struct the_editor_snapshot_diagnostic_underline_t the_editor_snapshot_diagnostic_underline_at(const the_editor_snapshot_t *snapshot, uintptr_t underline_index);
struct the_editor_snapshot_file_picker_t the_editor_snapshot_file_picker(const the_editor_snapshot_t *snapshot);
struct the_editor_snapshot_file_tree_t the_editor_snapshot_file_tree(const the_editor_snapshot_t *snapshot);
struct the_editor_snapshot_file_tree_row_t the_editor_snapshot_file_tree_row_at(const the_editor_snapshot_t *snapshot, uintptr_t row_index);
struct the_editor_snapshot_buffer_tabs_t the_editor_snapshot_buffer_tabs(const the_editor_snapshot_t *snapshot);
struct the_editor_snapshot_buffer_tab_t the_editor_snapshot_buffer_tab_at(const the_editor_snapshot_t *snapshot, uintptr_t row_index);
struct the_editor_snapshot_file_picker_item_t the_editor_snapshot_file_picker_item_at(const the_editor_snapshot_t *snapshot, uintptr_t item_index);
struct the_editor_snapshot_file_picker_preview_line_t the_editor_snapshot_file_picker_preview_line_at(const the_editor_snapshot_t *snapshot, uintptr_t line_index);
struct the_editor_snapshot_file_picker_preview_segment_t the_editor_snapshot_file_picker_preview_segment_at(const the_editor_snapshot_t *snapshot, uintptr_t line_index, uintptr_t segment_index);
struct the_editor_snapshot_line_t the_editor_snapshot_line_at(const the_editor_snapshot_t *snapshot, uintptr_t line_index);
struct the_editor_snapshot_span_t the_editor_snapshot_span_at(const the_editor_snapshot_t *snapshot, uintptr_t line_index, uintptr_t span_index);
struct the_editor_snapshot_text_cell_t the_editor_snapshot_text_cell_at(const the_editor_snapshot_t *snapshot, uintptr_t line_index, uintptr_t text_cell_index);
struct the_editor_snapshot_cursor_t the_editor_snapshot_cursor_at(const the_editor_snapshot_t *snapshot, uintptr_t cursor_index);
struct the_editor_snapshot_selection_t the_editor_snapshot_selection_at(const the_editor_snapshot_t *snapshot, uintptr_t selection_index);
struct the_editor_snapshot_overlay_t the_editor_snapshot_overlay_at(const the_editor_snapshot_t *snapshot, uintptr_t overlay_index);

void the_editor_string_free(char *value);

#ifdef __cplusplus
}
#endif

#endif
