use std::path::PathBuf;

fn main() {
  let cargo_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
  let cargo_manifest_dir = PathBuf::from(&cargo_dir);

  // Link against pre-built wrapper library
  // Assumes: zig build has been run in the-terminal directory
  let wrapper_lib_path = cargo_manifest_dir.join("zig-out/lib");
  if !wrapper_lib_path.exists() {
    eprintln!(
      "ERROR: Pre-built wrapper library not found at: {}",
      wrapper_lib_path.display()
    );
    eprintln!(
      "Please run: cd {} && zig build",
      cargo_manifest_dir.display()
    );
    panic!("Missing pre-built ghostty wrapper library");
  }

  println!(
    "cargo:rustc-link-search=native={}",
    wrapper_lib_path.display()
  );
  println!("cargo:rustc-link-arg=-Wl,--whole-archive");
  println!("cargo:rustc-link-lib=static=ghostty_wrapper");
  println!("cargo:rustc-link-arg=-Wl,--no-whole-archive");

  // Find libghostty-vt from either:
  // 1. LD_LIBRARY_PATH (set by Nix)
  // 2. Local ghostty project build
  let ghostty_lib_path = if let Ok(ld_lib_path) = std::env::var("LD_LIBRARY_PATH") {
    // Try to find libghostty-vt in LD_LIBRARY_PATH (set by Nix)
    ld_lib_path
      .split(':')
      .find_map(|path| {
        let candidate = PathBuf::from(path);
        if candidate.join("libghostty-vt.so").exists() {
          Some(candidate)
        } else {
          None
        }
      })
      .unwrap_or_else(|| {
        // Fall back to local ghostty build
        let ghostty_dir = cargo_manifest_dir
          .parent()
          .expect("Invalid cargo dir")
          .parent()
          .expect("Invalid cargo dir")
          .join("ghostty");
        ghostty_dir.join("zig-out/lib")
      })
  } else {
    // No LD_LIBRARY_PATH, look for local ghostty
    let ghostty_dir = cargo_manifest_dir
      .parent()
      .expect("Invalid cargo dir")
      .parent()
      .expect("Invalid cargo dir")
      .join("ghostty");
    ghostty_dir.join("zig-out/lib")
  };

  if !ghostty_lib_path.exists() {
    eprintln!(
      "ERROR: libghostty-vt not found at: {}",
      ghostty_lib_path.display()
    );
    eprintln!("This can happen if:");
    eprintln!("  1. Building locally: build ghostty with 'cd ~/code/ghostty && zig build'");
    eprintln!(
      "  2. Building in Nix: the dev shell should provide libghostty-vt via LD_LIBRARY_PATH"
    );
    panic!("Missing libghostty-vt");
  }

  println!(
    "cargo:rustc-link-search=native={}",
    ghostty_lib_path.display()
  );
  println!("cargo:rustc-link-lib=ghostty-vt");

  // Set LD_LIBRARY_PATH for runtime
  println!(
    "cargo:rustc-env=LD_LIBRARY_PATH={}:{}:$LD_LIBRARY_PATH",
    wrapper_lib_path.display(),
    ghostty_lib_path.display()
  );

  // Rerun if wrapper changes
  println!("cargo:rerun-if-changed={}/wrapper.zig", cargo_dir);
  println!("cargo:rerun-if-changed={}/build.zig", cargo_dir);
  println!(
    "cargo:rerun-if-changed={}/zig-out/lib/libghostty_wrapper.a",
    cargo_manifest_dir.display()
  );
}
