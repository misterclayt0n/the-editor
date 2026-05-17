import { Fragment, useEffect, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore, type CSSProperties, type ReactNode, type RefObject } from "react";
import type { RenderFrame, SurfaceId, Viewport } from "@the-editor/contracts/runtime";
import type { BufferDecoration, CommandPaletteIcon, DiagnosticRecord, HealthDiagnosticEntry, NotificationEntry } from "@the-editor/editor";
import {
  SurfaceViewportController,
  type SurfaceViewportControllerState,
} from "@the-editor/editor/surfaceViewportController";
import type { CanvasTextSurfaceStyleOptions } from "@the-editor/editor/textSurfaceCanvas";
import { surfaceCursorAnchors, type SurfaceCursorAnchor } from "@the-editor/editor/cursorAnchors";
import { resolveThemeStyleLoose, type ResolvedThemeDefinition } from "@the-editor/editor/themes";
import type { EditorUiModule, UiSessionClient, WorkbenchSnapshot } from "@the-editor/editor/ui";
import type { FileTreeToolState } from "./fileTree.js";
import { diagnosticLocationLabel, diagnosticPrimaryLine } from "./diagnosticsShared.js";
import { DefaultFileTree } from "./fileTreeUi.js";
import { MarkdownView } from "@the-editor/editor/markdownView";
import { DefaultWorkbenchTabstrip } from "./workbenchTabstrip.js";
import { Command, CommandDialog, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList, CommandSeparator, CommandShortcut } from "@the-editor/ui/command";
import { Columns2Icon, FileIcon, FolderIcon, FolderOpenIcon, RefreshCwIcon, Rows2Icon, SaveIcon, TerminalIcon, type LucideIcon } from "lucide-react";
import "./fileTree.css";
import "./hover.css";
import "./markdown.css";
import "./inputPrompt.css";
import "./picker.css";
import "./workbenchUi.css";

interface CommandPaletteItemView {
  id: string;
  title: string;
  detail?: string;
  group?: string;
  alias?: string;
  keybinding?: string;
  icon?: CommandPaletteIcon;
}

interface CommandPaletteViewState {
  visible: boolean;
  sessionId?: string;
}

interface CommandPaletteSessionViewState {
  query: string;
  placeholder?: string;
  mode?: "commands" | "argument";
  commandTitle?: string;
  stack?: unknown[];
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

interface HoverViewState {
  visible: boolean;
  markdown?: string;
  source?: string;
  surfaceId?: number;
}

interface CompletionViewState {
  visible: boolean;
  sessionId?: string;
  surfaceId?: number;
}

interface CompletionSessionState {
  query: string;
  selectedIndex: number;
  items: Array<{
    id: string;
    label: string;
    labelHighlights: number[];
    detail?: string;
    detailHighlights: number[];
    labelDetail?: string;
    labelDescription?: string;
    kind?: string;
    badge?: string;
    source?: string;
    deprecated?: boolean;
  }>;
  loading: boolean;
  emptyText?: string;
}

interface SignatureHelpRoleState {
  visible: boolean;
  surfaceId?: number;
  items?: Array<{
    label: string;
    documentation?: string;
    activeParameter?: number;
    activeParameterRange?: { start: number; end: number };
  }>;
  activeSignature?: number;
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
  kind: "text" | "surface" | "empty";
  title?: string;
  detail?: string;
  text?: string;
  startLine?: number;
  endLine?: number;
  truncated?: boolean;
  binary?: boolean;
  rows?: PickerPreviewViewRowState[];
  syntax?: PickerPreviewViewSyntaxSpan[];
  activeLine?: number;
  annotation?: { line: number; severity?: "error" | "warning" | "info" | "hint"; message: string };
  path?: string;
  frame?: RenderFrame;
  viewport?: Viewport;
  lineCount?: number;
  syntaxLegend?: Record<number, string>;
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
  diagnosticsSummary?: string;
  selectedCommandPalette?: string;
  lspStatus?: string;
}

interface NotificationsViewState {
  items: NotificationEntry[];
}

const inspectorToolTypeId = "default.inspector";
const defaultTabstripImplementationId = "default.tabstrip";
const PICKER_PREVIEW_SURFACE_STYLE: CanvasTextSurfaceStyleOptions = {
  paddingPx: 12,
  fontSizePx: 13,
  lineHeightPx: 19,
  gutterReservePx: 88,
};

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
      if (!state?.visible || !state.sessionId) return null;
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

  host.tabstrips.register({
    implementationId: defaultTabstripImplementationId,
    render({ commands, group, panes, activePaneId, showWindowControls, drag }) {
      return (
        <DefaultWorkbenchTabstrip
          dispatch={commands.dispatch}
          panes={panes}
          activePaneId={activePaneId}
          showWindowControls={showWindowControls}
          groupId={group.id}
          drag={drag}
          dragSnapshot={drag.snapshot()}
        />
      );
    },
  });

