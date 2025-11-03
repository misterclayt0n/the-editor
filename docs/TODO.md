### TODO
So, this is just a simple todo list that will grant me some sort of base for the editor's missing features overall. Note that this document represents my personal vision on what the editor shall become, but will likely change over time.

#### Alpha release 
- [ ] Remove ACP integration
  - The reason for this is that we are embedding ghostty, which let's us better integrate cli tools (which are mostly common in vibe coding) inside the editor. This is a simpler/more composable approach overall, so ACP is not needed anymore.
- [ ] Image rendering.
- [x] Local search in buffer (https://x.com/neogoose_btw/status/1981778716721393859/photo/1).
- [ ] End of line diagnostics.
- [ ] Buffer line
- [ ] VCS
  - [ ] First class jujutsu support (jujutsu will have priority over git).
  - [ ] Git blame (this will likely only support git btw).
- [ ] Configuration
  - [ ] Custom font loading.
- [ ] Embed ghostty
  - [x] Basic terminal rendering.
  - [x] Toggle terminal.
  - [x] Maintain terminal render state.
  - [x] Improve performance.
  - [ ] Add custom theme support - Still don't know if I pull ghostty theme from the user's ghostty config or if it should be custom. Probably custom since we're not really embedding all of ghostty defaults.
  - [x] Focus keybindings.
  - [ ] Investigate overall performance (quick benchmark - type `ps` a couple of times and then `clear` the screen).
  - [ ] Floating terminal panel.
  - [ ] Render different cursor shapes in the terminal.
  - [ ] Cursor animations? Like on the rest of the editor?
- [ ] File tree.
- [ ] Command palette.
- [ ] "reload-all" prompt command.
- [ ] Add command history in all prompts.

When it comes to ghostty, `libghostty` is not stable as an API yet, and the correct decision would be to wait until it's stable, but sometimes the heart speaks for itself you know.

#### Beyond alpha
- [ ] File watcher (I'll wait to see helix's implementation on this, they're making big advancements: https://github.com/helix-editor/helix/pull/14544).
