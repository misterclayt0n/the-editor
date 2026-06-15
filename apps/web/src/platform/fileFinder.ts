/**
 * FileFinderPort: renderer seam for fff's native index. The desktop host owns
 * @ff-labs/fff-node (Node/native only); React consumes a small async search
 * surface, capability-gated like terminal and syntax.
 */

export type FileFinderLocation =
  | { readonly type: "line"; readonly line: number }
  | { readonly type: "position"; readonly line: number; readonly col: number }
  | {
      readonly type: "range";
      readonly start: { readonly line: number; readonly col: number };
      readonly end: { readonly line: number; readonly col: number };
    };

export type FileFinderItem = {
  readonly relativePath: string;
  readonly fileName: string;
  readonly size: number;
  readonly gitStatus: string;
};

export type FileFinderSearchResult = {
  readonly items: readonly FileFinderItem[];
  readonly totalMatched: number;
  readonly totalFiles: number;
  readonly location?: FileFinderLocation;
};

export interface FileFinderPort {
  readonly search: (input: {
    readonly root: string;
    readonly query: string;
    readonly limit: number;
    readonly currentFile?: string;
  }) => Promise<FileFinderSearchResult>;
  readonly track: (input: {
    readonly root: string;
    readonly query: string;
    readonly relativePath: string;
  }) => Promise<void>;
}

type BridgeResult<T> = { readonly ok: true; readonly value: T } | { readonly ok: false; readonly error: string };

type Bridge = {
  readonly search: (input: {
    readonly root: string;
    readonly query: string;
    readonly limit: number;
    readonly currentFile?: string;
  }) => Promise<BridgeResult<FileFinderSearchResult>>;
  readonly track: (input: {
    readonly root: string;
    readonly query: string;
    readonly relativePath: string;
  }) => Promise<BridgeResult<boolean>>;
};

export function createFileFinderPort(): FileFinderPort | undefined {
  const w = window as unknown as {
    __testFileFinderPort?: FileFinderPort;
    theEditorFileFinder?: Bridge;
  };
  const isDev = location.hostname === "localhost" || location.hostname === "127.0.0.1";
  if (isDev && w.__testFileFinderPort) return w.__testFileFinderPort;
  const bridge = w.theEditorFileFinder;
  if (bridge === undefined) return undefined;
  return {
    search: async (input) => {
      const result = await bridge.search(input);
      if (!result.ok) throw new Error(result.error);
      return result.value;
    },
    track: async (input) => {
      const result = await bridge.track(input);
      if (!result.ok) throw new Error(result.error);
    },
  };
}
