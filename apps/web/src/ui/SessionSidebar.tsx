import { isMacDesktop } from "../platform/desktop";

export type SessionTab = {
  readonly id: string;
  readonly label: string;
};

type SessionSidebarProps = {
  readonly tabs: ReadonlyArray<SessionTab>;
  readonly activeId: string | null;
  readonly onSelect: (id: string) => void;
  readonly onAdd: () => void;
  readonly onClose: (id: string) => void;
};

/**
 * The session rail: one row per open workspace, vertical. It IS a workbench
 * panel (the shell chrome workbench's sidebar panel) — App portals this into
 * the panel's body. The top gutter reserves the macOS traffic lights and drags
 * the window; the `+` opens a folder as a new workspace.
 */
export function SessionSidebar({ tabs, activeId, onSelect, onAdd, onClose }: SessionSidebarProps) {
  return (
    <div className="session-sidebar" data-traffic-lights={isMacDesktop ? "" : undefined}>
      <div className="session-sidebar-gutter">
        <button
          type="button"
          className="session-add"
          onClick={onAdd}
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
              onClick={() => onSelect(tab.id)}
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
                  onClose(tab.id);
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
