import { describe, expect, it } from "@effect/vitest";
import { Context, Effect, Layer, Option, Stream } from "effect";
import * as EffectNamespace from "effect";
import {
  Attention,
  makeAttention,
  makePackageRuntime,
  makeStateCell,
  PackageRuntimeService,
} from "@the-editor/kernel";
import { Commands, makeCommands } from "@the-editor/commands";
import { DiffViewPreferences, makeDiffViewPreferences } from "@the-editor/diff-view";
import * as CommandsNamespace from "@the-editor/commands";
import { toPackageDescriptor } from "@the-editor/extensions";
import * as ExtensionsNamespace from "@the-editor/extensions";
import {
  DecorationProviders,
  DocumentRegistry,
  EditorVisuals,
  FileIoError,
  Files,
  FileOperations,
  FileSystemPort,
  FileViews,
  LspOperations,
  makeDecorationProviders,
  makeViewScope,
  makeWorkspaceScope,
  PaletteOperations,
  ThemeOperations,
  ViewMountHub,
  ViewOperations,
  WorkspaceNotifications,
  WorkspaceOperations,
  WorkspaceRoots,
  type DirEntry,
  type FileStat,
} from "@the-editor/editor";
import {
  CompletionProviders,
  HoverProviders,
  LanguageServers,
  makeCompletionProviders,
  makeHoverProviders,
  makeLanguageServers,
  makeSignatureProviders,
  SignatureProviders,
} from "@the-editor/intel";
import { makeVcsProviders, VcsProviders } from "@the-editor/vcs";
import {
  collectSlots,
  makePanelRenderers,
  makeStack,
  makeWorkbench,
  PanelRenderers,
  placeInStack,
  SurfaceKind,
} from "@the-editor/workbench";
import { buildExtensionVirtualModules, makeUserExtensionsPackage } from "./userExtensions";

const testVirtualModules = {
  effect: EffectNamespace,
  "@the-editor/extensions": ExtensionsNamespace,
  "@the-editor/commands": CommandsNamespace,
};

/** A minimal in-memory FileSystemPort: just enough readDir/readFile/stat for
 *  discovery, keyed by full path (no real directory semantics beyond one
 *  level, which is all discovery needs). `delayMs` inserts a real async gap
 *  before every read so concurrent callers actually interleave instead of
 *  each reconcile pass running to completion before the next one starts —
 *  without it, an in-memory fake resolves synchronously-fast enough that a
 *  missing semaphore never gets exercised. */
function makeFakeFs(delayMs = 0) {
  const files = new Map<string, { text: string; mtimeMs: number }>();
  let clock = 1;
  const maybeDelay = <A, E, R>(effect: Effect.Effect<A, E, R>) =>
    delayMs > 0 ? Effect.delay(effect, `${delayMs} millis`) : effect;

  const write = (path: string, text: string): void => {
    files.set(path, { text, mtimeMs: ++clock });
  };
  const remove = (path: string): void => {
    files.delete(path);
  };

  const port: FileSystemPort.Service = {
    readFile: (path) => {
      const entry = files.get(path);
      return maybeDelay(
        entry !== undefined
          ? Effect.succeed(entry.text)
          : Effect.fail(new FileIoError(path, "read", "not found")),
      );
    },
    writeFile: (path, text) => {
      write(path, text);
      const entry = files.get(path)!;
      return Effect.succeed({ mtimeMs: entry.mtimeMs, size: text.length });
    },
    stat: (path) => {
      const entry = files.get(path);
      const stat: FileStat | null =
        entry !== undefined ? { mtimeMs: entry.mtimeMs, size: entry.text.length } : null;
      return maybeDelay(Effect.succeed(Option.fromNullable(stat)));
    },
    watch: () => Effect.succeed(() => {}),
    pickFile: () => Effect.succeed(Option.none()),
    readDir: (dir) => {
      const prefix = dir.endsWith("/") ? dir : `${dir}/`;
      const seen = new Set<string>();
      const entries: DirEntry[] = [];
      for (const path of files.keys()) {
        if (!path.startsWith(prefix)) continue;
        const rest = path.slice(prefix.length);
        const head = rest.split("/")[0];
        if (head === undefined || head === "" || seen.has(head)) continue;
        seen.add(head);
        entries.push({ name: head, kind: rest.includes("/") ? "directory" : "file" });
      }
      return Effect.succeed(entries);
    },
    mkdir: () => Effect.succeed(undefined),
    capabilities: { watch: false },
  };

  return { port, write, remove };
}

