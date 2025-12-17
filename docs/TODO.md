### TODO
So, this is just a simple todo list that will grant me some sort of base for the editor's missing features overall. Note that this document represents my personal vision on what the editor shall become, but will likely change over time.

- [ ] Image rendering.
- [x] Local search in buffer (https://x.com/neogoose_btw/status/1981778716721393859/photo/1).
- [x] End of line diagnostics.
- [x] Buffer line

- [ ] VCS
  - [ ] First class jujutsu support (jujutsu will have priority over git).
  - [ ] Git blame (this will likely only support git btw).
  
- [ ] Configuration
  - [ ] Custom font loading.
  - [x] Full helix config engine support.
  
- [ ] File tree:
  - [x] File icons.
  - [x] File tree at the right.
  - [x] Basic configs.
  - [x] File explorer should remember the layout.
  - [x] File tree is not really following the current buffer correctly.
  - [x] Fix scrolling (it's not working on trackpad). 
  - [x] VCS indicator.
  - [x] Explorer should react to the open file - Auto reveal.
  - [x] Basic file operations -> Create/Delete files/folders.
  
- [ ] Command palette.
- [x] "reload-all" prompt command.

- [x] Add command history in all prompts - Thinking about this, maybe we don't need command history in the regular prompt, as we need in the shell, search, select (and similar). The main idea is that we don't want to have to repeat typing in commands used often, and that's not really the prompt itself (most commands have aliases to them, so why do we need history in those cases?). That seems overall reasonable.
- [x] Add `:repeat-last-shell` command - This should repeat the last executed shell command.
- [x] Render history as if it were a completion in shell, search, keep and similar commands.
- [ ] Add workspace commands.
  
- [x] Add paths in the completer.
- [x] Different cursor shapes.

- [ ] Scrolling should not depend on me having to click to focus. Just hovering my cursor in the correct component should scroll it.

- [x] Reimagine the compile command:
  - [x] Kill processes.
  - [x] Don't account for compile commad buffers in the save interface.
  - [x] Syntax highlighting? Maybe shell?
  
- [ ] Workspace features - Still reasoning about this.
- [ ] Tab completion in the file prompt should look for the correct directory entry, not just scroll (This is a special case tho, the behavior is correct for all other cases).

- [ ] Terminal
  - [x] Basic stuff
  - [x] Mouse handling

- [x] ACP Integration - We're pausing this for now, I think we're at a reasonable state currently.
  - [x] Model picker.
  - [x] Hover that streams agents response.
  - [x] Permission prompt -> should be something similar to a completion/code-action.
  - [x] Render tool calls -> Like, show what's being actually executed by the tool call.
  - [x] Command to interrupt agent.
  - [x] Fix the issue where every time I save a file it gets autoreloaded.
  - [x] Render TODOs
  - [ ] Opencode shell comamnds are not happening in the same nix enviroment as the one the editor is currently set.

- [ ] Since we now have a lot of special buffers, we need a system to easily toggle them, maybe in an overlay. We should be able to bind arbitrary buffers to this system tho, not just special buffers.

Now I kinda want this editor to look nice so here's some ideas:
- [ ] Cursor breathing animation.
- [ ] Render jumplist picker.
- [ ] Improve the window splitting animation (it currently sucks).
- [ ] File tree on click animation.
- [ ] File open should have a totally different animation - already open files should have one animation (if any) and recently open files should have another one (see raddebugger's animation on this)
  
### Fixes
- [x] Render fonts correctly in the picker prompt menu (they're too spaced).
- [x] Fix panic in prompt because it can't handle unicode properly.
- [x] Render tabs in hover component.
- [ ] Fix indent guides (sometimes it will be rendered as blank for some reason). - Hard to reproduce tho.
- [x] Some lsp diagnostics are not really being rendered into virtual lines.
- [x] Fix overflow in lsp helper completer popup.
- [x] Our code for handling lsp positioning (completer, signature helper, hover) does not work at all when we resize the-editor's window. It kinda just appears out of the blue.
- [x] We've stopped rendering multiple cursors after giving support for different cursor shapes.
- [x] Fix x11 scrolling (constant flickering).
- [x] Review how completer "accept" works - It does not automatically import files for instance, it also for some reason inserts the arguments of the function, and that's not ok.
- [x] Highlight on yank should grab color from theme config.
- [x] LSP diagnostics sometimes do not update correctly.
- [ ] Fix an issue regarding the touchpad scrolling (wayland in particular, simple way to reproduce this is block the screen and unlock it afterwards)
- [ ] Correctly stream in shell command (force redraw on every stream).
- [x] Freeze on completer happens a lot, also hard to reproduce tho. "Kinda" fixed I guess
- [ ] Fix cursor alignment on lsp components to account for bufferline when rendered.

### Beyond
- [x] File watcher (I'll wait to see helix's implementation on this, they're making big advancements: https://github.com/helix-editor/helix/pull/14544).
- [ ] JJ diff editor.
