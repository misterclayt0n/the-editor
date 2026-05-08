import { useEffect, useLayoutEffect, useRef } from "react";
import type { DiagnosticEntry } from "@the-editor/editor";
import type { EditorUiModule, WorkbenchSnapshot } from "@the-editor/editor/ui";
import "./commandPalette.css";
import "./inputPrompt.css";
import "./workbenchUi.css";

interface CommandPaletteItemView {
  id: string;
  title: string;
  detail?: string;
}

interface CommandPaletteViewState {
  visible: boolean;
  query: string;
  selectedIndex: number;
  placeholder?: string;
  items: CommandPaletteItemView[];
}

interface InputPromptViewState {
  visible: boolean;
  title?: string;
  query: string;
  placeholder?: string;
  error?: string;
}

interface StatuslineViewState {
  mode?: string;
  activePath?: string;
  activeRoleImplementation?: string;
  diagnosticsCount?: number;
  selectedCommandPalette?: string;
}

const inspectorToolTypeId = "default.inspector";

const uiModule: EditorUiModule = (host) => {
  host.roles.register<CommandPaletteViewState>({
    role: "commandPalette",
    implementationId: "default.commandPalette",
    render({ state, commands }) {
      if (!state?.visible) return null;
      return <CommandPaletteOverlay state={state} dispatch={commands.dispatch} />;
    },
  });

  host.roles.register<InputPromptViewState>({
    role: "inputPrompt",
    implementationId: "default.inputPrompt",
    render({ state, commands }) {
      if (!state?.visible) return null;
      return <InputPromptOverlay state={state} dispatch={commands.dispatch} />;
    },
  });

  host.roles.register<StatuslineViewState>({
    role: "statusline",
    implementationId: "default.statusline",
    render({ state }) {
      return <Statusline state={state} />;
    },
  });

  host.tools.register({
    typeId: inspectorToolTypeId,
    render({ commands, workbench }) {
      return <WorkbenchInspector diagnostics={workbench.diagnostics ?? []} dispatch={commands.dispatch} workbench={workbench} />;
    },
  });
};

export default uiModule;

