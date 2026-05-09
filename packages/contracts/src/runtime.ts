export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

export interface JsonRpcRequest<TParams = JsonValue> {
  id: number | string;
  method: string;
  params: TParams;
}

export interface JsonRpcSuccess<TResult = JsonValue> {
  id: number | string | null;
  result: TResult;
}

export interface JsonRpcFailure {
  id: number | string | null;
  error: RuntimeRpcError;
}

export type JsonRpcResponse<TResult = JsonValue> = JsonRpcSuccess<TResult> | JsonRpcFailure;

export interface RuntimeRpcError {
  code: string;
  message: string;
  data?: JsonValue;
}

export type DocumentId = number & { readonly __brand: "DocumentId" };
export type SurfaceId = number & { readonly __brand: "SurfaceId" };
export type ResourceId = number & { readonly __brand: "ResourceId" };

export interface Viewport {
  firstRow: number;
  rowCount: number;
  firstCol: number;
  wrapWidth: number;
}

export interface CellGridMetrics {
  cellWidthPx: number;
  rowHeightPx: number;
}

export interface DocumentOpened {
  documentId: DocumentId;
}

export interface SurfaceCreated {
  surfaceId: SurfaceId;
}

export interface OkResult {
  ok: true;
}


export interface SurfaceOperation {
  name: string;
  count?: number;
  text?: string;
  values?: string[];
  after?: boolean;
  lineEnding?: string;
  indent?: string;
  indentUnit?: string;
  indentWidth?: number;
  tabWidth?: number;
}

export interface SurfaceSelectionRange {
  cursorId: number;
  anchor: number;
  head: number;
}

export interface SurfaceChange {
  start: number;
  end: number;
  text?: string;
}

export interface SurfaceState {
  text: string;
  lineEnding: string;
  selection: SurfaceSelectionRange[];
  activeCursorId: number | null;
}

export interface SurfaceEdit {
  changes?: SurfaceChange[];
  selection?: SurfaceSelectionRange[];
}

export type ClipboardKind = "Clipboard" | "Selection";

export interface RenderRow {
  surfaceRow: number;
  text: string;
  docLine: number | null;
  startsLogicalLine: boolean;
  wrappedContinuation: boolean;
}

export interface SelectionSpan {
  cursorId: number;
  surfaceRow: number;
  startCol: number;
  endCol: number;
  active: boolean;
}

export type CursorSemantics = "InsertionPoint" | "Cell";

export interface CursorPlacement {
  cursorId: number;
  surfaceRow: number;
  col: number;
  semantics: CursorSemantics;
  active: boolean;
}

export interface SyntaxSpan {
  surfaceRow: number;
  startCol: number;
  endCol: number;
  kind: number;
}

export interface RenderFrame {
  surfaceId: SurfaceId;
  rows: RenderRow[];
  selections: SelectionSpan[];
  cursors: CursorPlacement[];
  syntax: SyntaxSpan[];
}

export interface RuntimeApi {
  server: {
    ping(): Promise<OkResult>;
  };
  syntax: {
    highlightLegend(): Promise<Record<number, string>>;
  };
  documents: {
    openFile(params: { path: string }): Promise<DocumentOpened>;
    createScratch(params: { text?: string }): Promise<DocumentOpened>;
  };
  surfaces: {
    create(params: { documentId: DocumentId; viewport?: Viewport }): Promise<SurfaceCreated>;
    setViewport(params: { surfaceId: SurfaceId; viewport: Viewport }): Promise<OkResult>;
    setMetrics(params: { surfaceId: SurfaceId; metrics: CellGridMetrics }): Promise<OkResult>;
    setSoftWrap(params: { surfaceId: SurfaceId; softWrap: boolean }): Promise<OkResult>;
    applyOperation(params: { surfaceId: SurfaceId; operation: SurfaceOperation }): Promise<OkResult>;
    readState(params: { surfaceId: SurfaceId }): Promise<SurfaceState>;
    setActiveCursor(params: { surfaceId: SurfaceId; cursorId: number | null }): Promise<OkResult>;
    applyEdit(params: { surfaceId: SurfaceId; edit: SurfaceEdit }): Promise<OkResult>;
    commitUndoCheckpoint(params: { surfaceId: SurfaceId }): Promise<OkResult>;
    undo(params: { surfaceId: SurfaceId; count?: number }): Promise<OkResult>;
    redo(params: { surfaceId: SurfaceId; count?: number }): Promise<OkResult>;
    earlier(params: { surfaceId: SurfaceId; count?: number }): Promise<OkResult>;
    later(params: { surfaceId: SurfaceId; count?: number }): Promise<OkResult>;
    renderFrame(params: {
      surfaceId: SurfaceId;
      viewport?: Viewport;
    }): Promise<RenderFrame>;
  };
  registers: {
    read(params: { name: string }): Promise<{ values: string[] | null }>;
    write(params: { name: string; values: string[] }): Promise<OkResult>;
  };
  clipboard: {
    read(params: { kind: ClipboardKind }): Promise<{ value: string | null }>;
    write(params: { kind: ClipboardKind; value: string }): Promise<OkResult>;
  };
  resources: {
    save(params: { surfaceId: SurfaceId }): Promise<OkResult>;
  };
}
