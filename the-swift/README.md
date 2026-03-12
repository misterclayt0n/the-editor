# Swift App (macOS)

SwiftUI macOS app that packages `the-ffi` through an xcframework and builds a real `.app`
bundle through Xcode.

## Prereqs
- macOS with Swift 5.9+
- Rust toolchain

## Build steps
1. Build `the-ffi`, regenerate `TheEditorFFI.xcframework`, build the Xcode app, and open it:
   ```bash
   ./scripts/macos.nu
   ```

If you prefer to call NuShell explicitly:
```bash
nu ./scripts/macos.nu
```

Useful variants:

```bash
# Build the Debug app bundle without launching it
nu ./scripts/macos.nu --build-only

# Build the ReleaseLocal app bundle without launching it
nu ./scripts/macos.nu --release --build-only

# Archive a Release build
nu ./scripts/macos.nu --archive

# Export a local macOS app bundle from the Release archive
nu ./scripts/macos.nu --export

# Build a drag-installable local DMG from the ReleaseLocal app
nu ./scripts/macos.nu --dmg

# Fall back to the SwiftPM executable path
THE_EDITOR_MACOS_LAUNCH_MODE=swift-run nu ./scripts/macos.nu
```

The default Xcode build outputs the app at:

```bash
/tmp/the-editor-xcode-derived/Build/Products/Debug/TheEditor.app
```

ReleaseLocal builds land at:

```bash
/tmp/the-editor-xcode-derived/Build/Products/ReleaseLocal/TheEditor.app
```

Release archives land at:

```bash
/tmp/the-editor-xcode-archive/TheEditor.xcarchive
```

Local exported apps land at:

```bash
/tmp/the-editor-xcode-export/TheEditor.app
```

Local DMGs land at:

```bash
dist/TheEditor.dmg
```

The distribution flow is:
- `--archive`: build or refresh the Release `.xcarchive`
- `--export`: archive if needed, then export a local `.app` from that archive
- `--dmg`: build the optimized `ReleaseLocal` app directly, then package it into a drag-installable DMG for unsigned/ad-hoc local distribution

By default the export path uses Xcode's `mac-application` export method, which works for local
ad-hoc packaging. The script is also ready for future Developer ID distribution via environment
overrides:

```bash
THE_EDITOR_MACOS_EXPORT_METHOD=developer-id
THE_EDITOR_MACOS_EXPORT_SIGNING_STYLE=automatic
THE_EDITOR_MACOS_EXPORT_TEAM_ID=YOURTEAMID
THE_EDITOR_MACOS_ALLOW_PROVISIONING_UPDATES=1
```

Useful override paths:

```bash
THE_EDITOR_MACOS_EXPORT_PATH=/tmp/the-editor-export
THE_EDITOR_MACOS_DMG_PATH=/tmp/TheEditor-local.dmg
THE_EDITOR_MACOS_EXPORT_OPTIONS_PLIST=/path/to/exportOptions.plist
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
