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
   operation. **Currently: 12.**
   ```
   HostFileActions  HostWorkspaceActions  HostPaletteActions  HostThemeActions
   HostViewActions  HostLspActions  HostIntelActions  HostLspIntel
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
| 8 | Render a pane (agent panel, browser, Doom) | ❌ `PanelRenderers` app-internal (B1) |
| 9 | Answer questions about a buffer (completion/hover/signature) | ❌ intel registries app-internal (B2) |
| 10 | Open a file into a visible view | ❌ `Files` not in services layer (C1) |
| 11 | Pick from a list (completing-read) | ❌ no seam; five bespoke React surfaces (C3) |
| 12 | Add statusline / context-menu items | ❌ no registry exists (E1) |
| 13 | Own a background process | ❌ `ProcessPort`/`TerminalPort` host-only (D1) |
| 14 | React to interaction (hover/scroll/viewport/pre-key) | ❌ `PaneHost` fields are single-assignment slots (D2) |
| 15 | Back a buffer with non-file content | 🟡 `DocumentRegistry.open({text})` exists; provider semantics + view attach missing (D3) |
| 16 | Contribute a language server / grammar | ❌ `LanguageServers` app-internal; grammars native `.so` (B3/D1) |
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

- [ ] **B1. Publish `PanelRenderers`.** Move from `apps/web/src/workbench/panelRenderers.ts`
  into `@the-editor/workbench` + allowlist. The registry is already `HTMLElement`-based
  and framework-agnostic (the file tree mounts a web component, not React) — an
  *extension* panel mounts its own DOM and needs nothing from `AppRuntime`, so this
  unlocks arbitrary extension panes immediately. **Do not block on** the follow-up:
  first-party renderers (`filetree/mount.ts`, `terminal/mount.ts`, `diffview/mount.ts`)
  take `AppRuntime` directly; dogfooding them onto a bounded panel-host API is
  separate debt (tracked under G).
- [ ] **B2. Publish intel providers.** `CompletionProviders`/`HoverProviders`/
  `SignatureProviders` (`apps/web/src/intel/types.ts`) are pure Ref-backed registries
  with scoped retraction; LSP already registers as `id:"lsp"` among equals. Move to a
  published package (e.g. `@the-editor/intel`) + allowlist. Highest value per line of
  work in the audit.
- [ ] **B3. Publish the thin registries.** `VcsProviders`, `LanguageServers`,
  `DiffViewPreferences`. Registration is pure and safe now even though the *default*
  providers need process access (D1) — an extension could register a pure-JS provider
  today if it could name the tag.
- [ ] **B4. Expose `MountHub`/`FloatHub`.** Element-based in-editor block widgets and
  anchored popovers (`apps/web/src/editor/mounts.ts`, `floats.ts`). Settles the old
  "block widgets" decision in favor of exposure — the seams are already
  element-based, so no framework contract leaks.

## C. Operations (retire `Host*` tags)

- [ ] **C1. Open-file-into-view op.** `Files.openPath` exists but `Files` is absent
  from the services layer; `DocumentRegistry.open` can't attach a rendered tab. For
  an agent-driving extension this is the most basic verb after "edit". Add `Files`
  to the layer or ship a small `openInView` operation.
- [ ] **C2. Actions as operations.** Turn privileged `AppRuntime` methods into public
  invokable ops, then delete the tag:
  - [ ] `HostFileActions` → `save` / `open` / `find` / `findInProject`
  - [ ] `HostWorkspaceActions` → `openFolder` / `openSwitcher` / `closeWorkspace`
  - [ ] `HostViewActions` → `splitRight` / `splitDown` / `closeTab` / `nextTab` / `previousTab` / `toggleTabs`
  - [ ] `HostThemeActions`, `HostPaletteActions` → fold into the picker op (C3)
  - [ ] `HostLspActions` → `goToDefinition` / `format` / `rename` / `toggleInlayHints` / `toggleLsp` (depends on D1)
- [ ] **C3. Picker as an operation.** One package-invokable
  `picker.open({ items, onSelect, preview? })`; the five bespoke surfaces
  (`CommandPalette`, `FileFinder`, `GlobalSearch`, theme selector, workspace
  switcher) become callers. Retires `onPaletteOpenRequested`-style host hooks +
  `HostPaletteActions` + `HostThemeActions`. Telescope's model: one picker, N
  sources. **Caveat found in audit:** the current React pickers render `items.map()`
  unvirtualized through cmdk — generalizing to arbitrary extension-supplied item
  counts forces the windowed-list question at the same time; solve both together.

## D. New primitives (real design work)

- [ ] **D1. Process ownership.** Publish `ProcessPort` (and the PTY-shaped
  `TerminalPort`) as capability tags handed to packages. De-specializes LSP, VCS,
  and the terminal at once; retires `HostTerminal`, unblocks `HostLspActions`,
  `HostVcsDefaults`, `HostLspIntel`. **This is a trust decision as much as
  plumbing** — the project-extension trust gate (`extensionTrust.ts`) exists because
  of exactly this, and `userExtensions.ts` already flags LSP exposure as "a separate,
  deliberate call". Sequence it after A–C, with the capability story written down.
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
- [x] **H2. Block widgets** — settled by audit finding: all mount seams are
  element-based, so expose them (B4); no React contract leaks.
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
