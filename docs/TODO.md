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
- [ ] File tree.
- [ ] Command palette.
- [x] "reload-all" prompt command.
- [ ] Add command history in all prompts.
- [x] Add paths in the completer.
- [x] Different cursor shapes.
- [ ] Reimagine the compile command:
  - [ ] Kill processes.
  - [ ] Don't account for compile commad buffers in the save interface.
  = [ ] Syntax highlighting? Maybe shell?

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

### Beyond
- [ ] File watcher (I'll wait to see helix's implementation on this, they're making big advancements: https://github.com/helix-editor/helix/pull/14544).
