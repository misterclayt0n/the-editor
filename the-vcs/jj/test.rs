use std::{
  fs::File,
  io::Write,
  path::Path,
  process::Command,
};

use tempfile::TempDir;

use crate::{
  FileChange,
  VcsStatuslineInfo,
  jj,
};

fn has_jj() -> bool {
  Command::new("jj")
    .arg("--version")
    .output()
    .map(|output| output.status.success())
    .unwrap_or(false)
}

fn require_jj() -> bool {
  if has_jj() {
    true
  } else {
    eprintln!("skipping jj tests: `jj` executable not available");
    false
  }
}

fn exec_jj_cmd(args: &[&str], repo: &Path) {
  let res = Command::new("jj")
    .arg("-R")
    .arg(repo)
    .args(args)
    .env_remove("GIT_DIR")
    .env_remove("GIT_ASKPASS")
    .env_remove("SSH_ASKPASS")
    .output()
    .unwrap_or_else(|_| panic!("`jj {args:?}` failed"));
  if !res.status.success() {
    println!("{}", String::from_utf8_lossy(&res.stdout));
    eprintln!("{}", String::from_utf8_lossy(&res.stderr));
    panic!("`jj {args:?}` failed (see output above)")
  }
}

fn empty_jj_repo() -> TempDir {
  let tmp = tempfile::tempdir().expect("create temp dir for jj testing");
  let res = Command::new("jj")
    .arg("git")
    .arg("init")
    .arg(tmp.path())
    .output()
    .unwrap_or_else(|_| panic!("`jj git init {}` failed", tmp.path().display()));
  if !res.status.success() {
    println!("{}", String::from_utf8_lossy(&res.stdout));
    eprintln!("{}", String::from_utf8_lossy(&res.stderr));
    panic!("`jj git init {}` failed", tmp.path().display());
  }
  tmp
}

fn create_base_commit(repo: &Path) {
  exec_jj_cmd(&["describe", "-m", "init"], repo);
  exec_jj_cmd(&["new", "-m", "work"], repo);
}

#[test]
fn missing_file() {
  if !require_jj() {
    return;
  }
  let temp_jj = empty_jj_repo();
  let file = temp_jj.path().join("file.txt");
  File::create(&file).unwrap().write_all(b"foo").unwrap();

  assert!(jj::get_diff_base(&file).is_err());
}

#[test]
fn unmodified_file() {
  if !require_jj() {
    return;
  }
  let temp_jj = empty_jj_repo();
  let file = temp_jj.path().join("file.txt");
  let contents = b"foo".as_slice();
  File::create(&file).unwrap().write_all(contents).unwrap();
  create_base_commit(temp_jj.path());

  assert_eq!(jj::get_diff_base(&file).unwrap(), Vec::from(contents));
}

#[test]
fn modified_file() {
  if !require_jj() {
    return;
  }
  let temp_jj = empty_jj_repo();
  let file = temp_jj.path().join("file.txt");
  let contents = b"foo".as_slice();
  File::create(&file).unwrap().write_all(contents).unwrap();
  create_base_commit(temp_jj.path());
  File::create(&file).unwrap().write_all(b"bar").unwrap();

  assert_eq!(jj::get_diff_base(&file).unwrap(), Vec::from(contents));
}

#[test]
fn statusline_info_supports_nested_file_paths() {
  if !require_jj() {
    return;
  }
  let temp_jj = empty_jj_repo();
  let nested_dir = temp_jj.path().join("the-lib/src");
  std::fs::create_dir_all(&nested_dir).unwrap();
  let file = nested_dir.join("command_line.rs");
  File::create(&file).unwrap().write_all(b"head").unwrap();
  create_base_commit(temp_jj.path());

  let info = jj::get_statusline_info(&file).expect("statusline info");
  assert_eq!(info, VcsStatuslineInfo::Jj {
    description: "work".to_string(),
    bookmark:    None,
  });
}

#[test]
fn current_head_name_prefers_bookmark_name() {
  if !require_jj() {
    return;
  }
  let temp_jj = empty_jj_repo();
  let file = temp_jj.path().join("file.txt");
  File::create(&file).unwrap().write_all(b"head").unwrap();
  create_base_commit(temp_jj.path());
  exec_jj_cmd(&["bookmark", "create", "main", "-r", "@"], temp_jj.path());

  let head = jj::get_current_head_name(&file).expect("head name");
  let current = head.load();
  assert!(
    current.split_whitespace().any(|name| name == "main"),
    "expected current head to contain bookmark `main`, got `{current}`"
  );
}

