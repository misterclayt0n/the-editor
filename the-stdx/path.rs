//! Functions for working with [Path].

use std::{
  borrow::Cow,
  ffi::OsString,
  ops::Range,
  path::{
    Component,
    MAIN_SEPARATOR_STR,
    Path,
    PathBuf,
  },
};

use eyre::Result;
use once_cell::sync::Lazy;
use regex_cursor::{
  Input,
  engines::meta::Regex,
};
use ropey::RopeSlice;

use crate::env::current_working_dir;

const IS_WINDOWS: bool = cfg!(windows);

/// Replaces users home directory from `path` with tilde `~` if the directory
/// is available, otherwise returns the path unchanged.
pub fn fold_home_dir<'a>(path: Cow<'a, Path>) -> Cow<'a, Path> {
  if let Ok(home) = etcetera::home_dir()
    && let Ok(stripped) = path.strip_prefix(&home)
  {
    let mut path = OsString::with_capacity(2 + stripped.as_os_str().len());
    path.push("~");
    path.push(MAIN_SEPARATOR_STR);
    path.push(stripped);
    return Cow::Owned(PathBuf::from(path));
  }

  path
}

/// Expands tilde `~` into users home directory if available, otherwise returns
/// the path unchanged.
///
/// The tilde will only be expanded when present as the first component of the
/// path and only slash follows it.
pub fn expand_tilde<'a>(path: Cow<'a, Path>) -> Cow<'a, Path> {
  let mut components = path.components();

  if let Some(Component::Normal(c)) = components.next()
    && c == "~"
    && let Ok(mut buf) = etcetera::home_dir()
  {
    buf.push(components);
    return Cow::Owned(buf);
  }

  path
}

/// Normalize a path without resolving symlinks.
// Strategy: start from the first component and move up. Canonicalize previous
// path, join component, canonicalize new path, strip prefix and join to the
// final result.
pub fn normalize(path: impl AsRef<Path>) -> PathBuf {
  normalize_impl(path.as_ref())
}

#[cfg(not(windows))]
fn normalize_impl(path: &Path) -> PathBuf {
  let mut components = path.components().peekable();
  let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().copied() {
    components.next();
    PathBuf::from(c.as_os_str())
  } else {
    PathBuf::new()
  };

  for component in components {
    match component {
      Component::Prefix(..) => unreachable!(),
      Component::RootDir => {
        ret.push(component.as_os_str());
      },
      Component::CurDir => {},
      Component::ParentDir => {
        ret.pop();
      },
      Component::Normal(c) => {
        ret.push(c);
      },
    }
  }
  dunce::simplified(&ret).to_path_buf()
}

#[cfg(windows)]
fn normalize_impl(path: &Path) -> PathBuf {
  let mut components = path.components().peekable();
  let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().copied() {
    components.next();
    PathBuf::from(c.as_os_str())
  } else {
    PathBuf::new()
  };

  for component in components {
    match component {
      Component::Prefix(..) => unreachable!(),
      Component::RootDir => {
        ret.push(component.as_os_str());
      },
      Component::CurDir => {},
      Component::ParentDir => {
        match ret.components().next_back() {
          None | Some(Component::Prefix(_) | Component::RootDir) => {},
          Some(Component::CurDir) => unreachable!(),
          Some(Component::ParentDir) => ret.push(".."),
          Some(Component::Normal(_)) if ret.is_symlink() => ret.push(".."),
          Some(Component::Normal(_)) => {
            ret.pop();
          },
        }
      },
      Component::Normal(c) => {
        use std::fs::canonicalize;

        let new_path = ret.join(c);
        if new_path.is_symlink() {
          ret = new_path;
          continue;
        }
        match (canonicalize(&new_path), canonicalize(&ret)) {
          (Ok(can_new), Ok(can_old)) => {
            ret.push(can_new.strip_prefix(can_old).unwrap_or_else(|_| c.as_ref()));
          },
          _ => ret.push(c),
        }
      },
    }
  }
  dunce::simplified(&ret).to_path_buf()
}

/// Returns the canonical, absolute form of a path with all intermediate
/// components normalized.
///
/// This function is used instead of [`std::fs::canonicalize`] because we don't
/// want to verify here if the path exists, just normalize it's components.
pub fn canonicalize(path: impl AsRef<Path>) -> Result<PathBuf> {
  let path = expand_tilde(Cow::Borrowed(path.as_ref()));
  let cwd = current_working_dir()?;
  Ok(canonicalize_with_cwd(path, &cwd))
}

fn canonicalize_with_cwd(path: Cow<'_, Path>, cwd: &Path) -> PathBuf {
  if path.is_absolute() {
    return normalize(path);
  }

  normalize(cwd.join(path.as_ref()))
}

