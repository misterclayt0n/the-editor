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
  git,
};

fn exec_git_cmd(args: &str, git_dir: &Path) {
  let res = Command::new("git")
        .arg("-C")
        .arg(git_dir) // execute the git command in this directory
        .args(args.split_whitespace())
        .env_remove("GIT_DIR")
        .env_remove("GIT_ASKPASS")
        .env_remove("SSH_ASKPASS")
        .env("GIT_TERMINAL_PROMPT", "false")
        .env("GIT_AUTHOR_DATE", "2000-01-01 00:00:00 +0000")
        .env("GIT_AUTHOR_EMAIL", "author@example.com")
        .env("GIT_AUTHOR_NAME", "author")
        .env("GIT_COMMITTER_DATE", "2000-01-02 00:00:00 +0000")
        .env("GIT_COMMITTER_EMAIL", "committer@example.com")
        .env("GIT_COMMITTER_NAME", "committer")
        .env("GIT_CONFIG_COUNT", "2")
        .env("GIT_CONFIG_KEY_0", "commit.gpgsign")
        .env("GIT_CONFIG_VALUE_0", "false")
        .env("GIT_CONFIG_KEY_1", "init.defaultBranch")
        .env("GIT_CONFIG_VALUE_1", "main")
        .output()
        .unwrap_or_else(|_| panic!("`git {args}` failed"));
  if !res.status.success() {
    println!("{}", String::from_utf8_lossy(&res.stdout));
    eprintln!("{}", String::from_utf8_lossy(&res.stderr));
    panic!("`git {args}` failed (see output above)")
  }
}

fn exec_git_cmd_output(args: &str, git_dir: &Path) -> String {
  let res = Command::new("git")
    .arg("-C")
    .arg(git_dir)
    .args(args.split_whitespace())
    .env_remove("GIT_DIR")
    .env_remove("GIT_ASKPASS")
    .env_remove("SSH_ASKPASS")
    .env("GIT_TERMINAL_PROMPT", "false")
    .output()
    .unwrap_or_else(|_| panic!("`git {args}` failed"));
  if !res.status.success() {
    println!("{}", String::from_utf8_lossy(&res.stdout));
    eprintln!("{}", String::from_utf8_lossy(&res.stderr));
    panic!("`git {args}` failed (see output above)")
  }
  String::from_utf8(res.stdout).unwrap_or_else(|_| panic!("`git {args}` produced non utf8 output"))
}

fn create_commit(repo: &Path, add_modified: bool) {
  if add_modified {
    exec_git_cmd("add -A", repo);
  }
  exec_git_cmd("commit -m message", repo);
}

fn empty_git_repo() -> TempDir {
  let tmp = tempfile::tempdir().expect("create temp dir for git testing");
  exec_git_cmd("init", tmp.path());
  exec_git_cmd("config user.email test@helix.org", tmp.path());
  exec_git_cmd("config user.name helix-test", tmp.path());
  tmp
}

#[test]
fn missing_file() {
  let temp_git = empty_git_repo();
  let file = temp_git.path().join("file.txt");
  File::create(&file).unwrap().write_all(b"foo").unwrap();

  assert!(git::get_diff_base(&file).is_err());
}

#[test]
fn unmodified_file() {
  let temp_git = empty_git_repo();
  let file = temp_git.path().join("file.txt");
  let contents = b"foo".as_slice();
  File::create(&file).unwrap().write_all(contents).unwrap();
  create_commit(temp_git.path(), true);
  assert_eq!(git::get_diff_base(&file).unwrap(), Vec::from(contents));
}

#[test]
fn modified_file() {
  let temp_git = empty_git_repo();
  let file = temp_git.path().join("file.txt");
  let contents = b"foo".as_slice();
  File::create(&file).unwrap().write_all(contents).unwrap();
  create_commit(temp_git.path(), true);
  File::create(&file).unwrap().write_all(b"bar").unwrap();

  assert_eq!(git::get_diff_base(&file).unwrap(), Vec::from(contents));
}

/// Test that `get_file_head` does not return content for a directory.
/// This is important to correctly cover cases where a directory is removed and
/// replaced by a file. If the contents of the directory object were returned a
/// diff between a path and the directory children would be produced.
#[test]
fn directory() {
  let temp_git = empty_git_repo();
  let dir = temp_git.path().join("file.txt");
  std::fs::create_dir(&dir).expect("");
  let file = dir.join("file.txt");
  let contents = b"foo".as_slice();
  File::create(file).unwrap().write_all(contents).unwrap();

  create_commit(temp_git.path(), true);

  std::fs::remove_dir_all(&dir).unwrap();
  File::create(&dir).unwrap().write_all(b"bar").unwrap();
  assert!(git::get_diff_base(&dir).is_err());
}

/// Test that `get_diff_base` resolves symlinks so that the same diff base is
/// used as the target file.
///
/// This is important to correctly cover cases where a symlink is removed and
/// replaced by a file. If the contents of the symlink object were returned
/// a diff between a literal file path and the actual file content would be
/// produced (bad ui).
#[cfg(any(unix, windows))]
#[test]
fn symlink() {
  #[cfg(unix)] use std::os::unix::fs::symlink;
  #[cfg(not(unix))]
  use std::os::windows::fs::symlink_file as symlink;

  let temp_git = empty_git_repo();
  let file = temp_git.path().join("file.txt");
  let contents = Vec::from(b"foo");
  File::create(&file).unwrap().write_all(&contents).unwrap();
  let file_link = temp_git.path().join("file_link.txt");

  symlink("file.txt", &file_link).unwrap();
  create_commit(temp_git.path(), true);

  assert_eq!(git::get_diff_base(&file_link).unwrap(), contents);
  assert_eq!(git::get_diff_base(&file).unwrap(), contents);
}

