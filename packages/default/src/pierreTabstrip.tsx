import { useEffect, useMemo, useRef, useState } from "react";
import { resolvePierreTabIcon, pierreTabIconSpriteMarkup, tabLabel, type PierreTabResolvedIcon } from "./pierreTabs/tabIcons.js";

export interface PierreWorkbenchTabPane {
  id: string;
  title: string;
  path?: string;
  closeable: boolean;
  kind: string;
}

export function PierreWorkbenchTabstrip({
  dispatch,
  panes,
  activePaneId,
  showWindowControls,
}: {
  dispatch: (dispatch: { id: string; args?: unknown }) => Promise<unknown>;
  panes: PierreWorkbenchTabPane[];
  activePaneId?: string;
  showWindowControls?: boolean;
}) {
  const scrollerRef = useRef<HTMLDivElement | null>(null);
  const [scrollState, setScrollState] = useState({ canScrollLeft: false, canScrollRight: false });
  const spriteMarkup = useMemo(() => pierreTabIconSpriteMarkup(), []);

  useEffect(() => {
    const scroller = scrollerRef.current;
    if (!scroller) return;
    const activeTab = scroller.querySelector<HTMLElement>('[data-pierre-tab-active="true"]');
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
    <div className="workbenchTopTabsBar pierreWorkbenchTabbarRoot" data-leading={showWindowControls ? "true" : undefined}>
      <div aria-hidden="true" className="pierreWorkbenchTabSprite" dangerouslySetInnerHTML={{ __html: spriteMarkup }} />
      <div className="pierreWorkbenchTabbarGroup">
        <div className="pierreWorkbenchTabScrollerShell">
          <div ref={scrollerRef} className="pierreWorkbenchTabScroller">
            {panes.map((pane) => {
              const isActive = pane.id === activePaneId;
              const icon = resolvePierreTabIcon(pane);
              return (
                <div key={pane.id} className="pierreWorkbenchTabItem" data-pierre-tab-active={isActive ? "true" : undefined}>
                  <PierreDefaultTab
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
          <div aria-hidden="true" className={`pierreWorkbenchTabFade left ${scrollState.canScrollLeft ? "isVisible" : ""}`} />
          <div aria-hidden="true" className={`pierreWorkbenchTabFade right ${scrollState.canScrollRight ? "isVisible" : ""}`} />
        </div>
      </div>
    </div>
  );
}

function PierreDefaultTab({
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
  icon: PierreTabResolvedIcon;
  isActive: boolean;
  label: string;
  path: string;
  closeable: boolean;
}) {
  return (
    <div className={`pierreWorkbenchTab ${isActive ? "isActive" : "isInactive"}`}>
      <button type="button" onMouseDown={(event) => { event.preventDefault(); activate(); }} title={path} className="pierreWorkbenchTabButton">
        <PierreWorkbenchTabIcon icon={icon} />
        <span className="pierreWorkbenchTabLabel">{label}</span>
      </button>
      {closeable ? (
        <>
          <div aria-hidden="true" className="pierreWorkbenchTabCloseFade" />
          <button
            type="button"
            onMouseDown={(event) => { event.preventDefault(); event.stopPropagation(); close(); }}
            title="Close tab"
            aria-label={`Close ${label}`}
            className="pierreWorkbenchTabClose"
          >
            <svg aria-hidden="true" viewBox="0 0 16 16" className="pierreWorkbenchTabCloseIcon">
              <path d="M4.15 4.15a.5.5 0 0 1 .7 0L8 7.29l3.15-3.14a.5.5 0 1 1 .7.7L8.71 8l3.14 3.15a.5.5 0 0 1-.7.7L8 8.71l-3.15 3.14a.5.5 0 1 1-.7-.7L7.29 8 4.15 4.85a.5.5 0 0 1 0-.7" fill="currentColor" />
            </svg>
          </button>
        </>
      ) : null}
    </div>
  );
}

function PierreWorkbenchTabIcon({ icon }: { icon: PierreTabResolvedIcon }) {
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 16 16"
      width={16}
      height={16}
      className="pierreWorkbenchTabIcon"
      style={icon.color ? { color: icon.color } : undefined}
    >
      <use href={`#${icon.name}`} />
    </svg>
  );
}
