import { Component, useEffect, useMemo, useRef, useState, useSyncExternalStore, type CSSProperties, type DragEvent, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import type { CellGridMetrics, RenderFrame, Viewport } from "@the-editor/contracts/runtime";
import type { BufferDecoration, GutterSnapshot, InteractiveSessionSnapshot, ThemeSnapshot, ToolSnapshot } from "@the-editor/editor";
import { surfaceCursorAnchors } from "@the-editor/editor/cursorAnchors";
import type {
  WorkbenchEditorLayoutNodeSnapshot,
  WorkbenchEditorLayoutSnapshot,
  WorkbenchGroupSnapshot,
  WorkbenchItemDescriptor,
  WorkbenchPaneDescriptor,
  WorkbenchSplitNodeSnapshot,
} from "@the-editor/editor/workbench";
import { clearCanvasTextSurface, configureCanvasTextSurface, loadEditorFont, paintCanvasTextSurface, type CanvasTextSurfaceStyleOptions } from "@the-editor/editor/textSurfaceCanvas";
import { RendererPackageRuntime, type WorkbenchSnapshot } from "./uiRuntime.js";
import { WorkbenchDragController, type WorkbenchDragSnapshot } from "./workbenchDrag.js";
import "./style.css";

const uiRuntime = new RendererPackageRuntime();
const EDITOR_SURFACE_STYLE: CanvasTextSurfaceStyleOptions = {
  paddingPx: 8,
  gutterReservePx: 88,
};

interface SurfacePaneSnapshotState {
  lineCount?: number;
  viewport?: Viewport;
  metrics?: CellGridMetrics;
  path?: string;
  title?: string;
  gutters?: GutterSnapshot[];
  decorations?: BufferDecoration[];
}

interface ToolPaneSnapshotState {
  tool?: ToolSnapshot;
}

function App() {
  const [workbench, setWorkbench] = useState<WorkbenchSnapshot>({
    roles: {},
    packages: [],
    diagnostics: [],
    tools: [],
    focus: { kind: "editor" },
    theme: { available: [] },
    syntaxLegend: {},
    sessions: [],
    notifications: [],
    workingDirectory: undefined,
    editorLayout: undefined,
  });
  const [selectionPickerActive, setSelectionPickerActive] = useState(false);
  const [leftSidebarVisible, setLeftSidebarVisible] = useState(true);
  const [leftSidebarWidth, setLeftSidebarWidth] = useState(320);
  const [, setUiRevision] = useState(0);
  const dragController = useMemo(() => new WorkbenchDragController({
    dispatch: (request) => window.theEditor.policyRequest("command.dispatch", request),
  }), []);
  const dragSnapshot = useSyncExternalStore(dragController.subscribe, dragController.snapshot, dragController.snapshot);

  useEffect(() => uiRuntime.subscribe(() => {
    setUiRevision((revision) => revision + 1);
  }), []);

  useEffect(() => {
    const clear = () => dragController.clear();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") clear();
    };
    window.addEventListener("dragend", clear);
    window.addEventListener("drop", clear);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("dragend", clear);
      window.removeEventListener("drop", clear);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [dragController]);

  useEffect(() => {
    dragController.clear();
  }, [dragController, workbench.editorLayout]);

  useEffect(() => {
    return window.theEditor.onPolicyEvent((event) => {
      if (!isObject(event)) return;
      if (event.type === "policy.error") console.error(String(event.message ?? "policy error"));
      if (event.type === "default.sessionUpdated") {
        if (isObject(event.session)) {
          const session = event.session as unknown as InteractiveSessionSnapshot;
          uiRuntime.updateSession(session);
        }
        return;
      }
      if (event.type === "default.sessionDisposed") {
        if (typeof event.sessionId === "string") {
          uiRuntime.removeSession(event.sessionId);
        }
        return;
      }
      if (event.type === "default.sessionChanged") {
        const snapshot: WorkbenchSnapshot = {
          roles: (isObject(event.roles) ? event.roles : {}) as WorkbenchSnapshot["roles"],
          packages: (Array.isArray(event.packages) ? event.packages : []) as WorkbenchSnapshot["packages"],
          diagnostics: (Array.isArray(event.diagnostics) ? event.diagnostics : []) as WorkbenchSnapshot["diagnostics"],
          interaction: isObject(event.interaction) ? event.interaction as unknown as WorkbenchSnapshot["interaction"] : undefined,
          tools: (Array.isArray(event.tools) ? event.tools : []) as WorkbenchSnapshot["tools"],
          focus: isObject(event.focus) ? event.focus as WorkbenchSnapshot["focus"] : undefined,
          theme: isObject(event.theme) ? event.theme as unknown as ThemeSnapshot : undefined,
          syntaxLegend: isObject(event.syntaxLegend) ? event.syntaxLegend as Record<number, string> : {},
          sessions: Array.isArray(event.sessions) ? event.sessions as InteractiveSessionSnapshot[] : [],
          notifications: Array.isArray(event.notifications) ? event.notifications as WorkbenchSnapshot["notifications"] : [],
          workingDirectory: isObject(event.workingDirectory)
            ? event.workingDirectory as unknown as WorkbenchSnapshot["workingDirectory"]
            : undefined,
          editorLayout: isObject(event.editorLayout)
            ? event.editorLayout as unknown as WorkbenchEditorLayoutSnapshot
            : undefined,
        };
        setSelectionPickerActive(Boolean(event.selectionPickerActive));
        if (Array.isArray(event.surfaceInvalidations)) {
          window.dispatchEvent(new CustomEvent("the-editor:surface-invalidated", { detail: event.surfaceInvalidations }));
        }
        setWorkbench(snapshot);
        void uiRuntime.update(snapshot).then(() => {
          setUiRevision((revision) => revision + 1);
        }).catch((error) => {
          console.error(error);
        });
      }
    });
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (isEditableKeyEvent(event)) return;
      if (event.metaKey) return;
      const input = toKeyInput(event);
      if (!input) return;
      event.preventDefault();
      void window.theEditor.policyRequest("input.keyDown", input).catch((error) => {
        console.error(error);
      });
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const leftSidebar = uiRuntime.renderToolRoot("leftSidebar");
  const leftSidebarToggle = (
    <SidebarToggleButton
      expanded={leftSidebarVisible}
      onToggle={() => setLeftSidebarVisible((visible) => !visible)}
    />
  );
  const rightSidebar = uiRuntime.renderToolRoot("rightSidebar");
  const bottomPanel = uiRuntime.renderToolRoot("bottomPanel");
  const overlayModal = uiRuntime.renderToolRoot("overlayModal");
  const overlayFloating = uiRuntime.renderToolRoot("overlayFloating");

  return (
    <main className="workbench">
      <section className="workbenchMainRow">
        {leftSidebar ? (
          <aside
            className="workbenchSidebar left"
            data-visible={leftSidebarVisible ? "true" : "false"}
            style={{ "--workbench-left-sidebar-width": `${leftSidebarWidth}px` } as CSSProperties}
          >
            <WorkbenchSidebarChrome workbench={workbench} trailingAccessory={leftSidebarToggle} />
            <div className="workbenchSidebarContent">{leftSidebar}</div>
            <div
              className="workbenchSidebarResizeHandle"
              role="separator"
              aria-orientation="vertical"
              aria-label="Resize file tree"
              onPointerDown={(event) => startSidebarResize(event, leftSidebarWidth, setLeftSidebarWidth)}
            />
          </aside>
        ) : null}
        <section className="workbenchCenterColumn">
          <div className="workbenchEditorArea">
            <EditorLayoutRoot
              layout={workbench.editorLayout}
              workbench={workbench}
              selectionPickerActive={selectionPickerActive}
              drag={dragController}
              dragSnapshot={dragSnapshot}
              showLeadingWindowChrome={!leftSidebarVisible}
              leadingAccessory={!leftSidebarVisible ? (
                <WorkbenchLeadingChrome workbench={workbench} trailingAccessory={leftSidebarToggle} />
              ) : undefined}
            />
          </div>
          {bottomPanel ? <section className="workbenchBottomPanel">{bottomPanel}</section> : null}
          <footer className="workbenchStatuslineRoot">{uiRuntime.renderRole("statusline")}</footer>
        </section>
        {rightSidebar ? (
          <aside className="workbenchSidebar right">
            <div className="workbenchSidebarContent">{rightSidebar}</div>
          </aside>
        ) : null}
      </section>

      <div className="workbenchOverlayRoot">
        <UiErrorBoundary name="overlayModal">{overlayModal}</UiErrorBoundary>
        <UiErrorBoundary name="overlayFloating">{overlayFloating}</UiErrorBoundary>
        <UiErrorBoundary name="notifications">{uiRuntime.renderRole("notifications")}</UiErrorBoundary>
        <UiErrorBoundary name="picker">{uiRuntime.renderRole("picker")}</UiErrorBoundary>
        <UiErrorBoundary name="inputPrompt">{uiRuntime.renderRole("inputPrompt")}</UiErrorBoundary>
        <UiErrorBoundary name="commandPalette">{uiRuntime.renderRole("commandPalette")}</UiErrorBoundary>
      </div>
    </main>
  );
}

function WorkbenchSidebarChrome({ workbench, trailingAccessory }: { workbench: WorkbenchSnapshot; trailingAccessory?: ReactNode }) {
  return (
    <div className="workbenchSidebarChrome" data-leading="true">
      <div className="workbenchChromeLeadingAccessory">
        <WorkbenchLeadingChrome workbench={workbench} trailingAccessory={trailingAccessory} />
      </div>
      <div className="workbenchSidebarChromeGlyphs">
        <button className="workbenchSidebarGlyphButton" title="Search files" onClick={() => dispatchFileTreeAction("openSearch")}><SidebarSearchGlyph /></button>
        <button className="workbenchSidebarGlyphButton" title="New file" onClick={() => dispatchFileTreeAction("newFile")}><SidebarAddFileGlyph /></button>
        <button className="workbenchSidebarGlyphButton" title="New folder" onClick={() => dispatchFileTreeAction("newFolder")}><SidebarAddFolderGlyph /></button>
      </div>
    </div>
  );
}

function dispatchFileTreeAction(kind: "openSearch" | "newFile" | "newFolder"): void {
  window.dispatchEvent(new CustomEvent("the-editor:file-tree-action", { detail: { kind } }));
}

function WorkbenchLeadingChrome({ workbench, trailingAccessory }: { workbench: WorkbenchSnapshot; trailingAccessory?: ReactNode }) {
  return (
    <div className="workbenchLeadingChrome">
      <div className="workbenchSidebarTitle">{basename(workbench.workingDirectory?.current) || "the-editor"}</div>
      {trailingAccessory ? <div className="workbenchChromeTrailingAccessory">{trailingAccessory}</div> : null}
    </div>
  );
}

function SidebarToggleButton({ expanded, onToggle }: { expanded: boolean; onToggle(): void }) {
  return (
    <button
      type="button"
      className="workbenchSidebarToggleButton"
      title={expanded ? "Hide file tree" : "Show file tree"}
      aria-label={expanded ? "Hide file tree" : "Show file tree"}
      aria-pressed={expanded}
      onMouseDown={(event) => event.stopPropagation()}
      onClick={(event) => {
        event.preventDefault();
        event.stopPropagation();
        onToggle();
      }}
    >
      <span />
    </button>
  );
}

function SidebarSearchGlyph() {
  return (
    <span className="workbenchSidebarGlyph" aria-hidden="true">
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
        <circle cx="7" cy="7" r="4.5" />
        <path d="M10.5 10.5 14 14" />
      </svg>
    </span>
  );
}

function SidebarAddFileGlyph() {
  return (
    <span className="workbenchSidebarGlyph" aria-hidden="true">
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
        <path d="M4 2.5h4.8L12 5.7v6.8a1 1 0 0 1-1 1H4.9a.9.9 0 0 1-.9-.9V3.4a.9.9 0 0 1 .9-.9Z" />
        <path d="M8 7v4" />
        <path d="M6 9h4" />
      </svg>
    </span>
  );
}

function SidebarAddFolderGlyph() {
  return (
    <span className="workbenchSidebarGlyph" aria-hidden="true">
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
        <path d="M1.8 4.5h4.1l1.2 1.3h7.1a1 1 0 0 1 1 1v4.8a1 1 0 0 1-1 1H2.8a1 1 0 0 1-1-1V5.5a1 1 0 0 1 1-1Z" />
        <path d="M9.6 7.7v3.6" />
        <path d="M7.8 9.5h3.6" />
      </svg>
    </span>
  );
}

function EditorLayoutRoot({
  layout,
  workbench,
  selectionPickerActive,
  drag,
  dragSnapshot,
  showLeadingWindowChrome,
  leadingAccessory,
}: {
  layout: WorkbenchEditorLayoutSnapshot | undefined;
  workbench: WorkbenchSnapshot;
  selectionPickerActive: boolean;
  drag: WorkbenchDragController;
  dragSnapshot: WorkbenchDragSnapshot;
  showLeadingWindowChrome?: boolean;
  leadingAccessory?: ReactNode;
}) {
  const items = useMemo(
    () => new Map((layout?.items ?? []).map((item) => [item.id, item] as const)),
    [layout?.items],
  );
  const panes = useMemo(
    () => new Map((layout?.panes ?? []).map((pane) => [pane.id, pane] as const)),
    [layout?.panes],
  );
  const groups = useMemo(
    () => new Map((layout?.groups ?? []).map((group) => [group.id, group] as const)),
    [layout?.groups],
  );

  if (!layout?.root) {
    return <div className="workbenchEmptyEditor">No editor group</div>;
  }

  return (
    <EditorLayoutNodeView
      node={layout.root}
      layout={layout}
      items={items}
      panes={panes}
      groups={groups}
      workbench={workbench}
      selectionPickerActive={selectionPickerActive}
      drag={drag}
      dragSnapshot={dragSnapshot}
      leadingGroupId={showLeadingWindowChrome ? findLeadingGroupId(layout.root) : undefined}
      leadingAccessory={leadingAccessory}
    />
  );
}

function EditorLayoutNodeView({
  node,
  layout,
  items,
  panes,
  groups,
  workbench,
  selectionPickerActive,
  drag,
  dragSnapshot,
  leadingGroupId,
  leadingAccessory,
}: {
  node: WorkbenchEditorLayoutNodeSnapshot;
  layout: WorkbenchEditorLayoutSnapshot;
  items: Map<string, WorkbenchItemDescriptor>;
  panes: Map<string, WorkbenchPaneDescriptor>;
  groups: Map<string, WorkbenchGroupSnapshot>;
  workbench: WorkbenchSnapshot;
  selectionPickerActive: boolean;
  drag: WorkbenchDragController;
  dragSnapshot: WorkbenchDragSnapshot;
  leadingGroupId?: string;
  leadingAccessory?: ReactNode;
}) {
  if (node.kind === "group") {
    const group = groups.get(node.groupId);
    if (!group) return null;
    return (
      <EditorGroupView
        group={group}
        layout={layout}
        items={items}
        panes={panes}
        workbench={workbench}
        selectionPickerActive={selectionPickerActive}
        drag={drag}
        dragSnapshot={dragSnapshot}
        showWindowChrome={group.id === leadingGroupId}
        leadingAccessory={group.id === leadingGroupId ? leadingAccessory : undefined}
      />
    );
  }

  return (
    <EditorSplitView
      node={node}
      layout={layout}
      items={items}
      panes={panes}
      groups={groups}
      workbench={workbench}
      selectionPickerActive={selectionPickerActive}
      drag={drag}
      dragSnapshot={dragSnapshot}
      leadingGroupId={leadingGroupId}
      leadingAccessory={leadingAccessory}
    />
  );
}

function EditorSplitView({
  node,
  layout,
  items,
  panes,
  groups,
  workbench,
  selectionPickerActive,
  drag,
  dragSnapshot,
  leadingGroupId,
  leadingAccessory,
}: {
  node: WorkbenchSplitNodeSnapshot;
  layout: WorkbenchEditorLayoutSnapshot;
  items: Map<string, WorkbenchItemDescriptor>;
  panes: Map<string, WorkbenchPaneDescriptor>;
  groups: Map<string, WorkbenchGroupSnapshot>;
  workbench: WorkbenchSnapshot;
  selectionPickerActive: boolean;
  drag: WorkbenchDragController;
  dragSnapshot: WorkbenchDragSnapshot;
  leadingGroupId?: string;
  leadingAccessory?: ReactNode;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const draggingRef = useRef(false);
  const [sizes, setSizes] = useState(node.sizes);
  const [activeDividerIndex, setActiveDividerIndex] = useState<number | null>(null);

  useEffect(() => {
    if (draggingRef.current) return;
    setSizes((current) => sameSizes(current, node.sizes) ? current : node.sizes);
  }, [node.id, node.sizes]);

  return (
    <div ref={containerRef} className={`editorSplit editorSplit-${node.axis}`}>
      {node.children.map((child, index) => (
        <FragmentWithDivider
          key={child.kind === "group" ? child.groupId : child.id}
          divider={index < node.children.length - 1 ? (
            <div
              className={`editorSplitDivider editorSplitDivider-${node.axis}`}
              data-active={activeDividerIndex === index ? "true" : undefined}
              onMouseDown={(event) => {
                startSplitResize(event, containerRef.current, node.axis, index, sizes, {
                  onStart() {
                    draggingRef.current = true;
                    setActiveDividerIndex(index);
                  },
                  onChange(nextSizes) {
                    setSizes(nextSizes);
                  },
                  onEnd(nextSizes) {
                    draggingRef.current = false;
                    setActiveDividerIndex(null);
                    void window.theEditor.policyRequest("command.dispatch", {
                      id: "workbench.resizeSplit",
                      args: { splitId: node.id, sizes: nextSizes },
                    }).catch((error) => {
                      console.error(error);
                    });
                  },
                  onCancel() {
                    draggingRef.current = false;
                    setActiveDividerIndex(null);
                  },
                });
              }}
            />
          ) : null}
        >
          <div className="editorSplitChild" style={{ flex: `${sizes[index] ?? 1} 1 0` }}>
            <EditorLayoutNodeView
              node={child}
              layout={layout}
              items={items}
              panes={panes}
              groups={groups}
              workbench={workbench}
              selectionPickerActive={selectionPickerActive}
              drag={drag}
              dragSnapshot={dragSnapshot}
              leadingGroupId={leadingGroupId}
              leadingAccessory={leadingAccessory}
            />
          </div>
        </FragmentWithDivider>
      ))}
    </div>
  );
}

function EditorGroupView({
  group,
  layout,
  items,
  panes,
  workbench,
  selectionPickerActive,
  drag,
  dragSnapshot,
  showWindowChrome,
  leadingAccessory,
}: {
  group: WorkbenchGroupSnapshot;
  layout: WorkbenchEditorLayoutSnapshot;
  items: Map<string, WorkbenchItemDescriptor>;
  panes: Map<string, WorkbenchPaneDescriptor>;
  workbench: WorkbenchSnapshot;
  selectionPickerActive: boolean;
  drag: WorkbenchDragController;
  dragSnapshot: WorkbenchDragSnapshot;
  showWindowChrome?: boolean;
  leadingAccessory?: ReactNode;
}) {
  const splitTarget = dragSnapshot.currentTarget?.kind === "split" && dragSnapshot.currentTarget.groupId === group.id ? dragSnapshot.currentTarget : undefined;
  const activePaneId = group.activePaneId;
  const activePane = activePaneId ? panes.get(activePaneId) : undefined;
  const active = layout.activeGroupId === group.id;
  const tabs = group.paneIds.map((paneId) => panes.get(paneId)).filter((pane): pane is WorkbenchPaneDescriptor => Boolean(pane));
  const tabItems = tabs.map((pane) => items.get(pane.itemId)).filter((item): item is WorkbenchItemDescriptor => Boolean(item));

  return (
    <section
      className="editorGroup"
      data-active={active ? "true" : undefined}
      data-drop-direction={splitTarget?.direction ?? undefined}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) focusGroup(group.id);
      }}
      onDragOver={(event) => {
        if (!hasDraggedWorkbenchPane(event)) return;
        event.preventDefault();
        event.dataTransfer.dropEffect = "move";
        drag.updateTarget({ kind: "split", groupId: group.id, direction: groupDropDirection(event) });
      }}
      onDragLeave={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) drag.updateTarget(undefined);
      }}
      onDrop={(event) => {
        const dragged = readDraggedWorkbenchPane(event);
        if (!dragged) return;
        event.preventDefault();
        event.stopPropagation();
        drag.updateTarget({ kind: "split", groupId: group.id, direction: groupDropDirection(event) });
        void drag.drop().catch(console.error);
      }}
    >
      <header className="editorGroupHeader">
        {leadingAccessory ? <div className="workbenchChromeLeadingAccessory">{leadingAccessory}</div> : null}
        {uiRuntime.renderWorkbenchTabstrip({
          group,
          items: tabItems,
          panes: tabs,
          activePaneId,
          showWindowControls: showWindowChrome && !leadingAccessory,
          drag,
        })}
      </header>
      <div className="editorGroupBody" onMouseDown={() => focusGroup(group.id)}>
        {activePane ? renderPane(activePane, active, workbench, selectionPickerActive) : <EmptyEditorGroup />}
      </div>
    </section>
  );
}

