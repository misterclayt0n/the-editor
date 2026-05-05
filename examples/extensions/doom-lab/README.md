# Doom Lab

A minimal extension example that mounts Doom as an editor tool.

## What it does

- contributes a `runtime` target
- contributes a `ui` target
- registers a singleton overlay tool
- adds `Extensions: Open Doom` to the command palette
- embeds shareware Doom in an iframe

## Install

Copy this folder to:

- `.the-editor/extensions/doom-lab`
- or `~/.the-editor/extensions/doom-lab`

Then restart Electron or run `:reload doom-lab`.

## Notes

- This example uses `https://js-dos.com/games/doom.exe.html`.
- If that URL stops working, edit `ui.tsx` and replace `doomUrl`.
- Keyboard focus belongs to the iframe after you click inside it.
- This is intentionally simple. A more serious version would use a dedicated tool focus/input seam and probably a local WASM port.
