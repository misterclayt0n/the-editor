import { useEffect, useMemo, useRef, useState, type DragEvent, type MouseEvent } from "react";
import type { WorkbenchPaneDragController, WorkbenchPaneDragSnapshot } from "@the-editor/editor/ui";
import { resolveTabFileIcon, fileIconSpriteMarkup, tabLabel, type ResolvedFileIcon } from "./fileIcons/tabIcons.js";

export interface WorkbenchTabPane {
  id: string;
  title: string;
  path?: string;
  closeable: boolean;
  kind: string;
  dirty?: boolean;
  conflict?: boolean;
}

interface DraggedWorkbenchPane {
  kind: "the-editor/pane";
  paneId: string;
}

export function DefaultWorkbenchTabstrip({
  dispatch,
  panes,
  activePaneId,
  showWindowControls,
  groupId,
  drag,
  dragSnapshot,
}: {
  dispatch: (dispatch: { id: string; args?: unknown }) => Promise<unknown>;
  panes: WorkbenchTabPane[];
  activePaneId?: string;
  showWindowControls?: boolean;
  groupId?: string;
  drag: WorkbenchPaneDragController;
  dragSnapshot: WorkbenchPaneDragSnapshot;
}) {
  const scrollerRef = useRef<HTMLDivElement | null>(null);
  const [scrollState, setScrollState] = useState({ canScrollLeft: false, canScrollRight: false });
  const spriteMarkup = useMemo(() => fileIconSpriteMarkup(), []);

  useEffect(() => {
    const scroller = scrollerRef.current;
    if (!scroller) return;
    const activeTab = scroller.querySelector<HTMLElement>('[data-workbench-tab-active="true"]');
    if (!activeTab) return;
    activeTab.scrollIntoView({ behavior: "smooth", block: "nearest", inline: "nearest" });
  }, [activePaneId, panes]);

  useEffect(() => {
    const scroller = scrollerRef.current;
    if (!scroller) return;
    const update = () => {
      const { scrollLeft, scrollWidth, clientWidth } = scroller;
      setScrollState({
        canScrollLeft: scrollLeft > 1,
        canScrollRight: scrollLeft + clientWidth < scrollWidth - 1,
      });
    };
    update();
    scroller.addEventListener("scroll", update, { passive: true });
    const observer = new ResizeObserver(update);
    observer.observe(scroller);
    for (const child of Array.from(scroller.children)) observer.observe(child);
    return () => {
      scroller.removeEventListener("scroll", update);
      observer.disconnect();
    };
  }, [panes.length]);

  const updateDragIndex = (event: DragEvent<HTMLElement>) => {
    if (!hasDraggedPane(event)) return;
    event.preventDefault();
    event.stopPropagation();
    event.dataTransfer.dropEffect = "move";
    const index = tabDropIndex(event, panes.length);
    drag.updateTarget({ kind: "tabstrip", groupId: groupId ?? "active", index });
  };

  const dropPane = (event: DragEvent<HTMLElement>) => {
    const dragged = readDraggedPane(event);
    if (!dragged) return;
    event.preventDefault();
    event.stopPropagation();
    if (!dragSnapshot.currentTarget || dragSnapshot.currentTarget.kind !== "tabstrip") {
      drag.updateTarget({ kind: "tabstrip", groupId: groupId ?? "active", index: tabDropIndex(event, panes.length) });
    }
    void drag.drop().catch(console.error);
  };

  return (
    <div className="workbenchTopTabsBar workbenchFileTabbarRoot" data-leading={showWindowControls ? "true" : undefined}>
      <div aria-hidden="true" className="workbenchFileTabSprite" dangerouslySetInnerHTML={{ __html: spriteMarkup }} />
      <div className="workbenchFileTabbarGroup">
        <div className="workbenchFileTabScrollerShell">
          <div
            ref={scrollerRef}
            className="workbenchFileTabScroller"
            onDragOver={updateDragIndex}
            onDragLeave={(event) => {
              if (!event.currentTarget.contains(event.relatedTarget as Node | null)) drag.updateTarget(undefined);
            }}
            onDrop={dropPane}
          >
            {panes.map((pane, index) => {
              const isActive = pane.id === activePaneId;
              const icon = resolveTabFileIcon(pane);
              return (
                <div
                  key={pane.id}
                  className="workbenchFileTabItem"
                  data-workbench-tab-active={isActive ? "true" : undefined}
                  data-drop-before={dragSnapshot.currentTarget?.kind === "tabstrip" && dragSnapshot.currentTarget.groupId === (groupId ?? "active") && dragSnapshot.currentTarget.index === index ? "true" : undefined}
                >
                  <DefaultFileTab
                    paneId={pane.id}
                    groupId={groupId ?? "active"}
                    drag={drag}
                    dragging={dragSnapshot.draggedPaneId === pane.id}
                    activate={(event) => {
                      event.currentTarget.blur();
                      void dispatch({ id: "workbench.focusPane", args: { paneId: pane.id } });
                    }}
                    close={() => {
                      void dispatch({ id: "workbench.closePane", args: { paneId: pane.id } });
                    }}
                    icon={icon}
                    isActive={isActive}
                    label={tabLabel(pane.path, pane.title)}
                    path={pane.path ?? pane.title}
                    closeable={pane.closeable}
                    dirty={pane.dirty}
                    conflict={pane.conflict}
                  />
                </div>
              );
            })}
            <div aria-hidden="true" className="workbenchFileTabDropTail" data-drop-before={dragSnapshot.currentTarget?.kind === "tabstrip" && dragSnapshot.currentTarget.groupId === (groupId ?? "active") && dragSnapshot.currentTarget.index === panes.length ? "true" : undefined} />
          </div>
          <div aria-hidden="true" className={`workbenchFileTabFade left ${scrollState.canScrollLeft ? "isVisible" : ""}`} />
          <div aria-hidden="true" className={`workbenchFileTabFade right ${scrollState.canScrollRight ? "isVisible" : ""}`} />
        </div>
      </div>
      <div className="workbenchFileTabbarDragFill" />
    </div>
  );
}

