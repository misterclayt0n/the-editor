import { Fragment, useEffect, useLayoutEffect, useMemo, useRef, useSyncExternalStore, type ReactNode } from "react";
import type { DiagnosticEntry, NotificationEntry } from "@the-editor/editor";
import { resolveThemeStyleLoose, type ResolvedThemeDefinition } from "@the-editor/editor/themes";
import type { EditorUiModule, UiSessionClient, WorkbenchSnapshot } from "@the-editor/editor/ui";
import "./commandPalette.css";
import "./inputPrompt.css";
import "./picker.css";
import "./workbenchUi.css";

interface CommandPaletteItemView {
  id: string;
  title: string;
  detail?: string;
}

interface CommandPaletteViewState {
  visible: boolean;
  sessionId?: string;
}

interface CommandPaletteSessionViewState {
  query: string;
  selectedIndex: number;
  placeholder?: string;
  items: CommandPaletteItemView[];
}

interface InputPromptViewState {
  visible: boolean;
  sessionId?: string;
}

interface InputPromptSessionViewState {
  title?: string;
  query: string;
  placeholder?: string;
  error?: string;
}

interface PickerItemView {
  id: string;
  title: string;
  subtitle?: string;
  detail?: string;
}

interface PickerPreviewViewRowState {
  text: string;
  lineNumber?: number | null;
}

interface PickerPreviewViewSyntaxSpan {
  row: number;
  startCol: number;
  endCol: number;
  kind: number;
  scope?: string;
}

interface PickerPreviewViewState {
  kind: "text" | "empty";
  title?: string;
  detail?: string;
  text?: string;
  startLine?: number;
  endLine?: number;
  truncated?: boolean;
  binary?: boolean;
  rows?: PickerPreviewViewRowState[];
  syntax?: PickerPreviewViewSyntaxSpan[];
}

interface PickerViewState {
  visible: boolean;
  sessionId?: string;
}

interface PickerSessionViewState {
  providerId?: string;
  title?: string;
  query: string;
  placeholder?: string;
  selectedIndex: number;
  items: PickerItemView[];
  status?: { primary?: string; secondary?: string };
  emptyText?: string;
  preview?: PickerPreviewViewState | null;
}

interface StatuslineViewState {
  mode?: string;
  activePath?: string;
  activeRoleImplementation?: string;
  diagnosticsCount?: number;
  selectedCommandPalette?: string;
}

interface NotificationsViewState {
  items: NotificationEntry[];
}

const inspectorToolTypeId = "default.inspector";

let pickerUiTraceSeq = 0;
let pickerOverlayInstanceSeq = 0;

function pickerUiTrace(payload: Record<string, unknown>): void {
  console.log(`[picker-ui] ${JSON.stringify({ seq: ++pickerUiTraceSeq, ...payload })}`);
}

function describeElement(element: Element | null | undefined): string | undefined {
  if (!(element instanceof HTMLElement)) return undefined;
  const id = element.id ? `#${element.id}` : "";
  const className = typeof element.className === "string"
    ? element.className.trim().split(/\s+/).filter(Boolean).slice(0, 3).map((name) => `.${name}`).join("")
    : "";
  return `${element.tagName.toLowerCase()}${id}${className}`;
}

