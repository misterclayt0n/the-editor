// The macOS traffic lights only exist in the Electron shell (hiddenInset
// title bar). Elsewhere there is nothing to reserve space for, so the bar
// hugs the left edge.
const desktop = (typeof window !== "undefined"
  ? (window as unknown as { theEditorDesktop?: { platform: string } }).theEditorDesktop
  : undefined);
const reserveTrafficLights = desktop?.platform === "darwin";

export type WorkspaceTab = {
  readonly id: string;
  readonly label: string;
};

type TitleBarProps = {
  readonly tabs: ReadonlyArray<WorkspaceTab>;
  readonly activeId: string | null;
  readonly onSelect: (id: string) => void;
  readonly onAdd: () => void;
  readonly onClose: (id: string) => void;
  readonly onToggleTree: () => void;
  readonly onToggleDiffView: () => void;
};

/**
 * The window's top band. It owns the traffic-light gutter and carries the
 * workspace tabs next to it — one tab per open session — plus a `+` to open a
 * folder as a new workspace. The file-tree toggle (⌘B) sits at the right,
 * acting on whichever workspace is active.
 */
export function TitleBar({ tabs, activeId, onSelect, onAdd, onClose, onToggleTree, onToggleDiffView }: TitleBarProps) {
  return (
    <div className="titlebar" data-traffic-lights={reserveTrafficLights ? "" : undefined}>
      <button
        type="button"
        className="titlebar-button titlebar-tree-toggle"
        onClick={onToggleTree}
        title="Toggle File Tree (⌘B)"
        aria-label="Toggle File Tree"
      >
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <rect
            x="1.85"
            y="2.85"
            width="12.3"
            height="10.3"
            rx="2.4"
            stroke="currentColor"
            strokeWidth="1.3"
          />
          <line x1="6.15" y1="3.1" x2="6.15" y2="12.9" stroke="currentColor" strokeWidth="1.3" />
        </svg>
      </button>
      <button
        type="button"
        className="titlebar-button titlebar-diff-toggle"
        onClick={onToggleDiffView}
        title="Toggle Diff View (⌘⇧G)"
        aria-label="Toggle Diff View"
      >
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <rect x="2.85" y="2.85" width="4.3" height="10.3" rx="1.4" stroke="currentColor" strokeWidth="1.3" />
          <rect x="8.85" y="2.85" width="4.3" height="10.3" rx="1.4" stroke="currentColor" strokeWidth="1.3" />
          <line x1="5.5" y1="5.5" x2="5.5" y2="10.5" stroke="currentColor" strokeWidth="1.3" />
          <line x1="11.5" y1="5.5" x2="11.5" y2="10.5" stroke="currentColor" strokeWidth="1.3" />
        </svg>
      </button>
      <div className="workspace-tabs">
        {tabs.map((tab) => (
          <div key={tab.id} className="workspace-tab" data-active={tab.id === activeId ? "" : undefined}>
            <button
              type="button"
              className="workspace-tab-label"
              onClick={() => onSelect(tab.id)}
              title={tab.label}
            >
              {tab.label}
            </button>
            {tabs.length > 1 && (
              <button
                type="button"
                className="workspace-tab-close"
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
        <button
          type="button"
          className="titlebar-button workspace-add"
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
    </div>
  );
}
