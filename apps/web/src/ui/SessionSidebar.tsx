import { useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import { isMacDesktop } from "../platform/desktop";
import type { PanelRenderer } from "../workbench/panelRenderers";
import type { SessionStore } from "../sessionStore";

/**
 * The session rail: one row per open workspace, vertical. It's an ordinary
 * workbench panel — its renderer self-mounts this component and reads the shell
 * session store (no App-portal bridge). The top gutter reserves the macOS
 * traffic lights and drags the window; the `+` opens a folder as a new
 * workspace.
 */
export function SessionSidebar({ store }: { readonly store: SessionStore }) {
  const { tabs, activeId } = useSyncExternalStore(store.subscribe, store.getSnapshot);
  return (
    <div className="session-sidebar" data-traffic-lights={isMacDesktop ? "" : undefined}>
      <div className="session-sidebar-gutter">
        <button
          type="button"
          className="session-add"
          onClick={() => store.onAdd()}
          title="New Workspace"
          aria-label="New Workspace"
        >
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <line x1="8" y1="3.4" x2="8" y2="12.6" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
            <line x1="3.4" y1="8" x2="12.6" y2="8" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
          </svg>
        </button>
      </div>
      <div className="session-list">
        {tabs.map((tab) => (
          <div
            key={tab.id}
            className="session-item"
            data-active={tab.id === activeId ? "" : undefined}
          >
            <button
              type="button"
              className="session-item-label"
              onClick={() => store.onSelect(tab.id)}
              title={tab.label}
            >
              {tab.label}
            </button>
            {tabs.length > 1 && (
              <button
                type="button"
                className="session-item-close"
                onClick={(event) => {
                  event.stopPropagation();
                  store.onClose(tab.id);
                }}
                aria-label={`Close workspace ${tab.label}`}
                title="Close Workspace"
              >
                <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
                  <path
                    d="M1 1l8 8M9 1l-8 8"
                    stroke="currentColor"
                    strokeWidth="1.3"
                    strokeLinecap="round"
                  />
                </svg>
              </button>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

/**
 * The session-sidebar panel renderer: mounts <SessionSidebar> into its own
 * React root, fed by the shell session store. A normal panel like the file
 * tree — registered, dockable, chrome-styled by its region.
 */
export function createSessionSidebarRenderer(store: SessionStore): PanelRenderer {
  return {
    title: () => "Sessions",
    mount: (element) => {
      const root = createRoot(element);
      root.render(<SessionSidebar store={store} />);
      return { dispose: () => root.unmount() };
    },
  };
}
