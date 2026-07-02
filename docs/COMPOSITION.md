# Composition Refactor

The goal: **the-editor is just a default set of packages assembled together; a user
can build their own.** A feature is "correct" under this model when it could be built
by a user extension using only public seams. Most of the editor should pass that test;
a few things (the raw text engine, the kernel itself) are allowed not to.

This is the working checklist. Revised 2026-07 after a second audit; the structural
finding that anchors everything below:

> Extensions and first-party packages activate through the **identical** `services`
> layer (`apps/web/src/runtime.ts` `boot()`). There is no second-class capability
> tier. The only gate is the import allowlist in `buildExtensionVirtualModules()`
> (`apps/web/src/product/userExtensions.ts`) — a capability is extension-reachable
> exactly when its `Context.Tag` class can be *named* from an allowlisted module.
> "Relocating" a seam is therefore usually just moving a tag; the deep items are the
> ones where the seam's *shape* is wrong (single-slot, AppRuntime-coupled, or absent).

---

## The running metrics

1. **`Host*` tag count → 0.** Every `Context.Tag("the-editor/web/Host…")` is a spot
   where a package reaches a privileged `AppRuntime` method instead of a public
   operation. **Currently: 3.**
   ```
   HostFileTree  HostTerminal  HostDiffView
   ```
2. **Every tag in the `services` layer is either nameable from the allowlist or on an
   explicit host-only list.** Sharper than the verb table: the layer already contains
   everything; reachability is purely tag location. Tags that stay host-only
   (capability/trust decisions) are fine — but the list must be deliberate, not
   accidental.
3. **Enforced, not asserted** (see section G): a lint rule + a load-through-the-
   extension-path test make regressions impossible.

---

## Done

- [x] **Dynamic extension loading.** Global (`~/.the-editor/extensions`) + project
  (`.the-editor/extensions`, trust-gated) packages via `esbuild-wasm`, reconciled
  through the kernel's `activate/reload/disposePackage` with per-extension fault
  isolation, serialized by one semaphore.
- [x] **One `reload` command** — the single funnel for all disk resources.
- [x] **Themes + fonts.** A theme is just an extension; registers AND switches.
- [x] **Decorations.** `DecorationProviders`/`EditorVisuals` in `@the-editor/editor`,
  allowlisted, RangeSet-mapped through edits. The template every relocate copies.
- [x] **Command execute/introspect.** `Commands.execute`/`executeIn`/`list`/
  `listAvailable` are public and allowlisted. The command registry is a
  self-describing operation surface — this is the spine of the agent-drive story.
- [x] **Keybinding replacement.** Resolver ties go to later registration
  (`resolver.ts` uses `>=`) and user extensions activate last, so user bindings
  shadow defaults by construction. No advice mechanism needed for the common case.
- [x] **Full editing loop.** Editor-scoped command → panel context provides
  `TextDocument`/`SelectionState` → engine `Transaction` builders →
  `commitTransaction`. Multi-cursor included: `Selection.addRange` is public.
  "An agent spawns a second cursor" is representable today.

---

## The verb table (corrected)