function DefaultFileTab({
  paneId,
  groupId,
  drag,
  dragging,
  activate,
  close,
  icon,
  isActive,
  label,
  path,
  closeable,
  dirty,
  conflict,
}: {
  paneId: string;
  groupId: string;
  drag: WorkbenchPaneDragController;
  dragging: boolean;
  activate: (event: MouseEvent<HTMLButtonElement>) => void;
  close: () => void;
  icon: ResolvedFileIcon;
  isActive: boolean;
  label: string;
  path: string;
  closeable: boolean;
  dirty?: boolean;
  conflict?: boolean;
}) {
  return (
    <div className={`workbenchFileTab ${isActive ? "isActive" : "isInactive"}`} data-dragging={dragging ? "true" : undefined}>
      <button
        type="button"
        draggable
        onDragStart={(event) => {
          event.currentTarget.blur();
          const payload: DraggedWorkbenchPane = { kind: "the-editor/pane", paneId };
          event.dataTransfer.effectAllowed = "move";
          event.dataTransfer.setData("application/x-the-editor-pane", JSON.stringify(payload));
          event.dataTransfer.setData("text/plain", label);
          drag.beginPaneDrag(paneId, groupId);
        }}
        onDragEnd={(event) => {
          event.currentTarget.blur();
          drag.clear();
        }}
        onClick={activate}
        title={path}
        className="workbenchFileTabButton"
      >
        <DefaultWorkbenchTabIcon icon={icon} />
        <span className="workbenchFileTabLabel">{label}</span>
      </button>
      {conflict ? <span className="workbenchFileTabStatusLine conflict" aria-hidden="true" /> : dirty ? <span className="workbenchFileTabStatusLine dirty" aria-hidden="true" /> : null}
      {closeable ? (
        <>
          <div aria-hidden="true" className="workbenchFileTabCloseFade" />
          <button
            type="button"
            onMouseDown={(event) => {
              event.preventDefault();
              event.stopPropagation();
            }}
            onClick={(event) => { event.stopPropagation(); close(); }}
            title="Close tab"
            aria-label={`Close ${label}`}
            className="workbenchFileTabClose"
          >
            <svg aria-hidden="true" viewBox="0 0 16 16" className="workbenchFileTabCloseIcon">
              <path d="M4.15 4.15a.5.5 0 0 1 .7 0L8 7.29l3.15-3.14a.5.5 0 1 1 .7.7L8.71 8l3.14 3.15a.5.5 0 0 1-.7.7L8 8.71l-3.15 3.14a.5.5 0 1 1-.7-.7L7.29 8 4.15 4.85a.5.5 0 0 1 0-.7" fill="currentColor" />
            </svg>
          </button>
        </>
      ) : null}
    </div>
  );
}

function DefaultWorkbenchTabIcon({ icon }: { icon: ResolvedFileIcon }) {
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 16 16"
      width={16}
      height={16}
      className="workbenchFileTabIcon"
      style={icon.color ? { color: icon.color } : undefined}
    >
      <use href={`#${icon.name}`} />
    </svg>
  );
}

function hasDraggedPane(event: DragEvent<HTMLElement>): boolean {
  return Array.from(event.dataTransfer.types).includes("application/x-the-editor-pane");
}

function readDraggedPane(event: DragEvent<HTMLElement>): DraggedWorkbenchPane | undefined {
  const raw = event.dataTransfer.getData("application/x-the-editor-pane");
  if (!raw) return undefined;
  try {
    const parsed = JSON.parse(raw) as Partial<DraggedWorkbenchPane>;
    return parsed.kind === "the-editor/pane" && typeof parsed.paneId === "string"
      ? { kind: "the-editor/pane", paneId: parsed.paneId }
      : undefined;
  } catch {
    return undefined;
  }
}

function tabDropIndex(event: DragEvent<HTMLElement>, fallback: number): number {
  const item = (event.target as Element | null)?.closest?.(".workbenchFileTabItem");
  if (!(item instanceof HTMLElement)) return fallback;
  const siblings = Array.from(item.parentElement?.querySelectorAll<HTMLElement>(".workbenchFileTabItem") ?? []);
  const index = siblings.indexOf(item);
  if (index < 0) return fallback;
  const rect = item.getBoundingClientRect();
  return event.clientX < rect.left + rect.width / 2 ? index : index + 1;
}