/** Poll a boolean-producing Effect until it settles true — the package's
 *  discovery/load work runs in forked background fibers, so tests observe it
 *  settle rather than awaiting a handle directly. */
const waitUntil = <E, R>(
  check: Effect.Effect<boolean, E, R>,
  attempts = 200,
): Effect.Effect<void, E, R> =>
  Effect.gen(function* () {
    for (let i = 0; i < attempts; i++) {
      if (yield* check) return;
      yield* Effect.sleep("5 millis");
    }
    yield* Effect.dieMessage("waitUntil: condition never became true");
  });

const identityTranspiler = { transform: async (source: string) => source };

function packageSource(
  id: string,
  commandId: string,
  title: string,
  options?: { readonly countActivationsAs?: string },
): string {
  const countLine =
    options?.countActivationsAs !== undefined
      ? `require(${JSON.stringify(options.countActivationsAs)}).increment();`
      : "";
  return `
    var definePackage = require("@the-editor/extensions").definePackage;
    var Commands = require("@the-editor/commands").Commands;
    var Effect = require("effect").Effect;
    module.exports.default = definePackage(${JSON.stringify(id)}, function () {
      ${countLine}
      return Effect.flatMap(Commands, function (commands) {
        return commands.register(${JSON.stringify(commandId)}, {
          title: ${JSON.stringify(title)},
          run: function () { return Effect.void; },
        });
      });
    });
  `;
}

function attentionPackageSource(): string {
  return `
    var ext = require("@the-editor/extensions");
    var definePackage = ext.definePackage;
    var Attention = ext.Attention;
    var makeNoticeTopic = ext.makeNoticeTopic;
    var makeRequestTopic = ext.makeRequestTopic;
    var Effect = require("effect").Effect;
    var probe = require("test:attention-probe");
    var Ready = makeNoticeTopic("acme/attention-ready");
    var Pick = makeRequestTopic("acme/attention-pick");
    module.exports.default = definePackage("acme.attention", function () {
      return Effect.gen(function* () {
        var attention = yield* Attention;
        yield* attention.notify(Ready, { message: "hello from extension" }, { level: "success" });
        var answer = yield* attention.ask(Pick, {
          title: "Pick one",
          description: "Round-trip through the public Attention fabric.",
          choices: [
            { id: "yes", label: "Yes" },
            { id: "no", label: "No", cancel: true }
          ]
        });
        probe.answer(answer);
      });
    });
  `;
}