function hasDraggedWorkbenchPane(event: DragEvent<HTMLElement>): boolean {
  return Array.from(event.dataTransfer.types).includes("application/x-the-editor-pane");
}

function readDraggedWorkbenchPane(event: DragEvent<HTMLElement>): { paneId: string } | undefined {
  const raw = event.dataTransfer.getData("application/x-the-editor-pane");
  if (!raw) return undefined;
  try {
    const parsed = JSON.parse(raw) as { kind?: unknown; paneId?: unknown };
    return parsed.kind === "the-editor/pane" && typeof parsed.paneId === "string" ? { paneId: parsed.paneId } : undefined;
  } catch {
    return undefined;
  }
}

function groupDropDirection(event: DragEvent<HTMLElement>): "left" | "right" | "up" | "down" {
  const rect = event.currentTarget.getBoundingClientRect();
  const x = (event.clientX - rect.left) / Math.max(1, rect.width);
  const y = (event.clientY - rect.top) / Math.max(1, rect.height);
  const distanceToLeft = x;
  const distanceToRight = 1 - x;
  const distanceToTop = y;
  const distanceToBottom = 1 - y;
  const nearest = Math.min(distanceToLeft, distanceToRight, distanceToTop, distanceToBottom);
  if (nearest === distanceToLeft) return "left";
  if (nearest === distanceToRight) return "right";
  if (nearest === distanceToTop) return "up";
  return "down";
}

