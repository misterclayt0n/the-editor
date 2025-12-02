//! File type to icon mapping for the file explorer and other UI components.
//!
//! This module provides a centralized way to determine which SVG icon to
//! display for a given file based on its extension or name.

/// Icon data for rendering. Contains the raw SVG bytes.
#[derive(Clone, Copy)]
pub struct FileIcon {
  pub svg_data: &'static [u8],
}

impl FileIcon {
  const fn new(svg_data: &'static [u8]) -> Self {
    Self { svg_data }
  }
}

// Folder icons
pub const FOLDER_CLOSED: FileIcon = FileIcon::new(include_bytes!("../../assets/folder.svg"));
pub const FOLDER_OPEN: FileIcon = FileIcon::new(include_bytes!("../../assets/folder_open.svg"));

// Generic file icon (fallback)
const FILE_GENERIC: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/file_generic.svg"));

// Programming language icons
const RUST: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/rust.svg"));
const PYTHON: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/python.svg"));
const JAVASCRIPT: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/javascript.svg"));
const TYPESCRIPT: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/typescript.svg"));
const GO: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/go.svg"));
const RUBY: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/ruby.svg"));
const JAVA: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/java.svg"));
const KOTLIN: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/kotlin.svg"));
const SWIFT: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/swift.svg"));
const C: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/c.svg"));
const CPP: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/cpp.svg"));
const CSHARP: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/fsharp.svg")); // Using fsharp as fallback
const FSHARP: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/fsharp.svg"));
const PHP: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/php.svg"));
const LUA: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/lua.svg"));
const LUAU: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/luau.svg"));
const R: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/r.svg"));
const JULIA: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/julia.svg"));
const DART: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/dart.svg"));
const ELIXIR: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/elixir.svg"));
const ERLANG: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/erlang.svg"));
const HASKELL: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/haskell.svg"));
const SCALA: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/scala.svg"));
const OCAML: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/ocaml.svg"));
const NIM: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/nim.svg"));
const NIX: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/nix.svg"));
const ZIG: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/zig.svg"));
const ODIN: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/odin.svg"));
const V: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/v.svg"));
const GLEAM: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/gleam.svg"));
const ROC: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/roc.svg"));
const ELM: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/elm.svg"));

// Web technologies
const HTML: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/html.svg"));
const CSS: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/css.svg"));
const SASS: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/sass.svg"));
const REACT: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/react.svg"));
const VUE: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/vue.svg"));
const ASTRO: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/astro.svg"));
const COFFEESCRIPT: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/coffeescript.svg"));

// Data/Config formats
const JSON: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/json.svg"));
const TOML: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/toml.svg"));
const KDL: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/kdl.svg"));
const GRAPHQL: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/graphql.svg"));

// Markup/Documentation
const MARKDOWN: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/file_markdown.svg"));

// DevOps/Infrastructure
const DOCKER: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/docker.svg"));
const TERRAFORM: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/terraform.svg"));
const HCL: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/hcl.svg"));
const PUPPET: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/puppet.svg"));

// Database
const PRISMA: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/prisma.svg"));
const SURREALQL: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/surrealql.svg"));

// Shader languages
const WGSL: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/wgsl.svg"));
const METAL: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/metal.svg"));

// Shell/Scripting
const TERMINAL: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/terminal.svg"));
const TCL: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/tcl.svg"));

// Version control
const GIT: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/git.svg"));

// Build tools/Package managers
const BUN: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/bun.svg"));

// Linting/Formatting
const ESLINT: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/eslint.svg"));
const PRETTIER: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/prettier.svg"));

// Binary/Archive
const BINARY: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/binary.svg"));
const ARCHIVE: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/archive.svg"));

// Media
const IMAGE: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/image.svg"));
const AUDIO: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/audio.svg"));
const VIDEO: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/video.svg"));

// Special file icons
const FILE_LOCK: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/file_lock.svg"));
const FILE_CODE: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/file_code.svg"));
const FILE_DOC: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/file_doc.svg"));
const NOTEBOOK: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/notebook.svg"));