  host.tools.register<FileTreeToolState>({
    typeId: "default.fileTree",
    render({ state, commands }) {
      return <DefaultFileTree state={state} commands={commands} contributions={host.fileTrees.all()} />;
    },
  });

  host.tools.register({
    typeId: inspectorToolTypeId,
    render({ commands, workbench }) {
      return <WorkbenchInspector diagnostics={workbench.diagnostics ?? []} healthDiagnostics={workbench.healthDiagnostics ?? []} dispatch={commands.dispatch} workbench={workbench} />;
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

function HoverCard({
  markdown,
  source,
  workbench,
  className,
  style,
  role,
  cardRef,
}: {
  markdown: string;
  source?: string;
  workbench: WorkbenchSnapshot;
  className?: string;
  style?: CSSProperties;
  role?: string;
  cardRef?: RefObject<HTMLDivElement | null>;
}) {
  return (
    <div
      ref={cardRef}
      className={["defaultHoverOverlay", className].filter(Boolean).join(" ")}
      style={style}
      role={role}
    >
      {source ? <div className="defaultHoverSource">{source}</div> : null}
      <MarkdownView
        className="editorMarkdown defaultHoverMarkdown"
        markdown={markdown}
        theme={workbench.theme?.active}
        cwd={workbench.workingDirectory?.current}
        onOpenFileLink={(target) => {
          void dispatchCommand("file.open", { path: target.path, line: target.line, column: target.column });
        }}
        onOpenExternalLink={(href) => {
          void window.theEditor.openExternal(href);
        }}
      />
    </div>
  );
}

function HoverOverlay({
  markdown,
  source,
  surfaceId,
  workbench,
}: {
  markdown: string;
  source?: string;
  surfaceId?: number;
  workbench: WorkbenchSnapshot;
}) {
  const overlayRef = useRef<HTMLDivElement>(null);
  const anchor = useSurfaceCursorAnchor(surfaceId ?? activeSurfaceIdFromWorkbench(workbench));
  const [position, setPosition] = useState<{ left: number; top: number } | undefined>(undefined);

  useLayoutEffect(() => {
    const element = overlayRef.current;
    if (!element) return;
    const margin = 12;
    const rect = element.getBoundingClientRect();
    const nextPosition = !anchor
      ? {
        left: clamp((window.innerWidth - rect.width) / 2, margin, Math.max(margin, window.innerWidth - rect.width - margin)),
        top: clamp(window.innerHeight * 0.28, margin, Math.max(margin, window.innerHeight - rect.height - margin)),
      }
      : (() => {
        const preferBelow = anchor.bottom + 10 + rect.height <= window.innerHeight - margin;
        const top = preferBelow
          ? anchor.bottom + 10
          : Math.max(margin, anchor.top - rect.height - 10);
        const left = clamp(anchor.left - 28, margin, Math.max(margin, window.innerWidth - rect.width - margin));
        return { left, top };
      })();
    setPosition((current) => current && current.left === nextPosition.left && current.top === nextPosition.top ? current : nextPosition);
  }, [anchor, markdown]);

  return <HoverCard markdown={markdown} source={source} workbench={workbench} cardRef={overlayRef} style={position} role="tooltip" />;
}

function SignatureHelpOverlay({
  state,
  workbench,
}: {
  state: SignatureHelpRoleState;
  workbench: WorkbenchSnapshot;
}) {
  const overlayRef = useRef<HTMLDivElement>(null);
  const anchor = useSurfaceCursorAnchor(state.surfaceId ?? activeSurfaceIdFromWorkbench(workbench));
  const [position, setPosition] = useState<{ left: number; top: number } | undefined>(undefined);
  const activeSignature = Math.max(0, Math.min(state.activeSignature ?? 0, Math.max(0, (state.items?.length ?? 1) - 1)));
  const item = state.items?.[activeSignature];

  useLayoutEffect(() => {
    const element = overlayRef.current;
    if (!element) return;
    const margin = 12;
    const rect = element.getBoundingClientRect();
    const nextPosition = !anchor
      ? {
        left: clamp((window.innerWidth - rect.width) / 2, margin, Math.max(margin, window.innerWidth - rect.width - margin)),
        top: clamp(window.innerHeight * 0.32, margin, Math.max(margin, window.innerHeight - rect.height - margin)),
      }
      : (() => {
        const left = clamp(anchor.left - 18, margin, Math.max(margin, window.innerWidth - rect.width - margin));
        const reserved = document.querySelector(".completionMenuOverlay")?.getBoundingClientRect();
        const below = anchor.bottom + 10;
        const above = anchor.top - rect.height - 10;
        const candidates = [
          below + rect.height <= window.innerHeight - margin ? below : undefined,
          above >= margin ? above : undefined,
          clamp(below, margin, Math.max(margin, window.innerHeight - rect.height - margin)),
          clamp(above, margin, Math.max(margin, window.innerHeight - rect.height - margin)),
        ].filter((value): value is number => value !== undefined);
        const top = candidates.find((candidate) => !reserved || !rectsOverlap({ left, top: candidate, right: left + rect.width, bottom: candidate + rect.height }, reserved)) ?? candidates[0] ?? margin;
        return { left, top };
      })();
    setPosition((current) => current && current.left === nextPosition.left && current.top === nextPosition.top ? current : nextPosition);
  }, [anchor, activeSignature, item?.documentation, item?.label]);

  if (!item) return null;

  return (
    <div ref={overlayRef} className="defaultHoverOverlay signatureHelpOverlay" style={position} role="tooltip">
      {state.items && state.items.length > 1 ? <div className="signatureHelpCounter">({activeSignature + 1}/{state.items.length})</div> : null}
      <div className="signatureHelpSignature">
        <code className="signatureHelpCode"><SignatureHelpLabel label={item.label} activeRange={item.activeParameterRange} /></code>
      </div>
      {item.documentation?.trim() ? (
        <>
          <div className="signatureHelpDivider" />
          <MarkdownView
            className="editorMarkdown signatureHelpMarkdown"
            markdown={item.documentation}
            theme={workbench.theme?.active}
            cwd={workbench.workingDirectory?.current}
            onOpenFileLink={(target) => {
              void dispatchCommand("file.open", { path: target.path, line: target.line, column: target.column });
            }}
            onOpenExternalLink={(href) => {
              void window.theEditor.openExternal(href);
            }}
          />
        </>
      ) : null}
    </div>
  );
}

function SignatureHelpLabel({
  label,
  activeRange,
}: {
  label: string;
  activeRange?: { start: number; end: number };
}) {
  if (!activeRange || activeRange.start < 0 || activeRange.end <= activeRange.start || activeRange.end > label.length) {
    return <>{label}</>;
  }
  return (
    <>
      {label.slice(0, activeRange.start)}
      <span className="signatureHelpActiveParameter">{label.slice(activeRange.start, activeRange.end)}</span>
      {label.slice(activeRange.end)}
    </>
  );
}

function CompletionOverlay({
  sessionId,
  surfaceId,
  sessions,
  workbench,
  dispatch,
}: {
  sessionId: string;
  surfaceId?: number;
  sessions: UiSessionClient;
  workbench: WorkbenchSnapshot;
  dispatch(dispatch: { id: string; args?: unknown }): Promise<unknown>;
}) {
  const state = useUiSession<CompletionSessionState>(sessions, sessionId);
  const overlayRef = useRef<HTMLDivElement>(null);
  const itemsRef = useRef<HTMLDivElement>(null);
  const anchor = useSurfaceCursorAnchor(surfaceId ?? activeSurfaceIdFromWorkbench(workbench));
  const [position, setPosition] = useState<{ left: number; top: number; direction: "right" } | undefined>(undefined);
  const selectedItem = state?.items[state.selectedIndex];

  useLayoutEffect(() => {
    if (!state) return;
    scrollActiveItemIntoView(itemsRef.current, '.completionMenuItem[data-active="true"]');
  }, [state?.items, state?.selectedIndex]);

  useLayoutEffect(() => {
    const element = overlayRef.current;
    if (!element) return;
    const margin = 12;
    const rect = element.getBoundingClientRect();
    const surfaceRect = element.querySelector(".completionMenuSurface")?.getBoundingClientRect() ?? rect;
    const listWidth = Math.min(surfaceRect.width, 320);
    const nextPosition = !anchor
      ? {
        left: clamp((window.innerWidth - rect.width) / 2, margin, Math.max(margin, window.innerWidth - rect.width - margin)),
        top: clamp(window.innerHeight * 0.34, margin, Math.max(margin, window.innerHeight - rect.height - margin)),
        direction: "right" as const,
      }
      : (() => {
        const reserved = document.querySelector(".signatureHelpOverlay")?.getBoundingClientRect();
        const menuHeight = surfaceRect.height;
        const below = anchor.bottom + 8;
        const above = anchor.top - menuHeight - 8;
        const direction = "right" as const;
        const totalWidth = listWidth;
        const left = direction === "left"
          ? clamp(anchor.left - totalWidth + listWidth, margin, Math.max(margin, window.innerWidth - totalWidth - margin))
          : clamp(anchor.left - 18, margin, Math.max(margin, window.innerWidth - totalWidth - margin));
        const candidates = [
          below + menuHeight <= window.innerHeight - margin ? below : undefined,
          above >= margin ? above : undefined,
          clamp(below, margin, Math.max(margin, window.innerHeight - menuHeight - margin)),
          clamp(above, margin, Math.max(margin, window.innerHeight - menuHeight - margin)),
        ].filter((value): value is number => value !== undefined);
        const top = candidates.find((candidate) => !reserved || !rectsOverlap({ left, top: candidate, right: left + listWidth, bottom: candidate + menuHeight }, reserved)) ?? candidates[0] ?? margin;
        return { left, top, direction };
      })();
    setPosition((current) => current
      && current.left === nextPosition.left
      && current.top === nextPosition.top
      && current.direction === nextPosition.direction
      ? current
      : nextPosition);
  }, [anchor, state?.items, state?.selectedIndex]);

  if (!state) return null;

  return (
    <div
      ref={overlayRef}
      className="completionMenuOverlay"
      data-direction={position?.direction ?? "right"}
      style={position ? { left: position.left, top: position.top } : undefined}
      role="listbox"
      aria-label="Completions"
      onMouseDown={(event) => event.preventDefault()}
    >
      <div className="completionMenuSurface">
        <div ref={itemsRef} className="completionMenuItems">
          {state.items.length ? state.items.map((item, index) => {
            const isActive = index === state.selectedIndex;
            return (
              <button
                key={item.id}
                type="button"
                className="completionMenuItem"
                data-active={isActive ? "true" : undefined}
                onMouseMove={() => {
                  if (isActive) return;
                  void dispatch({ id: "completion.selectIndex", args: { index } });
                }}
                onClick={() => {
                  void dispatch({ id: "completion.selectIndex", args: { index } }).then(() => dispatch({ id: "completion.accept" }));
                }}
              >
                <div className="completionMenuItemMain">
                  <div className="completionMenuItemLabelRow">
                    {item.kind ? <CompletionKindIcon kind={item.kind} /> : item.badge ? <span className="completionMenuItemBadge">{item.badge}</span> : null}
                    <span className="completionMenuItemLabel" data-deprecated={item.deprecated ? "true" : undefined}>{renderHighlightedText(item.label, item.labelHighlights)}</span>
                    {item.labelDetail ? <span className="completionMenuItemLabelDetail">{item.labelDetail}</span> : null}
                  </div>
                  {item.labelDescription ? <div className="completionMenuItemDescription">{item.labelDescription}</div> : null}
                </div>
                <div className="completionMenuItemMeta">
                  {item.detail ? <div className="completionMenuItemDetail">{renderHighlightedText(item.detail, item.detailHighlights)}</div> : null}
                  {item.source ? <div className="completionMenuItemSource">{item.source}</div> : null}
                </div>
              </button>
            );
          }) : <div className="completionMenuEmpty">{state.emptyText ?? "No completions"}</div>}
        </div>
        {state.loading ? <div className="completionMenuStatus">loading…</div> : null}
      </div>
    </div>
  );
}

function CompletionKindIcon({ kind }: { kind: string }) {
  const glyph = completionKindGlyph(kind);
  return (
    <span className="completionMenuItemIcon" data-group={completionKindGroup(kind)} aria-hidden="true">
      {glyph}
    </span>
  );
}

function completionKindGlyph(kind: string): string {
  switch (kind) {
    case "function":
    case "method":
    case "constructor":
      return "ƒ";
    case "class":
    case "interface":
    case "struct":
    case "typeParameter":
      return "◇";
    case "enum":
    case "enumMember":
      return "≡";
    case "variable":
    case "field":
    case "property":
    case "value":
    case "constant":
      return "●";
    case "module":
      return "▣";
    case "file":
      return "▤";
    case "folder":
      return "▢";
    case "keyword":
      return "⌘";
    case "snippet":
      return "⋯";
    case "color":
      return "◼";
    case "operator":
      return "±";
    case "text":
      return "¶";
    default:
      return "•";
  }
}

function completionKindGroup(kind: string): string {
  switch (kind) {
    case "function":
    case "method":
    case "constructor":
    case "operator":
      return "callable";
    case "class":
    case "interface":
    case "struct":
    case "enum":
    case "enumMember":
    case "typeParameter":
      return "type";
    case "variable":
    case "field":
    case "property":
    case "value":
    case "constant":
      return "value";
    case "module":
    case "file":
    case "folder":
      return "namespace";
    case "keyword":
    case "snippet":
    case "text":
    case "color":
      return "utility";
    default:
      return "default";
  }
}

function renderHighlightedText(value: string, highlights: readonly number[]): ReactNode {
  if (!highlights.length) return value;
  const highlightSet = new Set(highlights);
  const nodes: ReactNode[] = [];
  let start = 0;
  let active = highlightSet.has(0);
  for (let index = 1; index <= value.length; index += 1) {
    const nextActive = highlightSet.has(index);
    if (index < value.length && nextActive === active) continue;
    const chunk = value.slice(start, index);
    nodes.push(active ? <mark key={`${start}:${index}`}>{chunk}</mark> : <Fragment key={`${start}:${index}`}>{chunk}</Fragment>);
    start = index;
    active = nextActive;
  }
  return nodes;
}

function useSurfaceCursorAnchor(surfaceId: number | undefined): SurfaceCursorAnchor | undefined {
  return useSyncExternalStore(
    (listener) => surfaceCursorAnchors.subscribe(listener),
    () => surfaceId ? surfaceCursorAnchors.anchorForSurface(surfaceId as SurfaceId) : undefined,
    () => surfaceId ? surfaceCursorAnchors.anchorForSurface(surfaceId as SurfaceId) : undefined,
  );
}

function activeSurfaceIdFromWorkbench(workbench: WorkbenchSnapshot): number | undefined {
  const activePaneId = workbench.editorLayout?.activePaneId;
  const pane = activePaneId ? workbench.editorLayout?.panes.find((entry) => entry.id === activePaneId) : undefined;
  return pane?.kind === "surface" ? pane.surfaceId : undefined;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

function rectsOverlap(
  left: { left: number; top: number; right: number; bottom: number },
  right: { left: number; top: number; right: number; bottom: number },
  gap = 8,
): boolean {
  return left.left < right.right + gap
    && left.right + gap > right.left
    && left.top < right.bottom + gap
    && left.bottom + gap > right.top;
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

  useEffect(() => {
    const input = inputRef.current;
    if (!input || !state) return;
    input.focus();
  }, [sessionId, state]);

  if (!state) return null;

  const canGoBack = Boolean(state.stack?.length);

  return (
    <CommandDialog
      open
      onOpenChange={(open) => {
        if (!open) void sessions.dispatch(sessionId, "close");
      }}
    >
      <Command shouldFilter={false}>
        <CommandInput
          ref={inputRef}
          placeholder={state.placeholder || "Type a command or search..."}
          canGoBack={canGoBack}
          onBack={() => {
            void sessions.dispatch(sessionId, "back");
          }}
          spellCheck={false}
          value={state.query}
          onKeyDown={(event) => {
            if (event.key === "Backspace" && canGoBack && !state.query) {
              event.preventDefault();
              void sessions.dispatch(sessionId, "back");
            }
          }}
          onValueChange={(query) => {
            void sessions.dispatch(sessionId, "setQuery", { query });
          }}
        />
        <CommandList>
          <CommandEmpty>No matching commands</CommandEmpty>
          {groupCommandPaletteItems(state.items).map((group, groupIndex) => (
            <Fragment key={group.heading}>
              {groupIndex > 0 ? <CommandSeparator /> : null}
              <CommandGroup heading={group.heading}>
                {group.items.map((item) => (
                  <CommandItem
                    key={item.id}
                    value={item.id}
                    onSelect={() => {
                      void sessions.dispatch(sessionId, "acceptItem", { id: item.id });
                    }}
                  >
                    <CommandPaletteIconView icon={item.icon} fallback={commandPaletteIconLabel(item)} />
                    <span className="min-w-0 flex-1 truncate">{item.title}</span>
                    {item.alias ? <CommandShortcut>{item.alias}</CommandShortcut> : null}
                    {item.keybinding ? <CommandShortcut>{item.keybinding}</CommandShortcut> : null}
                  </CommandItem>
                ))}
              </CommandGroup>
            </Fragment>
          ))}
        </CommandList>
      </Command>
    </CommandDialog>
  );
}

function groupCommandPaletteItems(items: CommandPaletteItemView[]): Array<{ heading: string; items: CommandPaletteItemView[] }> {
  const groups: Array<{ heading: string; items: CommandPaletteItemView[] }> = [];
  const byHeading = new Map<string, CommandPaletteItemView[]>();
  for (const item of items) {
    const heading = item.group || "Commands";
    const group = byHeading.get(heading) ?? [];
    if (!byHeading.has(heading)) {
      byHeading.set(heading, group);
      groups.push({ heading, items: group });
    }
    group.push(item);
  }
  return groups;
}

function commandPaletteIconLabel(item: CommandPaletteItemView): string {
  const source = item.alias || item.title || item.id;
  return source.trim().slice(0, 1).toUpperCase() || "•";
}

const commandPaletteLucideIcons: Record<string, LucideIcon> = {
  "columns-2": Columns2Icon,
  file: FileIcon,
  folder: FolderIcon,
  "folder-open": FolderOpenIcon,
  "refresh-cw": RefreshCwIcon,
  "rows-2": Rows2Icon,
  save: SaveIcon,
  terminal: TerminalIcon,
};

function CommandPaletteIconView({ icon, fallback }: { icon?: CommandPaletteIcon; fallback: string }) {
  const className = "size-4 shrink-0 text-muted-foreground";
  if (!icon) return <span className="flex size-4 shrink-0 items-center justify-center text-[10px] font-medium text-muted-foreground">{fallback}</span>;

  if (icon.kind === "lucide") {
    const Icon = commandPaletteLucideIcons[icon.name];
    if (Icon) return <Icon className={className} />;
    return <span className="flex size-4 shrink-0 items-center justify-center text-[10px] font-medium text-muted-foreground">{fallback}</span>;
  }

  if (icon.kind === "file") {
    return icon.entryKind === "directory" ? <FolderIcon className={className} /> : <FileIcon className={className} />;
  }

  if (icon.kind === "text") {
    return <span className="flex size-4 shrink-0 items-center justify-center text-[10px] font-medium text-muted-foreground">{icon.value || fallback}</span>;
  }

  if (icon.kind === "uri") {
    return <img src={icon.src} alt="" aria-hidden="true" className="size-4 shrink-0" />;
  }

  return (
    <span
      aria-hidden="true"
      className="flex size-4 shrink-0 items-center justify-center [&_svg]:size-4"
      dangerouslySetInnerHTML={{ __html: icon.source }}
    />
  );
}

function scrollActiveItemIntoView(container: HTMLDivElement | null, selector: string): void {
  const active = container?.querySelector<HTMLElement>(selector);
  active?.scrollIntoView({ block: "nearest", inline: "nearest" });
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

  useEffect(() => {
    const input = inputRef.current;
    if (!input || !state) return;
    input.focus();
    const end = input.value.length;
    input.setSelectionRange(end, end);
  }, [sessionId]);

  useLayoutEffect(() => {
    if (!state) return;
    scrollActiveItemIntoView(itemsRef.current, '.pickerItem[data-active="true"]');
  }, [state?.items, state?.selectedIndex]);

  if (!state) return null;

  return (
    <section className="pickerOverlay">
      <div
        className="pickerBackdrop"
        onMouseDown={(event) => {
          event.preventDefault();
        }}
        onClick={(event) => {
          event.preventDefault();
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
                      }}
                      onClick={() => {
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
                        {state.preview.rows.map((row, rowIndex) => {
                          const active = row.lineNumber != null && row.lineNumber === state.preview?.activeLine;
                          const annotation = row.lineNumber != null && row.lineNumber === state.preview?.annotation?.line ? state.preview.annotation : undefined;
                          return (
                            <div key={`${row.lineNumber ?? rowIndex}:${rowIndex}`} className="pickerPreviewCodeRow" data-active={active ? "true" : undefined}>
                              <div className="pickerPreviewLineNumber">{row.lineNumber ?? ""}</div>
                              <div className="pickerPreviewLineText">
                                {renderPreviewLine(row.text, previewSyntax.get(rowIndex) ?? [], previewTheme, syntaxLegend)}
                                {annotation ? <div className="pickerPreviewAnnotation" data-severity={annotation.severity ?? "error"}>{annotation.message}</div> : null}
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    ) : (
                      <pre className="pickerPreviewText">{state.preview.text || ""}</pre>
                    )}
                  </>
                ) : state.preview?.kind === "surface" ? (
                  <PickerSurfacePreview preview={state.preview as PickerPreviewViewState & { kind: "surface" }} sessions={sessions} sessionId={sessionId} theme={workbench.theme} />
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

function PickerSurfacePreview({
  preview,
  sessions,
  sessionId,
  theme,
}: {
  preview: PickerPreviewViewState & { kind: "surface" };
  sessions: UiSessionClient;
  sessionId: string;
  theme: WorkbenchSnapshot["theme"];
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const selectionRef = useRef<HTMLPreElement>(null);
  const controllerRef = useRef<SurfaceViewportController | null>(null);
  if (!controllerRef.current) {
    controllerRef.current = new SurfaceViewportController({
      styleOptions: PICKER_PREVIEW_SURFACE_STYLE,
      paintOptions: { showCursors: false },
      overscanRows: 12,
    });
  }
  const controller = controllerRef.current;
  const controllerState = useSyncExternalStore(
    (listener) => controller.subscribe(listener),
    () => controller.snapshot(),
    () => controller.snapshot(),
  );
  const selectablePreviewText = useMemo(
    () => controllerState.frame?.rows.map((row) => row.text).join("\n") ?? "",
    [controllerState.frame],
  );
  const rowOffsets = useMemo(() => {
    const rows = controllerState.frame?.rows ?? [];
    const offsets: number[] = [];
    let offset = 0;
    for (let index = 0; index < rows.length; index += 1) {
      offsets.push(offset);
      offset += rows[index]!.text.length;
      if (index < rows.length - 1) offset += 1;
    }
    return offsets;
  }, [controllerState.frame]);

  useEffect(() => {
    const canvas = canvasRef.current;
    const scrollElement = scrollRef.current;
    if (!canvas || !scrollElement) return;
    controller.attach(scrollElement, canvas);
    return () => {
      controller.detach();
    };
  }, [controller]);

  useEffect(() => {
    controller.setTheme(theme);
  }, [controller, theme]);

  useEffect(() => {
    if (!preview.path) {
      controller.setSource(null);
      return;
    }
    controller.setSource({
      key: `${sessionId}:${preview.path}:${preview.viewport?.firstRow ?? 0}:${(preview as { activeLine?: number }).activeLine ?? ""}`,
      readFrame: async ({ viewport, metrics }) => {
        const requestViewport = preview.viewport ? { ...viewport, firstRow: preview.viewport.firstRow } : viewport;
        const result = await sessions.dispatch(sessionId, "readPreviewFrame", {
          path: preview.path,
          viewport: requestViewport,
          metrics,
        });
        if (!isSurfaceFrameResult(result)) throw new Error("invalid surface preview frame result");
        return result;
      },
    });
  }, [controller, preview, sessionId, sessions]);

  useEffect(() => {
    const element = selectionRef.current;
    const frame = controllerState.frame;
    const selectionBackground = resolveThemeStyleLoose(theme.active ?? { styles: {} }, "ui.selection")?.bg ?? "#264f78";
    const activeLineBackground = resolveThemeStyleLoose(theme.active ?? { styles: {} }, "ui.cursorline.primary", ["ui.cursorline"])?.bg ?? "rgba(120,120,120,0.18)";
    const activeLine = (preview as { activeLine?: number }).activeLine;
    if (!element || !frame) {
      controller.setDecorations([]);
      return;
    }

    const syncSelection = () => {
      const selection = document.getSelection();
      const offsets = readElementSelectionOffsets(element, selection);
      controller.setDecorations([
        ...buildPreviewActiveLineDecorations(frame.rows, activeLine, activeLineBackground),
        ...buildPreviewSelectionDecorations(frame.rows, rowOffsets, offsets, selectionBackground),
      ]);
    };

    syncSelection();
    document.addEventListener("selectionchange", syncSelection);
    return () => {
      document.removeEventListener("selectionchange", syncSelection);
      controller.setDecorations([]);
    };
  }, [controller, controllerState.frame, preview, rowOffsets, theme.active]);

  return (
    <>
      <div className="pickerPreviewHeader">
        <div className="pickerPreviewTitle">{preview.title || "Preview"}</div>
        {preview.detail ? <div className="pickerPreviewDetail">{preview.detail}</div> : null}
        <div className="pickerPreviewMeta">{surfacePreviewSummary(controllerState)}</div>
      </div>
      <div className="pickerPreviewViewport">
        <div ref={scrollRef} className="pickerPreviewSurface">
          <div className="pickerPreviewScrollSize" style={{ height: `${controllerState.scrollHeight}px` }} />
        </div>
        <div className="pickerPreviewCanvasViewport">
          <canvas ref={canvasRef} className="pickerPreviewCanvas" />
          {controllerState.status === "loading" && !controllerState.frame ? (
            <div className="pickerPreviewLoading">Loading preview…</div>
          ) : null}
          {selectablePreviewText ? (
            <pre
              ref={selectionRef}
              className="pickerPreviewSelectableText"
              style={surfacePreviewSelectableStyle(controllerState)}
              onWheel={(event) => {
                const scrollElement = scrollRef.current;
                if (!scrollElement) return;
                event.preventDefault();
                scrollElement.scrollTop += event.deltaY;
                scrollElement.scrollLeft += event.deltaX;
              }}
            >
              {selectablePreviewText}
            </pre>
          ) : null}
        </div>
        <PreviewScrollbar state={controllerState} />
      </div>
    </>
  );
}

function readElementSelectionOffsets(element: HTMLElement, selection: Selection | null): { start: number; end: number } | undefined {
  if (!selection || selection.rangeCount === 0 || selection.isCollapsed) return undefined;
  const range = selection.getRangeAt(0);
  if (!element.contains(range.startContainer) || !element.contains(range.endContainer)) return undefined;
  const startRange = range.cloneRange();
  startRange.selectNodeContents(element);
  startRange.setEnd(range.startContainer, range.startOffset);
  const endRange = range.cloneRange();
  endRange.selectNodeContents(element);
  endRange.setEnd(range.endContainer, range.endOffset);
  const start = startRange.toString().length;
  const end = endRange.toString().length;
  return end > start ? { start, end } : undefined;
}

function buildPreviewActiveLineDecorations(
  rows: readonly RenderFrame["rows"],
  activeLine: number | undefined,
  background: string,
): BufferDecoration[] {
  if (activeLine == null) return [];
  const row = rows.find((candidate) => candidate.docLine != null && candidate.docLine + 1 === activeLine);
  if (!row) return [];
  return [{
    kind: "range",
    id: `preview-active-line:${activeLine}`,
    surfaceRow: row.surfaceRow,
    startCol: 0,
    endCol: Math.max(1, row.text.length),
    background,
  }];
}

function buildPreviewSelectionDecorations(
  rows: readonly RenderFrame["rows"],
  rowOffsets: readonly number[],
  offsets: { start: number; end: number } | undefined,
  background: string,
): BufferDecoration[] {
  if (!offsets) return [];
  const decorations: BufferDecoration[] = [];
  for (let index = 0; index < rows.length; index += 1) {
    const row = rows[index]!;
    const rowStart = rowOffsets[index] ?? 0;
    const rowEnd = rowStart + row.text.length;
    const startCol = Math.max(0, offsets.start - rowStart);
    const endCol = Math.min(row.text.length, offsets.end - rowStart);
    if (offsets.start > rowEnd || offsets.end <= rowStart || endCol <= startCol) continue;
    decorations.push({
      kind: "range",
      id: `preview-selection:${index}:${startCol}:${endCol}`,
      surfaceRow: row.surfaceRow,
      startCol,
      endCol,
      background,
    });
  }
  return decorations;
}

function PreviewScrollbar({ state }: { state: SurfaceViewportControllerState }) {
  const metrics = state.metrics;
  const viewport = state.viewport;
  if (!metrics || !viewport) return null;
  const visibleHeight = viewport.rowCount * metrics.rowHeightPx + PICKER_PREVIEW_SURFACE_STYLE.paddingPx! * 2;
  if (!(state.scrollHeight > visibleHeight + 1)) return null;
  const maxScrollTop = Math.max(1, state.scrollHeight - visibleHeight);
  const thumbHeight = Math.max(28, (visibleHeight / state.scrollHeight) * visibleHeight);
  const thumbTop = (state.scrollTop / maxScrollTop) * Math.max(0, visibleHeight - thumbHeight);
  return (
    <div className="pickerPreviewScrollbar" aria-hidden="true">
      <div
        className="pickerPreviewScrollbarThumb"
        style={{ height: `${thumbHeight}px`, transform: `translateY(${thumbTop}px)` }}
      />
    </div>
  );
}

function surfacePreviewSelectableStyle(state: SurfaceViewportControllerState): CSSProperties | undefined {
  const metrics = state.metrics;
  const frameViewport = state.frameViewport;
  if (!metrics || !frameViewport) return undefined;
  const offsetXPx = Math.max(0, state.scrollLeft - frameViewport.firstCol * metrics.cellWidthPx);
  const offsetYPx = Math.max(0, state.scrollTop - frameViewport.firstRow * metrics.rowHeightPx);
  return {
    transform: `translate(${-offsetXPx}px, ${-offsetYPx}px)`,
  };
}

function surfacePreviewSummary(state: SurfaceViewportControllerState): string {
  if (state.viewport && state.lineCount) {
    const startLine = Math.min(state.lineCount, state.viewport.firstRow + 1);
    const endLine = Math.min(state.lineCount, state.viewport.firstRow + state.viewport.rowCount);
    return `Lines ${startLine}–${endLine} of ${state.lineCount}`;
  }
  if (state.lineCount) {
    return state.lineCount === 1 ? "1 line" : `${state.lineCount} lines`;
  }
  return state.status === "loading" ? "Loading preview…" : "Preview";
}

function isSurfaceFrameResult(value: unknown): value is {
  frame: RenderFrame;
  viewport: Viewport;
  lineCount: number;
  syntaxLegend?: Record<number, string>;
} {
  if (!value || typeof value !== "object") return false;
  const frame = (value as { frame?: unknown }).frame;
  const viewport = (value as { viewport?: unknown }).viewport;
  const lineCount = (value as { lineCount?: unknown }).lineCount;
  return Boolean(frame)
    && Boolean(viewport)
    && typeof lineCount === "number";
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
  } catch {
    const safeText = typeof text === "string" ? text : String(text ?? "");
    return <>{safeText || " "}</>;
  }
}

function previewSyntaxColor(theme: ResolvedThemeDefinition | undefined, scope: unknown): string | undefined {
  if (!theme) return undefined;
  if (typeof scope !== "string") {
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
      {state?.lspStatus ? <div className="statuslineSegment meta">{state.lspStatus}</div> : null}
      {state?.diagnosticsSummary ? <div className="statuslineSegment diagnostics">{state.diagnosticsSummary}</div> : null}
    </div>
  );
}

function WorkbenchInspector({
  diagnostics,
  healthDiagnostics,
  dispatch,
  workbench,
}: {
  diagnostics: DiagnosticRecord[];
  healthDiagnostics: HealthDiagnosticEntry[];
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
          {diagnostics.length ? diagnostics.slice(0, 24).map((diagnostic) => (
            <div key={diagnostic.id} className={`diagnosticRow ${diagnostic.severity}`}>
              <div className="diagnosticMessage">{diagnosticPrimaryLine(diagnostic)}</div>
              <div className="diagnosticDetails">{diagnosticLocationLabel(diagnostic)}</div>
            </div>
          )) : <div className="inspectorSubtle">No problems.</div>}
        </section>

        <section className="inspectorSection diagnostics">
          <h3>Health</h3>
          {healthDiagnostics.length ? healthDiagnostics.map((diagnostic) => (
            <div key={diagnostic.id} className={`diagnosticRow ${diagnostic.severity}`}>
              <div className="diagnosticMessage">{diagnostic.message}</div>
              {diagnostic.details ? <div className="diagnosticDetails">{diagnostic.details}</div> : null}
            </div>
          )) : <div className="inspectorSubtle">No health diagnostics.</div>}
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
    <svg className="h-4 w-4 shrink-0 text-muted-foreground" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M10.5 4a6.5 6.5 0 1 0 4.03 11.6l4.43 4.43 1.06-1.06-4.43-4.43A6.5 6.5 0 0 0 10.5 4Zm0 1.5a5 5 0 1 1 0 10 5 5 0 0 1 0-10Z" fill="currentColor" />
    </svg>
  );
}
