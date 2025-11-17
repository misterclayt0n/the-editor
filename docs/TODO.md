### TODO
So, this is just a simple todo list that will grant me some sort of base for the editor's missing features overall. Note that this document represents my personal vision on what the editor shall become, but will likely change over time.

#### 
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
- [ ] File tree.
- [ ] Command palette.
- [x] "reload-all" prompt command.
- [ ] Add command history in all prompts - Thinking about this, maybe we don't need command history in the regular prompt, as we need in the shell, search, select (and similar). The main idea is that we don't want to have to repeat typing in commands used often, and that's not really the prompt itself (most commands have aliases to them, so why do we need history in those cases?). That seems overall reasonable.
  - [ ] Add `:repeat-last-shell` command - This should repeat the last executed shell command.
  - [ ] Render history as if it werer a completion in shell, search, keep and similar commands.
- [x] Add paths in the completer.
- [x] Different cursor shapes.
- [x] Reimagine the compile command:
  - [x] Kill processes.
  - [x] Don't account for compile commad buffers in the save interface.
  - [x] Syntax highlighting? Maybe shell?
- [ ] Workspace features - Still reasoning about this.

#### Fixes
- [ ] Render fonts correctly in the picker prompt menu (they're too spaced).
- [ ] Fix panic in prompt because it can't handle unicode properly.
- [ ] Render tabs in hover component.
- [ ] Fix indent guides (sometimes it will be rendered as blank for some reason).
- [x] Some lsp diagnostics are not really being rendered into virtual lines.
- [x] Fix overflow in lsp helper completer popup.
- [x] Our code for handling lsp positioning (completer, signature helper, hover) does not work at all when we resize the-editor's window. It kinda just appears out of the blue.
- [x] We've stopped rendering multiple cursors after giving support for different cursor shapes.
- [x] Fix x11 scrolling (constant flickering).
- [x] Review how completer "accept" works - It does not automatically import files for instance, it also for some reason inserts the arguments of the function, and that's not ok.
- [x] Highlight on yank should grab color from theme config.
- [x] LSP diagnostics sometimes do not update correctly.

### Beyond
- [ ] File watcher (I'll wait to see helix's implementation on this, they're making big advancements: https://github.com/helix-editor/helix/pull/14544).