// Misc languages
const CAIRO: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/cairo.svg"));
const VYPER: FileIcon = FileIcon::new(include_bytes!("../../assets/icons/vyper.svg"));

/// Returns the appropriate icon for a file based on its name and extension.
///
/// # Arguments
/// * `filename` - The file name (not full path) to get an icon for
///
/// # Returns
/// A `FileIcon` containing the SVG data for the appropriate icon
pub fn icon_for_file(filename: &str) -> FileIcon {
  // Check for exact filename matches first (highest priority)
  if let Some(icon) = match_exact_filename(filename) {
    return icon;
  }

  // Check for extension-based matches
  if let Some(icon) = filename.rsplit('.').next().and_then(match_extension) {
    return icon;
  }

  // Default to generic file icon
  FILE_GENERIC
}

/// Match exact filenames (case-insensitive for most)
fn match_exact_filename(filename: &str) -> Option<FileIcon> {
  let lower = filename.to_lowercase();

  Some(match lower.as_str() {
    // Git files
    ".gitignore" | ".gitattributes" | ".gitmodules" => GIT,

    // Docker
    "dockerfile" | "docker-compose.yml" | "docker-compose.yaml" => DOCKER,

    // Package managers / Build tools
    "cargo.toml" | "cargo.lock" => RUST,
    "package.json" | "package-lock.json" => JSON,
    "bun.lockb" => BUN,
    "tsconfig.json" | "jsconfig.json" => TYPESCRIPT,
    "deno.json" | "deno.jsonc" => TYPESCRIPT,

    // Lock files
    "yarn.lock" | "pnpm-lock.yaml" | "gemfile.lock" | "poetry.lock" | "pipfile.lock" => FILE_LOCK,

    // Config files
    ".eslintrc" | ".eslintrc.json" | ".eslintrc.js" | ".eslintrc.cjs" | "eslint.config.js"
    | "eslint.config.mjs" => ESLINT,
    ".prettierrc" | ".prettierrc.json" | ".prettierrc.js" | "prettier.config.js" => PRETTIER,
    "makefile" | "gnumakefile" => TERMINAL,
    "justfile" => TERMINAL,
    "flake.nix" | "flake.lock" | "shell.nix" | "default.nix" => NIX,
    "go.mod" | "go.sum" => GO,
    "gemfile" => RUBY,
    "rakefile" => RUBY,
    "requirements.txt" | "pyproject.toml" | "setup.py" | "pipfile" => PYTHON,

    _ => return None,
  })
}