const uiModule: EditorUiModule = (host) => {
  host.roles.register<CommandPaletteViewState>({
    role: "commandPalette",
    implementationId: "default.commandPalette",
    render({ state, sessions }) {
      if (!state?.visible || !state.sessionId) return null;
      return <CommandPaletteOverlay sessionId={state.sessionId} sessions={sessions} />;
    },
  });

  host.roles.register<InputPromptViewState>({
    role: "inputPrompt",
    implementationId: "default.inputPrompt",
    render({ state, sessions }) {
      if (!state?.visible || !state.sessionId) return null;
      return <InputPromptOverlay sessionId={state.sessionId} sessions={sessions} />;
    },
  });

  host.roles.register<PickerViewState>({
    role: "picker",
    implementationId: "default.picker",
    render({ state, sessions, workbench }) {
      pickerUiTrace({
        event: "role.render",
        visible: Boolean(state?.visible),
        sessionId: state?.sessionId,
      });
      if (!state?.visible || !state.sessionId) {
        pickerUiTrace({
          event: "role.render.null",
          visible: Boolean(state?.visible),
          sessionId: state?.sessionId,
        });
        return null;
      }
      return <PickerOverlay sessionId={state.sessionId} sessions={sessions} workbench={workbench} />;
    },
  });

  host.roles.register<NotificationsViewState>({
    role: "notifications",
    implementationId: "default.notifications",
    render({ state }) {
      if (!state?.items?.length) return null;
      return <NotificationsOverlay items={state.items} />;
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

function useUiSession<TState>(sessions: UiSessionClient, sessionId: string): TState | undefined {
  return useSyncExternalStore(
    (listener) => sessions.subscribe(sessionId, listener),
    () => sessions.get<TState>(sessionId)?.state,
    () => sessions.get<TState>(sessionId)?.state,
  );
}

function CommandPaletteOverlay({
  sessionId,
  sessions,
}: {
  sessionId: string;
  sessions: UiSessionClient;
}) {
  const state = useUiSession<CommandPaletteSessionViewState>(sessions, sessionId);
  const inputRef = useRef<HTMLInputElement>(null);
  const itemsRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const input = inputRef.current;
    if (!input || !state) return;
    input.focus();
    const end = input.value.length;
    input.setSelectionRange(end, end);
  }, [sessionId]);

  useLayoutEffect(() => {
    if (!state) return;
    scrollActiveItemIntoView(itemsRef.current, '.commandPaletteItem[data-active="true"]');
  }, [state?.items, state?.selectedIndex]);

  if (!state) return null;

  return (
    <section className="commandPaletteOverlay">
      <button
        type="button"
        className="commandPaletteBackdrop"
        aria-label="Close command palette"
        onClick={() => {
          void sessions.dispatch(sessionId, "close");
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
                    void sessions.dispatch(sessionId, "setQuery", { query: event.target.value });
                  }}
                  onKeyDown={(event) => {
                    if (event.key === "ArrowUp") {
                      event.preventDefault();
                      void sessions.dispatch(sessionId, "moveSelection", { delta: -1 });
                      return;
                    }
                    if (event.key === "ArrowDown") {
                      event.preventDefault();
                      void sessions.dispatch(sessionId, "moveSelection", { delta: 1 });
                      return;
                    }
                    if (event.key === "Enter") {
                      event.preventDefault();
                      void sessions.dispatch(sessionId, "accept");
                      return;
                    }
                    if (event.key === "Escape") {
                      event.preventDefault();
                      void sessions.dispatch(sessionId, "close");
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
                        void sessions.dispatch(sessionId, "selectIndex", { index });
                      }}
                      onMouseDown={(event) => {
                        event.preventDefault();
                      }}
                      onClick={() => {
                        void sessions.dispatch(sessionId, "selectIndex", { index }).then(() => sessions.dispatch(sessionId, "accept"));
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

function scrollActiveItemIntoView(container: HTMLDivElement | null, selector: string): void {
  if (!container) return;
  const active = container.querySelector<HTMLElement>(selector);
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

function PickerOverlay({
  sessionId,
  sessions,
  workbench,
}: {
  sessionId: string;
  sessions: UiSessionClient;
  workbench: WorkbenchSnapshot;
}) {
  const state = useUiSession<PickerSessionViewState>(sessions, sessionId);
  const previewSyntax = useMemo(
    () => previewSyntaxByRow(state?.preview?.syntax ?? []),
    [state?.preview?.syntax],
  );
  const previewTheme = workbench.theme?.active;
  const syntaxLegend = workbench.syntaxLegend ?? {};
  const inputRef = useRef<HTMLInputElement>(null);
  const itemsRef = useRef<HTMLDivElement>(null);
  const overlayIdRef = useRef<number>(0);
  if (!overlayIdRef.current) overlayIdRef.current = ++pickerOverlayInstanceSeq;
  const overlayId = overlayIdRef.current;

  useEffect(() => {
    pickerUiTrace({
      event: "overlay.mount",
      overlayId,
      sessionId,
      activeElement: describeElement(document.activeElement),
    });
    return () => {
      pickerUiTrace({
        event: "overlay.unmount",
        overlayId,
        sessionId,
        activeElement: describeElement(document.activeElement),
      });
    };
  }, []);

  useEffect(() => {
    pickerUiTrace({
      event: "overlay.session",
      overlayId,
      sessionId,
      activeElement: describeElement(document.activeElement),
    });
  }, [overlayId, sessionId]);

  useEffect(() => {
    pickerUiTrace({
      event: state ? "overlay.state" : "overlay.state.missing",
      overlayId,
      sessionId,
      query: state?.query,
      selectedIndex: state?.selectedIndex,
      itemCount: state?.items.length,
      previewKind: state?.preview?.kind,
      previewTitle: state?.preview?.title,
      activeElement: describeElement(document.activeElement),
    });
  }, [overlayId, sessionId, state, state?.query, state?.selectedIndex, state?.items.length, state?.preview?.kind, state?.preview?.title]);

  useEffect(() => {
    const input = inputRef.current;
    if (!input || !state) {
      pickerUiTrace({
        event: "overlay.focus.skipped",
        overlayId,
        sessionId,
        hasInput: Boolean(input),
        hasState: Boolean(state),
        activeElement: describeElement(document.activeElement),
      });
      return;
    }
    pickerUiTrace({
      event: "overlay.focus.begin",
      overlayId,
      sessionId,
      activeElement: describeElement(document.activeElement),
      input: describeElement(input),
    });
    input.focus();
    const end = input.value.length;
    input.setSelectionRange(end, end);
    pickerUiTrace({
      event: "overlay.focus.done",
      overlayId,
      sessionId,
      activeElement: describeElement(document.activeElement),
      selectionStart: input.selectionStart,
      selectionEnd: input.selectionEnd,
    });
  }, [overlayId, sessionId]);

  useLayoutEffect(() => {
    if (!state) return;
    pickerUiTrace({
      event: "overlay.scrollActive",
      overlayId,
      sessionId,
      selectedIndex: state.selectedIndex,
      itemCount: state.items.length,
    });
    scrollActiveItemIntoView(itemsRef.current, '.pickerItem[data-active="true"]');
  }, [overlayId, sessionId, state?.items, state?.selectedIndex]);

  if (!state) {
    pickerUiTrace({
      event: "overlay.render.null",
      overlayId,
      sessionId,
      activeElement: describeElement(document.activeElement),
    });
    return null;
  }

  pickerUiTrace({
    event: "overlay.render",
    overlayId,
    sessionId,
    query: state.query,
    selectedIndex: state.selectedIndex,
    itemCount: state.items.length,
    previewKind: state.preview?.kind,
    activeElement: describeElement(document.activeElement),
  });

  return (
    <section
      className="pickerOverlay"
      onMouseDown={(event) => {
        pickerUiTrace({
          event: "overlay.mouseDown",
          overlayId,
          sessionId,
          target: describeElement(event.target as Element | null),
          activeElement: describeElement(document.activeElement),
        });
      }}
      onClick={(event) => {
        pickerUiTrace({
          event: "overlay.click",
          overlayId,
          sessionId,
          target: describeElement(event.target as Element | null),
          activeElement: describeElement(document.activeElement),
        });
      }}
    >
      <div
        className="pickerBackdrop"
        onMouseDown={(event) => {
          event.preventDefault();
          pickerUiTrace({
            event: "backdrop.mouseDown.ignored",
            overlayId,
            sessionId,
            target: describeElement(event.target as Element | null),
            activeElement: describeElement(document.activeElement),
          });
        }}
        onClick={(event) => {
          event.preventDefault();
          pickerUiTrace({
            event: "backdrop.click.ignored",
            overlayId,
            sessionId,
            target: describeElement(event.target as Element | null),
            activeElement: describeElement(document.activeElement),
          });
        }}
      />
      <div className="pickerViewport">
        <div className="pickerShell">
          <div className="pickerSurface">
            <div className="pickerHeader">
              <div className="pickerInputRow">
                <CommandPaletteSearchIcon />
                <input
                  ref={inputRef}
                  className="pickerInput"
                  placeholder={state.placeholder || "Find files"}
                  spellCheck={false}
                  value={state.query}
                  onFocus={(event) => {
                    pickerUiTrace({
                      event: "input.focus",
                      overlayId,
                      sessionId,
                      target: describeElement(event.target),
                      activeElement: describeElement(document.activeElement),
                    });
                  }}
                  onBlur={(event) => {
                    pickerUiTrace({
                      event: "input.blur",
                      overlayId,
                      sessionId,
                      target: describeElement(event.target),
                      relatedTarget: describeElement(event.relatedTarget as Element | null),
                      activeElement: describeElement(document.activeElement),
                    });
                  }}
                  onMouseDown={(event) => {
                    pickerUiTrace({
                      event: "input.mouseDown",
                      overlayId,
                      sessionId,
                      target: describeElement(event.target),
                      activeElement: describeElement(document.activeElement),
                    });
                  }}
                  onChange={(event) => {
                    pickerUiTrace({
                      event: "input.change",
                      overlayId,
                      sessionId,
                      query: event.target.value,
                      activeElement: describeElement(document.activeElement),
                    });
                    void sessions.dispatch(sessionId, "setQuery", { query: event.target.value });
                  }}
                  onKeyDown={(event) => {
                    pickerUiTrace({
                      event: "input.keyDown",
                      overlayId,
                      sessionId,
                      key: event.key,
                      metaKey: event.metaKey,
                      ctrlKey: event.ctrlKey,
                      altKey: event.altKey,
                      shiftKey: event.shiftKey,
                      activeElement: describeElement(document.activeElement),
                    });
                    if (event.key === "ArrowUp") {
                      event.preventDefault();
                      void sessions.dispatch(sessionId, "moveSelection", { delta: -1 });
                      return;
                    }
                    if (event.key === "ArrowDown") {
                      event.preventDefault();
                      void sessions.dispatch(sessionId, "moveSelection", { delta: 1 });
                      return;
                    }
                    if (event.key === "Enter") {
                      event.preventDefault();
                      void sessions.dispatch(sessionId, "accept");
                      return;
                    }
                    if (event.key === "Escape") {
                      event.preventDefault();
                      void sessions.dispatch(sessionId, "close");
                    }
                  }}
                />
                {state.title ? <span className="pickerTitleBadge">{state.title}</span> : null}
              </div>
            </div>
            <div className="pickerBody">
              <div ref={itemsRef} className="pickerResults">
                {state.items.length ? state.items.map((item, index) => {
                  const isActive = index === state.selectedIndex;
                  return (
                    <button
                      key={item.id}
                      type="button"
                      className="pickerItem"
                      data-active={isActive ? "true" : undefined}
                      onMouseDown={(event) => {
                        event.preventDefault();
                        pickerUiTrace({
                          event: "item.mouseDown",
                          overlayId,
                          sessionId,
                          index,
                          itemId: item.id,
                          active: isActive,
                          activeElement: describeElement(document.activeElement),
                        });
                      }}
                      onClick={() => {
                        pickerUiTrace({
                          event: "item.click",
                          overlayId,
                          sessionId,
                          index,
                          itemId: item.id,
                          active: isActive,
                          activeElement: describeElement(document.activeElement),
                        });
                        void sessions.dispatch(sessionId, "selectIndex", { index }).then(() => sessions.dispatch(sessionId, "accept"));
                      }}
                    >
                      <div className="pickerItemMain">
                        <div className="pickerItemTitle">{item.title}</div>
                        {item.subtitle ? <div className="pickerItemSubtitle">{item.subtitle}</div> : null}
                      </div>
                      {item.detail ? <div className="pickerItemDetail">{item.detail}</div> : null}
                    </button>
                  );
                }) : <div className="pickerEmpty">{state.emptyText || "No results"}</div>}
              </div>
              <aside className="pickerPreviewPane">
                {state.preview?.kind === "text" ? (
                  <>
                    <div className="pickerPreviewHeader">
                      <div className="pickerPreviewTitle">{state.preview.title || "Preview"}</div>
                      {state.preview.detail ? <div className="pickerPreviewDetail">{state.preview.detail}</div> : null}
                    </div>
                    {state.preview.binary ? (
                      <div className="pickerPreviewEmpty">Binary file</div>
                    ) : state.preview.rows?.length ? (
                      <div className="pickerPreviewCode">
                        {state.preview.rows.map((row, rowIndex) => (
                          <div key={`${row.lineNumber ?? rowIndex}:${rowIndex}`} className="pickerPreviewCodeRow">
                            <div className="pickerPreviewLineNumber">{row.lineNumber ?? ""}</div>
                            <div className="pickerPreviewLineText">
                              {renderPreviewLine(row.text, previewSyntax.get(rowIndex) ?? [], previewTheme, syntaxLegend)}
                            </div>
                          </div>
                        ))}
                      </div>
                    ) : (
                      <pre className="pickerPreviewText">{state.preview.text || ""}</pre>
                    )}
                  </>
                ) : state.preview?.kind === "empty" ? (
                  <div className="pickerPreviewEmpty">{state.preview.detail || state.preview.title || "No preview"}</div>
                ) : (
                  <div className="pickerPreviewEmpty">No preview</div>
                )}
              </aside>
            </div>
            <div className="pickerFooter">
              <span>{state.status?.primary ?? ""}</span>
              <span>{state.status?.secondary ?? ""}</span>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function previewSyntaxByRow(spans: readonly PickerPreviewViewSyntaxSpan[]): Map<number, PickerPreviewViewSyntaxSpan[]> {
  const rows = new Map<number, PickerPreviewViewSyntaxSpan[]>();
  for (const span of spans) {
    const list = rows.get(span.row) ?? [];
    list.push(span);
    rows.set(span.row, list);
  }
  for (const list of rows.values()) {
    list.sort((a, b) => a.startCol - b.startCol || a.endCol - b.endCol || a.kind - b.kind);
  }
  return rows;
}

function renderPreviewLine(
  text: string,
  spans: readonly PickerPreviewViewSyntaxSpan[],
  theme: ResolvedThemeDefinition | undefined,
  syntaxLegend: Record<number, string>,
) {
  try {
    const safeText = typeof text === "string" ? text : String(text ?? "");
    if (!spans.length) return <>{safeText || " "}</>;
    const cells = Array.from(safeText);
    const parts: ReactNode[] = [];
    let col = 0;

    for (const span of spans) {
      const start = Math.max(0, Math.min(cells.length, span.startCol));
      const end = Math.max(start, Math.min(cells.length, span.endCol));
      if (start > col) {
        parts.push(<Fragment key={`plain:${col}-${start}`}>{cells.slice(col, start).join("") || " "}</Fragment>);
      }
      const chunk = cells.slice(start, end).join("");
      if (chunk) {
        parts.push(
          <span
            key={`syntax:${start}-${end}-${span.kind}`}
            style={{ color: previewSyntaxColor(theme, span.scope ?? syntaxLegend[span.kind]) }}
          >
            {chunk}
          </span>,
        );
      }
      col = end;
    }

    if (col < cells.length) {
      parts.push(<Fragment key={`plain:${col}-${cells.length}`}>{cells.slice(col).join("") || " "}</Fragment>);
    }

    return parts.length ? parts : <>{safeText || " "}</>;
  } catch (error) {
    pickerUiTrace({
      event: "preview.line.error",
      message: error instanceof Error ? error.message : String(error),
      textType: typeof text,
      spanCount: spans.length,
    });
    const safeText = typeof text === "string" ? text : String(text ?? "");
    return <>{safeText || " "}</>;
  }
}

function previewSyntaxColor(theme: ResolvedThemeDefinition | undefined, scope: unknown): string | undefined {
  if (!theme) return undefined;
  if (typeof scope !== "string") {
    if (scope !== undefined) {
      pickerUiTrace({
        event: "preview.scope.invalid",
        scopeType: typeof scope,
        scope,
      });
    }
    return resolveThemeStyleLoose(theme, "ui.text")?.fg;
  }
  return resolveThemeStyleLoose(theme, scope, fallbackSyntaxScopes[scope as keyof typeof fallbackSyntaxScopes] ?? "ui.text")?.fg
    ?? resolveThemeStyleLoose(theme, "ui.text")?.fg;
}

const fallbackSyntaxScopes: Record<string, string> = {
  keyword: "keyword",
  type: "type",
  variable: "variable",
  string: "string",
  comment: "comment",
};

function InputPromptOverlay({
  sessionId,
  sessions,
}: {
  sessionId: string;
  sessions: UiSessionClient;
}) {
  const state = useUiSession<InputPromptSessionViewState>(sessions, sessionId);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const input = inputRef.current;
    if (!input || !state) return;
    input.focus();
    const end = input.value.length;
    input.setSelectionRange(end, end);
  }, [sessionId]);

  if (!state) return null;

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
              void sessions.dispatch(sessionId, "setQuery", { query: event.target.value });
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                void sessions.dispatch(sessionId, "accept");
                return;
              }
              if (event.key !== "Escape") return;
              event.preventDefault();
              void sessions.dispatch(sessionId, "close");
            }}
          />
          <div className="inputPromptActions">
            <span className="inputPromptTitle">{state.title ?? "INPUT"}</span>
            <button
              type="button"
              className="inputPromptClose"
              aria-label="Close input prompt"
              onClick={() => {
                void sessions.dispatch(sessionId, "close");
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

function NotificationsOverlay({ items }: { items: NotificationEntry[] }) {
  return (
    <section className="notificationStack">
      {items.map((item) => (
        <div key={item.id} className={`notificationToast ${item.severity}`}>
          <div className="notificationSource">{item.source ?? item.severity}</div>
          <div className="notificationMessage">{item.message}</div>
          {item.details ? <div className="notificationDetails">{item.details}</div> : null}
        </div>
      ))}
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