| # | Verb | Status |
|---|------|--------|
| 1 | Add a command / keybinding (incl. execute + introspect + override) | ✅ `Commands`, `Keymaps` |
| 2 | Edit a buffer / move + add cursors | ✅ engine + `commitTransaction` |
| 3 | Decorate a buffer | ✅ `DecorationProviders` |
| 4 | Style its own surface / ship a theme | ✅ scoped CSS vars + `Themes`/`Fonts` |
| 5 | Manage panes/tabs/splits/focus | ✅ `Workbench` + `surgery` |
| 6 | Hold / react to state | ✅ `context.stores` + public `makeStateCell`/`makeRegistry`/topic fabric via `@the-editor/extensions` |
| 7 | Ask / notify the user | ✅ `Attention` + public choice-request vocabulary via `@the-editor/extensions`; malformed requests fail fast |
| 8 | Render a pane (agent panel, browser, Doom) | ✅ `PanelRenderers` via `@the-editor/workbench` |
| 9 | Answer questions about a buffer (completion/hover/signature) | ✅ provider registries via `@the-editor/intel` |
| 10 | Open a file into a visible view | ✅ `Files` + `FileViews.openInView` (C1) |
| 11 | Pick from a list (completing-read) | ✅ `Picker` via `@the-editor/picker`; one windowed host, N sources — all five surfaces converted |
| 12 | Add statusline / context-menu items | ✅ `StatuslineSegments` + `ContextMenuProviders` in `@the-editor/workbench` (E1/E2) |
| 13 | Own a background process | ✅ `Processes` / `Ptys` via `@the-editor/process`; Scope-bound handles, killed on dispose |
| 14 | React to interaction (hover/scroll/viewport/pre-key) | ✅ multiplexed hub on the view context; extensions subscribe via `DecorationViewTarget.interaction` in `attachView` (D2) |
| 15 | Back a buffer with non-file content | ✅ `VirtualViews.open` + `DocumentPolicy` (read-only → edits become operations) (D3) |
| 16 | Contribute a language server / grammar | 🟡 `LanguageServers` public via `@the-editor/intel`; grammars native `.so` (D1) |
| 17 | Undo / redo | 🟡 engine `history` is pure data; not a public verb (F4) |
| 18 | Be driven from outside (CLI/socket/MCP) | ❌ no inbound channel exists at all (F1) |
| 19 | Replace the interaction model (Vim/Helix/Emacs) | ❓ undecided — no mode state machine (F3) |
| 20 | Opt OUT of / replace a default (not just add) | ✅ default set is a manifest; `~/.the-editor/config.ts` `disable`s a swappable package (I1) |

---

## A. Days of work, flips the scoreboard — do first

- [x] **A1. Expose `Attention`.** Done by narrow value re-export from
  `@the-editor/extensions`: `Attention`, `makeNoticeTopic`, `makeRequestTopic`,
  and the typed Attention errors. The public request vocabulary is the exported
  `ChoiceRequest`/`ChoiceTopic` shape; `AttentionLayer` declines non-choice
  requests so malformed dynamic asks fail with `AttentionUnhandledError` instead
  of hanging. Proven through the user-extension load path with a global extension
  that `yield* Attention`, sends a notice, asks a choice request, and receives
  the answer.
- [x] **A2. Decide kernel value exports.** Done by narrow value re-export from
  `@the-editor/extensions`: `makeStateCell`, `makeRegistry`, topic makers, and
  the id constructors needed to mint registry contributions. Kernel itself stays
  off the extension import allowlist (see H3).

## B. Relocate a tag (cheap, low-risk)

- [x] **B1. Publish `PanelRenderers`.** Moved from `apps/web/src/workbench/panelRenderers.ts`
  into allowlisted `@the-editor/workbench`. The registry is `HTMLElement`-based and
  framework-agnostic: an *extension* panel mounts its own DOM and needs nothing from
  `AppRuntime`, so arbitrary extension panes are now nameable. **Follow-up still
  separate:** first-party renderers (`filetree/mount.ts`, `terminal/mount.ts`,
  `diffview/mount.ts`) still take `AppRuntime` directly; dogfooding them onto a
  bounded panel-host API remains G debt.
- [x] **B2. Publish intel providers.** `CompletionProviders`/`HoverProviders`/
  `SignatureProviders` now live in allowlisted `@the-editor/intel`. They remain pure
  Ref-backed registries with scoped retraction; LSP registers as `id:"lsp"` among equals.
- [x] **B3. Publish the thin registries.** `LanguageServers` lives in
  `@the-editor/intel`, `VcsProviders` in `@the-editor/vcs`, and
  `DiffViewPreferences` in `@the-editor/diff-view`; all are allowlisted.
  `VcsProviders`/`DiffViewPreferences` are pure registry moves. `LanguageServers`
  is declarative but process-spawn-by-proxy: an extension can register a
  `{ command, languages }` entry that `LspManager` later starts through the host
  `ProcessPort` when a matching file opens. This fits the trusted-local extension
  model, but D1 must name this existing reachability instead of rediscovering it.
- [x] **B4. Expose `MountHub`/`FloatHub`.** The classes and live per-view
  instances now live on the public `@the-editor/editor` surface. `makeViewScope`
  installs `ViewMountHub`/`ViewFloatHub` in every editor view context, and
  `DecorationProviders` has a view-scoped `attachView` hook with a curated
  `DecorationViewTarget` (`mounts`, `floats`, view notifications). `AppRuntime`
  connects every editor pane to that driver, so a package can reserve rows via
  `EditorVisuals` and fill each renderer-owned mount container when it appears;
  floats are reachable from the same per-view target. Proven through the real
  user-extension loader path.