/// Match file extensions (case-insensitive)
fn match_extension(ext: &str) -> Option<FileIcon> {
  let lower = ext.to_lowercase();

  Some(match lower.as_str() {
    // Rust
    "rs" => RUST,

    // Python
    "py" | "pyi" | "pyw" | "pyx" | "pxd" => PYTHON,
    "ipynb" => NOTEBOOK,

    // JavaScript/TypeScript
    "js" | "mjs" | "cjs" => JAVASCRIPT,
    "ts" | "mts" | "cts" => TYPESCRIPT,
    "jsx" => REACT,
    "tsx" => REACT,

    // Web
    "html" | "htm" | "xhtml" => HTML,
    "css" => CSS,
    "scss" | "sass" => SASS,
    "vue" => VUE,
    "astro" => ASTRO,
    "coffee" => COFFEESCRIPT,
    "svelte" => FILE_CODE, // No specific icon, use code

    // Go
    "go" => GO,

    // Ruby
    "rb" | "erb" | "rake" | "gemspec" => RUBY,

    // Java/JVM
    "java" => JAVA,
    "kt" | "kts" => KOTLIN,
    "scala" | "sc" => SCALA,
    "groovy" | "gradle" => JAVA, // Use Java icon for Groovy/Gradle

    // C/C++
    "c" | "h" => C,
    "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => CPP,

    // C#/F#
    "cs" => CSHARP,
    "fs" | "fsi" | "fsx" => FSHARP,

    // Swift/Apple
    "swift" => SWIFT,

    // PHP
    "php" | "phtml" | "php3" | "php4" | "php5" | "phps" => PHP,

    // Lua
    "lua" => LUA,
    "luau" => LUAU,

    // R
    "r" | "rmd" => R,

    // Julia
    "jl" => JULIA,

    // Dart/Flutter
    "dart" => DART,

    // Elixir/Erlang
    "ex" | "exs" | "eex" | "heex" | "leex" => ELIXIR,
    "erl" | "hrl" => ERLANG,

    // Functional languages
    "hs" | "lhs" => HASKELL,
    "ml" | "mli" => OCAML,
    "elm" => ELM,
    "gleam" => GLEAM,
    "roc" => ROC,

    // Systems languages
    "nim" | "nims" | "nimble" => NIM,
    "nix" => NIX,
    "zig" => ZIG,
    "odin" => ODIN,
    "v" => V,

    // Shell
    "sh" | "bash" | "zsh" | "fish" | "nu" => TERMINAL,
    "ps1" | "psm1" | "psd1" => TERMINAL, // PowerShell
    "bat" | "cmd" => TERMINAL,
    "tcl" => TCL,

    // Data formats
    "json" | "jsonc" | "json5" => JSON,
    "toml" => TOML,
    "yaml" | "yml" => JSON, // Use JSON icon for YAML
    "xml" | "xsl" | "xslt" | "xsd" => FILE_CODE,
    "kdl" => KDL,
    "graphql" | "gql" => GRAPHQL,
    "csv" | "tsv" => FILE_CODE,

    // Markup/Documentation
    "md" | "markdown" | "mdx" => MARKDOWN,
    "rst" | "txt" | "text" => FILE_DOC,
    "org" => FILE_DOC,
    "tex" | "latex" => FILE_DOC,
    "pdf" => FILE_DOC,

    // DevOps/Infrastructure
    "tf" | "tfvars" => TERRAFORM,
    "hcl" => HCL,
    "pp" => PUPPET, // Puppet
    "prisma" => PRISMA,
    "surql" => SURREALQL,

    // Shader languages
    "wgsl" => WGSL,
    "metal" => METAL,
    "glsl" | "vert" | "frag" | "geom" | "tesc" | "tese" | "comp" => FILE_CODE, // GLSL shaders
    "hlsl" => FILE_CODE,

    // Blockchain/Smart contracts
    "cairo" => CAIRO,
    "vy" => VYPER,      // Vyper
    "sol" => FILE_CODE, // Solidity (no specific icon)

    // Archives
    "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" => ARCHIVE,

    // Binary/Executables
    "exe" | "dll" | "so" | "dylib" | "bin" | "o" | "obj" | "a" | "lib" => BINARY,
    "wasm" => BINARY,

    // Images
    "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "svg" | "webp" | "tiff" | "tif" | "heic"
    | "heif" | "avif" => IMAGE,

    // Audio
    "mp3" | "wav" | "ogg" | "flac" | "aac" | "m4a" | "wma" | "aiff" | "opus" => AUDIO,

    // Video
    "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpeg" | "mpg" => VIDEO,

    // Fonts
    "ttf" | "otf" | "woff" | "woff2" | "eot" => BINARY,

    _ => return None,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_rust_files() {
    let icon = icon_for_file("main.rs");
    assert_eq!(icon.svg_data, RUST.svg_data);
  }

  #[test]
  fn test_cargo_toml() {
    let icon = icon_for_file("Cargo.toml");
    assert_eq!(icon.svg_data, RUST.svg_data);
  }

  #[test]
  fn test_typescript() {
    let icon = icon_for_file("app.tsx");
    assert_eq!(icon.svg_data, REACT.svg_data);

    let icon = icon_for_file("utils.ts");
    assert_eq!(icon.svg_data, TYPESCRIPT.svg_data);
  }

  #[test]
  fn test_unknown_extension() {
    let icon = icon_for_file("mystery.xyz123");
    assert_eq!(icon.svg_data, FILE_GENERIC.svg_data);
  }

  #[test]
  fn test_case_insensitive() {
    let icon1 = icon_for_file("Main.RS");
    let icon2 = icon_for_file("main.rs");
    assert_eq!(icon1.svg_data, icon2.svg_data);
  }
}
