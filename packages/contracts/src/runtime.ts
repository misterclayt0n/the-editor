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
export type SearchIndexId = number & { readonly __brand: "SearchIndexId" };
export type LspServerId = number & { readonly __brand: "LspServerId" };

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
  revision: number;
  lineEnding: string;
  lineCount: number;
  selection: SurfaceSelectionRange[];
  activeCursorId: number | null;
  cursorSemantics: CursorSemantics;
  viewport: Viewport;
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

export interface SearchIndexReady {
  indexId: SearchIndexId;
  rootPath: string;
}

export interface SearchIndexStatus {
  indexId: SearchIndexId;
  rootPath: string;
  scanning: boolean;
  watcherReady: boolean;
  warmupComplete: boolean;
  indexedFiles: number;
}

export interface FileSearchItem {
  kind: "file";
  path: string;
  relativePath: string;
  fileName: string;
  parentPath: string;
  score: number;
  matchType: string;
  exactMatch: boolean;
}

export interface DirSearchItem {
  kind: "dir";
  path: string;
  relativePath: string;
  name: string;
  score: number;
  matchType: string;
  exactMatch: boolean;
}

export type MixedSearchItem = FileSearchItem | DirSearchItem;

export interface GrepSearchSpan {
  start: number;
  end: number;
}

export interface GrepSearchItem {
  kind: "grep";
  path: string;
  relativePath: string;
  fileName: string;
  parentPath: string;
  line: number;
  column: number;
  text: string;
  score: number;
  spans: GrepSearchSpan[];
}

export interface FileSearchResult {
  indexId: SearchIndexId;
  rootPath: string;
  query: string;
  totalMatched: number;
  totalFiles: number;
  scanning: boolean;
  watcherReady: boolean;
  warmupComplete: boolean;
  items: FileSearchItem[];
}

export interface DirSearchResult {
  indexId: SearchIndexId;
  rootPath: string;
  query: string;
  totalMatched: number;
  totalDirs: number;
  scanning: boolean;
  watcherReady: boolean;
  warmupComplete: boolean;
  items: DirSearchItem[];
}

export interface MixedSearchResult {
  indexId: SearchIndexId;
  rootPath: string;
  query: string;
  totalMatched: number;
  totalFiles: number;
  totalDirs: number;
  scanning: boolean;
  watcherReady: boolean;
  warmupComplete: boolean;
  items: MixedSearchItem[];
}

export interface GrepSearchResult {
  indexId: SearchIndexId;
  rootPath: string;
  query: string;
  totalFiles: number;
  totalFilesSearched: number;
  filesWithMatches: number;
  nextOffset: number;
  regexFallbackError?: string;
  scanning: boolean;
  watcherReady: boolean;
  warmupComplete: boolean;
  items: GrepSearchItem[];
}

export interface TextPreviewRow {
  text: string;
  lineNumber: number | null;
}

export interface TextPreviewSyntaxSpan {
  row: number;
  startCol: number;
  endCol: number;
  kind: number;
  scope?: string;
}

export interface TextPreviewResult {
  path: string;
  text: string;
  startLine: number;
  endLine: number;
  truncated: boolean;
  binary: boolean;
  rows?: TextPreviewRow[];
  syntax?: TextPreviewSyntaxSpan[];
}

export interface LspServerConfig {
  id: string;
  command: string;
  args?: string[];
  env?: Record<string, string>;
  languages?: string[];
  fileTypes?: string[];
  initializationOptions?: JsonValue;
}

export interface LspServerStatus {
  handle: LspServerId;
  id: string;
  state: string;
  message?: string | null;
  completionProvider?: boolean;
  completionTriggerCharacters?: string[];
}

export interface LspLogEntry {
  server: LspServerId;
  stream: string;
  message: string;
}

