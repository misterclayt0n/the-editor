use std::path::{
  Path,
  PathBuf,
};

use ropey::{
  Rope,
  RopeSlice,
};
use serde_json::{
  Value,
  json,
};
use the_lib::transaction::{
  ChangeSet,
  Operation,
};

use crate::TextDocumentSyncKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChangeType {
  Created,
  Changed,
  Deleted,
}

impl FileChangeType {
  fn as_lsp_code(self) -> u8 {
    match self {
      Self::Created => 1,
      Self::Changed => 2,
      Self::Deleted => 3,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Utf16Position {
  line:      u32,
  character: u32,
}

impl Utf16Position {
  fn as_json(self) -> Value {
    json!({
      "line": self.line,
      "character": self.character,
    })
  }
}

pub fn file_uri_for_path(path: &Path) -> Option<String> {
  let absolute = if path.is_absolute() {
    path.to_path_buf()
  } else {
    std::env::current_dir().ok()?.join(path)
  };

  let mut uri = String::from("file://");
  #[cfg(windows)]
  {
    let normalized = absolute.to_string_lossy().replace('\\', "/");
    uri.push('/');
    uri.push_str(&normalized);
  }
  #[cfg(not(windows))]
  {
    uri.push_str(&absolute.to_string_lossy());
  }
  Some(uri)
}

pub fn path_for_file_uri(uri: &str) -> Option<PathBuf> {
  let parsed = url::Url::parse(uri).ok()?;
  if parsed.scheme() != "file" {
    return None;
  }
  parsed.to_file_path().ok()
}

pub fn char_idx_to_utf16_position(text: &Rope, pos: usize) -> (u32, u32) {
  let utf16 = pos_to_utf16_position(text, pos);
  (utf16.line, utf16.character)
}

pub fn utf16_position_to_char_idx(text: &Rope, line: u32, character: u32) -> usize {
  if text.len_chars() == 0 {
    return 0;
  }

  let line = (line as usize).min(text.len_lines().saturating_sub(1));
  let line_start = text.line_to_char(line);
  let line_end = if line + 1 < text.len_lines() {
    text.line_to_char(line + 1)
  } else {
    text.len_chars()
  };

  let mut utf16_count = 0u32;
  let mut char_idx = line_start;
  for ch in text.slice(line_start..line_end).chars() {
    let next = utf16_count.saturating_add(ch.len_utf16() as u32);
    if next > character {
      break;
    }
    utf16_count = next;
    char_idx = char_idx.saturating_add(1);
  }

  char_idx
}

pub fn did_open_params(uri: &str, language_id: &str, version: i32, text: &Rope) -> Value {
  json!({
    "textDocument": {
      "uri": uri,
      "languageId": language_id,
      "version": version,
      "text": text.to_string(),
    }
  })
}

pub fn did_change_params(
  uri: &str,
  version: i32,
  old_text: &Rope,
  new_text: &Rope,
  changeset: &ChangeSet,
  sync_kind: TextDocumentSyncKind,
) -> Option<Value> {
  let content_changes = match sync_kind {
    TextDocumentSyncKind::None => return None,
    TextDocumentSyncKind::Full => {
      vec![json!({
        "text": new_text.to_string(),
      })]
    },
    TextDocumentSyncKind::Incremental => {
      changeset_to_content_changes(old_text, new_text, changeset)
    },
  };

  if content_changes.is_empty() {
    return None;
  }

  Some(json!({
    "textDocument": {
      "uri": uri,
      "version": version,
    },
    "contentChanges": content_changes,
  }))
}

pub fn did_save_params(uri: &str, text: Option<&str>) -> Value {
  match text {
    Some(text) => {
      json!({
        "textDocument": { "uri": uri },
        "text": text,
      })
    },
    None => {
      json!({
        "textDocument": { "uri": uri },
      })
    },
  }
}

pub fn did_close_params(uri: &str) -> Value {
  json!({
    "textDocument": {
      "uri": uri,
    }
  })
}

pub fn did_change_watched_files_params(
  changes: impl IntoIterator<Item = (String, FileChangeType)>,
) -> Value {
  let changes = changes
    .into_iter()
    .map(|(uri, change_type)| {
      json!({
        "uri": uri,
        "type": change_type.as_lsp_code(),
      })
    })
    .collect::<Vec<_>>();

  json!({
    "changes": changes,
  })
}

pub fn changeset_to_content_changes(
  old_text: &Rope,
  new_text: &Rope,
  changeset: &ChangeSet,
) -> Vec<Value> {
  use Operation::{
    Delete,
    Insert,
    Retain,
  };

  let mut iter = changeset.changes().iter().peekable();
  let mut old_pos = 0usize;
  let mut new_pos = 0usize;
  let old_slice = old_text.slice(..);
  let mut changes = Vec::new();

  while let Some(operation) = iter.next() {
    let len = match operation {
      Delete(count) | Retain(count) => *count,
      Insert(_) => 0,
    };
    let mut old_end = old_pos + len;

    match operation {
      Retain(count) => {
        new_pos += count;
      },
      Delete(_) => {
        let start = pos_to_utf16_position(new_text, new_pos);
        let end = traverse_utf16(start, old_slice.slice(old_pos..old_end));

        changes.push(json!({
          "range": {
            "start": start.as_json(),
            "end": end.as_json(),
          },
          "text": "",
        }));
      },
      Insert(text) => {
        let start = pos_to_utf16_position(new_text, new_pos);
        new_pos += text.chars().count();

        let end = if let Some(Delete(delete_count)) = iter.peek() {
          old_end = old_pos + *delete_count;
          iter.next();
          traverse_utf16(start, old_slice.slice(old_pos..old_end))
        } else {
          start
        };

        changes.push(json!({
          "range": {
            "start": start.as_json(),
            "end": end.as_json(),
          },
          "text": text.to_string(),
        }));
      },
    }

    old_pos = old_end;
  }

  changes
}

fn pos_to_utf16_position(text: &Rope, pos: usize) -> Utf16Position {
  let line = text.char_to_line(pos);
  let line_start = text.line_to_char(line);
  let utf16_col = text
    .slice(line_start..pos)
    .chars()
    .map(|ch| ch.len_utf16() as u32)
    .sum::<u32>();

  Utf16Position {
    line:      line as u32,
    character: utf16_col,
  }
}

fn traverse_utf16(pos: Utf16Position, text: RopeSlice<'_>) -> Utf16Position {
  let Utf16Position {
    mut line,
    mut character,
  } = pos;

  let mut chars = text.chars().peekable();
  while let Some(ch) = chars.next() {
    if ch == '\n' || ch == '\r' {
      if ch == '\r' && chars.peek() == Some(&'\n') {
        chars.next();
      }
      line += 1;
      character = 0;
    } else {
      character += ch.len_utf16() as u32;
    }
  }

  Utf16Position { line, character }
}