function renderPane(
  pane: WorkbenchPaneDescriptor,
  active: boolean,
  workbench: WorkbenchSnapshot,
  selectionPickerActive: boolean,
) {
  if (pane.kind === "surface") {
    const paneState = pane.state as SurfacePaneSnapshotState | undefined;
    return (
      <SurfaceEditorPane
        pane={pane}
        active={active}
        decorations={active ? paneState?.decorations ?? [] : []}
        theme={workbench.theme}
        syntaxLegend={workbench.syntaxLegend ?? {}}
        selectionPickerActive={active && selectionPickerActive}
        cursorShape={workbench.interaction?.mode?.cursorShape ?? "block"}
      />
    );
  }
  return <ToolEditorPane pane={pane} active={active} workbench={workbench} />;
}

function SurfaceEditorPane({
  pane,
  active,
  decorations,
  theme,
  syntaxLegend,
  selectionPickerActive,
  cursorShape,
}: {
  pane: Extract<WorkbenchPaneDescriptor, { kind: "surface" }>;
  active: boolean;
  decorations: BufferDecoration[];
  theme: ThemeSnapshot | undefined;
  syntaxLegend: Record<number, string>;
  selectionPickerActive: boolean;
  cursorShape: "bar" | "block" | "underline";
}) {
  const paneState = pane.state as SurfacePaneSnapshotState | undefined;
  const [renderFrame, setRenderFrame] = useState<RenderFrame | undefined>(undefined);
  const [invalidationVersion, setInvalidationVersion] = useState(0);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const lastConfigRef = useRef<string>("");
  const latestConfigMetricsRef = useRef<CellGridMetrics | undefined>(paneState?.metrics);
  const latestFrameRef = useRef<RenderFrame | undefined>(undefined);
  const latestDecorationsRef = useRef<BufferDecoration[]>([]);
  const latestGuttersRef = useRef<GutterSnapshot[]>([]);
  const latestThemeRef = useRef<ThemeSnapshot | undefined>(undefined);
  const latestSyntaxLegendRef = useRef<Record<number, string>>({});
  const latestSelectionPickerActiveRef = useRef(false);
  const latestCursorShapeRef = useRef<"bar" | "block" | "underline">("block");
  const renderRequestRef = useRef(0);
  const pendingRenderRequestRef = useRef<{ viewport: Viewport; metrics: CellGridMetrics } | undefined>(undefined);
  const renderScheduledRef = useRef(false);

  const frameGutters = useMemo(() => builtInFrameGutters(renderFrame, paneState?.lineCount), [renderFrame, paneState?.lineCount]);
  const gutters = useMemo(() => [...frameGutters, ...(paneState?.gutters ?? [])], [frameGutters, paneState?.gutters]);

  latestFrameRef.current = renderFrame;
  latestDecorationsRef.current = decorations;
  latestGuttersRef.current = gutters;
  latestThemeRef.current = theme;
  latestSyntaxLegendRef.current = syntaxLegend;
  latestSelectionPickerActiveRef.current = selectionPickerActive;
  latestCursorShapeRef.current = cursorShape;

  const requestRenderFrame = (configuration?: { viewport: Viewport; metrics: CellGridMetrics }) => {
    const viewport = configuration?.viewport ?? paneState?.viewport;
    const metrics = configuration?.metrics ?? paneState?.metrics ?? latestConfigMetricsRef.current;
    if (!viewport || !metrics) return;
    pendingRenderRequestRef.current = { viewport, metrics };
    if (renderScheduledRef.current) return;
    renderScheduledRef.current = true;
    window.requestAnimationFrame(() => {
      renderScheduledRef.current = false;
      const request = pendingRenderRequestRef.current;
      pendingRenderRequestRef.current = undefined;
      if (!request) return;
      const requestId = ++renderRequestRef.current;
      void window.theEditorRender.renderFrame({ surfaceId: pane.surfaceId, viewport: request.viewport, metrics: request.metrics }).then((frame) => {
        if (requestId !== renderRequestRef.current) return;
        setRenderFrame(frame);
      }).catch((error) => {
        console.error(error);
      });
    });
  };

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    lastConfigRef.current = "";
    let disposed = false;
    let observer: ResizeObserver | undefined;
    let raf = 0;

    const configure = () => {
      raf = 0;
      const config = configureCanvasTextSurface(canvas, EDITOR_SURFACE_STYLE);
      latestConfigMetricsRef.current = config.metrics;
      surfaceCursorAnchors.set({
        surfaceId: pane.surfaceId,
        canvas,
        frame: latestFrameRef.current,
        gutters: latestGuttersRef.current,
        metrics: config.metrics,
        styleOptions: EDITOR_SURFACE_STYLE,
      });
      const serialized = JSON.stringify(config);
      if (latestFrameRef.current) {
        paintCanvasTextSurface(
          canvas,
          latestFrameRef.current,
          latestDecorationsRef.current,
          latestThemeRef.current,
          latestSyntaxLegendRef.current,
          {
            selectionPickerActive: latestSelectionPickerActiveRef.current,
            gutters: latestGuttersRef.current,
            cursorShape: latestCursorShapeRef.current,
          }, 
          EDITOR_SURFACE_STYLE,
        );
      } else {
        clearCanvasTextSurface(canvas, latestThemeRef.current, EDITOR_SURFACE_STYLE);
      }
      if (serialized === lastConfigRef.current) return;
      lastConfigRef.current = serialized;
      requestRenderFrame(config);
      void window.theEditor.policyRequest("surface.configure", { paneId: pane.id, ...config }).catch((error) => {
        console.error(error);
      });
    };

    const scheduleConfigure = () => {
      if (raf) return;
      raf = window.requestAnimationFrame(configure);
    };

    void loadEditorFont(EDITOR_SURFACE_STYLE).finally(() => {
      if (disposed) return;
      scheduleConfigure();
      observer = new ResizeObserver(scheduleConfigure);
      observer.observe(canvas.parentElement ?? canvas);
    });

    return () => {
      disposed = true;
      renderRequestRef.current += 1;
      if (raf) window.cancelAnimationFrame(raf);
      observer?.disconnect();
      surfaceCursorAnchors.delete(pane.surfaceId);
    };
  }, [pane.id, pane.surfaceId]);

  useEffect(() => {
    const onInvalidated = (event: Event) => {
      const ids = (event as CustomEvent<unknown>).detail;
      if (!Array.isArray(ids) || !ids.includes(pane.surfaceId)) return;
      setInvalidationVersion((version) => version + 1);
    };
    window.addEventListener("the-editor:surface-invalidated", onInvalidated);
    return () => window.removeEventListener("the-editor:surface-invalidated", onInvalidated);
  }, [pane.surfaceId]);

  useEffect(() => {
    requestRenderFrame();
  }, [pane.surfaceId, invalidationVersion, paneState?.metrics, paneState?.viewport]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    surfaceCursorAnchors.set({
      surfaceId: pane.surfaceId,
      canvas,
      frame: renderFrame,
      gutters,
      metrics: paneState?.metrics ?? latestConfigMetricsRef.current,
      styleOptions: EDITOR_SURFACE_STYLE,
    });
  }, [gutters, pane.surfaceId, renderFrame, paneState?.metrics]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    if (!renderFrame) {
      clearCanvasTextSurface(canvas, theme, EDITOR_SURFACE_STYLE);
      return;
    }
    paintCanvasTextSurface(
      canvas,
      renderFrame,
      decorations,
      theme,
      syntaxLegend,
      { selectionPickerActive, gutters, cursorShape },
      EDITOR_SURFACE_STYLE,
    );
  }, [cursorShape, decorations, gutters, pane.id, pane.path, renderFrame, selectionPickerActive, syntaxLegend, theme]);

  return (
    <div className="editorSurfacePane" data-active={active ? "true" : undefined} onMouseDown={() => focusPane(pane.id)}>
      <canvas ref={canvasRef} />
    </div>
  );
}

