# Capability Lab

A mock extension for stress-testing the newest workbench seams.

## What it covers

- `editor.workbench.invalidate(...)`
- runtime-backed tool snapshots
- JS-rendered buffer decorations
- host-owned focus routing
- tool-scope keymaps
- swappable role implementations
- diagnostics flowing into the inspector
- reload restore for open tools/focus

## Install

Copy this folder to:

- `.the-editor/extensions/capability-lab`
- or `~/.the-editor/extensions/capability-lab`

Then restart Electron or run:

```txt
:reload capability-lab
```

## Try it

1. Run `Extensions: Open Capability Lab` from the palette.
2. Click **start ticker**.
   - the panel state should update every second
   - the custom statusline should update if selected
   - the in-buffer highlight / ghost text should move without more input
3. Click **use capability statusline**.
4. Click **focus tool** and try these keys:
   - `g` toggle ghost text
   - `h` toggle highlight
   - `t` start/stop ticker
   - `n` increment counter
   - `r` reset counter
   - `e` focus editor
   - `:` open the command palette
5. Click **report diagnostic** and confirm it appears in the workbench inspector.
6. Run `:reload capability-lab/runtime` while the tool is open.
   - the tool should reattach
   - focus should restore if possible
   - the counter/toggle state should survive because it lives in host state slots
   - the ticker intentionally stops during reload cleanup and can be started again

## Notes

- Decorations are intentionally anchored to the current visible frame.
- This is a mock extension, not a polished product feature.
