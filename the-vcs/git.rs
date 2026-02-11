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

use crate::{
    FileChange,
    VcsStatuslineInfo,
};

#[cfg(test)]
mod test;

fn run_git(cwd: &Path, args: &[&str]) -> Result<std::process::Output> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .wrap_err_with(|| format!("failed to run git in {}", cwd.display()))?;

    if output.status.success() {
        Ok(output)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(eyre!("git {:?} failed: {}", args, stderr.trim()))
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

fn git_repo_root(path: &Path) -> Result<PathBuf> {
    let dir = if path.is_dir() {
        path
    } else {
        path.parent()
            .ok_or_else(|| eyre!("file has no parent directory"))?
    };

    let output = run_git(dir, &["rev-parse", "--show-toplevel"])?;
    let root = String::from_utf8(output.stdout).wrap_err("invalid git root output")?;
    let root = root.trim();
    if root.is_empty() {
        return Err(eyre!("git repo root is empty"));
    }
    Ok(PathBuf::from(root))
}

fn repo_relative_git_path(path: &Path, repo_root: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(repo_root)
        .wrap_err_with(|| format!("{} not under {}", path.display(), repo_root.display()))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn parse_git_status_line(repo_root: &Path, line: &str) -> Option<FileChange> {
    if line.len() < 4 {
        return None;
    }
    let status = &line[..2];
    let mut payload = line[3..].trim();
    if payload.is_empty() {
        return None;
    }

    if status == "??" {
        return Some(FileChange::Untracked {
            path: repo_root.join(payload),
        });
    }

    if status.contains('U') || matches!(status, "AA" | "DD") {
        return Some(FileChange::Conflict {
            path: repo_root.join(payload),
        });
    }

    if status.contains('R') {
        if let Some((from, to)) = payload.split_once(" -> ") {
            return Some(FileChange::Renamed {
                from_path: repo_root.join(from.trim()),
                to_path: repo_root.join(to.trim()),
            });
        }
        return Some(FileChange::Modified {
            path: repo_root.join(payload),
        });
    }

    if status.contains('D') {
        return Some(FileChange::Deleted {
            path: repo_root.join(payload),
        });
    }

    if status.contains('M')
        || status.contains('A')
        || status.contains('C')
        || status.contains('T')
    {
        return Some(FileChange::Modified {
            path: repo_root.join(payload),
        });
    }

    payload = payload.trim_matches('"');
    Some(FileChange::Modified {
        path: repo_root.join(payload),
    })
}

pub fn get_diff_base(file: &Path) -> Result<Vec<u8>> {
    let file = canonical_file_path(file)?;
    let repo_root = git_repo_root(&file)?;
    let relative = repo_relative_git_path(&file, &repo_root)?;
    let spec = format!("HEAD:{relative}");
    let kind_output = run_git(&repo_root, &["cat-file", "-t", &spec])?;
    let kind = String::from_utf8(kind_output.stdout).wrap_err("invalid git object kind output")?;
    let kind = kind.trim();
    if kind != "blob" {
        return Err(eyre!(
            "git object for {relative} is {kind}, expected blob"
        ));
    }
    let output = run_git(&repo_root, &["show", &spec])?;
    Ok(output.stdout)
}

pub fn get_current_head_name(file: &Path) -> Result<Arc<ArcSwap<Box<str>>>> {
    let file = canonical_file_path(file)?;
    let repo_root = git_repo_root(&file)?;

    let branch = run_git(&repo_root, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());

    let head_name = match branch {
        Some(name) => name,
        None => {
            let output = run_git(&repo_root, &["rev-parse", "--short=8", "HEAD"])?;
            let commit = String::from_utf8(output.stdout).wrap_err("invalid git commit output")?;
            let commit = commit.trim();
            if commit.is_empty() {
                return Err(eyre!("HEAD commit is empty"));
            }
            commit.to_owned()
        }
    };

    Ok(Arc::new(ArcSwap::from_pointee(head_name.into_boxed_str())))
}

pub fn get_statusline_info(file: &Path) -> Result<VcsStatuslineInfo> {
    let file = canonical_file_path(file)?;
    let repo_root = git_repo_root(&file)?;
    let branch = run_git(&repo_root, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "(detached)".to_string());
    Ok(VcsStatuslineInfo::Git {
        branch,
    })
}

pub fn for_each_changed_file(cwd: &Path, f: impl Fn(Result<FileChange>) -> bool) -> Result<()> {
    let repo_root = git_repo_root(cwd)?;
    let output = run_git(
        &repo_root,
        &["status", "--porcelain=1", "--untracked-files=all"],
    )?;
    let text = String::from_utf8(output.stdout).wrap_err("invalid git status output")?;
    for line in text.lines() {
        let Some(change) = parse_git_status_line(&repo_root, line) else {
            continue;
        };
        if !f(Ok(change)) {
            break;
        }
    }
    Ok(())
}
