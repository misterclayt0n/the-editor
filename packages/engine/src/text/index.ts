export {
  TextBuffer,
  unsafeTextBufferDebugState,
  type TextBufferDebugState,
  type TextSnapshot,
  type LineRange,
  type VisibleLine,
  type LineChunk,
  type VisibleChunk,
  type LineColumn,
  type TextCursor,
} from "./buffer";

export type { TextStats, MutationStats } from "./stats";

export {
  Transaction,
  ChangeSet,
  ChangeDesc,
  Replacement,
  MappedPos,
  LengthMismatch,
  ComposeLengthMismatch,
  ComposeNotImplemented,
  InvalidSpan,
  SpanOutOfBounds,
  OverlappingReplacement,
  PositionsOutOfBounds,
  TrackingAssocInBatchUpdate,
  InvalidOperation,
  InvalidChangeSet,
  InvalidDocumentVersion,
  changeSetFromOperations,
  mapPosition,
  invert,
  compose,
  type TextError,
  type Span,
  type Replacement as ReplacementType,
  type ReplacementInput,
  type ReplacementOverlapPolicy,
  type DeletionOverlapPolicy,
  type Operation,
  type Assoc,
  type MappedDeletion,
  type MappedPos as MappedPosType,
  type ChangeRange,
  type MutablePosition,
  type RangeMapOptions,
} from "./transaction";

export {
  TextDocument,
  makeTextDocument,
  TextDocumentLive,
  type TextDocumentInput,
  type TextDocumentState,
  type TextDocumentSnapshot,
  type TextDocumentVersion,
} from "./effect";

export type TextSlice = string;
