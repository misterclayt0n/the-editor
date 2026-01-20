//! Functions for working with the host environment.

use std::{
  borrow::Cow,
  ffi::{
    OsStr,
    OsString,
  },
  path::{
    Path,
    PathBuf,
  },
};

use eyre::{
  Result,
  WrapErr,
};
use once_cell::sync::Lazy;
use parking_lot::RwLock;

// We keep the CWD as a static so that we can access it in places where we don't
// have access to the Editor
static CWD: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Get the current working directory.
/// This information is managed internally as the call to std::env::current_dir
/// might fail if the cwd has been deleted.
pub fn current_working_dir() -> Result<PathBuf> {
  if let Some(path) = &*CWD.read() {
    return Ok(path.clone());
  }

  // implementation of crossplatform pwd -L
  // we want pwd -L so that symlinked directories are handled correctly
  let mut cwd = std::env::current_dir().wrap_err("failed to get current working directory")?;

  let pwd = std::env::var_os("PWD");
  #[cfg(windows)]
  let pwd = pwd.or_else(|| std::env::var_os("CD"));

  if let Some(pwd) = pwd.map(PathBuf::from)
    && pwd.canonicalize().ok().as_ref() == Some(&cwd)
  {
    cwd = pwd;
  }

  let mut dst = CWD.write();
  *dst = Some(cwd.clone());

  Ok(cwd)
}

/// Update the current working directory.
pub fn set_current_working_dir(path: impl AsRef<Path>) -> Result<Option<PathBuf>> {
  let path = crate::path::canonicalize(path)?;
  std::env::set_current_dir(&path).wrap_err_with(|| {
    format!(
      "failed to set current working directory to '{}'",
      path.display()
    )
  })?;

  let mut cwd = CWD.write();
  Ok(cwd.replace(path))
}

/// Checks if the given environment variable is set.
pub fn env_var_is_set(env_var_name: &str) -> bool {
  std::env::var_os(env_var_name).is_some()
}

/// Checks if a binary with the given name exists.
pub fn binary_exists<T: AsRef<OsStr>>(binary_name: T) -> bool {
  which::which(binary_name).is_ok()
}

/// Attempts to find a binary of the given name. See [which](https://linux.die.net/man/1/which).
pub fn which<T: AsRef<OsStr>>(binary_name: T) -> Result<PathBuf> {
  let binary_name = binary_name.as_ref();
  which::which(binary_name)
    .wrap_err_with(|| format!("command '{}' not found", binary_name.to_string_lossy()))
}

/// Pattern types for environment variable substitution.
///
/// These correspond to POSIX shell parameter expansion syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VarPattern {
  /// `${VAR:-default}` - use default if VAR is unset OR empty
  DefaultIfUnsetOrEmpty,
  /// `${VAR:=default}` - assign default if VAR is unset OR empty
  AssignIfUnsetOrEmpty,
  /// `${VAR-default}` - use default only if VAR is unset
  DefaultIfUnset,
  /// `${VAR=default}` - assign default only if VAR is unset
  AssignIfUnset,
  /// `${VAR}` - simple braced variable
  Braced,
  /// `$VAR` - simple unbraced variable
  Simple,
}

impl VarPattern {
  fn from_index(index: usize) -> Option<Self> {
    match index {
      0 => Some(Self::DefaultIfUnsetOrEmpty),
      1 => Some(Self::AssignIfUnsetOrEmpty),
      2 => Some(Self::DefaultIfUnset),
      3 => Some(Self::AssignIfUnset),
      4 => Some(Self::Braced),
      5 => Some(Self::Simple),
      _ => None,
    }
  }

  /// `Braced` (`${VAR}`) still needs to find the closing brace,
  /// but it has no default value content.
  fn has_default(self) -> bool {
    matches!(
      self,
      Self::DefaultIfUnsetOrEmpty
        | Self::AssignIfUnsetOrEmpty
        | Self::DefaultIfUnset
        | Self::AssignIfUnset
    )
  }

  fn needs_closing_brace(self) -> bool {
    !matches!(self, Self::Simple)
  }

  fn use_default_when_empty(self) -> bool {
    matches!(
      self,
      Self::DefaultIfUnsetOrEmpty | Self::AssignIfUnsetOrEmpty
    )
  }

  fn resolve<'a>(self, value: Option<&'a OsString>, default: &'a [u8]) -> &'a [u8] {
    match value {
      Some(val) if !val.is_empty() || !self.use_default_when_empty() => val.as_encoded_bytes(),
      _ => default,
    }
  }
}

/// The byte slice must be a valid, codepoint-aligned substring of an OsStr.
fn bytes_to_osstr(bytes: &[u8]) -> &OsStr {
  unsafe { OsStr::from_encoded_bytes_unchecked(bytes) }
}

/// The bytes must be a composition of valid OsStr byte slices.
fn bytes_to_osstring(bytes: Vec<u8>) -> OsString {
  unsafe { OsString::from_encoded_bytes_unchecked(bytes) }
}

/// Find the position of the closing brace, accounting for nested braces.
fn find_closing_brace(src: &[u8]) -> Option<usize> {
  use regex_automata::meta::Regex;

  static REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::builder()
      .build("[{}]")
      .expect("brace regex should compile")
  });

  let mut depth = 0;
  for mat in REGEX.find_iter(src) {
    let pos = mat.start();
    match src[pos] {
      b'{' => depth += 1,
      b'}' if depth == 0 => return Some(pos),
      b'}' => depth -= 1,
      _ => {},
    }
  }
  None
}

