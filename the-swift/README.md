# the-swift POC

This is a macOS Swift/AppKit proof-of-concept client for `the-editor`.

## Build

From the repo root:

```bash
./scripts/build-the-editor-ffi-xcframework.sh
cd the-swift
swift run TheSwiftPOC -- ../README.md
```

The Rust side is packaged as a local `TheEditorFFI.xcframework`, mirroring the Ghostty-style Swift ↔ native packaging shape.
