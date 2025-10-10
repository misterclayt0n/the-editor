use std::{
  fs,
  path::Path,
  sync::{
    Arc,
    Mutex,
  },
};

use ropey::Rope;
use tempfile::tempdir;

use super::{
  FileResult,
  MatchControl,
  SearchOptions,
  walk_workspace_matches,
};
use crate::editor::FilePickerConfig;

fn collect_matches<P: AsRef<Path>>(
  query: &str,
  root: P,
  options: &SearchOptions,
) -> Vec<FileResult> {
  let matches = Arc::new(Mutex::new(Vec::new()));
  let handler_matches = Arc::clone(&matches);

  walk_workspace_matches(
    query,
    root.as_ref(),
    options,
    Arc::new(move |result| {
      handler_matches.lock().unwrap().push(result);
      MatchControl::Continue
    }),
  )
  .expect("search should succeed");

  Arc::try_unwrap(matches).unwrap().into_inner().unwrap()
}

#[test]
fn finds_matches_in_workspace_files() {
  let temp = tempdir().unwrap();
  let file = temp.path().join("sample.txt");
  fs::write(&file, "alpha\nbeta\nglobal_search matches here\n").unwrap();

  let options = SearchOptions {
    smart_case:  true,
    file_picker: FilePickerConfig::default(),
    documents:   Arc::new(Vec::new()),
  };

  let matches = collect_matches("global_search", temp.path(), &options);

  assert_eq!(matches.len(), 1, "should locate the single match");
  let result = &matches[0];
  assert_eq!(result.path, file);
  assert_eq!(result.line_num, 2);
  assert!(result.line_text.contains("global_search"));
}

#[test]
fn uses_open_document_contents() {
  let temp = tempdir().unwrap();
  let file = temp.path().join("buffer.txt");
  // On-disk contents do NOT have the search term.
  fs::write(&file, "nothing to see here\n").unwrap();

  let edited_contents = Rope::from_str("unsaved global_search change\n");
  let options = SearchOptions {
    smart_case:  true,
    file_picker: FilePickerConfig::default(),
    documents:   Arc::new(vec![(Some(file.clone()), edited_contents)]),
  };

  let matches = collect_matches("global_search", temp.path(), &options);

  assert_eq!(
    matches.len(),
    1,
    "unsaved buffer content should be searched"
  );
  assert_eq!(matches[0].path, file);
  assert_eq!(matches[0].line_num, 0);
  assert!(matches[0].line_text.contains("unsaved"));
}

#[cfg(unix)]
#[test]
fn avoids_duplicate_results_from_symlinks() {
  use std::os::unix::fs::symlink;

  let temp = tempdir().unwrap();
  let original = temp.path().join("original.txt");
  fs::write(&original, "global_search once\n").unwrap();
  let link = temp.path().join("link.txt");
  symlink(&original, &link).unwrap();

  let options = SearchOptions {
    smart_case:  true,
    file_picker: FilePickerConfig::default(),
    documents:   Arc::new(Vec::new()),
  };

  let matches = collect_matches("global_search", temp.path(), &options);

  assert_eq!(matches.len(), 1, "symlinked targets should be deduplicated");
  assert_eq!(matches[0].path, original);
}