/// Regex patterns for matching environment variable syntax.
fn var_expansion_regex() -> &'static regex_automata::meta::Regex {
  use regex_automata::meta::Regex;

  static REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::builder()
      .build_many(&[
        r"\$\{([^\}:]+):-", // 0: ${VAR:-default}
        r"\$\{([^\}:]+):=", // 1: ${VAR:=default}
        r"\$\{([^\}-]+)-",  // 2: ${VAR-default}
        r"\$\{([^\}=]+)=",  // 3: ${VAR=default}
        r"\$\{([^\}]+)",    // 4: ${VAR}
        r"\$(\w+)",         // 5: $VAR
      ])
      .expect("env var expansion regexes should compile")
  });

  &REGEX
}

/// Internal implementation of environment variable expansion.
fn expand_impl(src: &OsStr, mut resolve: impl FnMut(&OsStr) -> Option<OsString>) -> Cow<'_, OsStr> {
  let bytes = src.as_encoded_bytes();
  let mut result = Vec::with_capacity(bytes.len());
  let mut pos = 0;

  for captures in var_expansion_regex().captures_iter(bytes) {
    let Some(mat) = captures.get_match() else {
      continue;
    };

    let Some(var_group) = captures.get_group(1) else {
      continue;
    };

    let Some(pattern) = VarPattern::from_index(mat.pattern().as_usize()) else {
      continue;
    };

    let mut range = mat.range();

    // Skip if we've already processed past this position
    // (can happen with nested variables like `${HOME:-$HOME}`)
    if range.start < pos {
      continue;
    }

    // Handle closing brace for braced patterns
    let default = if pattern.needs_closing_brace() {
      let Some(brace_pos) = find_closing_brace(&bytes[range.end..]) else {
        break;
      };
      let default_bytes = if pattern.has_default() {
        &bytes[range.end..range.end + brace_pos]
      } else {
        &[]
      };
      range.end += brace_pos + 1;
      default_bytes
    } else {
      &[]
    };

    // Resolve the variable
    let var_name = bytes_to_osstr(&bytes[var_group.range()]);
    let var_value = resolve(var_name);
    let expansion = pattern.resolve(var_value.as_ref(), default);

    // Append literal text before this variable, then the expansion
    result.extend_from_slice(&bytes[pos..range.start]);
    result.extend_from_slice(expansion);
    pos = range.end;
  }

  // Return original if no expansions occurred
  if pos == 0 {
    return src.into();
  }

  // Append remaining literal text
  result.extend_from_slice(&bytes[pos..]);
  bytes_to_osstring(result).into()
}

/// Performs substitution of environment variables. Supports the following
/// (POSIX) syntax:
///
/// * `$<var>`, `${<var>}`
/// * `${<var>:-<default>}`, `${<var>-<default>}`
/// * `${<var>:=<default>}`, `${<var>=default}`
pub fn expand<S: AsRef<OsStr> + ?Sized>(src: &S) -> Cow<'_, OsStr> {
  expand_impl(src.as_ref(), |var| std::env::var_os(var))
}

#[cfg(test)]
mod tests {
  use std::ffi::{
    OsStr,
    OsString,
  };

  use super::{
    current_working_dir,
    expand_impl,
    set_current_working_dir,
  };

  #[test]
  fn current_dir_is_set() {
    let new_path = dunce::canonicalize(std::env::temp_dir()).unwrap();
    let cwd = current_working_dir().expect("should get cwd");
    assert_ne!(cwd, new_path);

    set_current_working_dir(&new_path).expect("Couldn't set new path");

    let cwd = current_working_dir().expect("should get cwd");
    assert_eq!(cwd, new_path);
  }

  macro_rules! assert_env_expand {
    ($env:expr, $lhs:expr, $rhs:expr) => {
      assert_eq!(&*expand_impl($lhs.as_ref(), $env), OsStr::new($rhs));
    };
  }

  /// paths that should work on all platforms
  #[test]
  fn test_env_expand() {
    let env = |var: &OsStr| -> Option<OsString> {
      match var.to_str().unwrap() {
        "FOO" => Some("foo".into()),
        "EMPTY" => Some("".into()),
        _ => None,
      }
    };
    assert_env_expand!(env, "pass_trough", "pass_trough");
    assert_env_expand!(env, "$FOO", "foo");
    assert_env_expand!(env, "bar/$FOO/baz", "bar/foo/baz");
    assert_env_expand!(env, "bar/${FOO}/baz", "bar/foo/baz");
    assert_env_expand!(env, "baz/${BAR:-bar}/foo", "baz/bar/foo");
    assert_env_expand!(env, "baz/${FOO:-$FOO}/foo", "baz/foo/foo");
    assert_env_expand!(env, "baz/${BAR:=bar}/foo", "baz/bar/foo");
    assert_env_expand!(env, "baz/${BAR-bar}/foo", "baz/bar/foo");
    assert_env_expand!(env, "baz/${BAR=bar}/foo", "baz/bar/foo");
    assert_env_expand!(env, "baz/${EMPTY:-bar}/foo", "baz/bar/foo");
    assert_env_expand!(env, "baz/${EMPTY:=bar}/foo", "baz/bar/foo");
    assert_env_expand!(env, "baz/${EMPTY-bar}/foo", "baz//foo");
    assert_env_expand!(env, "baz/${EMPTY=bar}/foo", "baz//foo");
  }
}
