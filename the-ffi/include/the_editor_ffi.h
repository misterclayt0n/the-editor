#ifndef THE_EDITOR_FFI_H
#define THE_EDITOR_FFI_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct the_editor_handle_t the_editor_handle_t;

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

enum {
  THE_EDITOR_MODIFIER_CTRL = 1 << 0,
  THE_EDITOR_MODIFIER_ALT = 1 << 1,
  THE_EDITOR_MODIFIER_SHIFT = 1 << 2,
};

typedef struct the_editor_key_event_t {
  uint32_t kind;
  uint32_t codepoint;
  uint8_t modifiers;
} the_editor_key_event_t;

the_editor_handle_t *the_editor_new(const char *path);
void the_editor_free(the_editor_handle_t *handle);

bool the_editor_open(the_editor_handle_t *handle, const char *path);
void the_editor_set_viewport(the_editor_handle_t *handle, uint16_t cols, uint16_t rows);
bool the_editor_handle_key(the_editor_handle_t *handle, the_editor_key_event_t event);
bool the_editor_scroll_lines(the_editor_handle_t *handle, int32_t delta_lines);
char *the_editor_snapshot_json(the_editor_handle_t *handle);
void the_editor_string_free(char *value);

#ifdef __cplusplus
}
#endif

#endif