function CommandPaletteOverlay({
  state,
  dispatch,
}: {
  state: CommandPaletteViewState;
  dispatch: (dispatch: { id: string; args?: unknown }) => Promise<unknown>;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const itemsRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const input = inputRef.current;
    if (!input) return;
    input.focus();
    const end = input.value.length;
    input.setSelectionRange(end, end);
  }, [state.visible]);

  useLayoutEffect(() => {
    scrollActivePaletteItemIntoView(itemsRef.current);
  }, [state.items, state.selectedIndex]);

  return (
    <section className="commandPaletteOverlay">
      <button
        type="button"
        className="commandPaletteBackdrop"
        aria-label="Close command palette"
        onClick={() => {
          void dispatch({ id: "command_palette.close" });
        }}
      />
      <div className="commandPaletteViewport">
        <div className="commandPaletteShell">
          <div className="commandPaletteSurface">
            <div className="commandPalettePanel">
              <div className="commandPaletteInputRow">
                <CommandPaletteSearchIcon />
                <input
                  ref={inputRef}
                  className="commandPaletteInput"
                  placeholder={state.placeholder || "Type a command"}
                  spellCheck={false}
                  value={state.query}
                  onChange={(event) => {
                    void dispatch({ id: "command_palette.setQuery", args: { query: event.target.value } });
                  }}
                  onKeyDown={(event) => {
                    if (event.key === "ArrowUp") {
                      event.preventDefault();
                      void dispatch({ id: "command_palette.moveSelection", args: { delta: -1 } });
                      return;
                    }
                    if (event.key === "ArrowDown") {
                      event.preventDefault();
                      void dispatch({ id: "command_palette.moveSelection", args: { delta: 1 } });
                      return;
                    }
                    if (event.key === "Enter") {
                      event.preventDefault();
                      void dispatch({ id: "command_palette.accept" });
                      return;
                    }
                    if (event.key === "Escape") {
                      event.preventDefault();
                      void dispatch({ id: "command_palette.close" });
                    }
                  }}
                />
              </div>
              <div ref={itemsRef} className="commandPaletteItems">
                {state.items.length ? state.items.map((item, index) => {
                  const isActive = index === state.selectedIndex;
                  return (
                    <button
                      key={item.id}
                      type="button"
                      className="commandPaletteItem"
                      data-active={isActive ? "true" : undefined}
                      onMouseMove={() => {
                        if (isActive) return;
                        void dispatch({ id: "command_palette.selectIndex", args: { index } });
                      }}
                      onMouseDown={(event) => {
                        event.preventDefault();
                      }}
                      onClick={() => {
                        void dispatch({ id: "command_palette.selectIndex", args: { index } }).then(() => dispatch({ id: "command_palette.accept" }));
                      }}
                    >
                      <div className="commandPaletteItemMain">
                        <div className="commandPaletteTitle">{item.title}</div>
                        {item.detail ? <div className="commandPaletteDetail">{item.detail}</div> : null}
                      </div>
                    </button>
                  );
                }) : <div className="commandPaletteEmpty">No matching commands</div>}
              </div>
            </div>
            <div className="commandPaletteFooter">
              <span>Enter to run</span>
              <span>Esc to close</span>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function scrollActivePaletteItemIntoView(container: HTMLDivElement | null): void {
  if (!container) return;
  const active = container.querySelector<HTMLElement>('.commandPaletteItem[data-active="true"]');
  if (!active) return;

  const activeRect = active.getBoundingClientRect();
  const containerRect = container.getBoundingClientRect();

  if (activeRect.top < containerRect.top) {
    container.scrollTop += activeRect.top - containerRect.top;
    return;
  }
  if (activeRect.bottom > containerRect.bottom) {
    container.scrollTop += activeRect.bottom - containerRect.bottom;
  }
}

function InputPromptOverlay({
  state,
  dispatch,
}: {
  state: InputPromptViewState;
  dispatch: (dispatch: { id: string; args?: unknown }) => Promise<unknown>;
}) {
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const input = inputRef.current;
    if (!input) return;
    input.focus();
    const end = input.value.length;
    input.setSelectionRange(end, end);
  }, [state.visible]);

  return (
    <section className="inputPromptOverlay">
      <div className="inputPromptShell">
        <div className="inputPromptBar" data-error={state.error ? "true" : undefined}>
          <CommandPaletteSearchIcon />
          <input
            ref={inputRef}
            className="inputPromptInput"
            placeholder={state.placeholder || "Type input"}
            spellCheck={false}
            value={state.query}
            onChange={(event) => {
              void dispatch({ id: "input_prompt.setQuery", args: { query: event.target.value } });
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                void dispatch({ id: "input_prompt.accept" });
                return;
              }
              if (event.key !== "Escape") return;
              event.preventDefault();
              void dispatch({ id: "input_prompt.close" });
            }}
          />
          <div className="inputPromptActions">
            <span className="inputPromptTitle">{state.title ?? "INPUT"}</span>
            <button
              type="button"
              className="inputPromptClose"
              aria-label="Close input prompt"
              onClick={() => {
                void dispatch({ id: "input_prompt.close" });
              }}
            >
              ×
            </button>
          </div>
        </div>
        {state.error ? <div className="inputPromptError">{state.error}</div> : null}
      </div>
    </section>
  );
}

function Statusline({ state }: { state: StatuslineViewState | undefined }) {
  const mode = normalizeStatuslineMode(state?.mode);
  return (
    <div className={`statusline mode-${mode}`}>
      <div className="statuslineSegment mode">{mode.toUpperCase()}</div>
      <div className="statuslineSegment path">{state?.activePath ?? "[scratch]"}</div>
      <div className="statuslineSegment meta">palette: {state?.selectedCommandPalette ?? state?.activeRoleImplementation ?? "default"}</div>
      <div className="statuslineSegment diagnostics">diagnostics: {state?.diagnosticsCount ?? 0}</div>
    </div>
  );
}