## C. Operations (retire `Host*` tags)

- [x] **C1. Open-file-into-view op.** `Files` is now in the package services
  layer when a filesystem exists, and `FileViews.openInView(path)` is the public
  visible-open operation. `Files.openPath` remains disk → document binding;
  `FileViews` is host-implemented because rendered tabs include editor surface
  vocabulary, pane creation, placement, and focus policy. Boot creates the first
  editor group before package activation, so even an extension activated from the
  loader path can call `openInView` immediately and get a focused rendered tab.
- [x] **C2. Actions as operations.** The old `Host*Actions` tags for files,
  workspaces, views, palette, themes, and LSP commands are gone. Their public
  replacements live in allowlisted `@the-editor/editor` as synchronous intent
  surfaces:
  - [x] `FileOperations` → `save` / `open` / `find` / `findInProject`
  - [x] `WorkspaceOperations` → `openFolder` / `openSwitcher` / `closeWorkspace`
  - [x] `ViewOperations` → `splitRight` / `splitDown` / `closeTab` / `nextTab` / `previousTab` / `toggleTabs`
  - [x] `PaletteOperations` / `ThemeOperations` were an interim bridge; C3
    retired both (the palette/theme packages call `Picker` directly).
  - [x] `LspOperations` replaces `HostLspActions` for command-level triggers;
    D1 still owns process capability publication.
- [x] **C3. Picker as an operation.** `@the-editor/picker` (allowlisted):
  data-driven `PickerItem`s, a `PickerSource` protocol (debounce, `preRanked`,
  sync-or-async answers), and one `Picker.open(request)` service. The
  controller owns the lifecycle every bespoke surface used to hand-roll —
  debounce, generation-counter staleness, tiered ranking (stable sort keeps
  group clusters), select-first-on-fresh-results, live `onSelectionChange`
  with choose/dismiss semantics (the theme preview transaction, generalized),
  and `refresh` for sources whose data mutates under an open picker. One
  `PickerHost` in the shell renders every request over `VirtualCommand`
  (windowed rows, cmdk removed) with the shared file-preview pane.
  Converted callers: command palette + theme selector are now **pure package
  code on public seams** (`Commands`/`Themes` + `Picker` — a user extension
  could ship those exact files); the file finder is a host-built fff source
  behind `FileOperations.find`; the workspace switcher is an App-built source
  (App owns sessions) using the `renderItem` escape hatch for its inline close
  button. Retired: `CommandPalette.tsx`, `FileFinder.tsx`,
  `PaletteOperations`, `ThemeOperations`, and the `onPaletteOpenRequested` /
  `onFileFinderOpenRequested` / `onThemeSelectorOpenRequested` host hooks.
  Proven through the loader path: an extension opens a picker with its own
  source, gets ranked items + selection callbacks, and receives the choice.
- [x] **C4. Global search as a picker source.** `GlobalSearch.tsx` is deleted;
  find-in-files is a host-built `PickerRequest` (`pickers/globalSearch.tsx`)
  behind `FileOperations.findInProject`. The item model grew the general
  pieces: `segments` (styled line slices with match marks), `header` rows
  (non-selectable group headings — always survive the filter, never re-sorted,
  skipped by selection/choose), and request-level `actions`
  (`{keys, label, run, closes?}` — Cmd+Enter → multibuffer closes; Alt+M
  cycles grep mode with `closes: false` + `Picker.refresh`, the same live-
  mutation primitive the workspace switcher uses). `emptyText`, `chrome`
  class hooks, and a `renderPreview` escape hatch round out the request; the
  grep preview panel (file window + context fallback) rides `renderPreview`.
  The per-row syntax-highlight concern resolved itself upstream: the ported
  surface already renders result rows as plain text + match marks (per-row
  tree-sitter was removed for cold-start freezes); only the preview pane
  highlights, where one file's cost is acceptable. The last picker host hook
  (`onSearchOpenRequested`) is gone — every picker surface now opens through
  `Picker.open`.

## D. New primitives (real design work)

