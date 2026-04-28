use std::{
  borrow::Cow,
  sync::Arc,
};

use the_lib::clipboard::{
  ClipboardError,
  ClipboardProvider,
  ClipboardType,
  NoClipboard,
  Result,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemClipboard {
  Pasteboard,
  Wayland,
  XClip,
  XSel,
  Win32Yank,
  Tmux,
  #[cfg(windows)]
  Windows,
  Termux,
  None,
}

impl SystemClipboard {
  pub fn detect() -> Self {
    Self::default()
  }

  pub fn provider() -> Arc<dyn ClipboardProvider> {
    Arc::new(Self::detect())
  }

  fn builtin(self) -> Option<CommandProvider> {
    match self {
      Self::Pasteboard => Some(PASTEBOARD),
      Self::Wayland => Some(WL_CLIPBOARD),
      Self::XClip => Some(XCLIP),
      Self::XSel => Some(XSEL),
      Self::Win32Yank => Some(WIN32),
      Self::Tmux => Some(TMUX),
      Self::Termux => Some(TERMUX),
      #[cfg(windows)]
      Self::Windows | Self::None => None,
      #[cfg(not(windows))]
      Self::None => None,
    }
  }
}

impl Default for SystemClipboard {
  #[cfg(windows)]
  fn default() -> Self {
    if binary_exists("win32yank.exe") {
      Self::Win32Yank
    } else {
      Self::Windows
    }
  }

  #[cfg(target_os = "macos")]
  fn default() -> Self {
    if env_var_is_set("TMUX") && binary_exists("tmux") {
      Self::Tmux
    } else if binary_exists("pbcopy") && binary_exists("pbpaste") {
      Self::Pasteboard
    } else {
      Self::None
    }
  }

  #[cfg(not(any(windows, target_os = "macos")))]
  fn default() -> Self {
    fn is_exit_success(program: &str, args: &[&str]) -> bool {
      std::process::Command::new(program)
        .args(args)
        .output()
        .ok()
        .is_some_and(|out| out.status.success())
    }

    if env_var_is_set("WAYLAND_DISPLAY") && binary_exists("wl-copy") && binary_exists("wl-paste") {
      Self::Wayland
    } else if env_var_is_set("DISPLAY") && binary_exists("xclip") {
      Self::XClip
    } else if env_var_is_set("DISPLAY")
      && binary_exists("xsel")
      && is_exit_success("xsel", &["-o", "-b"])
    {
      Self::XSel
    } else if binary_exists("termux-clipboard-set") && binary_exists("termux-clipboard-get") {
      Self::Termux
    } else if env_var_is_set("TMUX") && binary_exists("tmux") {
      Self::Tmux
    } else if binary_exists("win32yank.exe") {
      Self::Win32Yank
    } else {
      Self::None
    }
  }
}

impl ClipboardProvider for SystemClipboard {
  fn name(&self) -> Cow<'_, str> {
    match self {
      Self::Pasteboard => "pasteboard".into(),
      Self::Wayland => "wayland".into(),
      Self::XClip => "xclip".into(),
      Self::XSel => "xsel".into(),
      Self::Win32Yank => "win32yank".into(),
      Self::Tmux => "tmux".into(),
      #[cfg(windows)]
      Self::Windows => "windows".into(),
      Self::Termux => "termux".into(),
      Self::None => NoClipboard.name(),
    }
  }

  fn get_contents(&self, clipboard_type: ClipboardType) -> Result<String> {
    if let Some(provider) = self.builtin() {
      return yank_from_builtin(provider, clipboard_type);
    }

    match self {
      #[cfg(windows)]
      Self::Windows => {
        match clipboard_type {
          ClipboardType::Clipboard => {
            clipboard_win::get_clipboard(clipboard_win::formats::Unicode)
              .map_err(|err| ClipboardError::Platform(err.to_string()))
          },
          ClipboardType::Selection => Ok(String::new()),
        }
      },
      Self::None => Err(ClipboardError::ReadingNotSupported),
      #[cfg(not(windows))]
      _ => Err(ClipboardError::ReadingNotSupported),
    }
  }

  fn set_contents(&self, content: &str, clipboard_type: ClipboardType) -> Result<()> {
    if let Some(provider) = self.builtin() {
      return paste_to_builtin(provider, content, clipboard_type);
    }

    match self {
      #[cfg(windows)]
      Self::Windows => {
        match clipboard_type {
          ClipboardType::Clipboard => {
            clipboard_win::set_clipboard(clipboard_win::formats::Unicode, content)
              .map_err(|err| ClipboardError::Platform(err.to_string()))
          },
          ClipboardType::Selection => Ok(()),
        }
      },
      Self::None => Ok(()),
      #[cfg(not(windows))]
      _ => Ok(()),
    }
  }
}

#[derive(Clone, Copy)]
struct CommandProvider {
  yank:          Command,
  paste:         Command,
  yank_primary:  Option<Command>,
  paste_primary: Option<Command>,
}

#[derive(Clone, Copy)]
struct Command {
  program: &'static str,
  args:    &'static [&'static str],
}