function publishedPrimitivePackageSource(): string {
  return `
    var definePackage = require("@the-editor/extensions").definePackage;
    var Effect = require("effect").Effect;
    var Workbench = require("@the-editor/workbench");
    var Editor = require("@the-editor/editor");
    var Intel = require("@the-editor/intel");
    var Vcs = require("@the-editor/vcs");
    var DiffView = require("@the-editor/diff-view");
    var Engine = require("@the-editor/engine");
    var probe = require("test:published-primitives-probe");
    var PanelKind = Workbench.SurfaceKind("acme/published-panel");
    module.exports.default = definePackage("acme.published-primitives", function () {
      return Effect.gen(function* () {
        var renderers = yield* Workbench.PanelRenderers;
        var completions = yield* Intel.CompletionProviders;
        var hovers = yield* Intel.HoverProviders;
        var signatures = yield* Intel.SignatureProviders;
        var servers = yield* Intel.LanguageServers;
        var vcs = yield* Vcs.VcsProviders;
        var diff = yield* DiffView.DiffViewPreferences;
        var decorations = yield* Editor.DecorationProviders;
        var files = yield* Editor.Files;
        var fileViews = yield* Editor.FileViews;
        var fileOps = yield* Editor.FileOperations;
        var workspaceOps = yield* Editor.WorkspaceOperations;
        var viewOps = yield* Editor.ViewOperations;
        var paletteOps = yield* Editor.PaletteOperations;
        var themeOps = yield* Editor.ThemeOperations;
        var lspOps = yield* Editor.LspOperations;
        var opened = yield* fileViews.openInView("/project/open.txt");
        var openedAgain = yield* files.openPath("/project/open.txt");
        yield* renderers.register(PanelKind, {
          title: function () { return "Published"; },
          mount: function (element) {
            element.textContent = "mounted by extension";
            return { dispose: function () {} };
          }
        });
        yield* completions.register({ id: "acme", provide: function () { return { items: [] }; } });
        yield* hovers.register({ id: "acme", provide: function () { return null; } });
        yield* signatures.register({ id: "acme", provide: function () { return null; } });
        yield* servers.register({
          id: "acme-lsp",
          command: "acme-lsp",
          languages: [{ languageId: "acme", fileTypes: ["acme"] }]
        });
        yield* vcs.register({
          id: "acme-vcs",
          detect: function () { return Promise.resolve(false); },
          status: function () { return Promise.resolve([]); },
          headContent: function () { return Promise.resolve(null); },
          listFiles: function () { return Promise.resolve([]); },
          patch: function () { return Promise.resolve(null); },
          info: function () { return Promise.resolve(null); }
        });
        yield* diff.register({ id: "acme", settings: { wrapping: true } });
        yield* decorations.register({
          id: "acme-widget",
          attach: function (target) {
            return Effect.asVoid(target.visuals.contribute("acme-widget", {
              blockWidgets: Engine.RangeSet.of([
                { from: 0, to: 0, value: { id: "acme-widget", heightRows: 1 } }
              ])
            }));
          },
          attachView: function (target) {
            return Effect.acquireRelease(
              Effect.sync(function () {
                probe.viewAttached({
                  mountHub: target.mounts instanceof Editor.MountHub,
                  floatHub: target.floats instanceof Editor.FloatHub
                });
                return target.mounts.subscribe(function (event) {
                  if (event.type === "mounted") {
                    event.el.textContent = "widget mounted by extension";
                    probe.mounted(event.id, event.el.textContent);
                  }
                });
              }),
              function (unsubscribe) { return Effect.sync(unsubscribe); }
            );
          }
        });
        probe.ready({
          panelKind: PanelKind,
          mountHub: typeof Editor.MountHub,
          floatHub: typeof Editor.FloatHub,
          fileViews: typeof Editor.FileViews,
          files: typeof Editor.Files,
          fileOperations: typeof Editor.FileOperations,
          workspaceOperations: typeof Editor.WorkspaceOperations,
          viewOperations: typeof Editor.ViewOperations,
          paletteOperations: typeof Editor.PaletteOperations,
          themeOperations: typeof Editor.ThemeOperations,
          lspOperations: typeof Editor.LspOperations,
          operationMethods: [
            typeof fileOps.save,
            typeof workspaceOps.openFolder,
            typeof viewOps.closeTab,
            typeof paletteOps.open,
            typeof themeOps.openSelector,
            typeof lspOps.goToDefinition
          ],
          openedPanelId: opened.panelId,
          openedViewId: opened.viewId,
          sameDocument: openedAgain.id === opened.documentId
        });
      });
    });
  `;
}