function builtInFrameGutters(frame: RenderFrame | undefined, lineCount: number | undefined): GutterSnapshot[] {
  const rows = frame?.gutterRows ?? [];
  if (!rows.length) return [];
  const maxLine = Math.max(lineCount ?? 0, ...rows.map((row) => row.lineNumber ?? 0));
  const width = Math.max(2, String(maxLine).length);
  return [{
    providerId: "runtime.lineNumbers",
    lane: {
      id: "lineNumbers",
      side: "left",
      order: 100,
      widthCells: width,
      paddingLeftPx: 0,
      paddingRightPx: 22,
    },
    lines: rows.map((row) => ({
      id: `line:${row.surfaceRow}`,
      surfaceRow: row.surfaceRow,
      kind: "text" as const,
      text: row.lineNumber === null || row.lineNumber === undefined ? "" : String(row.lineNumber).padStart(width, " "),
      align: "right" as const,
      foreground: row.active ? "#e7b84a" : undefined,
      background: row.active ? "rgba(255, 255, 255, 0.045)" : undefined,
      opacity: row.active ? 0.95 : 0.58,
    })),
  }];
}

function ToolEditorPane({
  pane,
  active,
  workbench,
}: {
  pane: Extract<WorkbenchPaneDescriptor, { kind: "tool" }>;
  active: boolean;
  workbench: WorkbenchSnapshot;
}) {
  const tool = (pane.state as ToolPaneSnapshotState | undefined)?.tool;
  const content = tool ? uiRuntime.host.tools.renderTool(tool, workbench) as ReactNode : null;
  return (
    <div className="editorToolPane" data-active={active ? "true" : undefined} onMouseDown={() => focusPane(pane.id)}>
      {content ?? <div className="editorToolPlaceholder">Tool view unavailable</div>}
    </div>
  );
}

