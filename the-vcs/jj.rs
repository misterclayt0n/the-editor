use std::{
  path::{
    Path,
    PathBuf,
  },
  process::Command,
  sync::Arc,
};

use arc_swap::ArcSwap;
use eyre::{
  Result,
  WrapErr,
  eyre,
};

use crate::FileChange;

#[cfg(test)] mod test;

const DIFF_LINE_TEMPLATE: &str =
  r#"self.status_char() ++ "\t" ++ self.source().path() ++ "\t" ++ self.target().path() ++ "\n""#;

pub fn is_available() -> bool {
  Command::new("jj")
    .arg("--version")
    .output()
    .map(|output| output.status.success())
    .unwrap_or(false)
}

fn run_jj(cwd: &Path, args: &[&str]) -> Result<std::process::Output> {
  let output = Command::new("jj")
    .arg("--no-pager")
    .arg("--color=never")
    .arg("-R")
    .arg(cwd)
    .args(args)
    .env_remove("GIT_DIR")
    .env_remove("GIT_WORK_TREE")
    .output()
    .wrap_err_with(|| format!("failed to run jj in {}", cwd.display()))?;

  if output.status.success() {
    Ok(output)
  } else {
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(eyre!("jj {:?} failed: {}", args, stderr.trim()))
  }
}

fn canonical_file_path(path: &Path) -> Result<PathBuf> {
  if path.exists() {
    std::fs::canonicalize(path)
      .wrap_err_with(|| format!("failed to canonicalize {}", path.display()))
  } else {
    Err(eyre!("path does not exist: {}", path.display()))
  }
}

fn is_in_jj_workspace(path: &Path) -> bool {
  let start = if path.is_dir() {
    Some(path)
  } else {
    path.parent()
  };
  let Some(start) = start else {
    return false;
  };
  start
    .ancestors()
    .any(|ancestor| ancestor.join(".jj").is_dir())
}

fn jj_repo_root(path: &Path) -> Result<PathBuf> {
  let dir = if path.is_dir() {
    path
  } else {
    path
      .parent()
      .ok_or_else(|| eyre!("file has no parent directory"))?
  };

  if !is_in_jj_workspace(dir) {
    return Err(eyre!("not a jj workspace: {}", dir.display()));
  }

  let output = run_jj(dir, &["--ignore-working-copy", "root"])?;
  let root = String::from_utf8(output.stdout).wrap_err("invalid jj root output")?;
  let root = root.trim();
  if root.is_empty() {
    return Err(eyre!("jj repo root is empty"));
  }
  Ok(PathBuf::from(root))
}

fn repo_relative_jj_path(path: &Path, repo_root: &Path) -> Result<String> {
  let relative = path
    .strip_prefix(repo_root)
    .wrap_err_with(|| format!("{} not under {}", path.display(), repo_root.display()))?;
  Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn parse_jj_diff_entry(repo_root: &Path, line: &str) -> Option<FileChange> {
  if line.is_empty() {
    return None;
  }

  let mut fields = line.splitn(3, '\t');
  let status = fields.next()?.trim();
  let source = fields.next()?.trim();
  let target = fields.next()?.trim();

  let source_path = repo_root.join(source);
  let target_path = repo_root.join(target);

  match status {
    "R" => {
      Some(FileChange::Renamed {
        from_path: source_path,
        to_path:   target_path,
      })
    },
    "D" => Some(FileChange::Deleted { path: source_path }),
    "M" | "A" | "C" => Some(FileChange::Modified { path: target_path }),
    _ if !target.is_empty() => Some(FileChange::Modified { path: target_path }),
    _ => Some(FileChange::Modified { path: source_path }),
  }
}

pub fn get_diff_base(file: &Path) -> Result<Vec<u8>> {
  let file = canonical_file_path(file)?;
  let repo_root = jj_repo_root(&file)?;
  let relative = repo_relative_jj_path(&file, &repo_root)?;
  let fileset = format!("root:{relative}");

  let output = run_jj(&repo_root, &[
    "--ignore-working-copy",
    "--config",
    "templates.file_show=\"\"",
    "file",
    "show",
    "-r",
    "@-",
    &fileset,
  ])?;

  Ok(output.stdout)
}

pub fn get_current_head_name(file: &Path) -> Result<Arc<ArcSwap<Box<str>>>> {
  let file = canonical_file_path(file)?;
  let repo_root = jj_repo_root(&file)?;
  let output = run_jj(&repo_root, &[
    "--ignore-working-copy",
    "log",
    "-r",
    "@",
    "--no-graph",
    "-T",
    "if(self.bookmarks(), self.bookmarks(), self.change_id().shortest(8))",
  ])?;

  let head_name = String::from_utf8(output.stdout).wrap_err("invalid jj head output")?;
  let head_name = head_name.trim();
  if head_name.is_empty() {
    return Err(eyre!("jj head name is empty"));
  }

  Ok(Arc::new(ArcSwap::from_pointee(
    head_name.to_owned().into_boxed_str(),
  )))
}

pub fn for_each_changed_file(cwd: &Path, f: impl Fn(Result<FileChange>) -> bool) -> Result<()> {
  let repo_root = jj_repo_root(cwd)?;
  let output = run_jj(&repo_root, &["diff", "-r", "@", "-T", DIFF_LINE_TEMPLATE])?;
  let text = String::from_utf8(output.stdout).wrap_err("invalid jj diff output")?;

  for line in text.lines() {
    let Some(change) = parse_jj_diff_entry(&repo_root, line) else {
      continue;
    };
    if !f(Ok(change)) {
      break;
    }
  }

  Ok(())
}
