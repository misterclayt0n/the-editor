//! Terminal keyboard encoding
//!
//! This module implements proper keyboard event to terminal escape sequence encoding,
//! inspired by Ghostty's comprehensive approach. It supports:
//! - Legacy VT100/xterm sequences
//! - xterm modifyOtherKeys protocol
//! - Kitty keyboard protocol
//! - All modifier combinations
//! - Terminal mode awareness (cursor key mode, keypad mode, etc.)

use the_editor_renderer::{
  Key,
  KeyPress,
};

/// Modifier state for key encoding
#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
  pub shift: bool,
  pub ctrl:  bool,
  pub alt:   bool,
  pub super_: bool,
}

impl Modifiers {
  pub fn from_keypress(key_press: &KeyPress) -> Self {
    Self {
      shift: key_press.shift,
      ctrl:  key_press.ctrl,
      alt:   key_press.alt,
      super_: key_press.super_,
    }
  }

  /// Check if any modifiers are set
  pub fn has_any(&self) -> bool {
    self.shift || self.ctrl || self.alt || self.super_
  }

  /// Convert to xterm modifier number (1-based)
  /// Used in sequences like CSI 1;{mod}{letter}
  pub fn to_xterm_number(&self) -> u8 {
    let mut num = 1;
    if self.shift {
      num += 1;
    }
    if self.alt {
      num += 2;
    }
    if self.ctrl {
      num += 4;
    }
    if self.super_ {
      num += 8;
    }
    num
  }
}

/// Terminal mode state that affects key encoding
#[derive(Debug, Clone, Copy, Default)]
pub struct TerminalModes {
  /// Cursor key application mode (DECCKM)
  pub cursor_key_application:   bool,
  /// Keypad application mode
  pub keypad_application:       bool,
  /// xterm modifyOtherKeys mode (0, 1, or 2)
  pub modify_other_keys:        u8,
  /// Kitty keyboard protocol flags
  pub kitty_flags:              u32,
  /// Use ESC prefix for Alt modifier
  pub alt_esc_prefix:           bool,
}

/// A single entry in the key lookup table
#[derive(Debug, Clone)]
pub struct KeyEntry {
  /// Required modifiers for this entry
  pub mods:                Modifiers,
  /// Cursor key mode required (None = any, Some(true) = app mode, Some(false) = normal mode)
  pub cursor_mode:         Option<bool>,
  /// Keypad mode required
  pub keypad_mode:         Option<bool>,
  /// ModifyOtherKeys mode required
  pub modify_other_keys:   Option<u8>,
  /// The escape sequence to emit
  pub sequence:            &'static str,
}

impl KeyEntry {
  /// Create a simple entry with just a sequence (no modifiers or mode requirements)
  pub const fn simple(sequence: &'static str) -> Self {
    Self {
      mods:              Modifiers {
        shift: false,
        ctrl:  false,
        alt:   false,
        super_: false,
      },
      cursor_mode:       None,
      keypad_mode:       None,
      modify_other_keys: None,
      sequence,
    }
  }

  /// Create an entry for a specific modifier combination
  pub const fn with_mods(
    shift: bool,
    ctrl: bool,
    alt: bool,
    super_: bool,
    sequence: &'static str,
  ) -> Self {
    Self {
      mods:              Modifiers {
        shift,
        ctrl,
        alt,
        super_,
      },
      cursor_mode:       None,
      keypad_mode:       None,
      modify_other_keys: None,
      sequence,
    }
  }

  /// Check if this entry matches the given state
  pub fn matches(&self, mods: &Modifiers, modes: &TerminalModes) -> bool {
    // Modifiers must match exactly
    if self.mods.shift != mods.shift
      || self.mods.ctrl != mods.ctrl
      || self.mods.alt != mods.alt
      || self.mods.super_ != mods.super_
    {
      return false;
    }

    // Check cursor mode if specified
    if let Some(required) = self.cursor_mode {
      if required != modes.cursor_key_application {
        return false;
      }
    }

    // Check keypad mode if specified
    if let Some(required) = self.keypad_mode {
      if required != modes.keypad_application {
        return false;
      }
    }

    // Check modifyOtherKeys mode if specified
    if let Some(required) = self.modify_other_keys {
      if required != modes.modify_other_keys {
        return false;
      }
    }

    true
  }
}

/// Encode a key press to terminal escape sequence bytes
pub fn encode(key_press: &KeyPress, modes: &TerminalModes) -> Vec<u8> {
  let mods = Modifiers::from_keypress(key_press);

  // Only encode press events, not releases
  if !key_press.pressed {
    return Vec::new();
  }

  // Try Kitty protocol first if enabled
  if modes.kitty_flags != 0 {
    if let Some(seq) = encode_kitty(key_press, &mods, modes) {
      return seq.into_bytes();
    }
  }

  // Fall back to legacy/xterm encoding
  encode_legacy(key_press, &mods, modes).into_bytes()
}

/// Encode using Kitty keyboard protocol
fn encode_kitty(_key_press: &KeyPress, _mods: &Modifiers, _modes: &TerminalModes) -> Option<String> {
  // TODO: Implement Kitty protocol encoding
  // For now, return None to fall back to legacy
  None
}