#[test]
fn current_head_name_falls_back_to_change_id() {
  if !require_jj() {
    return;
  }
  let temp_jj = empty_jj_repo();
  let file = temp_jj.path().join("file.txt");
  File::create(&file).unwrap().write_all(b"head").unwrap();
  create_base_commit(temp_jj.path());

  let head = jj::get_current_head_name(&file).expect("head name");
  let current = head.load();
  assert!(current.len() >= 8);
  assert!(
    current
      .chars()
      .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
  );
}

#[test]
fn statusline_info_includes_description_without_bookmark() {
  if !require_jj() {
    return;
  }
  let temp_jj = empty_jj_repo();
  let file = temp_jj.path().join("file.txt");
  File::create(&file).unwrap().write_all(b"head").unwrap();
  create_base_commit(temp_jj.path());

  let info = jj::get_statusline_info(&file).expect("statusline info");
  assert_eq!(info, VcsStatuslineInfo::Jj {
    description: "work".to_string(),
    bookmark:    None,
  });
}

#[test]
fn statusline_info_includes_description_and_bookmark() {
  if !require_jj() {
    return;
  }
  let temp_jj = empty_jj_repo();
  let file = temp_jj.path().join("file.txt");
  File::create(&file).unwrap().write_all(b"head").unwrap();
  create_base_commit(temp_jj.path());
  exec_jj_cmd(&["describe", "-m", "vcs: add statusline"], temp_jj.path());
  exec_jj_cmd(&["bookmark", "create", "main", "-r", "@"], temp_jj.path());

  let info = jj::get_statusline_info(&file).expect("statusline info");
  assert_eq!(info, VcsStatuslineInfo::Jj {
    description: "vcs: add statusline".to_string(),
    bookmark:    Some("main".to_string()),
  });
}

#[test]
fn for_each_changed_file_reports_working_tree_changes() {
  if !require_jj() {
    return;
  }
  let temp_jj = empty_jj_repo();
  let modified = temp_jj.path().join("modified.txt");
  let deleted = temp_jj.path().join("deleted.txt");
  let old_path = temp_jj.path().join("old.txt");
  File::create(&modified)
    .unwrap()
    .write_all(b"modified")
    .unwrap();
  File::create(&deleted)
    .unwrap()
    .write_all(b"deleted")
    .unwrap();
  File::create(&old_path)
    .unwrap()
    .write_all(b"renamed")
    .unwrap();
  create_base_commit(temp_jj.path());

  File::create(&modified)
    .unwrap()
    .write_all(b"changed")
    .unwrap();
  std::fs::remove_file(&deleted).unwrap();
  let new_path = temp_jj.path().join("new.txt");
  std::fs::rename(&old_path, &new_path).unwrap();
  let added = temp_jj.path().join("added.txt");
  File::create(&added).unwrap().write_all(b"new").unwrap();

  let changes = std::cell::RefCell::new(Vec::new());
  jj::for_each_changed_file(temp_jj.path(), |entry| {
    changes.borrow_mut().push(entry.expect("file change entry"));
    true
  })
  .expect("collect changed files");
  let changes = changes.into_inner();

  assert!(changes.iter().any(|change| {
    matches!(
        change,
        FileChange::Modified { path } if path.file_name().is_some_and(|name| name == "modified.txt")
    )
  }));
  assert!(changes.iter().any(|change| {
    matches!(
        change,
        FileChange::Deleted { path } if path.file_name().is_some_and(|name| name == "deleted.txt")
    )
  }));
  assert!(changes.iter().any(|change| {
    matches!(
        change,
        FileChange::Modified { path } if path.file_name().is_some_and(|name| name == "added.txt")
    )
  }));
  assert!(changes.iter().any(|change| {
    matches!(
        change,
        FileChange::Renamed { from_path, to_path }
          if from_path.file_name().is_some_and(|name| name == "old.txt")
          && to_path.file_name().is_some_and(|name| name == "new.txt")
    )
  }));
}
