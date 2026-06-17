import { useCallback, useEffect, useRef, useState } from "react";
import { Effect } from "effect";
import type { EditorSnapshot } from "@the-editor/editor";
import { formatChords, type Chord } from "@the-editor/keymaps";
import type { Tile } from "@the-editor/workbench";
import { AppRuntime, type EditorPane } from "./runtime";
import { WorkbenchView } from "./workbench/WorkbenchView";
import { TitleBar } from "./ui/TitleBar";
import { CommandPalette, type CommandPaletteMode } from "./ui/CommandPalette";
import { FileFinder } from "./ui/FileFinder";
import { SearchDialog } from "./ui/SearchDialog";
import { RenameDialog } from "./ui/RenameDialog";
import { IntelController } from "./intel/controller";
import { IntelView } from "./intel/IntelView";

const fileLabel = (pane: EditorPane | undefined): string => {
  if (pane === undefined) return "";
  const info = pane.fileInfo();
  if (info === null) return "untitled";
  const name = info.path.split("/").pop() ?? info.path;
  if (info.status === "conflict") return `${name} · CONFLICT (file changed on disk)`;
  if (info.status === "missing") return `${name} · deleted on disk`;
  return info.status === "dirty" ? `${name} •` : name;
};

/** Serializable layout summary for dev tooling and headless verification. */
const layoutSummary = (runtime: AppRuntime): unknown => {
  const { arrangement, focusedSlot } = runtime.getWorkbenchSnapshot();
  const walk = (tile: Tile): unknown =>
    tile.kind === "stack"
      ? {
          stack: tile.id,
          focused: tile.id === focusedSlot,
          tabsHidden: tile.tabsHidden === true,
          active: tile.active,
          tabs: tile.panels.map((panelId) => ({
            panel: panelId,
            file: runtime.pane(panelId)?.fileInfo()?.path ?? null,
          })),
        }
      : {
          split: tile.direction,
          children: tile.children.map((child) => ({
            weight: Number(child.weight.toFixed(3)),
            tile: walk(child.tile),
          })),
        };
  return arrangement.tiles === null ? null : walk(arrangement.tiles);
};