function EmptyEditorGroup() {
  return <div className="editorToolPlaceholder">Empty group</div>;
}

function FragmentWithDivider({ children, divider }: { children: ReactNode; divider: ReactNode }) {
  return (
    <>
      {children}
      {divider}
    </>
  );
}

function startSidebarResize(
  event: React.PointerEvent<HTMLDivElement>,
  initialWidth: number,
  setWidth: (width: number) => void,
): void {
  if (event.button !== 0) return;
  event.preventDefault();
  event.stopPropagation();
  const handle = event.currentTarget;
  handle.setPointerCapture(event.pointerId);
  const startX = event.clientX;
  document.body.classList.add("isResizingSidebar");
  document.documentElement.style.cursor = "col-resize";

  const move = (moveEvent: PointerEvent) => {
    setWidth(clamp(initialWidth + moveEvent.clientX - startX, 240, 520));
  };
  const finish = (finishEvent: PointerEvent) => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", finish);
    window.removeEventListener("pointercancel", finish);
    document.body.classList.remove("isResizingSidebar");
    document.documentElement.style.removeProperty("cursor");
    if (handle.hasPointerCapture(finishEvent.pointerId)) handle.releasePointerCapture(finishEvent.pointerId);
  };

  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", finish);
  window.addEventListener("pointercancel", finish);
}

