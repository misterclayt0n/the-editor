# Swift Demo (macOS)

Minimal SwiftUI demo that calls into `the-ffi` and draws a render plan.

## Prereqs
- macOS with Swift 5.9+
- Rust toolchain

## Build steps
1. Build the xcframework, sync bridge files, and run the demo:
   ```bash
   ./scripts/macos.nu
   ```

If you prefer to call NuShell explicitly:
```bash
nu ./scripts/macos.nu
```

If SwiftPM fails with `SwiftShims` or module cache errors:
- If `xcode-select` points to CommandLineTools and you don't have Xcode installed,
  install Xcode (App Store) or update CLT:
  ```bash
  xcode-select -p
  sudo softwareupdate --all --install --force
  # or reinstall CLT:
  sudo rm -rf /Library/Developer/CommandLineTools
  xcode-select --install
  ```
- Ensure you are using a matching Xcode toolchain:
  ```bash
  sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
  ```
- Point the module cache somewhere writable:
  ```bash
  export SWIFT_MODULE_CACHE_PATH=/tmp/swift-module-cache
  mkdir -p "$SWIFT_MODULE_CACHE_PATH"
  ```

The demo renders a simple buffer using the render plan output and draws
spans, selections, and cursors using a fixed monospaced cell size.