- [x] **D1. Process ownership.** Shipped as `@the-editor/process` (allowlisted):
  `Processes` (stdout/stderr pipes) + `Ptys` (terminal PTYs), each with a
  **Scope-bound `spawn` returning a handle** — you touch only what you spawned,
  and the acquiring scope closing (package dispose/reload, or an `Effect.scoped`
  block) kills it. `Processes` also has `run(opts) → {code, stdout, stderr}` for
  one-shot consumers. Byte streaming stays plain callbacks; handles buffer
  output until the first listener then go live. Capability-gated: the service is
  always in the layer, but `spawn`/`run` fail `ProcessUnavailableError` when no
  host bridge exists — so a process-using extension activates and degrades at
  the call. The raw window bridges (`platform/process.ts`, `platform/terminal.ts`)
  are now the host-only `ProcessDriver`/`PtyDriver` behind
  `makeProcesses`/`makePtys`. Added `env` (merged over host env) to the spawn
  options + IPC. Refactored consumers: git provider (`vcs/git.ts`) on
  `processes.run`, registered by the `vcsDefaults` package on public seams only
  (retires `HostVcsDefaults` fully); `LspManager` spawns via `Processes` into
  per-client scopes (`HostLspIntel` unblocked, retired in D1b); terminal mount
  spawns via `Ptys` into a per-terminal scope, PTY handles held by the runtime
  for the foreground-process close prompt. Proxy equivalence stands: a
  `LanguageServers` entry ≡ holding `Processes`, kept as-is. The load-path test
  asserts the assertion that matters — reloading an extension kills the process
  it spawned. Host* count 6 → 5. Deferred with the renderer (G/D2): the
  terminal's clipboard + key-routing coupling and `HostTerminal` itself.
  - **Posture: trusted-local; consent at extension load, never per spawn.**
    `EXTENSIONS.md` §20 says extensions run local processes; `extensionTrust`
    already asks "full local code execution?" once per project root; global
    extensions are implicitly trusted; and the B3 `LanguageServers` proxy path
    would bypass any per-spawn prompt anyway. No capability manifests.
  - **The raw ports are the wrong shape to publish.** `ProcessPort` is an
    id-multiplexed global: any holder can `write`/`kill` any id, and nothing
    ties a process to its spawner's lifetime (reload would leak orphans).
  - **The seam: a new `@the-editor/process` package** with two tags,
    `Processes` (stdout/stderr pipes) and `Ptys` (terminal PTYs), whose
    `spawn` is **Scope-bound and returns a handle** — you can only touch what
    you spawned, and package dispose/reload kills its processes (ownership =
    lifetime, same rule as every registry). Byte streaming stays plain
    callbacks (hot-path rule). Capability-gated in the layer like
    `FileSystemPort`. Optional `label` for attribution; add a `run(cmd) →
    {stdout, code}` convenience for run-to-completion consumers (git,
    formatters). The raw window bridges become host-adapter internals on the
    explicit host-only list.
  - **What it retires:** `HostVcsDefaults` fully (git provider rebuilt on
    `Processes` inside the package; a jj provider becomes a droppable user
    extension). `HostTerminal` partially (PTY half → `Ptys`; the renderer's
    clipboard + key-routing coupling is G/D2 debt, not D1's). `HostLspIntel`
    is **unblocked, not retired** — retiring it is LSP-as-a-package (D1b,
    below); D1 swaps `LspManager`'s internals onto the public seam and stops.
  - **Proxy equivalence, written down:** registering a `LanguageServers`
    entry ≡ holding `Processes`. Keep the proxy (lazy, host-timed spawn is
    the better citizen for servers); no extra friction on either path.
  - **Order:** package + adapter + allowlist + loader-path test (asserting
    reload kills the spawn — the assertion that matters) → git provider →
    LspManager internals → terminal PTY half (+ publish clipboard).
- [x] **D1b. `HostLspIntel` retired.** The LSP query surface (completion / hover /
  signature / trigger chars) is now the public `Lsp` tag in `@the-editor/intel`,
  host-implemented from the manager — same relocate pattern C2 used for the action
  tags. The `lspIntel` package consumes `Lsp`; the `the-editor/web/HostLspIntel`
  tag is gone and an extension can query LSP directly. **Deferred (honest):** the
  `LspManager` still physically lives on `AppRuntime` — the deep "manager as pure
  package code / no runtime coupling" rewrite (panes, workspaceRoot getter,
  diagnostics callbacks) remains a separate effort. The Host* tag and
  extension-reachability goals are met; the god-object thinning is not.
  **Reassessed low priority:** every LSP capability is already reachable
  (`LanguageServers`, the intel registries, `Lsp`, `Processes` — an extension can
  add a server, add providers, query LSP, or ship a whole alternative client),
  and LSP contributes zero to the Host* count. The rewrite buys only dogfooding
  purity + `AppRuntime` thinning, is a large unverifiable-without-a-live-server
  change, and is only *meaningful* once I1 lets the LSP package be swapped out at
  all. Sequence it after I1, or leave it as documented debt.
