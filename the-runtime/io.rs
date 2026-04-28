use std::{
  fs,
  path::Path,
};

use ropey::Rope;

pub fn read_text_file(path: &Path) -> std::io::Result<Rope> {
  fs::read_to_string(path).map(Rope::from)
}

pub fn write_text_file(path: &Path, text: &Rope) -> std::io::Result<()> {
  fs::write(path, text.to_string())
}
