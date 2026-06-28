import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import { Effect } from "effect";
import type { EditorSnapshot } from "@the-editor/editor";
import { formatChords, type Chord } from "@the-editor/keymaps";
import type { Tile } from "@the-editor/workbench";
import { AppRuntime, type EditorPane } from "./runtime";
import { Shell } from "./shell";
import { WorkbenchView } from "./workbench/WorkbenchView";
import type { ChromeSlot } from "./workbench/shellHost";
import { isMacDesktop, isDesktop } from "./platform/desktop";
import { CommandPalette, type CommandPaletteMode } from "./ui/CommandPalette";
import { FileFinder } from "./ui/FileFinder";
import { GlobalSearch } from "./ui/GlobalSearch";
import { RenameDialog } from "./ui/RenameDialog";
import { AttentionLayer } from "./ui/AttentionLayer";
import { NotificationCenter } from "./ui/NotificationCenter";
import { IntelController } from "./intel/controller";
import { IntelView } from "./intel/IntelView";

/** Left inset reserving the macOS traffic lights (plus a window-drag handle).
 *  Sized to just clear the lights now that there's no session sidebar. */
const MAC_TRAFFIC_INSET = 80;
/** Non-mac desktop: a small top-left window-drag handle. */
const DESKTOP_INSET = 40;

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
          kind: "stack",
          id: tile.id,
          focused: tile.id === focusedSlot,
          active: tile.active,
          panels: tile.panels,
        }
      : {
          kind: "split",
          direction: tile.direction,
          children: tile.children.map((child) => walk(child.tile)),
        };
  return arrangement.tiles === null ? null : walk(arrangement.tiles);
};

type WorkspaceId = string;

/**
 * One open workspace: a fully isolated editor session (its own AppRuntime,
 * documents, layout, LSP/VCS/terminals) plus its intel controller. Sessions
 * stay mounted while inactive so their terminals and editor state survive a
 * switch; only the active one shows, receives the keyboard, and drives the
 * shared chrome (dialogs, status bar).
 */
type WorkspaceSession = {
  readonly id: WorkspaceId;
  readonly runtime: AppRuntime;
  readonly intel: IntelController;
  readonly root: string | null;
  readonly dispose: () => void;
};

let nextWorkspaceId = 1;

/** Tab label: the workspace folder's basename, or "untitled" before a folder
 *  is opened into the session. */
const workspaceLabel = (root: string | null): string => {
  if (root === null) return "untitled";
  const trimmed = root.replace(/\/+$/, "");
  return trimmed.split("/").pop() || trimmed || "untitled";
};

/** Track the live DOM element a chrome panel's renderer publishes (null until
 *  the shell workbench mounts it), so App can portal content into it. */
function useChromeSlotElement(slot: ChromeSlot | undefined): HTMLElement | null {
  return useSyncExternalStore(
    useCallback((cb: () => void) => slot?.subscribe(cb) ?? (() => {}), [slot]),
    useCallback(() => slot?.getElement() ?? null, [slot]),
  );
}

/** The file-tree toggle: a real action in the tab strip (never a window
 *  overlay). Its glyph's divider sits on the docked SIDE so the icon reads as
 *  the sidebar it controls; `SessionView` places it on that same side. */
function FileTreeToggle({
  runtime,
  side,
}: {
  readonly runtime: AppRuntime;
  readonly side: "left" | "right";
}) {
  const visible = useSyncExternalStore(
    useCallback((cb: () => void) => runtime.subscribeWorkbench(cb), [runtime]),
    useCallback(() => runtime.isFileTreeVisible(), [runtime]),
  );
  const dividerX = side === "left" ? 6.15 : 9.85;
  return (
    <button
      type="button"
      className="wb-tabbar-toggle"
      data-active={visible ? "" : undefined}
      onClick={() =>
        void Effect.runPromise(
          runtime.commands.execute("files.toggleTree").pipe(Effect.catchAll(() => Effect.void)),
        )
      }
      title="Toggle File Tree"
      aria-label="Toggle File Tree"
    >
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <rect x="1.85" y="2.85" width="12.3" height="10.3" rx="2.4" stroke="currentColor" strokeWidth="1.3" />
        <line x1={dividerX} y1="3.1" x2={dividerX} y2="12.9" stroke="currentColor" strokeWidth="1.3" />
      </svg>
    </button>
  );
}