- [x] **D2. Interaction events — multiplexed + extension-reachable.** The
  single-assignment slots are replaced by a fan-out hub (`ViewInteraction` in
  `@the-editor/editor`, `makeInteractionHub`): many subscribers, boolean events
  tried newest-first (later wins, matching the keymap resolver), void events
  broadcast. The hub lives on the **view context** (like `ViewMountHub`/
  `ViewFloatHub`), so `IntelController` is one subscriber, the multibuffer's
  intrinsic nav another, and — the verb-14 payoff — an **extension** subscribes
  through `DecorationViewTarget.interaction` in a decoration provider's
  `attachView` (proven by an editor test: a provider's `onPointerHover` fires on
  dispatch and retracts with the view). `HostIntelActions` retired the C2 way —
  the triggers are the public `IntelActions` tag in `@the-editor/intel`.
- [x] **D3. Virtual buffers.** `VirtualViews.open(spec)` in `@the-editor/editor`
  (allowlisted, host-implemented) opens a provider-backed in-memory document into a
  rendered tab and returns a handle (`setText` / `close`) — the C1 equivalent for a
  non-file doc. `DocumentPolicy` (optional per-document tag) + a commit-layer gate
  make a read-only virtual buffer reject typing at the single chokepoint, so "edits
  become operations": all interaction flows through commands + scoped keymaps while
  the provider drives content through the forced commit path. Proven by an editor
  policy test (typing dropped, forced write lands) and a loader-path test (an
  extension opens + drives a virtual view). REPL-style editable input regions
  (prompt zones) are a further layer, not needed for magit-style status buffers.

## E. Build a registry

- [x] **E1. Statusline contributions.** `StatuslineSegments` in
  `@the-editor/workbench` (allowlisted): pull-based data segments (side +
  priority), recomputed each render — no per-segment subscription. The default
  segments (branch, file+position, diagnostics, pending chords, buffer stats)
  are host-registered in boot reading live runtime state; `App.tsx` renders the
  bar purely from `activeRuntime.statuslineSegments`. An extension adds a segment
  through the same service.
- [x] **E2. Context-menu contributions — rendered by the OS.** Two allowlisted
  seams in `@the-editor/workbench`: `ContextMenuProviders` (the contribution
  registry — providers return entries for a named menu, grouped with separators)
  and `ContextMenus` (the host `open(request, position)` op). The platform draws
  the menu: Electron `Menu.popup` on desktop (new `menu:popup` IPC), a minimal
  plain-DOM `.os-menu` fallback on the web (`platform/menu.ts`) — no component
  library. The terminal's menu is now a registry provider (`terminal/menu.ts`,
  entries as data); its mount dropped the React root + shadcn trigger and pops
  the OS menu from a plain `contextmenu` listener. **shadcn's context menu is
  removed entirely** — `ui/context-menu.tsx`, `TerminalContextMenu.tsx`, and the
  shadcn context-menu preflight/CSS are gone.

## F. Agent-first items (the point of the whole exercise)

- [ ] **F1. External drive bridge.** There is **no inbound channel at all** — no CLI,
  socket, or MCP; Electron IPC is outbound capability plumbing only. The hard part is
  already done: `Commands.execute` + `list` (titles, groups, aliases) is a
  self-describing operation surface. A small bridge (socket or MCP server in the
  Electron main process) forwarding to `Commands.execute` and streaming
  `editorSnapshot` back makes the-editor drivable through its own public seam
  instead of synthetic keystrokes.
- [ ] **F2. Resolve `packages/acp` + `packages/agent`.** `acp/` is an empty
  node_modules-only shell; `agent/` is untracked dist-only dead code (a clean ACP
  session read-model — `foldSessionUpdate`, `makeAgentSession` — with no transport,
  no panel, no imports). Delete both, or resurrect `agent` as source when F-flagship
  starts. Dead packages contradict the premise.
