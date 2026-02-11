# TODO
- [ ] First class jj support
- [x] File watching (this may take a while to get nice) - Only thing left is better testing for this feature
- [x] File watch verification:
- [x] Clean buffer + external edit => auto reload, cursor/scroll preserved semantically.
- [x] Dirty buffer + external edit => no overwrite, explicit conflict/warning state.
- [x] Two rapid external edits => final content matches disk, no stale intermediate state.
- [x] Save self-event suppression window => no false reload after own save.
- [x] Watcher disconnect/recreate => watcher rebinds and continues processing.
- [x] Missing file then create => parent fallback path correctly triggers load.
- [x] Atomic save (temp file + rename) => reloads latest disk content.
- [x] Scope decision: keep `active-document` watch policy for now.
- [ ] Render selection on other pairs (like in flow/vscode/zed when I select something and the rest kinda get's selected as well, but we can probably skip one char selection for this I suppose)
- [x] Line wrap

### Swift
- [ ] Correct mouse support (scrolling and also selecting stuff with the cursor)
- [x] Refactor a bit of the statusline (show file icons, full file path)
- [ ] Figure out correct theming
- [x] Gutter line (line numbers, VCS stuff)
- [ ] Render LSP completer
- [x] Render '\n' selection
- [ ] Render multiple cursor formats (bar, block)

### Terminal
- [ ] Correct mouse support (scrolling and also selecting stuff with the cursor)
- [x] Gutter line (line numbers, VCS stuff)
- [ ] Render LSP completer
- [x] Render file icons in the statusline (flow editor shouuld be a good inspiration here)
- [x] Render '\n' selection
- [ ] Render multiple cursor formats (bar, underline, block)
 