/// Convert path into a relative path
pub fn get_relative_path<'a, P>(path: P) -> Result<Cow<'a, Path>>
where
  P: Into<Cow<'a, Path>>,
{
  Ok(get_relative_path_with_cwd(
    path.into(),
    &current_working_dir()?,
  ))
}

fn get_relative_path_with_cwd<'a>(path: Cow<'a, Path>, cwd: &Path) -> Cow<'a, Path> {
  if path.is_relative() {
    return path;
  }

  let cwd = normalize(cwd);
  let path_normalized = normalize(path.as_ref());

  if let Ok(stripped) = path_normalized.strip_prefix(cwd) {
    return Cow::Owned(PathBuf::from(stripped));
  }

  return fold_home_dir(path);
}

/// Returns a truncated filepath where the basepart of the path is reduced to
/// the first char of the folder and the whole filename appended.
///
/// Also strip the current working directory from the beginning of the path.
/// Note that this function does not check if the truncated path is unambiguous.
///
/// ```
/// use std::path::Path;
///
/// use the_stdx::path::get_truncated_path;
///
/// assert_eq!(
///   get_truncated_path("/home/cnorris/documents/jokes.txt")
///     .unwrap()
///     .as_path(),
///   Path::new("/h/c/d/jokes.txt")
/// );
/// assert_eq!(
///   get_truncated_path("jokes.txt").unwrap().as_path(),
///   Path::new("jokes.txt")
/// );
/// assert_eq!(
///   get_truncated_path("/jokes.txt").unwrap().as_path(),
///   Path::new("/jokes.txt")
/// );
/// assert_eq!(
///   get_truncated_path("/h/c/d/jokes.txt").unwrap().as_path(),
///   Path::new("/h/c/d/jokes.txt")
/// );
/// assert_eq!(get_truncated_path("").unwrap().as_path(), Path::new(""));
/// ```
pub fn get_truncated_path(path: impl AsRef<Path>) -> Result<PathBuf> {
  let cwd = current_working_dir()?;
  let path = path.as_ref().strip_prefix(cwd).unwrap_or(path.as_ref());

  let file = path.file_name().unwrap_or_default();
  let base = path.parent().unwrap_or(Path::new(""));
  let mut ret = PathBuf::with_capacity(file.len());

  for part in base {
    ret.push(
      part
        .to_string_lossy()
        .chars()
        .next()
        .expect("empty path component")
        .to_string(),
    );
  }

  ret.push(file);
  Ok(ret)
}

fn path_component_regex(windows: bool) -> String {
  // TODO: support backslash path escape on windows (when using git bash for
  // example)
  let space_escape = if windows { r"[\^`]\s" } else { r"[\\]\s" };
  // partially baesd on what's allowed in an url but with some care to avoid
  // false positives (like any kind of brackets or quotes)
  let mut regex = String::from(r"[\w@.\-+#$%?!,;~&]|");
  regex.push_str(space_escape);
  regex
}

/// Regex for delimited environment captures like `${HOME}`.
fn braced_env_regex(windows: bool) -> String {
  let mut regex = String::from(r"\$\{(?:");
  regex.push_str(&path_component_regex(windows));
  regex.push_str(r"|[/:=])+\}");
  regex
}

fn compile_path_regex(
  prefix: &str,
  postfix: &str,
  match_single_file: bool,
  windows: bool,
) -> Regex {
  let first_component = format!(
    "(?:{}|(?:{}))",
    braced_env_regex(windows),
    path_component_regex(windows),
  );

  let component = format!("(?:{first_component}|=)");
  let sep = if windows { r"[/\\]" } else { "/" };
  let url_prefix = r"[\w+\-.]+://??";

  let path_prefix = if windows { r"\\\\\?\\\w:|\w:|\\|" } else { "" };

  let path_start = format!("(?:{first_component}+|~|{path_prefix}{url_prefix})");

  let optional_file = match match_single_file {
    true => format!("|{path_start}"),
    false => String::new(),
  };

  let path_regex = format!(
    "{prefix}(?:{path_start}?(?:(?:{sep}{component}+)+{sep}?|{sep}){optional_file}){postfix}",
  );

  Regex::new(&path_regex).expect("path regex should compile")
}

