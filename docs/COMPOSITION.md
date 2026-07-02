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
   operation. **Currently: 6.**
   ```
   HostIntelActions  HostLspIntel
   HostFileTree  HostTerminal  HostDiffView  HostVcsDefaults
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
| 11 | Pick from a list (completing-read) | ✅ `Picker` via `@the-editor/picker`; one windowed host, N sources (GlobalSearch conversion pending) |
| 12 | Add statusline / context-menu items | ❌ no registry exists (E1) |
| 13 | Own a background process | ❌ direct `ProcessPort`/`TerminalPort` host-only; LSP spawn-by-proxy exists via #16 (D1) |
| 14 | React to interaction (hover/scroll/viewport/pre-key) | ❌ `PaneHost` fields are single-assignment slots (D2) |
| 15 | Back a buffer with non-file content | 🟡 `DocumentRegistry.open({text})` exists; provider semantics + view attach missing (D3) |
| 16 | Contribute a language server / grammar | 🟡 `LanguageServers` public via `@the-editor/intel`; grammars native `.so` (D1) |
| 17 | Undo / redo | 🟡 engine `history` is pure data; not a public verb (F4) |
| 18 | Be driven from outside (CLI/socket/MCP) | ❌ no inbound channel exists at all (F1) |
| 19 | Replace the interaction model (Vim/Helix/Emacs) | ❓ undecided — no mode state machine (F3) |

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
- [ ] **C4. Global search as a picker source.** Follow-up from C3: `GlobalSearch`
  still renders bespoke (grouped rows, per-row syntax segments, grep-mode
  cycling, Cmd+Enter → multibuffer) even though it already rides
  `VirtualCommand`. Converting it to a `PickerSource` needs styled-segment rows
  and secondary actions in the item model, and its per-row synchronous highlight
  pass should move to idle-time upgrade when touched.

## D. New primitives (real design work)

- [ ] **D1. Process ownership.** Publish `ProcessPort` (and the PTY-shaped
  `TerminalPort`) as capability tags handed to packages. De-specializes LSP, VCS,
  and the terminal at once; retires `HostTerminal`, `HostVcsDefaults`, and
  `HostLspIntel`. **This is a trust decision as much as
  plumbing** — the project-extension trust gate (`extensionTrust.ts`) exists because
  of exactly this. B3 already created one narrow process-spawn-by-proxy path:
  a public `LanguageServers` registration can cause the host LSP manager to start an
  arbitrary command for a matching file. The D1 write-up must acknowledge that
  existing reachability and then decide whether to formalize/generalize it or
  constrain it. Sequence it after A–C, with the capability story written down.
- [ ] **D2. Interaction events — with multiplexing.** `PaneHost.pointerHover` /
  `pointerScrolled` / `viewportChanged` / `keyInterceptor` are **single-assignment
  slots** that `IntelController` overwrites (`intel/controller.ts`) — even internally
  only one consumer can exist. The design work is a fan-out subscription seam, not a
  tag move. Frees `IntelController`, retires `HostIntelActions`.
- [ ] **D3. Virtual buffers — the missing half.** Unbound in-memory documents already
  exist (`DocumentRegistry.open({text})`). Missing: provider-backed semantics (edits
  become operations — REPL, magit-style status buffer) and attaching to a view (C1).
  Smaller than previously scoped.

## E. Build a registry

- [ ] **E1. Statusline contributions.** Hardcoded JSX in `App.tsx` reading
  `runtime.vcs?.branch()` directly. Build a segment registry, publish it, render the
  statusline *from* it.
- [ ] **E2. Context-menu contributions.** No registry; menus exist only for the
  terminal, hardcoded per-use. Same registry pattern as E1.

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

- [ ] **H1. Command override / advice.** Registry refuses duplicate ids; keymap
  shadowing already covers the common case (see Done). Recommend: keep strict
  ownership + rebind; revisit only when a real extension needs advice.
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

---

## Suggested order

1. **A1 + A2** — days of work; flips verbs 6–7 and unblocks every extension that
   wants to talk to the user.
2. **B1 + B2** — relocations; unlocks extension panes + non-LSP intel. With A, the
   verb table goes from ~6/19 to ~11/19 on tag moves alone.
3. **C1** (open-file op) — the most basic missing agent verb.
4. **C2 + C3** — retires the bulk of the `Host*` tags; picker collapses five surfaces.
5. **D1** (process) — highest leverage, but do the trust write-up first.
6. **F1 + F-flagship** — the agent bridge and the agent-panel-as-extension proof.
7. **D2, E1/E2, D3** — interaction fan-out, statusline/menu registries, virtual buffers.
8. **G1 + G2 early and continuously** — cheap to add, and they hold the line while
   everything above lands.

Definition of done: `Host*` count is 0, G1/G2 green in CI, and the F-flagship agent
panel runs from `~/.the-editor/extensions` with no privileged imports. At that point
the-editor is, literally, a package like any other.