- [ ] **F-flagship. The agent panel is a user extension.** The definition-of-done
  demo: an agent panel living in `~/.the-editor/extensions`, using only allowlisted
  imports — panel rendering (B1), attention (A1), process/transport (D1 or F1),
  commands, decorations. When this works, the premise is proven.
- [ ] **F3. Interaction-model decision.** `docs/EXTENSIONS.md` promises Vim/Helix/
  Emacs replacement via interaction profiles; current `Keymaps` has scopes + chords
  but no mode state machine. Decide: extension-owned mode state + scoped keymaps +
  pre-key interceptor (D2), or a first-class profile seam. Settle it deliberately.
- [ ] **F4. Undo/redo as a public verb.** Engine `history` is pure data, product-
  wired; an extension can only invoke undo via whatever command id the default
  package registered. Small, but it belongs in the table.

## G. Enforcement (make the premise a CI assertion)

- [ ] **G1. Lint rule:** `apps/web/src/product/*.ts` may import only from the
  allowlisted module set. Every violation is exactly one unit of dogfooding debt,
  automatically counted — this *is* the `Host*` metric, mechanized.
- [ ] **G2. Load-path test:** an integration test that loads each first-party product
  package through `loadExtensionPackage` itself. When it passes for the file tree,
  "the-editor is just a default package" is proven, not asserted.
- [ ] **G3. AppRuntime shrinks** — a consequence, not a task. As C/D land, the
  privileged methods become operations/primitives and the god-object thins. Track via
  G1, not by refactoring directly.

## H. Decisions

