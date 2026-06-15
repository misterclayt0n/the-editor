import { Effect } from "effect";
import type { AppRuntime } from "../runtime";

// The macOS traffic lights only exist in the Electron shell (hiddenInset
// title bar). Elsewhere there is nothing to reserve space for, so the bar
// hugs the left edge.
const desktop = (typeof window !== "undefined"
  ? (window as unknown as { theEditorDesktop?: { platform: string } }).theEditorDesktop
  : undefined);
const reserveTrafficLights = desktop?.platform === "darwin";

/**
 * The window's top band. It owns the traffic-light gutter and carries the
 * chrome controls that used to have nowhere to live — starting with the file
 * tree toggle, mirroring the native command (`files.toggleTree`, ⌘B).
 */
export function TitleBar({ runtime }: { runtime: AppRuntime }) {
  const toggleTree = () =>
    void Effect.runPromise(
      runtime.commands.execute("files.toggleTree").pipe(Effect.catchAll(() => Effect.void)),
    );

  return (
    <div className="titlebar" data-traffic-lights={reserveTrafficLights ? "" : undefined}>
      <button
        type="button"
        className="titlebar-button"
        onClick={toggleTree}
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
    </div>
  );
}
