import { useEffect, useMemo, useRef, useState } from "react";
import { resolveTabFileIcon, fileIconSpriteMarkup, tabLabel, type ResolvedFileIcon } from "./fileIcons/tabIcons.js";

export interface WorkbenchTabPane {
  id: string;
  title: string;
  path?: string;
  closeable: boolean;
  kind: string;
}

export function DefaultWorkbenchTabstrip({
  dispatch,
  panes,
  activePaneId,
  showWindowControls,
}: {
  dispatch: (dispatch: { id: string; args?: unknown }) => Promise<unknown>;
  panes: WorkbenchTabPane[];
  activePaneId?: string;
  showWindowControls?: boolean;
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

  return (
    <div className="workbenchTopTabsBar workbenchFileTabbarRoot" data-leading={showWindowControls ? "true" : undefined}>
      <div aria-hidden="true" className="workbenchFileTabSprite" dangerouslySetInnerHTML={{ __html: spriteMarkup }} />
      <div className="workbenchFileTabbarGroup">
        <div className="workbenchFileTabScrollerShell">
          <div ref={scrollerRef} className="workbenchFileTabScroller">
            {panes.map((pane) => {
              const isActive = pane.id === activePaneId;
              const icon = resolveTabFileIcon(pane);
              return (
                <div key={pane.id} className="workbenchFileTabItem" data-workbench-tab-active={isActive ? "true" : undefined}>
                  <DefaultFileTab
                    activate={() => {
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
                  />
                </div>
              );
            })}
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
  activate,
  close,
  icon,
  isActive,
  label,
  path,
  closeable,
}: {
  activate: () => void;
  close: () => void;
  icon: ResolvedFileIcon;
  isActive: boolean;
  label: string;
  path: string;
  closeable: boolean;
}) {
  return (
    <div className={`workbenchFileTab ${isActive ? "isActive" : "isInactive"}`}>
      <button type="button" onClick={activate} title={path} className="workbenchFileTabButton">
        <DefaultWorkbenchTabIcon icon={icon} />
        <span className="workbenchFileTabLabel">{label}</span>
      </button>
      {closeable ? (
        <>
          <div aria-hidden="true" className="workbenchFileTabCloseFade" />
          <button
            type="button"
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
