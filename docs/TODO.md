### TODO
So, this is just a simple todo list that will grant me some sort of base for the editor's missing features overall. Note that this document represents my personal vision on what the editor shall become, but will likely change over time.

#### Alpha release 
- [ ] Full ACP integration (tho I'm not sure what that means yet).
  - [ ] Remove ACP gutter (no reason for it to exist).
- [ ] Image rendering.
- [ ] Local search in buffer (https://x.com/neogoose_btw/status/1981778716721393859/photo/1).
- [ ] End of line diagnostics.
- [ ] Buffer line .
- [ ] VCS .
  - [ ] First class jujutsu support (jujutsu will have priority over git).
  - [ ] Git blame (this will likely only support git btw).
- [ ] Configuration:.
  - [ ] Custom font loading.
- [ ] Embed ghostty? .

When it comes to ghostty, `libghostty` is not stable as an API yet, and the correct decision would be to wait until it's stable, but sometimes the heart speaks for itself you know.

#### Beyond alpha
- [ ] File watcher (I'll wait to see helix's implementation on this, they're making big advancements: https://github.com/helix-editor/helix/pull/14544).
