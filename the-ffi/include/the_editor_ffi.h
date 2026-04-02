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
  uint16_t viewport_width;
  uint16_t viewport_height;
  uint16_t content_offset_x;
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
  uint64_t cursor_blink_generation;
  uint32_t scroll_row;
  uint32_t scroll_col;
  uint32_t document_line_count;
  uintptr_t line_count;
  uintptr_t cursor_count;
  uintptr_t selection_count;
  uintptr_t overlay_count;
} the_editor_snapshot_info_t;

typedef struct the_editor_snapshot_line_t {
  uint16_t row;
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
bool the_editor_handle_key(the_editor_handle_t *handle, the_editor_key_event_t event);
bool the_editor_insert_text(the_editor_handle_t *handle, const char *text);
uint32_t the_editor_primary_selection_utf16_location(the_editor_handle_t *handle);
uint32_t the_editor_primary_selection_utf16_length(the_editor_handle_t *handle);
char *the_editor_primary_selection_text(the_editor_handle_t *handle);

the_editor_snapshot_t *the_editor_snapshot_create(the_editor_handle_t *handle);
void the_editor_snapshot_free(the_editor_snapshot_t *snapshot);
struct the_editor_snapshot_info_t the_editor_snapshot_info(const the_editor_snapshot_t *snapshot);
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
