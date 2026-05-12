import { contributionKind } from "./contributions.js";
import type { EditorHost } from "./editorHost.js";
import { toDisposable, type Disposable } from "./disposable.js";

export type DiagnosticSeverity = "error" | "warning" | "info" | "hint";
export type DiagnosticTag = "unnecessary" | "deprecated";

export interface DiagnosticPosition {
  line: number;
  character: number;
}

export interface DiagnosticRange {
  start: DiagnosticPosition;
  end: DiagnosticPosition;
}

export interface DiagnosticRelatedInformation {
  uri: string;
  path?: string;
  range: DiagnosticRange;
  message: string;
}

export interface DiagnosticInput {
  id?: string;
  severity?: DiagnosticSeverity;
  range: DiagnosticRange;
  message: string;
  source?: string;
  code?: string | number;
  codeHref?: string;
  tags?: readonly DiagnosticTag[];
  relatedInformation?: readonly DiagnosticRelatedInformation[];
  data?: unknown;
}

export interface DiagnosticRecord {
  id: string;
  collectionId: string;
  collectionTitle?: string;
  uri: string;
  path?: string;
  severity: DiagnosticSeverity;
  range: DiagnosticRange;
  message: string;
  source?: string;
  code?: string | number;
  codeHref?: string;
  tags: DiagnosticTag[];
  relatedInformation: DiagnosticRelatedInformation[];
  data?: unknown;
}

export interface DiagnosticSummary {
  errorCount: number;
  warningCount: number;
  infoCount: number;
  hintCount: number;
  totalCount: number;
  fileCount: number;
}

export interface DiagnosticChangeEvent {
  version: number;
  collectionIds: string[];
  uris: string[];
  paths: string[];
}

export interface DiagnosticCollection {
  readonly id: string;
  readonly title?: string;
  set(uri: string, diagnostics: readonly DiagnosticInput[]): void;
  replace(entries: Iterable<{ uri: string; diagnostics: readonly DiagnosticInput[] }>): void;
  delete(uri: string): void;
  clear(): void;
  dispose(): void;
}

export interface DiagnosticCollectionDefinition {
  id: string;
  title?: string;
}

interface DiagnosticCollectionState {
  id: string;
  title?: string;
  order: number;
  byUri: Map<string, DiagnosticRecord[]>;
}

const diagnosticSeverityRank: Record<DiagnosticSeverity, number> = {
  error: 0,
  warning: 1,
  info: 2,
  hint: 3,
};

export class DiagnosticStore {
  private nextOrder = 0;
  private versionValue = 0;
  private readonly collections = new Map<string, DiagnosticCollectionState>();
  private readonly listeners = new Set<(event: DiagnosticChangeEvent) => void>();
  private allCacheVersion = -1;
  private allCache: DiagnosticRecord[] = [];

  constructor(private readonly editor: EditorHost) {}

  createCollection(definition: DiagnosticCollectionDefinition, moduleId = this.editor.currentModuleId()): DiagnosticCollection {
    const state: DiagnosticCollectionState = {
      id: definition.id,
      title: definition.title,
      order: this.nextOrder++,
      byUri: new Map(),
    };
    this.collections.set(definition.id, state);

    const unregister = toDisposable(() => {
      const current = this.collections.get(definition.id);
      if (current !== state) return;
      const uris = [...current.byUri.keys()];
      this.collections.delete(definition.id);
      this.bump({ collectionIds: [definition.id], uris });
    });

    const contribution = this.editor.contributions.add({
      moduleId,
      kind: contributionKind("diagnosticCollection"),
      localId: definition.id,
      label: definition.title,
      disposable: unregister,
    });

    const store = this;
    return {
      id: definition.id,
      title: definition.title,
      set(uri, diagnostics) {
        store.setCollectionUri(definition.id, uri, diagnostics);
      },
      replace(entries) {
        store.replaceCollection(definition.id, entries);
      },
      delete(uri) {
        store.deleteCollectionUri(definition.id, uri);
      },
      clear() {
        store.clearCollection(definition.id);
      },
      dispose() {
        contribution.dispose();
      },
    };
  }

  version(): number {
    return this.versionValue;
  }

  all(): DiagnosticRecord[] {
    if (this.allCacheVersion === this.versionValue) return this.allCache;
    const records: DiagnosticRecord[] = [];
    const collections = [...this.collections.values()].sort((a, b) => a.order - b.order || a.id.localeCompare(b.id));
    for (const collection of collections) {
      for (const entries of collection.byUri.values()) records.push(...entries);
    }
    records.sort(compareDiagnosticsByLocation);
    this.allCache = records;
    this.allCacheVersion = this.versionValue;
    return records;
  }