describe("makeUserExtensionsPackage", () => {
  it.live("loads a global extension at startup and reloads it after an edit", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const fs = makeFakeFs();
        fs.write(
          "/home/user/.the-editor/extensions/hello.ts",
          packageSource("acme.hello", "acme.hello", "Hello v1"),
        );

        const runtime = yield* makePackageRuntime;
        const attention = yield* makeAttention();
        const workspaceRoots = yield* makeStateCell<ReadonlyArray<string>>([]);
        const commands = yield* makeCommands(runtime.contributionIndex);

        const servicesForExtensions = Layer.mergeAll(
          Layer.succeed(Commands, commands),
          Layer.succeed(FileSystemPort, fs.port),
          Layer.succeed(PackageRuntimeService, runtime),
          Layer.succeed(Attention, attention),
          Layer.succeed(WorkspaceRoots, workspaceRoots),
        );

        const userExtensions = makeUserExtensionsPackage({
          servicesForExtensions,
          transpiler: identityTranspiler,
          virtualModules: testVirtualModules,
          homeDir: "/home/user",
        });

        yield* runtime.activateGraph([toPackageDescriptor(userExtensions, servicesForExtensions)]);

        yield* waitUntil(commands.get("acme.hello").pipe(Effect.map((c) => c !== undefined)));
        const loaded = yield* commands.get("acme.hello");
        expect(loaded?.contribution.title).toEqual("Hello v1");

        // Edit the file (bumps mtime) and reload.
        fs.write(
          "/home/user/.the-editor/extensions/hello.ts",
          packageSource("acme.hello", "acme.hello", "Hello v2"),
        );
        yield* commands.execute("reload");
        yield* waitUntil(
          commands.get("acme.hello").pipe(Effect.map((c) => c?.contribution.title === "Hello v2")),
        );

        // Delete the file and reload: the command should disappear.
        fs.remove("/home/user/.the-editor/extensions/hello.ts");
        yield* commands.execute("reload");
        yield* waitUntil(commands.get("acme.hello").pipe(Effect.map((c) => c === undefined)));
      }),
    ),
  );

  it.live("lets a loaded extension notify and ask through public Attention", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const fs = makeFakeFs();
        fs.write("/home/user/.the-editor/extensions/attention.ts", attentionPackageSource());

        const runtime = yield* makePackageRuntime;
        const attention = yield* makeAttention();
        const workspaceRoots = yield* makeStateCell<ReadonlyArray<string>>([]);
        const commands = yield* makeCommands(runtime.contributionIndex);
        const questions: unknown[] = [];
        const notices: unknown[] = [];
        let answer: string | undefined;
        const probe = { answer: (value: string) => void (answer = value) };

        const servicesForExtensions = Layer.mergeAll(
          Layer.succeed(Commands, commands),
          Layer.succeed(FileSystemPort, fs.port),
          Layer.succeed(PackageRuntimeService, runtime),
          Layer.succeed(Attention, attention),
          Layer.succeed(WorkspaceRoots, workspaceRoots),
        );

        yield* attention.registerResponder;
        yield* Effect.forkScoped(
          Stream.runForEach(attention.notices, (notice) =>
            Effect.sync(() => {
              notices.push(notice);
            }),
          ),
        );
        yield* Effect.forkScoped(
          Stream.runForEach(attention.requestChanges, (pending) =>
            Effect.forEach(
              pending,
              (request) =>
                Effect.sync(() => {
                  questions.push(request.question);
                }).pipe(Effect.zipRight(request.respond("yes"))),
              { discard: true },
            ),
          ),
        );

        const userExtensions = makeUserExtensionsPackage({
          servicesForExtensions,
          transpiler: identityTranspiler,
          virtualModules: { ...testVirtualModules, "test:attention-probe": probe },
          homeDir: "/home/user",
        });

        yield* runtime.activateGraph([toPackageDescriptor(userExtensions, servicesForExtensions)]);
        yield* waitUntil(Effect.sync(() => answer === "yes"));

        expect(answer).toEqual("yes");
        expect(notices).toContainEqual({
          topicId: "acme/attention-ready",
          level: "success",
          payload: { message: "hello from extension" },
        });
        expect(questions).toEqual([
          {
            title: "Pick one",
            description: "Round-trip through the public Attention fabric.",
            choices: [
              { id: "yes", label: "Yes" },
              { id: "no", label: "No", cancel: true },
            ],
          },
        ]);
      }),
    ),
  );

  it.live("lets a loaded extension register published B-surface primitives", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const fs = makeFakeFs();
        fs.write(
          "/home/user/.the-editor/extensions/published.ts",
          publishedPrimitivePackageSource(),
        );
        fs.write("/project/open.txt", "opened from disk\n");

        const runtime = yield* makePackageRuntime;
        const attention = yield* makeAttention();
        const workspaceRoots = yield* makeStateCell<ReadonlyArray<string>>([]);
        const commands = yield* makeCommands(runtime.contributionIndex);
        const panelRenderers = makePanelRenderers();
        const completions = yield* makeCompletionProviders;
        const hovers = yield* makeHoverProviders;
        const signatures = yield* makeSignatureProviders;
        const languageServers = yield* makeLanguageServers;
        const vcsProviders = yield* makeVcsProviders;
        const diffPreferences = makeDiffViewPreferences();
        const workbench = yield* makeWorkbench();
        const ws = yield* makeWorkspaceScope({ fileSystem: fs.port });
        const registry = Context.get(ws.context, DocumentRegistry);
        const workspaceBus = Context.get(ws.context, WorkspaceNotifications);
        const decorations = yield* makeDecorationProviders(registry, workspaceBus);
        const files = Context.get(ws.context, Files);
        const fileViews: FileViews.Service = {
          openInView: (path) =>
            Effect.gen(function* () {
              const opened = yield* files.openPath(path);
              const openedView = yield* makeViewScope(opened);
              yield* decorations.connectView(openedView);
              const panel = yield* workbench.createPanel({
                kind: SurfaceKind("the-editor/editor"),
                context: openedView.context,
              });
              yield* workbench
                .updateArrangement((arrangement) => {
                  const stack = collectSlots(arrangement).find((slot) => slot.kind === "stack");
                  if (arrangement.tiles === null || stack === undefined) {
                    return { ...arrangement, tiles: makeStack([panel.id]) };
                  }
                  return placeInStack(arrangement, stack.id, panel.id);
                })
                .pipe(Effect.orDie);
              return {
                documentId: opened.id,
                viewId: openedView.id,
                panelId: String(panel.id),
              };
            }),
        };
        const document = yield* registry.open({ text: "a" });
        const view = yield* makeViewScope(document);
        yield* decorations.connectView(view);
        const viewMounts = Context.get(view.context, ViewMountHub);
        const probe: {
          readyPayload?: {
            readonly panelKind: string;
            readonly mountHub: string;
            readonly floatHub: string;
            readonly fileViews: string;
            readonly files: string;
            readonly fileOperations: string;
            readonly workspaceOperations: string;
            readonly viewOperations: string;
            readonly paletteOperations: string;
            readonly themeOperations: string;
            readonly lspOperations: string;
            readonly operationMethods: readonly string[];
            readonly openedPanelId: string;
            readonly openedViewId: string;
            readonly sameDocument: boolean;
          };
          viewPayload?: {
            readonly mountHub: boolean;
            readonly floatHub: boolean;
          };
          mountedPayload?: {
            readonly id: string;
            readonly text: string;
          };
        } = {};
        const readyProbe = {
          ready: (payload: {
            readonly panelKind: string;
            readonly mountHub: string;
            readonly floatHub: string;
            readonly fileViews: string;
            readonly files: string;
            readonly fileOperations: string;
            readonly workspaceOperations: string;
            readonly viewOperations: string;
            readonly paletteOperations: string;
            readonly themeOperations: string;
            readonly lspOperations: string;
            readonly operationMethods: readonly string[];
            readonly openedPanelId: string;
            readonly openedViewId: string;
            readonly sameDocument: boolean;
          }) => void (probe.readyPayload = payload),
          viewAttached: (payload: { readonly mountHub: boolean; readonly floatHub: boolean }) =>
            void (probe.viewPayload = payload),
          mounted: (id: string, text: string) => void (probe.mountedPayload = { id, text }),
        };

        const servicesForExtensions = Layer.mergeAll(
          Layer.succeed(Commands, commands),
          Layer.succeed(FileSystemPort, fs.port),
          Layer.succeed(PackageRuntimeService, runtime),
          Layer.succeed(Attention, attention),
          Layer.succeed(WorkspaceRoots, workspaceRoots),
          Layer.succeed(Files, files),
          Layer.succeed(FileViews, fileViews),
          Layer.succeed(FileOperations, {
            save: () => {},
            open: () => {},
            find: () => {},
            findInProject: () => {},
          }),
          Layer.succeed(WorkspaceOperations, {
            openFolder: () => {},
            openSwitcher: () => {},
            closeWorkspace: () => {},
          }),
          Layer.succeed(ViewOperations, {
            splitRight: () => {},
            splitDown: () => {},
            closeTab: () => {},
            nextTab: () => {},
            previousTab: () => {},
            toggleTabs: () => {},
          }),
          Layer.succeed(PaletteOperations, { open: () => {} }),
          Layer.succeed(ThemeOperations, { openSelector: () => {} }),
          Layer.succeed(LspOperations, {
            goToDefinition: () => {},
            formatDocument: () => {},
            renameSymbol: () => {},
            toggleInlayHints: () => {},
            toggleLsp: () => {},
          }),
          Layer.succeed(PanelRenderers, panelRenderers),
          Layer.succeed(CompletionProviders, completions),
          Layer.succeed(HoverProviders, hovers),
          Layer.succeed(SignatureProviders, signatures),
          Layer.succeed(LanguageServers, languageServers),
          Layer.succeed(VcsProviders, vcsProviders),
          Layer.succeed(DiffViewPreferences, diffPreferences),
          Layer.succeed(DecorationProviders, decorations),
        );

        const userExtensions = makeUserExtensionsPackage({
          servicesForExtensions,
          transpiler: identityTranspiler,
          virtualModules: {
            ...buildExtensionVirtualModules(),
            "test:published-primitives-probe": readyProbe,
          },
          homeDir: "/home/user",
        });

        yield* runtime.activateGraph([toPackageDescriptor(userExtensions, servicesForExtensions)]);
        yield* waitUntil(Effect.sync(() => probe.readyPayload !== undefined));

        expect(probe.readyPayload).toMatchObject({
          panelKind: "acme/published-panel",
          mountHub: "function",
          floatHub: "function",
          fileViews: "function",
          files: "function",
          fileOperations: "function",
          workspaceOperations: "function",
          viewOperations: "function",
          paletteOperations: "function",
          themeOperations: "function",
          lspOperations: "function",
          operationMethods: [
            "function",
            "function",
            "function",
            "function",
            "function",
            "function",
          ],
          sameDocument: true,
        });
        expect(probe.readyPayload?.openedPanelId).toMatch(/^panel:/);
        expect(probe.readyPayload?.openedViewId).toMatch(/^view:/);
        expect(yield* workbench.panels).toHaveLength(1);
        const arrangedPanels = collectSlots(yield* workbench.arrangement).flatMap((slot) =>
          slot.kind === "stack" ? slot.stack.panels.map(String) : [String(slot.overlay.panelId)],
        );
        expect(arrangedPanels).toContain(probe.readyPayload?.openedPanelId);
        expect(probe.viewPayload).toEqual({ mountHub: true, floatHub: true });
        const fakeEl = { textContent: "" } as HTMLElement;
        viewMounts.mounted("acme-widget", fakeEl);
        expect(probe.mountedPayload).toEqual({
          id: "acme-widget",
          text: "widget mounted by extension",
        });
        expect(fakeEl.textContent).toBe("widget mounted by extension");
        expect((yield* Context.get(document.context, EditorVisuals).snapshot).totalWidgetRows).toBe(
          1,
        );
        expect(panelRenderers.get(SurfaceKind("acme/published-panel"))?.title({} as never)).toBe(
          "Published",
        );
        expect((yield* completions.list).map((provider) => provider.id)).toEqual(["acme"]);
        expect((yield* hovers.list).map((provider) => provider.id)).toEqual(["acme"]);
        expect((yield* signatures.list).map((provider) => provider.id)).toEqual(["acme"]);
        expect((yield* languageServers.list).map((server) => server.id)).toEqual(["acme-lsp"]);
        expect((yield* vcsProviders.list).map((provider) => provider.id)).toEqual(["acme-vcs"]);
        expect(diffPreferences.current().wrapping).toBe(true);
      }),
    ),
  );

  it.live("refuses to load an extension whose id collides with an active package", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const fs = makeFakeFs();
        fs.write(
          "/home/user/.the-editor/extensions/a.ts",
          packageSource("acme.shared", "acme.a", "From A"),
        );
        fs.write(
          "/home/user/.the-editor/extensions/b.ts",
          packageSource("acme.shared", "acme.b", "From B"),
        );

        const runtime = yield* makePackageRuntime;
        const attention = yield* makeAttention();
        const workspaceRoots = yield* makeStateCell<ReadonlyArray<string>>([]);
        const commands = yield* makeCommands(runtime.contributionIndex);

        const servicesForExtensions = Layer.mergeAll(
          Layer.succeed(Commands, commands),
          Layer.succeed(FileSystemPort, fs.port),
          Layer.succeed(PackageRuntimeService, runtime),
          Layer.succeed(Attention, attention),
          Layer.succeed(WorkspaceRoots, workspaceRoots),
        );

        const userExtensions = makeUserExtensionsPackage({
          servicesForExtensions,
          transpiler: identityTranspiler,
          virtualModules: testVirtualModules,
          homeDir: "/home/user",
        });

        yield* runtime.activateGraph([toPackageDescriptor(userExtensions, servicesForExtensions)]);

        // Exactly one of the two same-id extensions wins; the other is
        // refused (surfaced as an attention notice, not a crash).
        yield* waitUntil(
          Effect.all([commands.get("acme.a"), commands.get("acme.b")]).pipe(
            Effect.map(([a, b]) => (a !== undefined) !== (b !== undefined)),
          ),
        );
      }),
    ),
  );

  it.live("does not load project-local extensions without trust", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const fs = makeFakeFs();
        fs.write(
          "/workspace/.the-editor/extensions/proj.ts",
          packageSource("acme.project", "acme.project", "Project ext"),
        );

        const runtime = yield* makePackageRuntime;
        const attention = yield* makeAttention(); // no responder registered
        const workspaceRoots = yield* makeStateCell<ReadonlyArray<string>>([]);
        const commands = yield* makeCommands(runtime.contributionIndex);

        const servicesForExtensions = Layer.mergeAll(
          Layer.succeed(Commands, commands),
          Layer.succeed(FileSystemPort, fs.port),
          Layer.succeed(PackageRuntimeService, runtime),
          Layer.succeed(Attention, attention),
          Layer.succeed(WorkspaceRoots, workspaceRoots),
        );

        const userExtensions = makeUserExtensionsPackage({
          servicesForExtensions,
          transpiler: identityTranspiler,
          virtualModules: testVirtualModules,
          homeDir: undefined, // no global dir either: only the project path is exercised
        });

        yield* runtime.activateGraph([toPackageDescriptor(userExtensions, servicesForExtensions)]);

        yield* workspaceRoots.set(["/workspace"]);
        // With no responder, `ask`'s `default: "skip"` resolves immediately —
        // give the reactive fiber a moment to run, then assert nothing loaded.
        yield* Effect.sleep("30 millis");
        const command = yield* commands.get("acme.project");
        expect(command).toBeUndefined();
      }),
    ),
  );

  it.live("loads project-local extensions once the workspace is trusted", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const fs = makeFakeFs();
        fs.write(
          "/workspace/.the-editor/extensions/proj.ts",
          packageSource("acme.project", "acme.project", "Project ext"),
        );

        const runtime = yield* makePackageRuntime;
        const attention = yield* makeAttention();
        const workspaceRoots = yield* makeStateCell<ReadonlyArray<string>>([]);
        const commands = yield* makeCommands(runtime.contributionIndex);

        const servicesForExtensions = Layer.mergeAll(
          Layer.succeed(Commands, commands),
          Layer.succeed(FileSystemPort, fs.port),
          Layer.succeed(PackageRuntimeService, runtime),
          Layer.succeed(Attention, attention),
          Layer.succeed(WorkspaceRoots, workspaceRoots),
        );

        const userExtensions = makeUserExtensionsPackage({
          servicesForExtensions,
          transpiler: identityTranspiler,
          virtualModules: testVirtualModules,
          homeDir: undefined,
        });

        yield* runtime.activateGraph([toPackageDescriptor(userExtensions, servicesForExtensions)]);

        // A responder that always answers "trust", mirroring the app's real
        // trust dialog answering yes. Registered in this same scope, so it's
        // torn down automatically when the test's `Effect.scoped` closes.
        yield* attention.registerResponder;
        yield* Effect.forkScoped(
          Stream.runForEach(attention.requestChanges, (pending) =>
            Effect.forEach(pending, (p) => p.respond("trust"), { discard: true }),
          ),
        );

        yield* workspaceRoots.set(["/workspace"]);
        yield* waitUntil(commands.get("acme.project").pipe(Effect.map((c) => c !== undefined)));
      }),
    ),
  );

  it.live(
    "survives concurrent reload command invocations and a root change racing each other",
    () =>
      Effect.scoped(
        Effect.gen(function* () {
          const fs = makeFakeFs(3);
          const helloActivations = { count: 0, increment: () => void helloActivations.count++ };
          fs.write(
            "/home/user/.the-editor/extensions/hello.ts",
            packageSource("acme.hello", "acme.hello", "Hello", {
              countActivationsAs: "test:hello-counter",
            }),
          );
          fs.write(
            "/workspace/.the-editor/extensions/proj.ts",
            packageSource("acme.project", "acme.project", "Project ext"),
          );

          const runtime = yield* makePackageRuntime;
          const attention = yield* makeAttention();
          const workspaceRoots = yield* makeStateCell<ReadonlyArray<string>>([]);
          const commands = yield* makeCommands(runtime.contributionIndex);

          const servicesForExtensions = Layer.mergeAll(
            Layer.succeed(Commands, commands),
            Layer.succeed(FileSystemPort, fs.port),
            Layer.succeed(PackageRuntimeService, runtime),
            Layer.succeed(Attention, attention),
            Layer.succeed(WorkspaceRoots, workspaceRoots),
          );

          const userExtensions = makeUserExtensionsPackage({
            servicesForExtensions,
            transpiler: identityTranspiler,
            virtualModules: { ...testVirtualModules, "test:hello-counter": helloActivations },
            homeDir: "/home/user",
          });

          yield* attention.registerResponder;
          const failures: string[] = [];
          yield* Effect.forkScoped(
            Stream.runForEach(attention.notices, (notice) =>
              Effect.sync(() => {
                if (notice.level === "error") failures.push(JSON.stringify(notice.payload));
              }),
            ),
          );
          yield* Effect.forkScoped(
            Stream.runForEach(attention.requestChanges, (pending) =>
              Effect.forEach(pending, (p) => p.respond("trust"), { discard: true }),
            ),
          );

          // Package activation registers the command synchronously and only
          // FORKS its first discover+reconcile pass — nothing is loaded yet
          // when this resolves.
          yield* runtime.activateGraph([
            toPackageDescriptor(userExtensions, servicesForExtensions),
          ]);

          // Fire the whole storm immediately: the boot fork's own first
          // reconcile pass races directly against several manual reloads and
          // a workspace-root change, all trying to be the first to discover
          // and activate "acme.hello" — exactly the boot-fork / reactive-
          // subscription / command triangle `reloadAll`'s semaphore exists to
          // serialize.
          yield* Effect.all(
            [
              ...Array.from({ length: 8 }, () => commands.execute("reload")),
              workspaceRoots.set(["/workspace"]),
              commands.execute("reload"),
            ],
            { concurrency: "unbounded", discard: true },
          );

          // 10 producers, fully serialized through one semaphore permit,
          // each paying the fake fs's 10ms delay several times over — give
          // this a wider poll budget than the single-producer tests above.
          yield* waitUntil(
            commands.get("acme.project").pipe(Effect.map((c) => c !== undefined)),
            1000,
          );
          yield* waitUntil(
            commands.get("acme.hello").pipe(Effect.map((c) => c !== undefined)),
            1000,
          );

          // Exactly one live registration each — no duplicate-activation
          // collisions, no dispose/activate failures surfaced. The activation
          // counter is the direct signal: without `reloadAll`'s semaphore,
          // several concurrent reconcile passes each see "acme.hello" as
          // not-yet-active and independently activate it.
          const hello = yield* commands.get("acme.hello");
          const project = yield* commands.get("acme.project");
          expect(hello?.contribution.title).toEqual("Hello");
          expect(project?.contribution.title).toEqual("Project ext");
          expect(failures).toEqual([]);
          expect(helloActivations.count).toBe(1);
        }),
      ),
    10_000,
  );
});