/// Test that `get_diff_base` returns content when the file is a symlink to
/// another file that is in a git repo, but the symlink itself is not.
#[cfg(any(unix, windows))]
#[test]
fn symlink_to_git_repo() {
  #[cfg(unix)] use std::os::unix::fs::symlink;
  #[cfg(not(unix))]
  use std::os::windows::fs::symlink_file as symlink;

  let temp_dir = tempfile::tempdir().expect("create temp dir");
  let temp_git = empty_git_repo();

  let file = temp_git.path().join("file.txt");
  let contents = Vec::from(b"foo");
  File::create(&file).unwrap().write_all(&contents).unwrap();
  create_commit(temp_git.path(), true);

  let file_link = temp_dir.path().join("file_link.txt");
  symlink(&file, &file_link).unwrap();

  assert_eq!(git::get_diff_base(&file_link).unwrap(), contents);
  assert_eq!(git::get_diff_base(&file).unwrap(), contents);
}

#[test]
fn current_head_name_prefers_branch_name() {
  let temp_git = empty_git_repo();
  let file = temp_git.path().join("file.txt");
  File::create(&file).unwrap().write_all(b"head").unwrap();
  create_commit(temp_git.path(), true);

  let head = git::get_current_head_name(&file).expect("head name");
  let current = head.load();
  assert_eq!(current.as_ref().as_ref(), "main");
}

#[test]
fn current_head_name_falls_back_to_short_commit_when_detached() {
  let temp_git = empty_git_repo();
  let file = temp_git.path().join("file.txt");
  File::create(&file).unwrap().write_all(b"head").unwrap();
  create_commit(temp_git.path(), true);
  exec_git_cmd("checkout --detach HEAD", temp_git.path());

  let head = git::get_current_head_name(&file).expect("head name");
  let current = head.load();
  assert_eq!(current.as_ref().len(), 8);
  assert!(current.chars().all(|ch| ch.is_ascii_hexdigit()));
}

#[test]
fn statusline_info_uses_branch_name() {
  let temp_git = empty_git_repo();
  let file = temp_git.path().join("file.txt");
  File::create(&file).unwrap().write_all(b"head").unwrap();
  create_commit(temp_git.path(), true);

  let info = git::get_statusline_info(&file).expect("statusline info");
  assert_eq!(info, VcsStatuslineInfo::Git {
    branch: "main".to_string(),
  });
}

#[test]
fn statusline_info_uses_detached_marker() {
  let temp_git = empty_git_repo();
  let file = temp_git.path().join("file.txt");
  File::create(&file).unwrap().write_all(b"head").unwrap();
  create_commit(temp_git.path(), true);
  exec_git_cmd("checkout --detach HEAD", temp_git.path());

  let info = git::get_statusline_info(&file).expect("statusline info");
  assert_eq!(info, VcsStatuslineInfo::Git {
    branch: "(detached)".to_string(),
  });
}

#[test]
fn for_each_changed_file_reports_working_tree_changes() {
  let temp_git = empty_git_repo();
  let modified = temp_git.path().join("modified.txt");
  let deleted = temp_git.path().join("deleted.txt");
  File::create(&modified)
    .unwrap()
    .write_all(b"modified")
    .unwrap();
  File::create(&deleted)
    .unwrap()
    .write_all(b"deleted")
    .unwrap();
  create_commit(temp_git.path(), true);

  File::create(&modified)
    .unwrap()
    .write_all(b"changed")
    .unwrap();
  std::fs::remove_file(&deleted).unwrap();
  let untracked = temp_git.path().join("new.txt");
  File::create(&untracked).unwrap().write_all(b"new").unwrap();

  let changes = std::cell::RefCell::new(Vec::new());
  git::for_each_changed_file(temp_git.path(), |entry| {
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
        FileChange::Untracked { path } if path.file_name().is_some_and(|name| name == "new.txt")
    )
  }));
}

#[test]
fn for_each_changed_file_reports_staged_rename() {
  let temp_git = empty_git_repo();
  let old_path = temp_git.path().join("old.txt");
  File::create(&old_path)
    .unwrap()
    .write_all(b"renamed")
    .unwrap();
  create_commit(temp_git.path(), true);

  let new_path = temp_git.path().join("new.txt");
  std::fs::rename(&old_path, &new_path).unwrap();
  exec_git_cmd("add -A", temp_git.path());

  let changes = std::cell::RefCell::new(Vec::new());
  git::for_each_changed_file(temp_git.path(), |entry| {
    changes.borrow_mut().push(entry.expect("file change entry"));
    true
  })
  .expect("collect changed files");
  let changes = changes.into_inner();

  assert!(changes.iter().any(|change| {
    matches!(
        change,
        FileChange::Renamed { from_path, to_path }
          if from_path.file_name().is_some_and(|name| name == "old.txt")
          && to_path.file_name().is_some_and(|name| name == "new.txt")
    )
  }));

  let status = exec_git_cmd_output(
    "status --porcelain=1 --untracked-files=all",
    temp_git.path(),
  );
  assert!(
    status.lines().any(|line| line.starts_with("R ")),
    "expected rename entry in porcelain output, got:\n{status}"
  );
}
