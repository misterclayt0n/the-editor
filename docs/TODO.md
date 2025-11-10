### TODO
So, this is just a simple todo list that will grant me some sort of base for the editor's missing features overall. Note that this document represents my personal vision on what the editor shall become, but will likely change over time.

#### 
- [x] Remove ACP integration - The reason for this is that we are embedding ghostty, which let's us better integrate cli tools (which ar e mostly common in vibe coding) inside the editor. This is a simpler/more composable approach overall, so ACP is not needed anymore.
- [ ] Image rendering.
- [x] Local search in buffer (https://x.com/neogoose_btw/status/1981778716721393859/photo/1).
- [ ] End of line diagnostics.
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

#### Fixes
- [ ] Render fonts correctly in the picker prompt menu (they're too spaced).

When it comes to ghostty, `libghostty` is not stable as an API yet, and the correct decision would be to wait until it's stable, but sometimes the heart speaks for itself you know.

### Beyond
- [ ] File watcher (I'll wait to see helix's implementation on this, they're making big advancements: https://github.com/helix-editor/helix/pull/14544).
