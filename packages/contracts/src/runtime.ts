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
export type WorkspaceRootId = number & { readonly __brand: "WorkspaceRootId" };
export type WorkspaceEntryId = number & { readonly __brand: "WorkspaceEntryId" };

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

export interface ResourceDocumentStatus {
  documentId: DocumentId;
  resourceId?: ResourceId;
  path?: string;
  dirty: boolean;
  conflict: boolean;
  deletedOnDisk: boolean;
  externalChangePending: boolean;
  readonly: boolean;
  diskState: "new" | "present" | "deleted" | "unknown";
}

export type WorkspaceEntryKind = "file" | "directory" | "symlink";
export type WorkspaceEntryChangeKind = "added" | "removed" | "modified";

export interface WorkspaceEntry {
  id: WorkspaceEntryId;
  rootId: WorkspaceRootId;
  path: string;
  kind: WorkspaceEntryKind;
  mtime?: number;
}

export interface WorkspaceEntryChange {
  kind: WorkspaceEntryChangeKind;
  entry: WorkspaceEntry;
}

export interface WorkspaceSnapshot {
  rootId: WorkspaceRootId;
  rootPath: string;
  revision: number;
  entries: WorkspaceEntry[];
}

export interface WorkspaceEntriesChanged {
  rootId: WorkspaceRootId;
  rootPath: string;
  revision: number;
  changes: WorkspaceEntryChange[];
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

export interface GutterRow {
  surfaceRow: number;
  docLine: number | null;
  lineNumber: number | null;
  active: boolean;
  selected: boolean;
  startsLogicalLine: boolean;
  wrappedContinuation: boolean;
}

export interface RenderFrame {
  surfaceId: SurfaceId;
  rows: RenderRow[];
  gutterRows: GutterRow[];
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

export interface VcsHunk {
  start: number;
  count: number;
  kind: "added" | "modified" | "deleted";
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

export interface RuntimeApi {
  server: {
    ping(): Promise<OkResult>;
  };
  syntax: {
    highlightLegend(): Promise<Record<number, string>>;
    drainWork(params?: { maxDocuments?: number; budgetMs?: number }): Promise<{ advanced: number }>;
  };
  documents: {
    openFile(params: { path: string }): Promise<DocumentOpened>;
    createScratch(params: { text?: string }): Promise<DocumentOpened>;
  };
  workspace: {
    addRoot(params: { path: string }): Promise<WorkspaceSnapshot>;
    snapshot(params: { rootId: WorkspaceRootId }): Promise<WorkspaceSnapshot>;
    refreshRoot(params: { rootId: WorkspaceRootId }): Promise<WorkspaceEntriesChanged>;
    refresh(params?: Record<string, never>): Promise<WorkspaceEntriesChanged[]>;
  };
  surfaces: {
    create(params: { documentId: DocumentId; viewport?: Viewport }): Promise<SurfaceCreated>;
    destroy(params: { surfaceId: SurfaceId }): Promise<OkResult>;
    loadFile(params: { surfaceId: SurfaceId; path: string }): Promise<OkResult>;
    lineCount(params: { surfaceId: SurfaceId }): Promise<{ lineCount: number }>;
    readViewport(params: { surfaceId: SurfaceId }): Promise<{ viewport: Viewport }>;
    setViewport(params: { surfaceId: SurfaceId; viewport: Viewport }): Promise<OkResult>;
    setMetrics(params: { surfaceId: SurfaceId; metrics: CellGridMetrics }): Promise<OkResult>;
    setSoftWrap(params: { surfaceId: SurfaceId; softWrap: boolean }): Promise<OkResult>;
    setCursorSemantics(params: { surfaceId: SurfaceId; semantics: CursorSemantics }): Promise<OkResult>;
    applyOperation(params: { surfaceId: SurfaceId; operation: SurfaceOperation }): Promise<OkResult>;
    readState(params: { surfaceId: SurfaceId }): Promise<SurfaceState>;
    readText(params: { surfaceId: SurfaceId }): Promise<{ text: string }>;
    readRevision(params: { surfaceId: SurfaceId }): Promise<{ revision: number }>;
    readLineEnding(params: { surfaceId: SurfaceId }): Promise<{ lineEnding: string }>;
    readSelection(params: { surfaceId: SurfaceId }): Promise<{ selection: SurfaceSelectionRange[]; activeCursorId: number | null }>;
    readCursorSemantics(params: { surfaceId: SurfaceId }): Promise<{ cursorSemantics: CursorSemantics }>;
    completeWords(params: { surfaceId: SurfaceId; cursorOffset: number; query: string; limit?: number }): Promise<Array<{ label: string }>>;
    setActiveCursor(params: { surfaceId: SurfaceId; cursorId: number | null }): Promise<OkResult>;
    applyEdit(params: { surfaceId: SurfaceId; edit: SurfaceEdit }): Promise<OkResult>;
    commitUndoCheckpoint(params: { surfaceId: SurfaceId }): Promise<OkResult>;
    undo(params: { surfaceId: SurfaceId; count?: number }): Promise<OkResult>;
    redo(params: { surfaceId: SurfaceId; count?: number }): Promise<OkResult>;
    earlier(params: { surfaceId: SurfaceId; count?: number }): Promise<OkResult>;
    later(params: { surfaceId: SurfaceId; count?: number }): Promise<OkResult>;
    renderFrame(params: {
      surfaceId: SurfaceId;
      viewport: Viewport;
      metrics: CellGridMetrics;
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
  vcs: {
    queryHunks(params: { surfaceId: SurfaceId; rootPath: string; relativePath: string }): Promise<{ hunks: VcsHunk[] }>;
  };
  resources: {
    status(params: { surfaceId: SurfaceId }): Promise<ResourceDocumentStatus>;
    save(params: { surfaceId: SurfaceId; conflict?: "fail" | "overwrite" }): Promise<OkResult>;
    reload(params: { surfaceId: SurfaceId }): Promise<OkResult>;
  };
}
