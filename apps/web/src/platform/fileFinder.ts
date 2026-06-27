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

export type FileFinderGrepMode = "plain" | "regex" | "fuzzy";
export type FileFinderGrepCursor = { readonly __brand: "GrepCursor"; readonly _offset: number };

export type FileFinderGrepMatch = FileFinderItem & {
  readonly modified: number;
  readonly isBinary: boolean;
  readonly totalFrecencyScore: number;
  readonly accessFrecencyScore: number;
  readonly modificationFrecencyScore: number;
  readonly lineNumber: number;
  readonly col: number;
  readonly byteOffset: number;
  readonly lineContent: string;
  readonly matchRanges: readonly (readonly [number, number])[];
  readonly fuzzyScore?: number;
  readonly contextBefore?: readonly string[];
  readonly contextAfter?: readonly string[];
  readonly isDefinition?: boolean;
};

export type FileFinderGrepResult = {
  readonly items: readonly FileFinderGrepMatch[];
  readonly totalMatched: number;
  readonly totalFilesSearched: number;
  readonly totalFiles: number;
  readonly filteredFileCount: number;
  readonly nextCursor: FileFinderGrepCursor | null;
  readonly regexFallbackError?: string;
};

export type FileFinderGrepInput = {
  readonly root: string;
  readonly query: string;
  readonly limit: number;
  readonly mode?: FileFinderGrepMode;
  readonly cursor?: FileFinderGrepCursor | null;
  readonly maxFileSize?: number;
  readonly maxMatchesPerFile?: number;
  readonly smartCase?: boolean;
  readonly timeBudgetMs?: number;
  readonly beforeContext?: number;
  readonly afterContext?: number;
  readonly classifyDefinitions?: boolean;
};

export type FileFinderMultiGrepInput = Omit<FileFinderGrepInput, "query" | "mode"> & {
  readonly patterns: readonly string[];
  readonly constraints?: string;
};

export interface FileFinderPort {
  readonly search: (input: {
    readonly root: string;
    readonly query: string;
    readonly limit: number;
    readonly currentFile?: string;
  }) => Promise<FileFinderSearchResult>;
  readonly grep: (input: FileFinderGrepInput) => Promise<FileFinderGrepResult>;
  readonly multiGrep: (input: FileFinderMultiGrepInput) => Promise<FileFinderGrepResult>;
  readonly track: (input: {
    readonly root: string;
    readonly query: string;
    readonly relativePath: string;
  }) => Promise<void>;
}

type BridgeResult<T> =
  | { readonly ok: true; readonly value: T }
  | { readonly ok: false; readonly error: string };

type Bridge = {
  readonly search: (input: {
    readonly root: string;
    readonly query: string;
    readonly limit: number;
    readonly currentFile?: string;
  }) => Promise<BridgeResult<FileFinderSearchResult>>;
  readonly grep: (input: FileFinderGrepInput) => Promise<BridgeResult<FileFinderGrepResult>>;
  readonly multiGrep: (
    input: FileFinderMultiGrepInput,
  ) => Promise<BridgeResult<FileFinderGrepResult>>;
  readonly track: (input: {
    readonly root: string;
    readonly query: string;
    readonly relativePath: string;
  }) => Promise<BridgeResult<boolean>>;
};

/**
 * fff reports `matchRanges` as UTF-8 BYTE offsets within `lineContent` (it's
 * ripgrep-based; Neovim, its first consumer, is byte-oriented). JS string
 * slicing and editor caret positions are UTF-16 code units, so we normalize at
 * this boundary — every consumer (grep picker, multibuffer) then works in
 * UTF-16. ASCII lines are unchanged (1 byte == 1 code unit).
 */
export function byteToUtf16Offset(text: string, byteOffset: number): number {
  if (byteOffset <= 0) return 0;
  let bytes = 0;
  let i = 0;
  while (i < text.length) {
    if (bytes >= byteOffset) return i;
    const code = text.codePointAt(i)!;
    bytes += code <= 0x7f ? 1 : code <= 0x7ff ? 2 : code <= 0xffff ? 3 : 4;
    i += code > 0xffff ? 2 : 1;
  }
  return text.length;
}

const normalizeGrepResult = (result: FileFinderGrepResult): FileFinderGrepResult => ({
  ...result,
  items: result.items.map((item) =>
    item.matchRanges.length === 0
      ? item
      : {
          ...item,
          matchRanges: item.matchRanges.map(([from, to]): readonly [number, number] => [
            byteToUtf16Offset(item.lineContent, from),
            byteToUtf16Offset(item.lineContent, to),
          ]),
        },
  ),
});

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
    grep: async (input) => {
      const result = await bridge.grep(input);
      if (!result.ok) throw new Error(result.error);
      return normalizeGrepResult(result.value);
    },
    multiGrep: async (input) => {
      const result = await bridge.multiGrep(input);
      if (!result.ok) throw new Error(result.error);
      return normalizeGrepResult(result.value);
    },
    track: async (input) => {
      const result = await bridge.track(input);
      if (!result.ok) throw new Error(result.error);
    },
  };
}
