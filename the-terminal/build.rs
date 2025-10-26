use std::path::PathBuf;

fn main() {
    let cargo_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

    let cargo_manifest_dir = PathBuf::from(&cargo_dir);

    // Link against pre-built wrapper library
    // Assumes: zig build has been run in the-terminal directory
    let wrapper_lib_path = cargo_manifest_dir.join("zig-out/lib");
    if !wrapper_lib_path.exists() {
        eprintln!("ERROR: Pre-built wrapper library not found at: {}", wrapper_lib_path.display());
        eprintln!("Please run: cd {} && zig build", cargo_manifest_dir.display());
        panic!("Missing pre-built ghostty wrapper library");
    }

    println!("cargo:rustc-link-search=native={}", wrapper_lib_path.display());
    println!("cargo:rustc-link-lib=static=ghostty_wrapper");

    // Link against pre-built libghostty-vt from ghostty project
    let ghostty_dir = cargo_manifest_dir
        .parent()
        .expect("Invalid cargo dir")
        .parent()
        .expect("Invalid cargo dir")
        .join("ghostty");
    let ghostty_lib_path = ghostty_dir.join("zig-out/lib");

    if !ghostty_lib_path.exists() {
        eprintln!("ERROR: libghostty-vt not found at: {}", ghostty_lib_path.display());
        eprintln!("Please build ghostty from: {}", ghostty_dir.display());
        panic!("Missing libghostty-vt");
    }

    println!("cargo:rustc-link-search=native={}", ghostty_lib_path.display());
    println!("cargo:rustc-link-lib=ghostty-vt");

    // Set LD_LIBRARY_PATH for runtime
    println!("cargo:rustc-env=LD_LIBRARY_PATH={}:{}:$LD_LIBRARY_PATH",
        wrapper_lib_path.display(),
        ghostty_lib_path.display()
    );

    // Rerun if wrapper changes
    println!("cargo:rerun-if-changed={}/wrapper.zig", cargo_dir);
    println!("cargo:rerun-if-changed={}/build.zig", cargo_dir);
}