/// Encode using legacy VT100/xterm sequences
fn encode_legacy(key_press: &KeyPress, mods: &Modifiers, modes: &TerminalModes) -> String {
  // Try table lookup first
  if let Some(seq) = table_lookup(&key_press.code, mods, modes) {
    return seq.to_string();
  }

  // Try C0 control sequences for Ctrl+letter
  if let Some(seq) = encode_c0_control(&key_press.code, mods) {
    return seq;
  }

  // Try Alt-prefix encoding
  if let Some(seq) = encode_alt_prefix(&key_press.code, mods, modes) {
    return seq;
  }

  // Try modifyOtherKeys CSI 27 encoding
  if modes.modify_other_keys > 0 {
    if let Some(seq) = encode_modify_other_keys(&key_press.code, mods) {
      return seq;
    }
  }

  // Fall back to UTF-8 for character keys
  match key_press.code {
    Key::Char(ch) => ch.to_string(),
    _ => String::new(),
  }
}

/// Look up a key in the function key tables
fn table_lookup(key: &Key, mods: &Modifiers, modes: &TerminalModes) -> Option<&'static str> {
  // Get the entries for this key
  let entries = match key {
    Key::Up => &UP_SEQUENCES[..],
    Key::Down => &DOWN_SEQUENCES[..],
    Key::Right => &RIGHT_SEQUENCES[..],
    Key::Left => &LEFT_SEQUENCES[..],
    Key::Home => &HOME_SEQUENCES[..],
    Key::End => &END_SEQUENCES[..],
    Key::PageUp => &PAGEUP_SEQUENCES[..],
    Key::PageDown => &PAGEDOWN_SEQUENCES[..],
    Key::Insert => &INSERT_SEQUENCES[..],
    Key::Delete => &DELETE_SEQUENCES[..],
    Key::Backspace => &BACKSPACE_SEQUENCES[..],
    Key::Tab => &TAB_SEQUENCES[..],
    Key::Enter => &ENTER_SEQUENCES[..],
    Key::Escape => &ESCAPE_SEQUENCES[..],
    Key::F1 => &F1_SEQUENCES[..],
    Key::F2 => &F2_SEQUENCES[..],
    Key::F3 => &F3_SEQUENCES[..],
    Key::F4 => &F4_SEQUENCES[..],
    Key::F5 => &F5_SEQUENCES[..],
    Key::F6 => &F6_SEQUENCES[..],
    Key::F7 => &F7_SEQUENCES[..],
    Key::F8 => &F8_SEQUENCES[..],
    Key::F9 => &F9_SEQUENCES[..],
    Key::F10 => &F10_SEQUENCES[..],
    Key::F11 => &F11_SEQUENCES[..],
    Key::F12 => &F12_SEQUENCES[..],
    _ => return None,
  };

  // Find first matching entry
  entries.iter().find(|e| e.matches(mods, modes)).map(|e| e.sequence)
}

/// Encode C0 control sequences (Ctrl+letter -> 0x01-0x1A)
fn encode_c0_control(key: &Key, mods: &Modifiers) -> Option<String> {
  if !mods.ctrl || mods.alt || mods.super_ {
    return None;
  }

  match key {
    Key::Char(ch) => {
      let ch_lower = ch.to_ascii_lowercase();
      if ('a'..='z').contains(&ch_lower) {
        // Ctrl+A = 0x01, Ctrl+B = 0x02, ..., Ctrl+Z = 0x1A
        let code = (ch_lower as u8 - b'a') + 1;
        Some(String::from_utf8_lossy(&[code]).to_string())
      } else {
        None
      }
    },
    _ => None,
  }
}

/// Encode Alt-prefix sequences (Alt+key -> ESC + key)
fn encode_alt_prefix(key: &Key, mods: &Modifiers, modes: &TerminalModes) -> Option<String> {
  if !mods.alt || mods.ctrl || mods.super_ || !modes.alt_esc_prefix {
    return None;
  }

  match key {
    Key::Char(ch) => Some(format!("\x1b{}", ch)),
    Key::Backspace => Some("\x1b\x7f".to_string()),
    _ => None,
  }
}

/// Encode using xterm modifyOtherKeys CSI 27 sequences
fn encode_modify_other_keys(key: &Key, mods: &Modifiers) -> Option<String> {
  if !mods.has_any() {
    return None;
  }

  // modifyOtherKeys uses CSI 27 ; modifier ; code ~
  match key {
    Key::Char(ch) => {
      let modifier_num = mods.to_xterm_number();
      let char_code = *ch as u32;
      Some(format!("\x1b[27;{};{}~", modifier_num, char_code))
    },
    _ => None,
  }
}

// Lookup tables for special keys
// These will be populated in the next task with comprehensive entries from Ghostty