  forUri(uri: string): DiagnosticRecord[] {
    return this.all().filter((diagnostic) => diagnostic.uri === uri);
  }

  forPath(path: string): DiagnosticRecord[] {
    return this.all().filter((diagnostic) => diagnostic.path === path);
  }

  forLine(params: { uri?: string; path?: string; line: number }): DiagnosticRecord[] {
    return this.selectResource(params).filter((diagnostic) => diagnostic.range.start.line <= params.line && diagnostic.range.end.line >= params.line)
      .sort(compareDiagnosticsBySeverityAndLocation);
  }

  atPosition(params: { uri?: string; path?: string; line: number; character: number }): DiagnosticRecord[] {
    return this.selectResource(params).filter((diagnostic) => diagnosticContainsPosition(diagnostic, params.line, params.character))
      .sort(compareDiagnosticsBySeverityAndLocation);
  }

  summary(): DiagnosticSummary {
    return summarizeDiagnostics(this.all());
  }

  summaryForUri(uri: string): DiagnosticSummary {
    return summarizeDiagnostics(this.forUri(uri));
  }

  summaryForPath(path: string): DiagnosticSummary {
    return summarizeDiagnostics(this.forPath(path));
  }

  subscribe(listener: (event: DiagnosticChangeEvent) => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  private selectResource(params: { uri?: string; path?: string }): DiagnosticRecord[] {
    if (params.uri) return this.forUri(params.uri);
    if (params.path) return this.forPath(params.path);
    return [];
  }

  private setCollectionUri(collectionId: string, uri: string, diagnostics: readonly DiagnosticInput[]): void {
    const collection = this.collections.get(collectionId);
    if (!collection) return;
    const previous = collection.byUri.get(uri) ?? [];
    const next = diagnostics.map((diagnostic, index) => normalizeDiagnostic(collection, uri, diagnostic, index));
    if (diagnosticListsEqual(previous, next)) return;
    collection.byUri.set(uri, next);
    this.bump({ collectionIds: [collectionId], uris: [uri] });
  }

  private replaceCollection(collectionId: string, entries: Iterable<{ uri: string; diagnostics: readonly DiagnosticInput[] }>): void {
    const collection = this.collections.get(collectionId);
    if (!collection) return;
    const next = new Map<string, DiagnosticRecord[]>();
    const changedUris = new Set<string>();

    for (const { uri, diagnostics } of entries) {
      const normalized = diagnostics.map((diagnostic, index) => normalizeDiagnostic(collection, uri, diagnostic, index));
      next.set(uri, normalized);
      const previous = collection.byUri.get(uri) ?? [];
      if (!diagnosticListsEqual(previous, normalized)) changedUris.add(uri);
    }

    for (const uri of collection.byUri.keys()) {
      if (!next.has(uri)) changedUris.add(uri);
    }

    if (!changedUris.size) return;
    collection.byUri = next;
    this.bump({ collectionIds: [collectionId], uris: [...changedUris] });
  }

  private deleteCollectionUri(collectionId: string, uri: string): void {
    const collection = this.collections.get(collectionId);
    if (!collection?.byUri.has(uri)) return;
    collection.byUri.delete(uri);
    this.bump({ collectionIds: [collectionId], uris: [uri] });
  }

  private clearCollection(collectionId: string): void {
    const collection = this.collections.get(collectionId);
    if (!collection?.byUri.size) return;
    const uris = [...collection.byUri.keys()];
    collection.byUri.clear();
    this.bump({ collectionIds: [collectionId], uris });
  }

  private bump(change: { collectionIds: string[]; uris: string[] }): void {
    this.versionValue += 1;
    this.allCacheVersion = -1;
    const paths = [...new Set(change.uris.map(pathForFileUri).filter((path): path is string => Boolean(path)))];
    const event: DiagnosticChangeEvent = {
      version: this.versionValue,
      collectionIds: [...new Set(change.collectionIds)],
      uris: [...new Set(change.uris)],
      paths,
    };
    for (const listener of this.listeners) listener(event);
  }
}

export function compareDiagnosticsByLocation(left: DiagnosticRecord, right: DiagnosticRecord): number {
  return compareResourcePath(left.path, right.path)
    || left.range.start.line - right.range.start.line
    || left.range.start.character - right.range.start.character
    || left.range.end.line - right.range.end.line
    || left.range.end.character - right.range.end.character
    || diagnosticSeverityRank[left.severity] - diagnosticSeverityRank[right.severity]
    || left.message.localeCompare(right.message)
    || left.id.localeCompare(right.id);
}

export function compareDiagnosticsBySeverityAndLocation(left: DiagnosticRecord, right: DiagnosticRecord): number {
  return diagnosticSeverityRank[left.severity] - diagnosticSeverityRank[right.severity]
    || compareDiagnosticsByLocation(left, right);
}

export function diagnosticContainsPosition(diagnostic: Pick<DiagnosticRecord, "range">, line: number, character: number): boolean {
  const { start, end } = diagnostic.range;
  if (line < start.line || line > end.line) return false;
  if (line === start.line && character < start.character) return false;
  if (line === end.line && character > end.character) return false;
  return true;
}

export function pathForFileUri(uri: string): string | undefined {
  if (!uri.startsWith("file://")) return undefined;
  try {
    return decodeURIComponent(new URL(uri).pathname);
  } catch {
    return undefined;
  }
}

function normalizeDiagnostic(
  collection: DiagnosticCollectionState,
  uri: string,
  diagnostic: DiagnosticInput,
  index: number,
): DiagnosticRecord {
  return {
    id: diagnostic.id ?? stableDiagnosticId(collection.id, uri, diagnostic, index),
    collectionId: collection.id,
    collectionTitle: collection.title,
    uri,
    path: pathForFileUri(uri),
    severity: diagnostic.severity ?? "warning",
    range: normalizeRange(diagnostic.range),
    message: diagnostic.message,
    source: diagnostic.source,
    code: diagnostic.code,
    codeHref: diagnostic.codeHref,
    tags: [...new Set(diagnostic.tags ?? [])],
    relatedInformation: [...(diagnostic.relatedInformation ?? [])].map((info) => ({
      ...info,
      path: info.path ?? pathForFileUri(info.uri),
      range: normalizeRange(info.range),
    })),
    data: diagnostic.data,
  };
}

function normalizeRange(range: DiagnosticRange): DiagnosticRange {
  const start = {
    line: Math.max(0, Math.floor(range.start.line)),
    character: Math.max(0, Math.floor(range.start.character)),
  };
  const end = {
    line: Math.max(start.line, Math.floor(range.end.line)),
    character: Math.max(0, Math.floor(range.end.character)),
  };
  if (end.line === start.line && end.character < start.character) end.character = start.character;
  return { start, end };
}

function stableDiagnosticId(collectionId: string, uri: string, diagnostic: DiagnosticInput, index: number): string {
  const code = diagnostic.code == null ? "" : String(diagnostic.code);
  return [
    collectionId,
    uri,
    diagnostic.severity ?? "warning",
    diagnostic.range.start.line,
    diagnostic.range.start.character,
    diagnostic.range.end.line,
    diagnostic.range.end.character,
    code,
    diagnostic.source ?? "",
    diagnostic.message,
    index,
  ].join("\0");
}

function diagnosticListsEqual(left: readonly DiagnosticRecord[], right: readonly DiagnosticRecord[]): boolean {
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index += 1) {
    if (!diagnosticEquals(left[index]!, right[index]!)) return false;
  }
  return true;
}

