# the-editor pi extension

This is the standalone `pi` extension package for the `the-editor` bridge.

What it does:

- auto-attaches to a running `the-editor` instance in the same workspace
- updates the live `pi` editor when `the-editor` sends a selection prefill
- sends a real user message when `the-editor` sends a selection directly
- routes workspace text `read`, `edit`, and `write` operations through `the-editor`
- falls back to normal `pi` behavior for non-workspace paths and likely-binary files

Install it from this repo with:

```bash
pi install /Users/misterclayt0n/code/the-editor/the-editor-pi-extension
```

Then run `the-editor` and `pi` from the same workspace. The extension will auto-attach when it finds `.the-editor/pi-bridge.json`.
In Git workspaces, `the-editor` now writes the manifest under `.git/the-editor/pi-bridge.json` instead, with `.the-editor/pi-bridge.json` kept only as a fallback for non-Git directories.

Useful `pi` commands:

- `/the-editor-status`
- `/the-editor-reconnect`
