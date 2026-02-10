# TODO
- [ ] First class jj support
- [ ] File watching (this may take a while to get nice)
- [ ] Render selection on other pairs (like in flow/vscode/zed when I select something and the rest kinda get's selected as well, but we can probably skip one char selection for this I suppose)
- [ ] Line wrap
- [ ] TODO 1 — syntax: deterministic simulation harness
  - [ ] Seeded edit streams on fixed corpora with reproducible failures
  - [ ] Invariant checks for parse tree + highlight range integrity
- [ ] TODO 2 — syntax: differential oracle checks
  - [ ] Compare incremental updates vs fresh `Syntax::new` parses
  - [ ] Assert equivalent highlight/range results on sampled windows
- [ ] TODO 3 — syntax: async interleaving simulation
  - [ ] Deterministic timeout/delay/out-of-order parse completion scenarios
  - [ ] Assert latest-result-only apply and version/cache monotonicity
- [x] TODO 4 — syntax: crash-focused fuzzing
  - [x] Add `cargo-fuzz` targets for edit/update/interpolate pipelines
  - [x] Run with sanitizers and keep deterministic repro inputs
- [x] TODO 5 — syntax: client stress suites
  - [x] Long-running headless edit/render loops for `the-term` and `the-ffi`
  - [x] Fixture matrix (rust/markdown/toml/nix + unicode-heavy text)

### Swift
- [ ] Correct mouse support (scrolling and also selecting stuff with the cursor)
- [x] Refactor a bit of the statusline (show file icons, full file path)
- [ ] Figure out correct theming
- [ ] Gutter line (line numbers, VCS stuff)
- [ ] Render LSP completer
- [ ] Render '\n' selection
- [ ] Render multiple cursor formats (bar, block)

### Terminal
- [ ] Correct mouse support (scrolling and also selecting stuff with the cursor)
- [ ] Gutter line (line numbers, VCS stuff)
- [ ] Render LSP completer
- [x] Render file icons in the statusline (flow editor shouuld be a good inspiration here)
- [ ] Render '\n' selection
- [ ] Render multiple cursor formats (bar, underline, block)
 