function diagnosticEquals(left: DiagnosticRecord, right: DiagnosticRecord): boolean {
  return left.id === right.id
    && left.collectionId === right.collectionId
    && left.collectionTitle === right.collectionTitle
    && left.uri === right.uri
    && left.path === right.path
    && left.severity === right.severity
    && left.range.start.line === right.range.start.line
    && left.range.start.character === right.range.start.character
    && left.range.end.line === right.range.end.line
    && left.range.end.character === right.range.end.character
    && left.message === right.message
    && left.source === right.source
    && left.code === right.code
    && left.codeHref === right.codeHref
    && jsonEquals(left.tags, right.tags)
    && jsonEquals(left.relatedInformation, right.relatedInformation)
    && jsonEquals(left.data, right.data);
}

function summarizeDiagnostics(diagnostics: readonly DiagnosticRecord[]): DiagnosticSummary {
  const paths = new Set<string>();
  let errorCount = 0;
  let warningCount = 0;
  let infoCount = 0;
  let hintCount = 0;
  for (const diagnostic of diagnostics) {
    if (diagnostic.path) paths.add(diagnostic.path);
    switch (diagnostic.severity) {
      case "error":
        errorCount += 1;
        break;
      case "warning":
        warningCount += 1;
        break;
      case "info":
        infoCount += 1;
        break;
      case "hint":
        hintCount += 1;
        break;
    }
  }
  return {
    errorCount,
    warningCount,
    infoCount,
    hintCount,
    totalCount: diagnostics.length,
    fileCount: paths.size,
  };
}

function compareResourcePath(left: string | undefined, right: string | undefined): number {
  if (left && right) return left.localeCompare(right);
  if (left) return -1;
  if (right) return 1;
  return 0;
}

function jsonEquals(left: unknown, right: unknown): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}