function startSplitResize(
  event: React.MouseEvent<HTMLDivElement>,
  element: HTMLDivElement | null,
  axis: "row" | "column",
  index: number,
  initialSizes: readonly number[],
  handlers: {
    onStart(): void;
    onChange(sizes: number[]): void;
    onEnd(sizes: number[]): void;
    onCancel(): void;
  },
): void {
  if (!element) return;
  event.preventDefault();
  event.stopPropagation();
  handlers.onStart();
  document.body.classList.add("isResizingSplits");
  document.documentElement.style.cursor = axis === "row" ? "col-resize" : "row-resize";
  const rect = element.getBoundingClientRect();
  const total = Math.max(1, axis === "row" ? rect.width : rect.height);
  const start = axis === "row" ? event.clientX : event.clientY;
  const left = initialSizes[index] ?? 0.5;
  const right = initialSizes[index + 1] ?? 0.5;
  const pairTotal = left + right;
  let latest = [...initialSizes];

  const move = (moveEvent: MouseEvent) => {
    const current = axis === "row" ? moveEvent.clientX : moveEvent.clientY;
    const delta = (current - start) / total;
    const min = Math.min(0.45, 96 / total);
    const nextLeft = clamp(left + delta, min, pairTotal - min);
    const nextRight = pairTotal - nextLeft;
    latest = [...initialSizes];
    latest[index] = nextLeft;
    latest[index + 1] = nextRight;
    latest = normalizeSizes(latest);
    handlers.onChange(latest);
  };

  const finish = (commit: boolean) => {
    window.removeEventListener("mousemove", move);
    window.removeEventListener("mouseup", up);
    window.removeEventListener("blur", cancel);
    document.body.classList.remove("isResizingSplits");
    document.documentElement.style.removeProperty("cursor");
    if (commit) handlers.onEnd(latest);
    else handlers.onCancel();
  };

  const up = () => finish(true);
  const cancel = () => finish(false);

  window.addEventListener("mousemove", move);
  window.addEventListener("mouseup", up);
  window.addEventListener("blur", cancel);
}