- [x] **H1. Command override / advice — settled: no advice.** Registry refuses
  duplicate ids; keymap shadowing covers "change what a key does" (see Done). For
  the harder "remove/replace a default entirely," the answer is **I1's manifest
  opt-out (don't load the package), not per-contribution advice** — advice breaks
  single-owner + reload (see I1). Keep strict ownership + rebind; opt-out is a
  manifest edit.
- [x] **H2. Block widgets / floats.** Settled: the contract stays element-based
  (no React leak), and package code gets live per-view `MountHub`/`FloatHub`
  instances through `ViewMountHub`/`ViewFloatHub` and `DecorationViewTarget`.
- [x] **H3. Do not allowlist raw kernel.** A1/A2 use narrow re-exports from
  `@the-editor/extensions`; `@the-editor/kernel` stays unallowlisted because the
  services layer already contains `PackageRuntimeService`, and naming it would
  let ordinary extensions activate/reload/dispose packages. Meta-extensions
  (extension-manager UI, package dashboard) are deliberately deferred. When that
  need arrives, prefer a bounded package-introspection service (list + statuses,
  no dispose/reload) over exposing the raw runtime.

## I. The default set is data (opt-out / replace)

The thesis is "the-editor is just a default *set* of packages." Sections A–H made
every *capability* reachable — an extension can **add** commands, panes, intel,
statusline segments, processes, decorations, interaction listeners. The last
structural gap is the other half: a user can add alongside a default but cannot
**drop or replace** one. The first-party set is a hardcoded `activateGraph([...])`
list in `boot()`, and contributions are owned by their package, so nobody can
retract a first-party contribution. Wanting "no built-in `"lsp"` provider" or
"*only* my palette" is currently impossible.

**This is the highest-leverage remaining architectural item** — it makes the
premise true across the board rather than per-feature. D1b (LSP as pure package
code) is a *subset* of it: relocating `LspManager` only matters once the LSP
package can actually be swapped out, so do this before any god-object thinning.

### The two needs, kept separate

- **Add alongside — already solved.** Most "replace the default" desires are
  really composition: a statusline segment beside the branch segment, a
  completion provider competing with `"lsp"`, a keybinding shadowing the default
  (later-registration-wins is live), a command opening your palette. The additive
  registries + shadowing cover the large majority. Don't over-build for this.
- **Remove / truly replace — the gap.** Narrow but real: *no* built-in `"lsp"`
  provider, *only* my palette, *these* default keymaps should not exist.

### The design (settled)

- [x] **I1. Default set as a manifest; package is the unit of opt-out.** Shipped.
  `apps/web/src/product/manifest.ts` is the hardcoded boot list turned into
  data: an ordered `defaultPackages()` of `{ pkg, required }` entries
  (`editorDefaults` is the sole `required` foundation; the rest swappable). The
  config-as-a-package layer (`@the-editor/extensions`: `EditorConfig`,
  `defineConfig`, `loadConfig`, sharing `evaluateModuleDefault` with the
  extension loader) reads `~/.the-editor/config.ts` + a trust-gated project
  `.the-editor/config.ts` (`makeReadEditorConfig`, merging `disable` lists over
  a shared trust gate — one prompt per root). `makeDefaultPackagesController`
  computes the effective set and drives the kernel: `initialDescriptors()` for
  boot's `activateGraph` (dependency-safe), `reconcile()` for `reload`
  (dispose newly-disabled, re-activate re-enabled). Disabling `the-editor/lsp-intel`
  means its providers **never register** — not suppressed, never activated;
  "replace" = disable + ship your own extension. A `disable` naming a `required`
  package is ignored with a notice. Reload rides the existing `reload` command
  (a `beforeReload` hook runs the manifest reconcile before the disk-extension
  pass). Proven: config-layer tests (source → `EditorConfig`), read-merge tests
  (global + trusted/untrusted project), and controller tests against the real
  `PackageRuntime` (disabled swappable never loads, required protected, reload
  disposes/re-activates). Browser-verified boot has no regression.
  - **Reject per-contribution suppression / advice.** A "config disables
    contribution X" or "advice wraps Y" mechanism breaks the one rule the whole
    model rests on — a contribution has exactly one owner, and reload disposes
    exactly what a package registered. A second party retracting a first party's
    contribution makes reload semantics undefined (what happens when the
    suppressed package reloads? the suppressor?) and reintroduces the global
    mutable bag the architecture bans. It also doesn't compose (two extensions
    suppressing each other). The manifest approach is reload-clean *because* it's
    just "which packages activate" — the existing dispose-on-unload mechanism, no
    new suppression tracking. (This is the concrete answer to H1: opt-out is a
    manifest edit, not command advice.)
  - **Package is the right granularity; coarseness is a split signal.** Dropping
    `lspIntel` drops its completion+hover+signature together — correct: the
    package is the replacement unit. If a default bundles two things you want to
    split, *split the package*; don't add finer suppression. That pressure
    improves the default set over time.
  - **This needs config read EARLY — it's the config-as-a-package layer.**
    Opt-out declarations must be known *before* the defaults activate, but user
    extensions load *last*. So I1 is really "build the minimal config-as-a-package
    layer" (`EXTENSIONS.md` §6, still aspirational), split into a **manifest
    phase** (read first: what loads, with the transform applied) and a
    **contribution phase** (loaded last: add behavior). The existing `reload`
    discovery pipeline re-runs it, so opt-out rides reload for free.
  - **Retire roles/arbitration (`EXTENSIONS.md` §11) — subsumed.** The
    registry + host-render-surface pattern (statusline segments, picker sources,
    one `PickerHost`) already models "N contributions, host composes/picks,"
    singletons included. Manifest (what loads) + registries (what they
    contribute) + host surfaces (that compose) cover it; don't build a separate
    role registry.
  - **First step: prove one swap end to end.** Make the boot set a manifest with
    required/swappable marks, and demonstrate a config that drops `lspIntel` and
    loads a replacement, reloadable. One vertical slice validates the model before
    converting the whole set — and only then does D1b's "LSP as pure package
    code" become meaningful (it can otherwise stay a documented non-priority).

---

## Suggested order

A1–E2 and I1 are **done** (see checkboxes above). What remains, in leverage order:

1. **F1 + F-flagship** — the agent bridge and the agent-panel-as-extension proof.
   This is the actual payoff of the whole arc: it exercises every seam built in
   A–E/I and demonstrates *why* they mattered. Do it first.
2. **G1 + G2** — the lint rule + load-path test; cheap, and they hold the line so
   the finished seams can't silently regress.
3. **F3 / F4** — interaction-model decision and undo-as-a-verb, as real extensions
   need them.
4. **D1b (LSP as pure package code)** — I1 now makes the LSP package swappable, so
   this is only worth it if god-object thinning earns its keep; otherwise
   documented debt.
5. **G3 (AppRuntime shrinks)** — a consequence to track, not a task.

Definition of done: `Host*` count is 0, G1/G2 green in CI, the F-flagship agent
panel runs from `~/.the-editor/extensions` with no privileged imports, and I1 lets
a user config drop a first-party default and load a replacement. At that point
the-editor is, literally, a package like any other — one you can add to, drive,
*and* subtract from.