static UP_SEQUENCES: &[KeyEntry] = &[
  // Normal mode, no modifiers
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: false, alt: false, super_: false },
    cursor_mode:       Some(false),
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x1b[A",
  },
  // Application mode, no modifiers
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: false, alt: false, super_: false },
    cursor_mode:       Some(true),
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x1bOA",
  },
  // With modifiers (PC-style)
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: true, alt: false, super_: false },
    cursor_mode:       None,
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x1b[1;5A",
  },
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: false, alt: true, super_: false },
    cursor_mode:       None,
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x1b[1;3A",
  },
];

static DOWN_SEQUENCES: &[KeyEntry] = &[
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: false, alt: false, super_: false },
    cursor_mode:       Some(false),
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x1b[B",
  },
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: false, alt: false, super_: false },
    cursor_mode:       Some(true),
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x1bOB",
  },
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: true, alt: false, super_: false },
    cursor_mode:       None,
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x1b[1;5B",
  },
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: false, alt: true, super_: false },
    cursor_mode:       None,
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x1b[1;3B",
  },
];

static LEFT_SEQUENCES: &[KeyEntry] = &[
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: false, alt: false, super_: false },
    cursor_mode:       Some(false),
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x1b[D",
  },
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: false, alt: false, super_: false },
    cursor_mode:       Some(true),
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x1bOD",
  },
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: true, alt: false, super_: false },
    cursor_mode:       None,
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x1b[1;5D",
  },
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: false, alt: true, super_: false },
    cursor_mode:       None,
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x1b[1;3D",
  },
];

static RIGHT_SEQUENCES: &[KeyEntry] = &[
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: false, alt: false, super_: false },
    cursor_mode:       Some(false),
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x1b[C",
  },
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: false, alt: false, super_: false },
    cursor_mode:       Some(true),
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x1bOC",
  },
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: true, alt: false, super_: false },
    cursor_mode:       None,
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x1b[1;5C",
  },
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: false, alt: true, super_: false },
    cursor_mode:       None,
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x1b[1;3C",
  },
];

static HOME_SEQUENCES: &[KeyEntry] = &[
  KeyEntry::simple("\x1b[H"),
];

static END_SEQUENCES: &[KeyEntry] = &[
  KeyEntry::simple("\x1b[F"),
];

static PAGEUP_SEQUENCES: &[KeyEntry] = &[
  KeyEntry::simple("\x1b[5~"),
];

static PAGEDOWN_SEQUENCES: &[KeyEntry] = &[
  KeyEntry::simple("\x1b[6~"),
];

static INSERT_SEQUENCES: &[KeyEntry] = &[
  KeyEntry::simple("\x1b[2~"),
];

static DELETE_SEQUENCES: &[KeyEntry] = &[
  KeyEntry::simple("\x1b[3~"),
];

static BACKSPACE_SEQUENCES: &[KeyEntry] = &[
  // Plain backspace
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: false, alt: false, super_: false },
    cursor_mode:       None,
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x7f",
  },
  // Alt+Backspace
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: false, alt: true, super_: false },
    cursor_mode:       None,
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x1b\x7f",
  },
];

// Tab sequences
static TAB_SEQUENCES: &[KeyEntry] = &[
  // Plain tab
  KeyEntry {
    mods:              Modifiers { shift: false, ctrl: false, alt: false, super_: false },
    cursor_mode:       None,
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\t",
  },
  // Shift+Tab (backtab)
  KeyEntry {
    mods:              Modifiers { shift: true, ctrl: false, alt: false, super_: false },
    cursor_mode:       None,
    keypad_mode:       None,
    modify_other_keys: None,
    sequence:          "\x1b[Z",
  },
];

// Enter sequences
static ENTER_SEQUENCES: &[KeyEntry] = &[
  KeyEntry::simple("\r"),
];

// Escape sequences
static ESCAPE_SEQUENCES: &[KeyEntry] = &[
  KeyEntry::simple("\x1b"),
];

// Function keys F1-F12
static F1_SEQUENCES: &[KeyEntry] = &[KeyEntry::simple("\x1bOP")];
static F2_SEQUENCES: &[KeyEntry] = &[KeyEntry::simple("\x1bOQ")];
static F3_SEQUENCES: &[KeyEntry] = &[KeyEntry::simple("\x1bOR")];
static F4_SEQUENCES: &[KeyEntry] = &[KeyEntry::simple("\x1bOS")];
static F5_SEQUENCES: &[KeyEntry] = &[KeyEntry::simple("\x1b[15~")];
static F6_SEQUENCES: &[KeyEntry] = &[KeyEntry::simple("\x1b[17~")];
static F7_SEQUENCES: &[KeyEntry] = &[KeyEntry::simple("\x1b[18~")];
static F8_SEQUENCES: &[KeyEntry] = &[KeyEntry::simple("\x1b[19~")];
static F9_SEQUENCES: &[KeyEntry] = &[KeyEntry::simple("\x1b[20~")];
static F10_SEQUENCES: &[KeyEntry] = &[KeyEntry::simple("\x1b[21~")];
static F11_SEQUENCES: &[KeyEntry] = &[KeyEntry::simple("\x1b[23~")];
static F12_SEQUENCES: &[KeyEntry] = &[KeyEntry::simple("\x1b[24~")];