fn path_regex(match_single_file: bool, anchored: bool) -> &'static Regex {
  match (anchored, match_single_file) {
    (true, true) => {
      static REGEX: Lazy<Regex> = Lazy::new(|| compile_path_regex("", "$", true, IS_WINDOWS));
      &REGEX
    },
    (true, false) => {
      static REGEX: Lazy<Regex> = Lazy::new(|| compile_path_regex("", "$", false, IS_WINDOWS));
      &REGEX
    },
    (false, true) => {
      static REGEX: Lazy<Regex> = Lazy::new(|| compile_path_regex("", "", true, IS_WINDOWS));
      &REGEX
    },
    (false, false) => {
      static REGEX: Lazy<Regex> = Lazy::new(|| compile_path_regex("", "", false, IS_WINDOWS));
      &REGEX
    },
  }
}

/// If `src` ends with a path then this function returns the part of the slice.
pub fn get_path_suffix(src: RopeSlice<'_>, match_single_file: bool) -> Option<RopeSlice<'_>> {
  let regex = path_regex(match_single_file, true);

  regex
    .find(Input::new(src))
    .map(|mat| src.byte_slice(mat.range()))
}

/// Returns an iterator of the **byte** ranges in src that contain a path.
pub fn find_paths(
  src: RopeSlice<'_>,
  match_single_file: bool,
) -> impl Iterator<Item = Range<usize>> + '_ {
  let regex = path_regex(match_single_file, false);
  regex.find_iter(Input::new(src)).map(|mat| mat.range())
}

/// Performs substitution of `~` and environment variables, see
/// [`env::expand`](crate::env::expand) and [`expand_tilde`]
pub fn expand<T: AsRef<Path> + ?Sized>(path: &T) -> Cow<'_, Path> {
  let path = expand_tilde(Cow::Borrowed(path.as_ref()));
  match crate::env::expand(&*path) {
    Cow::Borrowed(_) => path,
    Cow::Owned(path) => PathBuf::from(path).into(),
  }
}

#[cfg(test)]
mod tests {
  use std::{
    borrow::Cow,
    ffi::OsStr,
    path::{
      Component,
      Path,
    },
  };

  use regex_cursor::Input;
  use ropey::RopeSlice;

  use crate::path::{
    self,
    compile_path_regex,
  };

  #[test]
  fn expand_tilde() {
    for path in ["~", "~/foo"] {
      let expanded = path::expand_tilde(Cow::Borrowed(Path::new(path)));

      let tilde = Component::Normal(OsStr::new("~"));

      let mut component_count = 0;
      for component in expanded.components() {
        // No tilde left.
        assert_ne!(component, tilde);
        component_count += 1;
      }

      // The path was at least expanded to something.
      assert_ne!(component_count, 0);
    }
  }

  #[test]
  fn expand_tilde_leaves_tilde_in_later_components_unchanged() {
    let path = Path::new("foo/~/bar");

    let expanded = path::expand_tilde(Cow::Borrowed(path));

    assert_eq!(expanded.as_ref(), path);
  }

  #[test]
  fn normalize_removes_current_dir_and_parent_components() {
    let normalized = path::normalize("__missing_project__/./src/../lib.rs");

    assert_eq!(normalized, Path::new("__missing_project__/lib.rs"));
  }

  #[test]
  fn canonicalize_with_cwd_normalizes_relative_paths() {
    let cwd = Path::new("/workspace/project");
    let path = Cow::Borrowed(Path::new("../src/./lib.rs"));

    let canonical = super::canonicalize_with_cwd(path, cwd);

    assert_eq!(canonical, Path::new("/workspace/src/lib.rs"));
  }

  #[test]
  fn get_relative_path_with_cwd_normalizes_before_stripping_prefix() {
    let cwd = Path::new("/workspace/project");
    let path = Cow::Borrowed(Path::new("/workspace/project/src/../src/lib.rs"));

    let relative = super::get_relative_path_with_cwd(path, cwd);

    assert_eq!(relative.as_ref(), Path::new("src/lib.rs"));
  }

  #[test]
  fn get_path_suffix_only_matches_paths_at_the_end() {
    assert_eq!(
      path::get_path_suffix(RopeSlice::from("open ./src/main.rs"), false)
        .expect("path at the end should match")
        .to_string(),
      "./src/main.rs"
    );
    assert!(path::get_path_suffix(RopeSlice::from("open ./src/main.rs next"), false).is_none());
  }

  #[test]
  fn get_path_suffix_can_match_single_file_when_requested() {
    let src = RopeSlice::from("open README.md");

    assert!(path::get_path_suffix(src, false).is_none());
    assert_eq!(
      path::get_path_suffix(src, true)
        .expect("single file at the end should match")
        .to_string(),
      "README.md"
    );
  }

  #[test]
  fn find_paths_returns_byte_ranges() {
    let src = "λ ./src/lib.rs";
    let ranges: Vec<_> = path::find_paths(RopeSlice::from(src), false).collect();

    assert_eq!(ranges, vec![3..15]);
    assert_eq!(&src[ranges[0].clone()], "./src/lib.rs");
  }