class UiErrorBoundary extends Component<{ name: string; children: ReactNode }, { error?: Error }> {
  state: { error?: Error } = {};

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error) {
    console.error(`ui render failed for ${this.props.name}`, error);
  }

  componentDidUpdate(prevProps: Readonly<{ name: string; children: ReactNode }>) {
    if (this.state.error && prevProps.children !== this.props.children) {
      this.setState({ error: undefined });
    }
  }

  render() {
    if (this.state.error) return null;
    return this.props.children;
  }
}

function toKeyInput(event: KeyboardEvent): { key: string; text?: string } | undefined {
  const key = canonicalKey(event);
  if (!key) return undefined;
  const text = printableText(event);
  return text === undefined ? { key } : { key, text };
}

function canonicalKey(event: KeyboardEvent): string | undefined {
  const key = event.key;
  const modifiers: string[] = [];
  if (event.ctrlKey) modifiers.push("C");
  if (event.altKey) modifiers.push("A");
  if (event.metaKey) modifiers.push("M");

  let atom: string | undefined;
  if (key.length === 1) {
    atom = key === " " ? "space" : key;
  } else {
    atom = namedKey(key);
    if (!atom) return undefined;
    if (event.shiftKey) modifiers.push("S");
  }

  return modifiers.length ? `${modifiers.join("-")}-${atom}` : atom;
}