/** One mounted session's workbench. Subscribes to the runtime so the file-tree
 *  toggle follows its sidebar: the toggle (and its glyph) render on whichever
 *  side the tree docks to, flipping live when the user flips the sidebar. */
function SessionView({
  session,
  active,
  onActiveSnapshot,
}: {
  readonly session: WorkspaceSession;
  readonly active: boolean;
  readonly onActiveSnapshot: (snapshot: EditorSnapshot) => void;
}) {
  const side = useSyncExternalStore(
    useCallback((cb: () => void) => session.runtime.subscribeWorkbench(cb), [session.runtime]),
    useCallback(() => session.runtime.fileTreeSide(), [session.runtime]),
  );
  const toggle = <FileTreeToggle runtime={session.runtime} side={side} />;
  return (
    <WorkbenchView
      runtime={session.runtime}
      active={active}
      firstTabInset={isMacDesktop ? MAC_TRAFFIC_INSET : isDesktop ? DESKTOP_INSET : 0}
      {...(side === "left" ? { leadingAction: toggle } : { trailingAction: toggle })}
      {...(active ? { onActiveSnapshot } : {})}
    />
  );
}

export function App() {
  // The shell (shared theme + appearance) and the workspace sessions are
  // external resources, so the owning effect creates AND disposes them: under
  // React StrictMode's mount -> cleanup -> remount cycle, each mount gets a
  // fresh shell + sessions instead of reusing disposed ones.
  const [shell, setShell] = useState<Shell | null>(null);
  const [sessions, setSessions] = useState<ReadonlyArray<WorkspaceSession>>([]);
  const [activeId, setActiveId] = useState<WorkspaceId | null>(null);

  const [snapshot, setSnapshot] = useState<EditorSnapshot | null>(null);
  const [pending, setPending] = useState<ReadonlyArray<Chord>>([]);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [paletteMode, setPaletteMode] = useState<CommandPaletteMode>("commands");
  const [fileFinderOpen, setFileFinderOpen] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [rename, setRename] = useState<{ open: boolean; initial: string }>({ open: false, initial: "" });
  // Generic string prompt (new file/folder names, etc.); resolves null on cancel.
  const [prompt, setPrompt] = useState<{ initial: string; placeholder: string } | null>(null);
  const promptResolve = useRef<((value: string | null) => void) | null>(null);
  // Bumped on focus/file changes so the status bar re-reads the focused pane.
  const [, setStatusEpoch] = useState(0);

  const active = sessions.find((session) => session.id === activeId) ?? null;
  // Stable handles for the document-level listener and dialog callbacks, which
  // outlive any single session and must always reach whichever one is active.
  const activeIdRef = useRef<WorkspaceId | null>(null);
  activeIdRef.current = activeId;
  const sessionsRef = useRef<ReadonlyArray<WorkspaceSession>>(sessions);
  sessionsRef.current = sessions;
  const activeRuntimeRef = useRef<AppRuntime | null>(null);
  activeRuntimeRef.current = active?.runtime ?? null;
  const activeIntelRef = useRef<IntelController | null>(null);
  activeIntelRef.current = active?.intel ?? null;
  // Workspace lifecycle is owned here but reached from runtime command hooks
  // (set in makeSession, which captures the first render). Route through refs
  // so the commands always hit the current handlers.
  const openFolderRef = useRef<() => void>(() => {});
  const closeWorkspaceRef = useRef<(id: WorkspaceId) => void>(() => {});

  // The viewport panel's DOM element; App portals the per-session workbenches
  // into it.
  const viewportEl = useChromeSlotElement(shell?.viewportSlot);

  // Build a session: boot a runtime, wire its host hooks to the shared chrome,
  // and attach its intel controller. Callbacks that fire from BACKGROUND work
  // (status, diagnostics, snapshots) are gated to the active session so a
  // hidden workspace never clobbers what the chrome shows; dialog-open hooks
  // need no gate (only the active session receives the keys that trigger them).
  const makeSession = useCallback((host: Shell, root: string | null): WorkspaceSession => {
    const id = `ws-${nextWorkspaceId++}`;
    const runtime = AppRuntime.boot("", host);
    const isActive = () => activeIdRef.current === id;

    runtime.onFileStateChanged = () => {
      if (isActive()) setStatusEpoch((epoch) => epoch + 1);
    };
    runtime.onPaletteOpenRequested = () => {
      setPaletteMode("commands");
      setPaletteOpen(true);
    };
    runtime.onFileFinderOpenRequested = () => setFileFinderOpen(true);
    runtime.onSearchOpenRequested = () => setSearchOpen(true);
    runtime.onThemeSelectorOpenRequested = () => {
      setPaletteMode("themes");
      setPaletteOpen(true);
    };
    runtime.onWorkspaceSwitcherRequested = () => {
      setPaletteMode("workspaces");
      setPaletteOpen(true);
    };
    runtime.onWorkspaceOpenFolderRequested = () => openFolderRef.current();
    runtime.onWorkspaceCloseRequested = () => {
      const id = activeIdRef.current;
      if (id !== null) closeWorkspaceRef.current(id);
    };
    runtime.onDiagnosticsChanged = () => {
      if (isActive()) setStatusEpoch((epoch) => epoch + 1);
    };
    runtime.onRenameRequested = (currentName) => setRename({ open: true, initial: currentName });
    runtime.onPrompt = (options) =>
      new Promise<string | null>((resolve) => {
        promptResolve.current = resolve;
        setPrompt({ initial: options.initial ?? "", placeholder: options.placeholder ?? "New name" });
      });

    const intel = new IntelController(
      runtime,
      runtime.intel.completions,
      runtime.intel.hovers,
      runtime.intel.signatures,
    );
    runtime.onTriggerCompletion = () => intel.triggerCompletion();
    runtime.onTriggerSignatureHelp = () => intel.triggerSignatureHelp();
    runtime.onPendingChords = (chords) => {
      if (isActive()) setPending(chords);
    };
    intel.setPane(runtime.focusedPaneHost() ?? null);

    const unsubscribeWorkbench = runtime.subscribeWorkbench(() => {
      intel.setPane(runtime.focusedPaneHost() ?? null);
      if (isActive()) {
        setStatusEpoch((epoch) => epoch + 1);
        setSnapshot(runtime.focusedPaneHost()?.snapshot() ?? null);
      }
    });
    const unsubscribeVcs = runtime.vcs?.subscribe(() => {
      if (isActive()) setStatusEpoch((epoch) => epoch + 1);
    });
    const unsubscribeRoot = runtime.subscribeWorkspaceRoot((nextRoot) =>
      setSessions((prev) => prev.map((session) => (session.id === id ? { ...session, root: nextRoot } : session))),
    );

    if (root !== null) runtime.openFolder(root);

    const dispose = () => {
      unsubscribeWorkbench();
      unsubscribeVcs?.();
      unsubscribeRoot();
      intel.dispose();
      runtime.dispose();
    };
    return { id, runtime, intel, root, dispose };
  }, []);

  useEffect(() => {
    const bootedShell = Shell.boot();
    // The chrome host's focusActivePanel delegates here, so a sidebar click
    // keeps the keyboard in whichever session is active.
    bootedShell.onFocusActiveSession = () => activeRuntimeRef.current?.focusActivePanel();
    const unsubscribeTheme = bootedShell.subscribeTheme(() => setStatusEpoch((epoch) => epoch + 1));
    const first = makeSession(bootedShell, null);
    setShell(bootedShell);
    setSessions([first]);
    setActiveId(first.id);

    // ONE keyboard entry point for the whole app: bubble-phase, document-level,
    // routed to the ACTIVE session. Cooperative leaves (the owned editor,
    // @pierre/trees) stop propagation on the keys they own; everything else
    // bubbles here and the active runtime dispatches it.
    const onKeyDown = (event: KeyboardEvent) => activeRuntimeRef.current?.dispatchKeyboardEvent(event);
    document.addEventListener("keydown", onKeyDown);

    const isDev = location.hostname === "localhost" || location.hostname === "127.0.0.1";
    if (isDev) {
      const host = () => activeRuntimeRef.current;
      (window as unknown as Record<string, unknown>).__editorHost = {
        open: (path: string) => host()?.openPath(path),
        save: () => host()?.saveActiveFile(),
        info: () => host()?.focusedPane()?.fileInfo() ?? null,
        layout: () => {
          const runtime = host();
          return runtime ? layoutSummary(runtime) : null;
        },
        definition: () => host()?.goToDefinition(),
        format: () => host()?.formatActiveDocument(),
        intel: () => {
          const controller = activeIntelRef.current;
          return controller ? JSON.parse(JSON.stringify(controller.getState())) : null;
        },
        intelDebug: () => activeIntelRef.current?.debugInfo(),
        openFolder: (path: string) => host()?.openFolder(path),
        toggleTree: () => {
          const runtime = host();
          if (runtime === null) return;
          void Effect.runPromise(
            runtime.commands.execute("files.toggleTree").pipe(Effect.catchAll(() => Effect.void)),
          );
        },
        openDiffView: () => {
          const runtime = host();
          if (runtime === null) return;
          void Effect.runPromise(
            runtime.commands.execute("diffs.openDiffView").pipe(Effect.catchAll(() => Effect.void)),
          );
        },
        findFile: () => setFileFinderOpen(true),
        searchProject: (query: string) => host()?.searchProject(query),
        multiBuffer: (
          specs: readonly { path: string; text: string; matches: readonly { start: number; end: number }[] }[],
          title?: string,
        ) => host()?.openMultiBufferFromText(specs, title ?? "Search results"),
        vcsDebug: () => ({
          branch: host()?.vcs?.branch() ?? null,
          status: host()?.vcs?.treeStatusEntries() ?? [],
          ...(host()?.vcs?.debugInfo() ?? {}),
        }),
        // Workspace controls for headless verification.
        workspaces: () =>
          sessionsRef.current.map((session) => ({
            id: session.id,
            root: session.root,
            active: session.id === activeIdRef.current,
          })),
        addWorkspace: (path?: string) => {
          const session = makeSession(bootedShell, path ?? null);
          setSessions((prev) => [...prev, session]);
          setActiveId(session.id);
          return session.id;
        },
        switchWorkspace: (id: string) => setActiveId(id),
      };
    }

    return () => {
      document.removeEventListener("keydown", onKeyDown);
      unsubscribeTheme();
      setSessions((prev) => {
        for (const session of prev) session.dispose();
        return [];
      });
      setActiveId(null);
      setShell(null);
      bootedShell.dispose();
    };
  }, [makeSession]);

  // On switch (only): close dialogs so they don't act on the previous session,
  // re-sync the chrome to the new active session, and land the keyboard in it.
  useEffect(() => {
    if (activeId === null) return;
    const session = sessionsRef.current.find((entry) => entry.id === activeId);
    if (session === undefined) return;
    setPaletteOpen(false);
    setFileFinderOpen(false);
    setSearchOpen(false);
    setPending([]);
    setSnapshot(session.runtime.focusedPaneHost()?.snapshot() ?? null);
    setStatusEpoch((epoch) => epoch + 1);
    window.requestAnimationFrame(() => {
      sessionsRef.current.find((entry) => entry.id === activeId)?.runtime.focusActivePanel();
    });
  }, [activeId]);

  // The one creation primitive: open a folder. If the active workspace is still
  // an untitled scratch (no root), open the folder INTO it so we don't pile up
  // empty tabs; otherwise spawn a fresh workspace. Cancelling the dialog leaves
  // whatever was there (an untitled scratch the user can close).
  const openWorkspaceFolder = useCallback(() => {
    if (shell === null) return;
    const activeSession = sessionsRef.current.find((entry) => entry.id === activeIdRef.current);
    if (activeSession !== undefined && activeSession.root === null) {
      void activeSession.runtime.openFolderViaDialog();
      return;
    }
    const session = makeSession(shell, null);
    setSessions((prev) => [...prev, session]);
    setActiveId(session.id);
    void session.runtime.openFolderViaDialog();
  }, [shell, makeSession]);

  const switchWorkspace = useCallback((id: WorkspaceId) => setActiveId(id), []);

  const closeWorkspace = useCallback((id: WorkspaceId) => {
    const prev = sessionsRef.current;
    if (prev.length <= 1) return; // never close the last workspace
    const index = prev.findIndex((session) => session.id === id);
    if (index === -1) return;
    const target = prev[index];
    const next = prev.filter((session) => session.id !== id);
    setSessions(next);
    if (activeIdRef.current === id) {
      setActiveId(next[Math.min(index, next.length - 1)]?.id ?? null);
    }
    // Defer teardown until React has unmounted this session's WorkbenchView
    // (portals torn down) so we never dispose a runtime its view still renders.
    if (target !== undefined) window.requestAnimationFrame(() => target.dispose());
  }, []);

  // Keep the runtime-command hooks pointed at the live lifecycle handlers.
  openFolderRef.current = openWorkspaceFolder;
  closeWorkspaceRef.current = closeWorkspace;

  // The switcher's data + actions, projected from App's own session state — the
  // command palette is a pure view over this; it never owns sessions. The
  // switcher doubles as the manager (open a folder, close a row).
  const workspaces = useMemo(
    () => ({
      items: sessions.map((session) => ({ id: session.id, label: workspaceLabel(session.root) })),
      activeId,
      canClose: sessions.length > 1,
      onSelect: switchWorkspace,
      onClose: closeWorkspace,
      onOpenFolder: openWorkspaceFolder,
    }),
    [sessions, activeId, switchWorkspace, closeWorkspace, openWorkspaceFolder],
  );

  const handleActiveSnapshot = useCallback((next: EditorSnapshot) => setSnapshot(next), []);
  // Resolve the pending prompt exactly once (Enter fires onSubmit then onClose).
  const finishPrompt = useCallback((value: string | null) => {
    const resolve = promptResolve.current;
    promptResolve.current = null;
    setPrompt(null);
    resolve?.(value);
    window.requestAnimationFrame(() => activeRuntimeRef.current?.focusActivePanel());
  }, []);
  const handlePaletteOpenChange = useCallback((open: boolean) => {
    setPaletteOpen(open);
    // Radix restores focus to the previously focused element, but command
    // execution may have swapped the layout out from under it.
    if (!open) {
      window.requestAnimationFrame(() => activeRuntimeRef.current?.focusActivePanel());
      setPaletteMode("commands");
    }
  }, []);

  if (shell === null || active === null) return <div className="app" />;

  const activeRuntime = active.runtime;
  const focusedPane = activeRuntime.focusedPane();
  const head = snapshot?.selection.primary.head;
  const line = snapshot && head !== undefined ? snapshot.text.lineRangeAt(head) : null;
  // Diagnostics for the focused document — the caret's SOURCE doc in a multibuffer.
  const diagnosticDocId =
    head !== undefined ? (activeRuntime.focusedPaneHost()?.resolveSource(head)?.docId ?? null) : null;
  const diagnostics = diagnosticDocId !== null ? activeRuntime.diagnosticsFor(diagnosticDocId) : null;

  return (
    <div className="app">
      <WorkbenchView runtime={shell.chromeHost} active />
      {viewportEl !== null &&
        createPortal(
          sessions.map((session) => (
            <SessionView
              key={session.id}
              session={session}
              active={session.id === activeId}
              onActiveSnapshot={handleActiveSnapshot}
            />
          )),
          viewportEl,
        )}
      <CommandPalette
        open={paletteOpen}
        mode={paletteMode}
        onOpenChange={handlePaletteOpenChange}
        commands={activeRuntime.commands}
        themes={activeRuntime.themes}
        workspaces={workspaces}
      />
      <FileFinder
        open={fileFinderOpen}
        runtime={activeRuntime}
        onOpenChange={(open) => {
          setFileFinderOpen(open);
          if (!open) window.requestAnimationFrame(() => activeRuntime.focusActivePanel());
        }}
      />
      <GlobalSearch
        open={searchOpen}
        runtime={activeRuntime}
        onOpenChange={(open) => {
          setSearchOpen(open);
          if (!open) window.requestAnimationFrame(() => activeRuntime.focusActivePanel());
        }}
      />
      <AttentionLayer runtime={activeRuntime} />
      <NotificationCenter runtime={activeRuntime} />
      <IntelView key={activeId} controller={active.intel} />
      <RenameDialog
        open={rename.open}
        initialValue={rename.initial}
        onSubmit={(value) => void activeRuntime.performRename(value)}
        onClose={() => {
          setRename((state) => ({ ...state, open: false }));
          window.requestAnimationFrame(() => activeRuntime.focusActivePanel());
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
          {activeRuntime.vcs?.branch() != null && (
            <span className="status-branch">{activeRuntime.vcs.branch()} · </span>
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