fn yank_from_builtin(provider: CommandProvider, clipboard_type: ClipboardType) -> Result<String> {
  match clipboard_type {
    ClipboardType::Clipboard => {
      execute_command(provider.yank, None, true)?.ok_or(ClipboardError::MissingStdout)
    },
    ClipboardType::Selection => {
      match provider.yank_primary {
        Some(cmd) => execute_command(cmd, None, true)?.ok_or(ClipboardError::MissingStdout),
        None => Ok(String::new()),
      }
    },
  }
}

fn paste_to_builtin(
  provider: CommandProvider,
  content: &str,
  clipboard_type: ClipboardType,
) -> Result<()> {
  let cmd = match clipboard_type {
    ClipboardType::Clipboard => provider.paste,
    ClipboardType::Selection => {
      match provider.paste_primary {
        Some(cmd) => cmd,
        None => return Ok(()),
      }
    },
  };

  execute_command(cmd, Some(content), false).map(|_| ())
}

fn execute_command(cmd: Command, input: Option<&str>, pipe_output: bool) -> Result<Option<String>> {
  use std::{
    io::Write,
    process::{
      Command as ProcessCommand,
      Stdio,
    },
  };

  let stdin = input.map(|_| Stdio::piped()).unwrap_or_else(Stdio::null);
  let stdout = pipe_output.then(Stdio::piped).unwrap_or_else(Stdio::null);

  let mut command = ProcessCommand::new(cmd.program);
  command
    .args(cmd.args)
    .stdin(stdin)
    .stdout(stdout)
    .stderr(Stdio::null());

  #[cfg(unix)]
  unsafe {
    use std::os::unix::process::CommandExt;
    command.pre_exec(|| {
      match libc::setsid() {
        -1 => Err(std::io::Error::last_os_error()),
        _ => Ok(()),
      }
    });
  }

  let mut child = command.spawn()?;

  if let Some(input) = input {
    let Some(mut stdin) = child.stdin.take() else {
      return Err(ClipboardError::StdinWriteFailed);
    };
    stdin
      .write_all(input.as_bytes())
      .map_err(|_| ClipboardError::StdinWriteFailed)?;
  }

  let output = child.wait_with_output()?;
  if !output.status.success() {
    return Err(ClipboardError::CommandFailed);
  }

  if pipe_output {
    String::from_utf8(output.stdout)
      .map(Some)
      .map_err(Into::into)
  } else {
    Ok(None)
  }
}

fn binary_exists(binary: &str) -> bool {
  which::which(binary).is_ok()
}

fn env_var_is_set(name: &str) -> bool {
  std::env::var_os(name).is_some()
}

const TMUX: CommandProvider = CommandProvider {
  yank:          Command {
    program: "tmux",
    args:    &["save-buffer", "-"],
  },
  paste:         Command {
    program: "tmux",
    args:    &["load-buffer", "-w", "-"],
  },
  yank_primary:  None,
  paste_primary: None,
};
const PASTEBOARD: CommandProvider = CommandProvider {
  yank:          Command {
    program: "pbpaste",
    args:    &[],
  },
  paste:         Command {
    program: "pbcopy",
    args:    &[],
  },
  yank_primary:  None,
  paste_primary: None,
};
const WL_CLIPBOARD: CommandProvider = CommandProvider {
  yank:          Command {
    program: "wl-paste",
    args:    &["--no-newline"],
  },
  paste:         Command {
    program: "wl-copy",
    args:    &["--type", "text/plain"],
  },
  yank_primary:  Some(Command {
    program: "wl-paste",
    args:    &["-p", "--no-newline"],
  }),
  paste_primary: Some(Command {
    program: "wl-copy",
    args:    &["-p", "--type", "text/plain"],
  }),
};
const XCLIP: CommandProvider = CommandProvider {
  yank:          Command {
    program: "xclip",
    args:    &["-o", "-selection", "clipboard"],
  },
  paste:         Command {
    program: "xclip",
    args:    &["-i", "-selection", "clipboard"],
  },
  yank_primary:  Some(Command {
    program: "xclip",
    args:    &["-o"],
  }),
  paste_primary: Some(Command {
    program: "xclip",
    args:    &["-i"],
  }),
};
const XSEL: CommandProvider = CommandProvider {
  yank:          Command {
    program: "xsel",
    args:    &["-o", "-b"],
  },
  paste:         Command {
    program: "xsel",
    args:    &["-i", "-b"],
  },
  yank_primary:  Some(Command {
    program: "xsel",
    args:    &["-o"],
  }),
  paste_primary: Some(Command {
    program: "xsel",
    args:    &["-i"],
  }),
};
const WIN32: CommandProvider = CommandProvider {
  yank:          Command {
    program: "win32yank.exe",
    args:    &["-o", "--lf"],
  },
  paste:         Command {
    program: "win32yank.exe",
    args:    &["-i", "--crlf"],
  },
  yank_primary:  None,
  paste_primary: None,
};
const TERMUX: CommandProvider = CommandProvider {
  yank:          Command {
    program: "termux-clipboard-get",
    args:    &[],
  },
  paste:         Command {
    program: "termux-clipboard-set",
    args:    &[],
  },
  yank_primary:  None,
  paste_primary: None,
};

#[cfg(test)]
mod tests {
  use the_lib::clipboard::ClipboardProvider;

  use super::SystemClipboard;

  #[test]
  fn detected_clipboard_has_a_name() {
    let provider = SystemClipboard::detect();
    assert!(!provider.name().is_empty());
  }
}