function printableText(event: KeyboardEvent): string | undefined {
  if (event.ctrlKey || event.altKey || event.metaKey) return undefined;
  if (event.key === " ") return " ";
  return event.key.length === 1 ? event.key : undefined;
}

function namedKey(key: string): string | undefined {
  switch (key) {
    case "Escape":
      return "esc";
    case "Enter":
      return "ret";
    case "Backspace":
      return "backspace";
    case "Delete":
      return "del";
    case "ArrowLeft":
      return "left";
    case "ArrowRight":
      return "right";
    case "ArrowUp":
      return "up";
    case "ArrowDown":
      return "down";
    case "PageUp":
      return "pageup";
    case "PageDown":
      return "pagedown";
    case "Home":
      return "home";
    case "End":
      return "end";
    case "Tab":
      return "tab";
    default:
      return undefined;
  }
}

function basename(path: string | undefined): string {
  if (!path) return "";
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

function dispatchCommand(id: string, args?: unknown): void {
  void window.theEditor.policyRequest("command.dispatch", { id, args }).catch((error) => {
    console.error(error);
  });
}

function focusPane(paneId: string): void {
  void window.theEditor.policyRequest("command.dispatch", { id: "workbench.focusPane", args: { paneId } }).catch((error) => {
    console.error(error);
  });
}

function focusGroup(groupId: string): void {
  void window.theEditor.policyRequest("command.dispatch", { id: "workbench.focusGroup", args: { groupId } }).catch((error) => {
    console.error(error);
  });
}

function closePane(paneId: string): void {
  void window.theEditor.policyRequest("command.dispatch", { id: "workbench.closePane", args: { paneId } }).catch((error) => {
    console.error(error);
  });
}

function sameSizes(left: readonly number[], right: readonly number[]): boolean {
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index += 1) {
    if (Math.abs((left[index] ?? 0) - (right[index] ?? 0)) > 0.0001) return false;
  }
  return true;
}

function normalizeSizes(sizes: readonly number[]): number[] {
  const safe = sizes.map((size) => Math.max(0.01, size));
  const total = safe.reduce((sum, size) => sum + size, 0) || safe.length || 1;
  return safe.map((size) => size / total);
}

function findLeadingGroupId(node: WorkbenchEditorLayoutNodeSnapshot): string | undefined {
  if (node.kind === "group") return node.groupId;
  const first = node.children[0];
  return first ? findLeadingGroupId(first) : undefined;
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function isEditableKeyEvent(event: KeyboardEvent): boolean {
  for (const target of event.composedPath()) {
    if (isEditableTarget(target)) return true;
  }
  return isEditableTarget(event.target);
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target instanceof HTMLInputElement) return true;
  if (target instanceof HTMLTextAreaElement) return true;
  const tagName = target.tagName.toLowerCase();
  if (tagName === "input" || tagName === "textarea" || tagName === "select") return true;
  return target.isContentEditable;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

createRoot(document.getElementById("root")!).render(<App />);
