import type { EditorUiModule, WorkbenchSnapshot } from "@the-editor/editor/ui";
import "./capability.css";

interface CapabilityToolState {
  counter: number;
  ghostTextEnabled: boolean;
  highlightEnabled: boolean;
  tickerRunning: boolean;
  lastEvent: string;
  focus: string;
  selectedStatusline?: string;
  thisToolFocused: boolean;
}

interface CapabilityStatuslineState {
  counter: number;
  ghostTextEnabled: boolean;
  highlightEnabled: boolean;
  tickerRunning: boolean;
  lastEvent: string;
  focus: string;
}

const capabilityToolTypeId = "capability-lab.panel";
const capabilityStatuslineImplementationId = "capability-lab.statusline";

const capabilityLabUi: EditorUiModule = (host) => {
  host.roles.register<CapabilityStatuslineState>({
    role: "statusline",
    implementationId: capabilityStatuslineImplementationId,
    render({ state, workbench }) {
      return <CapabilityStatusline state={state} workbench={workbench} />;
    },
  });

  host.tools.register<CapabilityToolState>({
    typeId: capabilityToolTypeId,
    render({ commands, state, tool, workbench }) {
      return <CapabilityLabPanel dispatch={commands.dispatch} state={state} toolId={tool.id} workbench={workbench} />;
    },
  });
};

export default capabilityLabUi;

function CapabilityLabPanel({
  dispatch,
  state,
  toolId,
  workbench,
}: {
  dispatch: (dispatch: { id: string; args?: unknown }) => Promise<unknown>;
  state: CapabilityToolState | undefined;
  toolId: string;
  workbench: WorkbenchSnapshot;
}) {
  const statusline = workbench.roles.statusline;
  const focus = workbench.focus?.kind === "tool"
    ? `tool:${workbench.focus.toolId}`
    : workbench.focus?.kind === "role"
      ? `role:${workbench.focus.role}`
      : "editor";

  return (
    <section className="capabilityLabPanel">
      <header className="capabilityLabHeader">
        <div>
          <div className="capabilityLabEyebrow">mock extension</div>
          <h2>Capability Lab</h2>
          <p>Exercises invalidation, tool snapshots, focus routing, decorations, role switching, and diagnostics.</p>
        </div>
        <div className="capabilityLabHeaderActions">
          <button type="button" onClick={() => { void dispatch({ id: "capability_lab.focusTool" }); }}>focus tool</button>
          <button type="button" onClick={() => { void dispatch({ id: "capability_lab.close" }); }}>close</button>
        </div>
      </header>

      <div className="capabilityLabGrid">
        <section className="capabilityLabCard">
          <h3>Live state</h3>
          <dl>
            <div><dt>counter</dt><dd>{state?.counter ?? 0}</dd></div>
            <div><dt>ticker</dt><dd>{state?.tickerRunning ? "running" : "stopped"}</dd></div>
            <div><dt>ghost text</dt><dd>{state?.ghostTextEnabled ? "on" : "off"}</dd></div>
            <div><dt>highlight</dt><dd>{state?.highlightEnabled ? "on" : "off"}</dd></div>
            <div><dt>host focus</dt><dd>{focus}</dd></div>
            <div><dt>tool focus</dt><dd>{state?.thisToolFocused ? `focused (${toolId})` : "not focused"}</dd></div>
            <div><dt>statusline</dt><dd>{statusline?.implementationId ?? "none"}</dd></div>
            <div><dt>last event</dt><dd>{state?.lastEvent ?? "ready"}</dd></div>
          </dl>
        </section>

        <section className="capabilityLabCard">
          <h3>Workbench invalidation</h3>
          <p>
            Start the ticker or fire an async ping. The panel, statusline, and in-buffer decorations should update
            without further input.
          </p>
          <div className="capabilityLabButtons">
            <button type="button" onClick={() => { void dispatch({ id: "capability_lab.toggleTicker" }); }}>
              {state?.tickerRunning ? "stop ticker" : "start ticker"}
            </button>
            <button type="button" onClick={() => { void dispatch({ id: "capability_lab.kickAsyncUpdate" }); }}>async ping</button>
            <button type="button" onClick={() => { void dispatch({ id: "capability_lab.bumpCounter" }); }}>step counter</button>
            <button type="button" onClick={() => { void dispatch({ id: "capability_lab.resetCounter" }); }}>reset counter</button>
          </div>
        </section>

        <section className="capabilityLabCard">
          <h3>Decorations</h3>
          <p>Toggle the JS-rendered highlight and ghost text drawn on top of the visible frame.</p>
          <div className="capabilityLabButtons">
            <button type="button" onClick={() => { void dispatch({ id: "capability_lab.toggleHighlight" }); }}>
              {state?.highlightEnabled ? "disable highlight" : "enable highlight"}
            </button>
            <button type="button" onClick={() => { void dispatch({ id: "capability_lab.toggleGhostText" }); }}>
              {state?.ghostTextEnabled ? "disable ghost text" : "enable ghost text"}
            </button>
          </div>
        </section>

        <section className="capabilityLabCard">
          <h3>Focus + routing</h3>
          <p>With tool focus, use tool-scope keys: <code>g</code> <code>h</code> <code>t</code> <code>n</code> <code>r</code> <code>e</code> <code>:</code>.</p>
          <div className="capabilityLabButtons">
            <button type="button" onClick={() => { void dispatch({ id: "capability_lab.focusTool" }); }}>focus tool</button>
            <button type="button" onClick={() => { void dispatch({ id: "capability_lab.focusEditor" }); }}>focus editor</button>
            <button type="button" onClick={() => { void dispatch({ id: "capability_lab.focusPalette" }); }}>focus palette</button>
          </div>
        </section>

        <section className="capabilityLabCard">
          <h3>Roles</h3>
          <p>Swap the host-owned statusline between automatic selection and this extension's implementation.</p>
          <div className="capabilityLabButtons">
            <button type="button" onClick={() => { void dispatch({ id: "capability_lab.selectStatusline" }); }}>use capability statusline</button>
            <button type="button" onClick={() => { void dispatch({ id: "capability_lab.clearStatuslineSelection" }); }}>use automatic statusline</button>
          </div>
        </section>

        <section className="capabilityLabCard">
          <h3>Diagnostics</h3>
          <p>Emit a mock warning and confirm it appears in the workbench inspector.</p>
          <div className="capabilityLabButtons">
            <button type="button" onClick={() => { void dispatch({ id: "capability_lab.reportDiagnostic" }); }}>report diagnostic</button>
            <button type="button" onClick={() => { void dispatch({ id: "capability_lab.clearDiagnostic" }); }}>clear diagnostic</button>
          </div>
        </section>
      </div>
    </section>
  );
}

function CapabilityStatusline({ state, workbench }: { state: CapabilityStatuslineState | undefined; workbench: WorkbenchSnapshot }) {
  return (
    <div className="capabilityLabStatusline">
      <span className="pill accent">CAP LAB</span>
      <span className="pill">count {state?.counter ?? 0}</span>
      <span className="pill">ticker {state?.tickerRunning ? "on" : "off"}</span>
      <span className="pill">ghost {state?.ghostTextEnabled ? "on" : "off"}</span>
      <span className="pill">highlight {state?.highlightEnabled ? "on" : "off"}</span>
      <span className="pill">focus {state?.focus ?? "editor"}</span>
      <span className="pill subtle">{state?.lastEvent ?? "ready"}</span>
      {workbench.diagnostics?.length ? <span className="pill warning">diagnostics {workbench.diagnostics.length}</span> : null}
    </div>
  );
}