function WorkbenchInspector({
  diagnostics,
  dispatch,
  workbench,
}: {
  diagnostics: DiagnosticEntry[];
  dispatch: (dispatch: { id: string; args?: unknown }) => Promise<unknown>;
  workbench: WorkbenchSnapshot;
}) {
  return (
    <section className="inspectorPanel">
      <header className="inspectorHeader">
        <div>
          <strong>Workbench Inspector</strong>
          <div className="inspectorSubtle">packages, roles, diagnostics</div>
        </div>
        <button className="inspectorButton" type="button" onClick={() => { void dispatch({ id: "workbench.toggleInspector" }); }}>Close</button>
      </header>

      <div className="inspectorBody">
        <section className="inspectorSection">
          <h3>Packages</h3>
          {workbench.packages.map((pkg) => (
            <div key={pkg.id} className="inspectorRow">
              <div>
                <div className="inspectorLabel">{pkg.id}</div>
                <div className="inspectorSubtle">loaded: {pkg.loadedTargets.join(", ") || "none"}</div>
              </div>
            </div>
          ))}
        </section>

        <section className="inspectorSection">
          <h3>Tools</h3>
          {workbench.tools?.length ? workbench.tools.map((tool) => (
            <div key={tool.id} className="inspectorRow">
              <div>
                <div className="inspectorLabel">{tool.title}</div>
                <div className="inspectorSubtle">{tool.typeId}</div>
              </div>
            </div>
          )) : <div className="inspectorSubtle">No tools.</div>}
        </section>

        <section className="inspectorSection">
          <h3>Roles</h3>
          {Object.entries(workbench.roles).map(([role, snapshot]) => (
            <div key={role} className="inspectorRoleCard">
              <div className="inspectorRoleHeader">
                <div>
                  <div className="inspectorLabel">{role}</div>
                  <div className="inspectorSubtle">active: {snapshot.implementationId ?? "none"}</div>
                </div>
                {snapshot.selectedImplementationId ? <div className="inspectorBadge">selected</div> : null}
              </div>
              <div className="inspectorRoleActions">
                {snapshot.available.map((available) => (
                  <button
                    key={available.packageId ? `${available.packageId}:${available.implementationId}` : available.implementationId}
                    className={available.implementationId === snapshot.implementationId ? "inspectorButton active" : "inspectorButton"}
                    type="button"
                    onClick={() => {
                      void dispatch({
                        id: "role.activate",
                        args: { role, implementationId: available.implementationId },
                      });
                    }}
                  >
                    {available.packageId ?? available.implementationId}
                  </button>
                ))}
                {snapshot.selectedImplementationId ? (
                  <button className="inspectorButton" type="button" onClick={() => { void dispatch({ id: "role.clearSelection", args: { role } }); }}>
                    automatic
                  </button>
                ) : null}
              </div>
              {snapshot.fallbackReason ? <div className="inspectorWarning">{snapshot.fallbackReason}</div> : null}
            </div>
          ))}
        </section>

        <section className="inspectorSection diagnostics">
          <h3>Focus</h3>
          <div className="inspectorSubtle">
            {workbench.focus?.kind === "tool"
              ? `tool: ${workbench.focus.toolId}`
              : workbench.focus?.kind === "role"
                ? `role: ${workbench.focus.role}`
                : "editor"}
          </div>
        </section>

        <section className="inspectorSection diagnostics">
          <h3>Diagnostics</h3>
          {diagnostics.length ? diagnostics.map((diagnostic) => (
            <div key={diagnostic.id} className={`diagnosticRow ${diagnostic.severity}`}>
              <div className="diagnosticMessage">{diagnostic.message}</div>
              {diagnostic.details ? <div className="diagnosticDetails">{diagnostic.details}</div> : null}
            </div>
          )) : <div className="inspectorSubtle">No diagnostics.</div>}
        </section>
      </div>
    </section>
  );
}

function normalizeStatuslineMode(mode?: string): string {
  if (mode === "insert" || mode === "select") return mode;
  return "normal";
}

function CommandPaletteSearchIcon() {
  return (
    <svg className="commandPaletteSearchIcon" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M10.5 4a6.5 6.5 0 1 0 4.03 11.6l4.43 4.43 1.06-1.06-4.43-4.43A6.5 6.5 0 0 0 10.5 4Zm0 1.5a5 5 0 1 1 0 10 5 5 0 0 1 0-10Z" fill="currentColor" />
    </svg>
  );
}