export function App() {
  // The runtime is an external resource, so the owning effect creates AND
  // disposes it: under React StrictMode's mount -> cleanup -> remount cycle,
  // each mount gets a fresh runtime instead of reusing a disposed one.
  const [runtime, setRuntime] = useState<AppRuntime | null>(null);
  const [intel, setIntel] = useState<IntelController | null>(null);
  const [snapshot, setSnapshot] = useState<EditorSnapshot | null>(null);
  const [pending, setPending] = useState<ReadonlyArray<Chord>>([]);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [paletteMode, setPaletteMode] = useState<CommandPaletteMode>("commands");
  const [fileFinderOpen, setFileFinderOpen] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [rename, setRename] = useState<{ open: boolean; initial: string }>({
    open: false,
    initial: "",
  });
  // Generic string prompt (new file/folder names, etc.); resolves null on cancel.
  const [prompt, setPrompt] = useState<{ initial: string; placeholder: string } | null>(null);
  const promptResolve = useRef<((value: string | null) => void) | null>(null);
  // Bumped on focus/file changes so the status bar re-reads the focused pane.
  const [, setStatusEpoch] = useState(0);

  useEffect(() => {
    const booted = AppRuntime.boot("");
    booted.onFileStateChanged = () => setStatusEpoch((epoch) => epoch + 1);
    booted.onPaletteOpenRequested = () => {
      setPaletteMode("commands");
      setPaletteOpen(true);
    };
    booted.onFileFinderOpenRequested = () => setFileFinderOpen(true);
    booted.onSearchOpenRequested = () => setSearchOpen(true);
    booted.onThemeSelectorOpenRequested = () => {
      setPaletteMode("themes");
      setPaletteOpen(true);
    };
    booted.onDiagnosticsChanged = () => setStatusEpoch((epoch) => epoch + 1);
    const unsubscribeVcs = booted.vcs?.subscribe(() => setStatusEpoch((epoch) => epoch + 1));
    const unsubscribeTheme = booted.subscribeTheme(() => setStatusEpoch((epoch) => epoch + 1));
    booted.onRenameRequested = (currentName) => setRename({ open: true, initial: currentName });
    booted.onPrompt = (options) =>
      new Promise<string | null>((resolve) => {
        promptResolve.current = resolve;
        setPrompt({ initial: options.initial ?? "", placeholder: options.placeholder ?? "New name" });
      });
    const controller = new IntelController(
      booted,
      booted.intel.completions,
      booted.intel.hovers,
      booted.intel.signatures,
    );
    booted.onTriggerCompletion = () => controller.triggerCompletion();
    booted.onTriggerSignatureHelp = () => controller.triggerSignatureHelp();
    booted.onPendingChords = (chords) => setPending(chords);
    controller.setPane(booted.focusedPane() ?? null);

    // ONE keyboard entry point for the whole app: bubble-phase, document-level.
    // Cooperative leaves (the owned editor, @pierre/trees) stop propagation on
    // the keys they own, so those never reach here; everything else (the global
    // palette binding, splits, etc.) bubbles up and the runtime dispatches it —
    // which is what makes every surface, file tree included, keyboard-driven.
    const onKeyDown = (event: KeyboardEvent) => booted.dispatchKeyboardEvent(event);
    document.addEventListener("keydown", onKeyDown);
    setIntel(controller);
    const unsubscribe = booted.subscribeWorkbench(() => {
      setStatusEpoch((epoch) => epoch + 1);
      setSnapshot(booted.focusedPane()?.snapshot() ?? null);
      controller.setPane(booted.focusedPane() ?? null);
    });
    setRuntime(booted);
    setSnapshot(booted.focusedPane()?.snapshot() ?? null);
    const isDev = location.hostname === "localhost" || location.hostname === "127.0.0.1";
    if (isDev) {
      (window as unknown as Record<string, unknown>).__editorHost = {
        open: (path: string) => booted.openPath(path),
        save: () => booted.saveActiveFile(),
        info: () => booted.focusedPane()?.fileInfo() ?? null,
        layout: () => layoutSummary(booted),
        definition: () => booted.goToDefinition(),
        format: () => booted.formatActiveDocument(),
        intel: () => JSON.parse(JSON.stringify(controller.getState())),
        intelDebug: () => controller.debugInfo(),
        openFolder: (path: string) => booted.openFolder(path),
        toggleTree: () => {
          void Effect.runPromise(
            booted.commands.execute("files.toggleTree").pipe(Effect.catchAll(() => Effect.void)),
          );
        },
        findFile: () => setFileFinderOpen(true),
        searchProject: (query: string) => booted.searchProject(query),
        multiBuffer: (
          specs: readonly { path: string; text: string; matches: readonly { start: number; end: number }[] }[],
          title?: string,
        ) => booted.openMultiBufferFromText(specs, title ?? "Search results"),
        vcsDebug: () => ({
          branch: booted.vcs?.branch() ?? null,
          status: booted.vcs?.treeStatusEntries() ?? [],
          ...(booted.vcs?.debugInfo() ?? {}),
        }),
      };
    }
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      unsubscribe();
      unsubscribeVcs?.();
      unsubscribeTheme();
      controller.dispose();
      setIntel(null);
      setRuntime(null);
      booted.dispose();
    };
  }, []);

  const handleActiveSnapshot = useCallback((next: EditorSnapshot) => setSnapshot(next), []);
  // Resolve the pending prompt exactly once (Enter fires onSubmit then onClose).
  const finishPrompt = useCallback((value: string | null) => {
    const resolve = promptResolve.current;
    promptResolve.current = null;
    setPrompt(null);
    resolve?.(value);
    window.requestAnimationFrame(() => runtime?.focusActivePanel());
  }, [runtime]);
  const handlePaletteOpenChange = useCallback((open: boolean) => {
    setPaletteOpen(open);
    // Radix restores focus to the previously focused element, but command
    // execution may have swapped the layout out from under it.
    if (!open) {
      window.requestAnimationFrame(() => runtime?.focusActivePanel());
      setPaletteMode("commands");
    }
  }, [runtime]);

  if (runtime === null) return <div className="app" />;

  const focusedPane = runtime.focusedPane();
  const head = snapshot?.selection.primary.head;
  const line = snapshot && head !== undefined ? snapshot.text.lineRangeAt(head) : null;
  const diagnostics = focusedPane ? runtime.diagnosticsFor(focusedPane.document.id) : null;

  return (
    <div className="app">
      <TitleBar runtime={runtime} />
      <WorkbenchView
        runtime={runtime}
        onActiveSnapshot={handleActiveSnapshot}
      />
      <CommandPalette
        open={paletteOpen}
        mode={paletteMode}
        onOpenChange={handlePaletteOpenChange}
        commands={runtime.commands}
        themes={runtime.themes}
      />
      <FileFinder
        open={fileFinderOpen}
        runtime={runtime}
        onOpenChange={(open) => {
          setFileFinderOpen(open);
          if (!open) window.requestAnimationFrame(() => runtime.focusActivePanel());
        }}
      />
      <SearchDialog
        open={searchOpen}
        runtime={runtime}
        onOpenChange={(open) => {
          setSearchOpen(open);
          if (!open) window.requestAnimationFrame(() => runtime.focusActivePanel());
        }}
      />
      {intel !== null && <IntelView controller={intel} />}
      <RenameDialog
        open={rename.open}
        initialValue={rename.initial}
        onSubmit={(value) => void runtime.performRename(value)}
        onClose={() => {
          setRename((state) => ({ ...state, open: false }));
          window.requestAnimationFrame(() => runtime?.focusActivePanel());
        }}
      />
      <RenameDialog
        open={prompt !== null}
        initialValue={prompt?.initial ?? ""}
        placeholder={prompt?.placeholder ?? "New name"}
        ariaLabel="Prompt"
        onSubmit={(value) => finishPrompt(value.length > 0 ? value : null)}
        onClose={() => finishPrompt(null)}
      />
      <div className="status-bar">
        <span>
          {runtime?.vcs?.branch() != null && (
            <span className="status-branch">{runtime.vcs.branch()} · </span>
          )}
          {fileLabel(focusedPane)}
          {line !== null && head !== undefined
            ? ` · Ln ${line.index + 1}, Col ${head - line.from + 1}`
            : ""}
          {diagnostics !== null && diagnostics.errors + diagnostics.warnings > 0 && (
            <span className="status-diagnostics">
              {diagnostics.errors > 0 && (
                <span className="status-diagnostics-errors"> ✗ {diagnostics.errors}</span>
              )}
              {diagnostics.warnings > 0 && (
                <span className="status-diagnostics-warnings"> ⚠ {diagnostics.warnings}</span>
              )}
            </span>
          )}
          {pending.length > 0 ? ` · ${formatChords(pending)} …` : ""}
        </span>
        <span>
          {snapshot !== null ? `${snapshot.text.length} chars · v${snapshot.version}` : ""}
          {snapshot?.dirty ? " · modified" : ""}
        </span>
      </div>
    </div>
  );
}
