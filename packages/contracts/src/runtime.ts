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

export interface CursorRangeSpec {
  id?: number;
  anchor: number;
  head: number;
}

export interface SelectionSpec {
  ranges: CursorRangeSpec[];
}

export interface ReplacementSpec {
  start: number;
  end: number;
  text?: string;
}

export interface TransactionSpec {
  replacements: ReplacementSpec[];
}

export interface SurfaceSnapshot {
  text: string;
  selection: SelectionSpec;
  activeCursorId?: number;
}

export interface RowPlacement {
  surfaceRow: number;
  yPx: number;
  heightPx: number;
}

export interface RenderRow {
  id: number;
  generation: number;
  placement: RowPlacement;
  text: string;
}

export interface RowMove {
  id: number;
  from: RowPlacement | null;
  to: RowPlacement;
}

export type RowKind = "Document" | { Widget: { widgetId: number; fragment: number } };

export interface RowMetadata {
  rowId: number;
  surfaceRow: number;
  kind: RowKind;
  docLine: number | null;
  startsLogicalLine: boolean;
  wrappedContinuation: boolean;
}

export interface RowMetadataDelta {
  rows: RowMetadata[];
}

export interface Damage {
  full: boolean;
  startRow: number;
  endRow: number;
}

export interface SelectionSpan {
  cursorId: number;
  surfaceRow: number;
  startCol: number;
  endCol: number;
  xPx: number;
  widthPx: number;
}

export type CursorSemantics = "InsertionPoint" | "Cell";

export interface CursorPlacement {
  cursorId: number;
  surfaceRow: number;
  col: number;
  xPx: number;
  yPx: number;
  widthPx: number;
  heightPx: number;
  semantics: CursorSemantics;
}

export interface SyntaxSpan {
  surfaceRow: number;
  startCol: number;
  endCol: number;
  xPx: number;
  widthPx: number;
  kind: number;
}

export interface WidgetPlacement {
  widgetId: number;
  surfaceRow: number;
  rowSpan: number;
  xPx: number;
  yPx: number;
  widthPx: number;
  heightPx: number;
}

export interface LayerDeltaSet {
  gutters: Record<string, never> | null;
  selections: { spans: SelectionSpan[] } | null;
  cursors: { cursors: CursorPlacement[] } | null;
  syntax: { spans: SyntaxSpan[] } | null;
  overlays: Record<string, never> | null;
  widgets: { widgets: WidgetPlacement[] } | null;
}

export interface RenderFrameDelta {
  surfaceId: SurfaceId;
  generation: number;
  full: boolean;
  retainedRows: RowMove[];
  changedRows: RenderRow[];
  removedRows: number[];
  rowMetadata: RowMetadataDelta | null;
  layers: LayerDeltaSet;
  damage: Damage;
}

export interface RuntimeApi {
  server: {
    ping(): Promise<OkResult>;
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
    snapshot(params: { surfaceId: SurfaceId }): Promise<SurfaceSnapshot>;
    indentForNewline(params: { surfaceId: SurfaceId; pos: number }): Promise<{ indent: string }>;
    setSelection(params: { surfaceId: SurfaceId; selection: SelectionSpec }): Promise<OkResult>;
    applyTransaction(params: {
      surfaceId: SurfaceId;
      transaction: TransactionSpec;
      selection: SelectionSpec;
    }): Promise<OkResult>;
    renderFrame(params: {
      surfaceId: SurfaceId;
      viewport?: Viewport;
      sinceGeneration?: number | null;
    }): Promise<RenderFrameDelta>;
  };
  resources: {
    save(params: { surfaceId: SurfaceId }): Promise<OkResult>;
  };
}