export interface LspDiagnosticEntry {
  server: LspServerId;
  uri: string;
  diagnostics: JsonValue[];
  version?: number | null;
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
    destroy(params: { surfaceId: SurfaceId }): Promise<OkResult>;
    loadFile(params: { surfaceId: SurfaceId; path: string }): Promise<OkResult>;
    lineCount(params: { surfaceId: SurfaceId }): Promise<{ lineCount: number }>;
    setViewport(params: { surfaceId: SurfaceId; viewport: Viewport }): Promise<OkResult>;
    setMetrics(params: { surfaceId: SurfaceId; metrics: CellGridMetrics }): Promise<OkResult>;
    setSoftWrap(params: { surfaceId: SurfaceId; softWrap: boolean }): Promise<OkResult>;
    setCursorSemantics(params: { surfaceId: SurfaceId; semantics: CursorSemantics }): Promise<OkResult>;
    applyOperation(params: { surfaceId: SurfaceId; operation: SurfaceOperation }): Promise<SurfaceState>;
    readState(params: { surfaceId: SurfaceId }): Promise<SurfaceState>;
    setActiveCursor(params: { surfaceId: SurfaceId; cursorId: number | null }): Promise<SurfaceState>;
    applyEdit(params: { surfaceId: SurfaceId; edit: SurfaceEdit }): Promise<SurfaceState>;
    commitUndoCheckpoint(params: { surfaceId: SurfaceId }): Promise<OkResult>;
    undo(params: { surfaceId: SurfaceId; count?: number }): Promise<SurfaceState>;
    redo(params: { surfaceId: SurfaceId; count?: number }): Promise<SurfaceState>;
    earlier(params: { surfaceId: SurfaceId; count?: number }): Promise<SurfaceState>;
    later(params: { surfaceId: SurfaceId; count?: number }): Promise<SurfaceState>;
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
  search: {
    ensureIndex(params: { rootPath: string }): Promise<SearchIndexReady>;
    status(params: { indexId: SearchIndexId }): Promise<SearchIndexStatus>;
    refresh(params: { indexId: SearchIndexId }): Promise<OkResult>;
    queryFiles(params: { indexId: SearchIndexId; query: string; offset?: number; limit?: number; currentPath?: string }): Promise<FileSearchResult>;
    queryDirs(params: { indexId: SearchIndexId; query: string; offset?: number; limit?: number; currentPath?: string }): Promise<DirSearchResult>;
    queryMixed(params: { indexId: SearchIndexId; query: string; offset?: number; limit?: number; currentPath?: string }): Promise<MixedSearchResult>;
    queryGrep(params: {
      indexId: SearchIndexId;
      query: string;
      offset?: number;
      limit?: number;
      currentPath?: string;
      beforeContext?: number;
      afterContext?: number;
    }): Promise<GrepSearchResult>;
    recordSelection(params: { indexId: SearchIndexId; query: string; path: string }): Promise<OkResult>;
    previewPathCacheKey(params: { path: string }): Promise<string>;
    readTextPreview(params: {
      path: string;
      maxBytes?: number;
      maxLines?: number;
      aroundLine?: number;
      contextLines?: number;
    }): Promise<TextPreviewResult>;
  };
  lsp: {
    registerServer(params: LspServerConfig): Promise<OkResult>;
    configuredServers(): Promise<LspServerConfig[]>;
    startServer(params: { id: string; rootUri?: string }): Promise<{ serverId: LspServerId }>;
    shutdownServer(params: { serverId: LspServerId }): Promise<OkResult>;
    status(): Promise<LspServerStatus[]>;
    diagnostics(): Promise<LspDiagnosticEntry[]>;
    logs(params: { serverId: LspServerId }): Promise<LspLogEntry[]>;
    request(params: { serverId: LspServerId; method: string; params?: JsonValue; timeoutMs?: number }): Promise<JsonValue>;
    notify(params: { serverId: LspServerId; method: string; params?: JsonValue }): Promise<OkResult>;
  };
  resources: {
    save(params: { surfaceId: SurfaceId }): Promise<OkResult>;
  };
}