  macro_rules! assert_match {
    ($regex:expr, $haystack:expr) => {
      let haystack = Input::new(RopeSlice::from($haystack));
      assert!(
        $regex.is_match(haystack),
        "regex should match {}",
        $haystack
      );
    };
  }
  macro_rules! assert_no_match {
    ($regex:expr, $haystack:expr) => {
      let haystack = Input::new(RopeSlice::from($haystack));
      assert!(
        !$regex.is_match(haystack),
        "regex should not match {}",
        $haystack
      );
    };
  }

  macro_rules! assert_matches {
        ($regex: expr, $haystack: expr, [$($matches: expr),*]) => {
            let src = $haystack;
            let matches: Vec<_> = $regex
                .find_iter(Input::new(RopeSlice::from(src)))
                .map(|it| &src[it.range()])
                .collect();
            assert_eq!(matches, vec![$($matches),*]);
        };
    }

  /// Linux-only path
  #[test]
  fn path_regex_unix() {
    // due to ambiguity with the `\` path separator we can't support space escapes
    // `\ ` on windows
    let regex = compile_path_regex("^", "$", false, false);
    assert_match!(regex, "${FOO}/hello\\ world");
    assert_match!(regex, "${FOO}/\\ ");
  }

  /// Windows-only paths
  #[test]
  fn path_regex_windows() {
    let regex = compile_path_regex("^", "$", false, true);
    assert_match!(regex, "${FOO}/hello^ world");
    assert_match!(regex, "${FOO}/hello` world");
    assert_match!(regex, "${FOO}/^ ");
    assert_match!(regex, "${FOO}/` ");
    assert_match!(regex, r"foo\bar");
    assert_match!(regex, r"foo\bar");
    assert_match!(regex, r"..\bar");
    assert_match!(regex, r"..\");
    assert_match!(regex, r"C:\");
    assert_match!(regex, r"\\?\C:\foo");
    assert_match!(regex, r"\\server\foo");
  }

  /// Paths that should work on all platforms
  #[test]
  fn path_regex() {
    for windows in [false, true] {
      let regex = compile_path_regex("^", "$", false, windows);
      assert_no_match!(regex, "foo");
      assert_no_match!(regex, "");
      assert_match!(regex, "https://github.com/notifications/query=foo");
      assert_match!(regex, "file:///foo/bar");
      assert_match!(regex, "foo/bar");
      assert_match!(regex, "$HOME/foo");
      assert_match!(regex, "${FOO:-bar}/baz");
      assert_match!(regex, "foo/bar_");
      assert_match!(regex, "/home/bar");
      assert_match!(regex, "foo/");
      assert_match!(regex, "./");
      assert_match!(regex, "../");
      assert_match!(regex, "../..");
      assert_match!(regex, "./foo");
      assert_match!(regex, "./foo.rs");
      assert_match!(regex, "/");
      assert_match!(regex, "~/");
      assert_match!(regex, "~/foo");
      assert_match!(regex, "~/foo");
      assert_match!(regex, "~/foo/../baz");
      assert_match!(regex, "${HOME}/foo");
      assert_match!(regex, "$HOME/foo");
      assert_match!(regex, "/$FOO");
      assert_match!(regex, "/${FOO}");
      assert_match!(regex, "/${FOO}/${BAR}");
      assert_match!(regex, "/${FOO}/${BAR}/foo");
      assert_match!(regex, "/${FOO}/${BAR}");
      assert_match!(regex, "${FOO}/hello_$WORLD");
      assert_match!(regex, "${FOO}/hello_${WORLD}");
      let regex = compile_path_regex("", "", false, windows);
      assert_no_match!(regex, "");
      assert_matches!(
        regex,
        r#"${FOO}/hello_${WORLD}  ${FOO}/hello_${WORLD} foo("./bar", "/home/foo")""#,
        [
          "${FOO}/hello_${WORLD}",
          "${FOO}/hello_${WORLD}",
          "./bar",
          "/home/foo"
        ]
      );
      assert_matches!(regex, r#"--> helix-stdx/src/path.rs:427:13"#, [
        "helix-stdx/src/path.rs"
      ]);
      assert_matches!(
        regex,
        r#"PATH=/foo/bar:/bar/baz:${foo:-/foo}/bar:${PATH}"#,
        ["/foo/bar", "/bar/baz", "${foo:-/foo}/bar"]
      );
      let regex = compile_path_regex("^", "$", true, windows);
      assert_no_match!(regex, "");
      assert_match!(regex, "foo");
      assert_match!(regex, "foo/");
      assert_match!(regex, "$FOO");
      assert_match!(regex, "${BAR}");
    }
  }
}
