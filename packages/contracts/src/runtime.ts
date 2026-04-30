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

export interface DocumentOpened {
  documentId: DocumentId;
}

export interface SurfaceCreated {
  surfaceId: SurfaceId;
}

export interface OkResult {
  ok: true;
}

export type Direction = "forward" | "backward";
export type MotionMode = "move" | "extend";
export type WordKind = "word" | "longWord" | "subWord";
export type WordTarget = "start" | "end";

export type SelectionTransform =
  | { kind: "horizontal"; direction: Direction; count?: number; mode?: MotionMode }
  | { kind: "vertical"; direction: Direction; count?: number; mode?: MotionMode }
  | { kind: "visualVertical"; direction: Direction; count?: number; mode?: MotionMode }
  | {
      kind: "word";
      direction: Direction;
      wordKind: WordKind;
      wordTarget: WordTarget;
      count?: number;
      mode?: MotionMode;
    }
  | { kind: "paragraph"; direction: Direction; count?: number; mode?: MotionMode }
  | { kind: "lineStart"; mode?: MotionMode }
  | { kind: "lineEnd"; mode?: MotionMode }
  | { kind: "firstNonWhitespace"; mode?: MotionMode }
  | { kind: "documentStart"; mode?: MotionMode }
  | { kind: "documentEnd"; mode?: MotionMode };

export type SurfaceOperation =
  | { kind: "transformSelection"; transform: SelectionTransform }
  | { kind: "insertText"; text: string }
  | { kind: "insertNewline" }
  | { kind: "deleteBackward" }
  | { kind: "deleteForward" };

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
    applyOperation(params: { surfaceId: SurfaceId; operation: SurfaceOperation }): Promise<OkResult>;
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
