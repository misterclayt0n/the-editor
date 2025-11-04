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

  // Find libghostty-vt from:
  // 1. Vendored pre-built libraries (checked into repo)
  // 2. LD_LIBRARY_PATH (set by Nix dev shell)
  let vendored_path = cargo_manifest_dir.join("vendored/linux-x86_64");

  let ghostty_lib_path = if vendored_path.join("libghostty-vt.so").exists() {
    // Use vendored library (default for most users)
    vendored_path
  } else if let Ok(ld_lib_path) = std::env::var("LD_LIBRARY_PATH") {
    // Try to find libghostty-vt in LD_LIBRARY_PATH (Nix users)
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
        eprintln!("ERROR: libghostty-vt not found in LD_LIBRARY_PATH");
        eprintln!("Searched paths: {}", ld_lib_path);
        panic!("Missing libghostty-vt");
      })
  } else {
    eprintln!("ERROR: libghostty-vt not found");
    eprintln!("Expected location: {}", vendored_path.display());
    eprintln!("\nThis shouldn't happen if you cloned from git.");
    eprintln!("The vendored library should be checked into the repository.");
    panic!("Missing vendored libghostty-vt");
  };

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
