import { useCallback, useEffect, useState } from "react";
import { Effect } from "effect";
import type { EditorSnapshot } from "@the-editor/editor";
import { formatChords, type Chord } from "@the-editor/keymaps";
import type { Tile } from "@the-editor/workbench";
import { AppRuntime, type EditorPane } from "./runtime";
import { WorkbenchView } from "./workbench/WorkbenchView";
import { CommandPalette } from "./ui/CommandPalette";
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
  const [rename, setRename] = useState<{ open: boolean; initial: string }>({
    open: false,
    initial: "",
  });
  // Bumped on focus/file changes so the status bar re-reads the focused pane.
  const [, setStatusEpoch] = useState(0);

  useEffect(() => {
    const booted = AppRuntime.boot("");
    booted.onFileStateChanged = () => setStatusEpoch((epoch) => epoch + 1);
    booted.onPaletteOpenRequested = () => setPaletteOpen(true);
    booted.onDiagnosticsChanged = () => setStatusEpoch((epoch) => epoch + 1);
    const unsubscribeVcs = booted.vcs?.subscribe(() => setStatusEpoch((epoch) => epoch + 1));
    booted.onRenameRequested = (currentName) => setRename({ open: true, initial: currentName });
    const controller = new IntelController(
      booted,
      booted.intel.completions,
      booted.intel.hovers,
      booted.intel.signatures,
    );
    booted.onTriggerCompletion = () => controller.triggerCompletion();
    booted.onTriggerSignatureHelp = () => controller.triggerSignatureHelp();
    controller.setPane(booted.focusedPane() ?? null);
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
        vcsDebug: () => ({
          branch: booted.vcs?.branch() ?? null,
          status: booted.vcs?.treeStatusEntries() ?? [],
          ...(booted.vcs?.debugInfo() ?? {}),
        }),
      };
    }
    return () => {
      unsubscribe();
      unsubscribeVcs?.();
      controller.dispose();
      setIntel(null);
      setRuntime(null);
      booted.dispose();
    };
  }, []);

  const handleActiveSnapshot = useCallback((next: EditorSnapshot) => setSnapshot(next), []);
  const handlePending = useCallback((chords: ReadonlyArray<Chord>) => setPending(chords), []);
  const handlePaletteOpenChange = useCallback((open: boolean) => {
    setPaletteOpen(open);
    // Radix restores focus to the previously focused element, but command
    // execution may have swapped the layout out from under it.
    if (!open) {
      window.requestAnimationFrame(() => {
        document
          .querySelector<HTMLElement>(".wb-pane--focused .editor")
          ?.focus({ preventScroll: true });
      });
    }
  }, []);

  if (runtime === null) return <div className="app" />;

  const focusedPane = runtime.focusedPane();
  const head = snapshot?.selection.primary.head;
  const line = snapshot && head !== undefined ? snapshot.text.lineRangeAt(head) : null;
  const diagnostics = focusedPane ? runtime.diagnosticsFor(focusedPane.document.id) : null;

  return (
    <div className="app">
      <WorkbenchView
        runtime={runtime}
        onActiveSnapshot={handleActiveSnapshot}
        onPending={handlePending}
      />
      <CommandPalette
        open={paletteOpen}
        onOpenChange={handlePaletteOpenChange}
        commands={runtime.commands}
      />
      {intel !== null && <IntelView controller={intel} />}
      <RenameDialog
        open={rename.open}
        initialValue={rename.initial}
        onSubmit={(value) => void runtime.performRename(value)}
        onClose={() => {
          setRename((state) => ({ ...state, open: false }));
          window.requestAnimationFrame(() => {
            document
              .querySelector<HTMLElement>(".wb-pane--focused .editor")
              ?.focus({ preventScroll: true });
          });
        }}
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
