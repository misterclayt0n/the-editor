# Writing Extensions

An extension is a TypeScript file that `export default`s the result of
`definePackage(...)` — the exact same shape every first-party feature in
`apps/web/src/product/*.ts` uses. There is no separate plugin API: whatever a
first-party package can reach, your extension can reach, because both activate
through the identical services layer. The first-party editor is just the
default extension set (and your config can drop pieces of it — see
[Configuration](#configuration)).

```
~/.the-editor/extensions/hello.ts          ← single file
~/.the-editor/extensions/my-thing/index.ts ← or a directory (index.ts / index.tsx)
<workspace>/.the-editor/extensions/…       ← project-local (trust-gated, asked once per root)
```

Run the **Reload** command (palette) after editing — no restart, no file
watcher. Old contributions are disposed, new ones register, spawned processes
are killed and restarted. If something breaks, run **Extension Issues** for
the full error report (cause chain + stack); the toast only shows the first
line.

## The smallest extension

```ts
import { Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";

export default definePackage("me.hello", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    yield* commands.register("me.hello", {
      title: "Say Hello",
      palette: { group: "Mine" },
      run: () => Effect.sync(() => console.log("hello")),
    });
  }),
);
```

Everything you register is owned by your package's activation scope: on
reload/dispose it retracts automatically. Never hand-track cleanup for
registrations; for your own resources use `Effect.acquireRelease` inside
activation.

## What you can import

Extensions run against a **closed module surface** — these and nothing else
(no npm, no relative imports; see [Dependencies](#dependencies-react-shadcn-anything)
for how to use anything beyond it):

| Module | What it gives you |
|---|---|
| `effect` | The Effect runtime (Effect, Layer, Scope, Stream, …) |
| `@the-editor/extensions` | `definePackage`, `defineConfig`, `h`/`Fragment` (JSX → DOM), `Attention` (dialogs/toasts), stores, `makeStateCell`/`makeRegistry`/topics |
| `@the-editor/commands` | Register / execute / list commands — the operation surface |
| `@the-editor/keymaps` | Keybindings (yours load last, so you shadow defaults on conflict) |
| `@the-editor/workbench` | Panels (`Workbench` + `PanelRenderers` + `surgery`), statusline segments, OS context menus |
| `@the-editor/editor` | Documents, views, `FileSystemPort`, `Files`/`FileViews`, `VirtualViews`, decorations (`DecorationProviders`), view interaction events, commit/selection |
| `@the-editor/engine` | Text primitives: `Transaction`, `Selection` (multi-cursor), search, diff |
| `@the-editor/intel` | Completion / hover / signature providers, `LanguageServers`, `Lsp` queries |
| `@the-editor/picker` | `Picker.open` — one completing-read surface, your source |
| `@the-editor/process` | `Processes.spawn/run`, `Ptys` — killed automatically on reload |
| `@the-editor/vcs`, `@the-editor/diff-view` | VCS providers, diff-view preferences |
| `@the-editor/theme`, `@the-editor/fonts` | Register themes / fonts, switch them |

## UI: the DOM is the component model

Every UI seam hands you a raw `HTMLElement` (a panel body, a block-widget
mount, a float). There is deliberately no blessed framework — like pi-tui's
`render(): string[]`, the platform's own primitive is the contract. Two ways
to build on it:

**1. `.tsx` + the native `h` builder (no React, no runtime).** Name your file
`.tsx` and JSX compiles straight to DOM nodes via `h`:

```tsx
import { Effect } from "effect";
import { definePackage, h, Fragment } from "@the-editor/extensions";
import { PanelRenderers, SurfaceKind, Workbench, dockEdge } from "@the-editor/workbench";

const MySurface = SurfaceKind("me/panel");

export default definePackage("me.panel", () =>
  Effect.gen(function* () {
    const renderers = yield* PanelRenderers;
    yield* renderers.register(MySurface, {
      title: () => "Mine",
      mount(element) {
        let count = 0;
        const label = (<span>0</span>) as HTMLElement;
        element.append(
          <div class="me-panel" style={{ display: "flex", gap: "8px", padding: "8px" }}>
            <button onclick={() => (label.textContent = String(++count))}>+1</button>
            {label}
          </div>,
        );
        return { dispose: () => element.replaceChildren() };
      },
    });
  }),
);
```

`h` is ~100 lines of hyperscript: `class`/`className`, `style` (string or
object, CSS vars included), `on*` → `addEventListener`, `dataset`, boolean
attributes, `ref={(el) => …}`, function components, `<>` fragments. No vdom,
no state — you own the nodes.

**2. Look native: theme tokens.** The editor's palette is CSS variables,
live-updated on theme switch. Use them instead of hardcoded colors:

```
--bg  --bg-panel  --bg-editor      backgrounds
--fg  --fg-dim                     text
--accent  --selection  --caret     highlights
--border  --status-bg  --chrome    chrome
--radius-sm/-md/-lg                corners
--font-ui-family/-size             UI text
--font-buffer-family/-size         code text
--shadow-card  --ease-out          polish
```

A panel styled `background: var(--bg-panel); color: var(--fg); font: var(--font-ui-size) var(--font-ui-family)`
matches every theme automatically. Scope your CSS under your own class names
(inject a `<style>` element in `mount`, remove it in `dispose`).

**Bulletproof isolation: shadow DOM** (how `@pierre/diffs`/`@pierre/trees` do
it — the file tree in this very editor runs this pattern). Attach a shadow root
to your mount element and adopt a constructed stylesheet: editor styles can't
leak in, yours can't leak out, and the theme tokens still reach you — CSS
custom properties are the one thing that pierces the shadow boundary, which is
exactly why they're the theming API:

```ts
const sheet = new CSSStyleSheet();
sheet.replaceSync(`.root { background: var(--bg-panel); color: var(--fg); }`);
const shadow = element.attachShadow({ mode: "open" });
shadow.adoptedStyleSheets = [sheet];
shadow.append(root);
```

Rule of thumb: class-name scoping + tokens for simple panels; shadow DOM when
your CSS is substantial or you're shipping the extension to others.

## Managing state (there is no framework, on purpose)

`h` is create-only — no vdom, no reactivity, no render loop. State management
is the pre-framework model, which is also how pierre's DOM core and pi-tui's
components work: **state lives in plain closures/class fields; you hold
references to the nodes you'll update and mutate them when your events fire.**
Nothing repaints unless you change it.

```tsx
mount(element) {
  // state: a closure. refs: just variables.
  let items: string[] = [];
  const list = (<ul class="me-list" />) as HTMLElement;

  const rerenderList = () => {
    list.replaceChildren(...items.map((item) => (<li>{item}</li>) as HTMLElement));
  };

  // updates come from the editor's reactive fabric — subscribe, mutate, done.
  const unsubscribe = someStateCell.changes.pipe(/* Stream.runForEach … */);

  element.append(<div class="me-panel">{list}</div>);
  return { dispose: () => element.replaceChildren() /* + unsubscribe */ };
}
```

Sources of change to subscribe to: `StateCell` (`.get`/`.changes`), document /
view / workspace notification buses, `DecorationViewTarget.interaction`
(hover/scroll/keys), registries' `onChanged`, your `ProcessHandle.onData`.
Rule of thumb: a coarse `rerender()` that rebuilds one subtree from state is
fine — panels are small; measure before getting clever. When a panel genuinely
outgrows this (deep forms, complex derived state), don't fight it: pre-bundle a
real framework into your element (next section) — your framework, your version,
invisible to the host.

## Dependencies (React, shadcn, anything)

The module surface is closed, so `import react from "react"` fails — **but you
can bring anything by pre-bundling**. Author a real multi-file project and
compile it to one file with `@the-editor/*` and `effect` as externals. The
verified recipe (this exact configuration is CI-tested in
`packages/extensions/test/prebundle.test.ts`, which bundles a React extension
with hooks + a CSS import and runs it through the real loader):

```bash
esbuild src/index.tsx --bundle --format=cjs --platform=browser \
  --external:effect --external:"@the-editor/*" \
  --define:process.env.NODE_ENV='"production"' \
  --loader:.css=text \
  --outfile=index.ts
```

Two flags are load-bearing, learned the hard way:

- `--define:process.env.NODE_ENV='"production"'` — React (and much of npm)
  branches on `process.env.NODE_ENV`; there is no `process` in the renderer,
  so an undefined define crashes at first render.
- `--loader:.css=text` — a bundle can't emit a second `.css` file anywhere the
  loader would pick up. Import your compiled stylesheet (Tailwind output,
  shadcn styles) as a string and inject it yourself: a `<style>` element, or
  `adoptedStyleSheets` on a shadow root (see Styling).

Everything else — React, Radix, vendored shadcn components, Solid, Vue,
foldkit, whatever compiles to JS+DOM — inlines into the bundle. Drop the
output at `~/.the-editor/extensions/my-thing/index.ts` and it loads through
the same pipeline. Rule of thumb: prototype in one `.tsx` with `h`; graduate
to a bundled project when you outgrow it.

(Heads-up if you bundle: the loader compiles plain-`.ts` files too, so a
bundled output named `.ts` must not contain JSX — esbuild's `--bundle` already
compiled it away.)

## The seams, by task

- **Run a command / drive the editor**: `Commands.execute(id)` — everything is
  a command, and `Commands.list` is a self-describing catalog.
- **Edit text**: inside an editor-scoped command handler, `editorSnapshot()` →
  `Transaction.fromReplacements` → `commitTransaction(tx, selection?)`.
  Multi-cursor via `Selection.addRange`.
- **A pane** (agent panel, browser, whatever): `Workbench.createPanel({kind})`
  + `updateArrangement((a) => dockEdge(a, panel.id, "right"))` +
  `PanelRenderers.register(kind, { title, mount })`.
- **Decorate buffers**: `DecorationProviders.register({ attach, attachView })`
  — text styles, gutters, virtual text, block widgets (`target.mounts`),
  floats (`target.floats`), and interaction events
  (`target.interaction.onPointerHover(...)`).
- **A picker**: `Picker.open({ source, onChoose, preview? })` — you provide
  data; the host renders the same windowed surface the command palette uses.
- **Ask / notify**: `Attention.ask(topic, choiceRequest)` (modal with exactly
  one `cancel: true` choice) and `Attention.notify` (toast).
- **A process**: `Processes.spawn({ command, args, cwd, env })` inside
  activation — the handle dies with your package. `Processes.run` for one-shots.
- **Non-file documents**: `VirtualViews.open({ title, initialText })` → a
  rendered read-only tab you drive with `setText` (magit-style surfaces).
- **Statusline / context menus**: `StatuslineSegments.register`,
  `ContextMenuProviders.register` (the OS draws the menu).

## Configuration

`~/.the-editor/config.ts` is also code, read before the defaults activate:

```ts
import { defineConfig } from "@the-editor/extensions";
export default defineConfig({
  // Drop first-party defaults by package id; ship your replacement as an
  // ordinary extension. (Required foundation packages ignore this.)
  disable: ["the-editor/lsp-intel"],
});
```

## Debugging workflow

1. Edit → **Reload** (palette). Reload is explicit; there is no file watcher.
2. Failure toast shows `file: <deepest cause message>`; **Extension Issues**
   opens the full report (cause chain + stack, per file, current-state only).
3. A broken extension never takes others down — failures are isolated per file.
4. DevTools (⌥⌘I in the desktop app) sees your `console.log` and real stacks.
5. Processes you spawned show under your `label` and die on reload — if your
   agent/server survived a reload, you spawned it outside the activation scope.
