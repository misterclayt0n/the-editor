import { Data, Effect } from "effect";
import type { TextBuffer } from "./buffer";

export type Span = { readonly start: number; readonly end: number };
export type Replacement = { readonly span: Span; readonly text: string | null };
export type LegacyReplacement = {
  readonly from: number;
  readonly to: number;
  readonly text: string;
};
export type ReplacementInput = Replacement | LegacyReplacement;
export type ReplacementOverlapPolicy = "reject" | "ignoreLater";
export type DeletionOverlapPolicy = "reject" | "ignoreLater" | "merge";
export type Operation =
  | { readonly kind: "retain"; readonly length: number }
  | { readonly kind: "delete"; readonly length: number }
  | { readonly kind: "insert"; readonly text: string };
export type Assoc =
  | "before"
  | "after"
  | "beforeSticky"
  | "afterSticky"
  | "beforeWord"
  | "afterWord"
  | "trackDel"
  | "trackDelSticky"
  | "trackDelBefore"
  | "trackDelAfter"
  | "trackDelBeforeSticky"
  | "trackDelAfterSticky";
export type MappedDeletion = "before" | "after" | "collapse";
export type MappedPos =
  | { readonly type: "pos"; readonly pos: number }
  | {
      readonly type: "deleted";
      readonly pos: number;
      readonly offset: number;
      readonly length: number;
    };
export type ChangeRange = {
  readonly fromA: number;
  readonly toA: number;
  readonly fromB: number;
  readonly toB: number;
  readonly insertedLeadingWordLength: number;
  readonly insertedTrailingWordLength: number;
};
export type MutablePosition = { value: number };
export type RangeMapOptions = { readonly startAssoc?: Assoc; readonly endAssoc?: Assoc };

export class LengthMismatch extends Data.TaggedError("LengthMismatch")<{
  readonly expected: number;
  readonly actual: number;
}> {}
export class ComposeLengthMismatch extends Data.TaggedError("ComposeLengthMismatch")<{
  readonly leftLengthAfter: number;
  readonly rightLength: number;
}> {}
export class InvalidSpan extends Data.TaggedError("InvalidSpan")<{
  readonly start: number;
  readonly end: number;
}> {}
export class SpanOutOfBounds extends Data.TaggedError("SpanOutOfBounds")<{
  readonly start: number;
  readonly end: number;
  readonly length: number;
}> {}
export class OverlappingReplacement extends Data.TaggedError("OverlappingReplacement")<{
  readonly previousEnd: number;
  readonly start: number;
  readonly end: number;
}> {}
export class PositionsOutOfBounds extends Data.TaggedError("PositionsOutOfBounds")<{
  readonly positions: readonly number[];
  readonly length: number;
}> {}
export class TrackingAssocInBatchUpdate extends Data.TaggedError(
  "TrackingAssocInBatchUpdate",
)<{}> {}
export class ComposeNotImplemented extends Data.TaggedError("ComposeNotImplemented")<{}> {}
export class InvalidOperation extends Data.TaggedError("InvalidOperation")<{
  readonly operation: Operation;
  readonly reason: string;
}> {}
export class InvalidChangeSet extends Data.TaggedError("InvalidChangeSet")<{
  readonly message: string;
}> {}
export class InvalidDocumentVersion extends Data.TaggedError("InvalidDocumentVersion")<{
  readonly version: number;
}> {}
export type TextError =
  | LengthMismatch
  | ComposeLengthMismatch
  | InvalidSpan
  | SpanOutOfBounds
  | OverlappingReplacement
  | PositionsOutOfBounds
  | TrackingAssocInBatchUpdate
  | ComposeNotImplemented
  | InvalidOperation
  | InvalidChangeSet
  | InvalidDocumentVersion;

const isLegacy = (r: ReplacementInput): r is LegacyReplacement => "from" in r;
const insertedText = (r: Replacement): string => r.text ?? "";
const isWordCodeUnit = (code: number): boolean =>
  (code >= 48 && code <= 57) ||
  (code >= 65 && code <= 90) ||
  (code >= 97 && code <= 122) ||
  code === 95;
const validateSpan = (span: Span, length?: number): TextError | undefined => {
  if (
    !Number.isInteger(span.start) ||
    !Number.isInteger(span.end) ||
    span.start < 0 ||
    span.end < span.start
  )
    return new InvalidSpan({ start: span.start, end: span.end });
  if (length !== undefined && span.end > length)
    return new SpanOutOfBounds({ start: span.start, end: span.end, length });
  return undefined;
};
const validateOperation = (op: Operation): TextError | undefined => {
  if ((op.kind === "retain" || op.kind === "delete") && !Number.isInteger(op.length)) {
    return new InvalidOperation({ operation: op, reason: "length must be an integer" });
  }
  if ((op.kind === "retain" || op.kind === "delete") && op.length < 0) {
    return new InvalidOperation({ operation: op, reason: "length must be non-negative" });
  }
  return undefined;
};

const normalizeOperations = (
  ops: Iterable<Operation>,
): Effect.Effect<readonly Operation[], TextError> => {
  const out: Operation[] = [];
  for (const op of ops) {
    const error = validateOperation(op);
    if (error) return Effect.fail(error);
    if (op.kind === "insert" && op.text.length === 0) continue;
    if ((op.kind === "retain" || op.kind === "delete") && op.length === 0) continue;

    const last = out[out.length - 1];
    if (last?.kind === op.kind) {
      if (op.kind === "insert" && last.kind === "insert") {
        out[out.length - 1] = { kind: "insert", text: last.text + op.text };
      } else if (op.kind === "retain" && last.kind === "retain") {
        out[out.length - 1] = { kind: "retain", length: last.length + op.length };
      } else if (op.kind === "delete" && last.kind === "delete") {
        out[out.length - 1] = { kind: "delete", length: last.length + op.length };
      }
    } else {
      out.push(op);
    }
  }
  return Effect.succeed(out);
};

export const Replacement = {
  insert: (at: number, text: string): Replacement => ({ span: { start: at, end: at }, text }),
  delete: (span: Span): Replacement => ({ span, text: null }),
  replace: (span: Span, text: string): Replacement => ({ span, text }),
  isInsert: (r: Replacement): boolean => r.span.start === r.span.end && r.text !== null,
  isDelete: (r: Replacement): boolean => r.span.start < r.span.end && r.text === null,
  isReplace: (r: Replacement): boolean => r.span.start < r.span.end && r.text !== null,
  normalize: (input: ReplacementInput): Replacement =>
    isLegacy(input)
      ? {
          span: { start: input.from, end: input.to },
          text: input.text === "" && input.from < input.to ? null : input.text,
        }
      : input,
};
export const isTrackingAssoc = (assoc: Assoc): boolean => assoc.startsWith("trackDel");
export const MappedPos = {
  pos: (pos: number): MappedPos => ({ type: "pos", pos }),
  deleted: (args: {
    readonly pos: number;
    readonly offset: number;
    readonly length: number;
  }): MappedPos => ({ type: "deleted", ...args }),
  position: (mapped: MappedPos): number => mapped.pos,
  isDeleted: (mapped: MappedPos): boolean => mapped.type === "deleted",
  ok: (mapped: MappedPos): number | undefined => (mapped.type === "pos" ? mapped.pos : undefined),
};

const insertedWordMetadata = (
  text: string,
): Pick<ChangeRange, "insertedLeadingWordLength" | "insertedTrailingWordLength"> => {
  let insertedLeadingWordLength = 0;
  while (
    insertedLeadingWordLength < text.length &&
    isWordCodeUnit(text.charCodeAt(insertedLeadingWordLength))
  ) {
    insertedLeadingWordLength++;
  }
  let insertedTrailingWordLength = 0;
  while (
    insertedTrailingWordLength < text.length &&
    isWordCodeUnit(text.charCodeAt(text.length - 1 - insertedTrailingWordLength))
  ) {
    insertedTrailingWordLength++;
  }
  return { insertedLeadingWordLength, insertedTrailingWordLength };
};

const makeRange = (
  fromA: number,
  toA: number,
  fromB: number,
  toB: number,
  inserted: string,
): ChangeRange => ({
  fromA,
  toA,
  fromB,
  toB,
  ...insertedWordMetadata(inserted),
});

const insertionOffset = (range: ChangeRange, assoc: Assoc): number => {
  const insertedLength = range.toB - range.fromB;
  switch (assoc) {
    case "after":
    case "afterSticky":
    case "trackDelAfterSticky":
      return insertedLength;
    case "before":
    case "beforeSticky":
    case "trackDelBeforeSticky":
      return 0;
    case "afterWord":
      return range.insertedLeadingWordLength;
    case "beforeWord":
      return insertedLength - range.insertedTrailingWordLength;
    default:
      // Rust reference maps non-sticky deletion-tracking modes before insertions.
      return 0;
  }
};
const sticky = (assoc: Assoc): boolean => assoc.endsWith("Sticky");
const staysAtGaps = (assoc: Assoc): boolean => assoc !== "beforeWord" && assoc !== "afterWord";
const tracksDeletionOffset = (offset: number, length: number, assoc: Assoc): boolean => {
  switch (assoc) {
    case "trackDel":
    case "trackDelSticky":
    case "trackDelBefore":
    case "trackDelBeforeSticky":
    case "trackDelAfterSticky":
      return offset > 0;
    case "trackDelAfter":
      return offset < length;
    default:
      return false;
  }
};

export class ChangeDesc {
  private constructor(
    readonly length: number,
    readonly lengthAfter: number,
    readonly ranges: readonly ChangeRange[],
  ) {}
  static empty(length: number): ChangeDesc {
    return new ChangeDesc(length, length, []);
  }
  static unsafeMake(
    length: number,
    lengthAfter: number,
    ranges: readonly ChangeRange[],
  ): ChangeDesc {
    return new ChangeDesc(length, lengthAfter, ranges);
  }
  mapPosition(pos: number, assoc: Assoc = "after"): number {
    return MappedPos.position(this.mapPositionTracked(pos, assoc));
  }
  mapPositionTracked(pos: number, assoc: Assoc = "after"): MappedPos {
    let diff = 0;

    for (const r of this.ranges) {
      const oldLen = r.toA - r.fromA;
      const newLen = r.toB - r.fromB;

      if (pos < r.fromA) break;

      if (oldLen === 0) {
        if (pos === r.fromA) {
          return MappedPos.pos(r.fromB + insertionOffset(r, assoc));
        }
        diff += newLen;
        continue;
      }

      if (pos < r.toA) {
        const offset = pos - r.fromA;
        if (isTrackingAssoc(assoc) && tracksDeletionOffset(offset, oldLen, assoc)) {
          return MappedPos.deleted({ pos: r.fromB, offset, length: oldLen });
        }
        if (newLen > 0 && offset === 0 && staysAtGaps(assoc)) return MappedPos.pos(r.fromB);
        if (newLen > 0 && sticky(assoc) && oldLen === newLen)
          return MappedPos.pos(r.fromB + offset);
        return MappedPos.pos(r.fromB + insertionOffset(r, assoc));
      }

      if (pos === r.toA && assoc === "trackDelBefore") {
        return MappedPos.deleted({ pos: r.toB, offset: oldLen, length: oldLen });
      }

      diff += newLen - oldLen;
    }

    return MappedPos.pos(pos + diff);
  }
  mapRange(span: Span, optionsOrAssoc?: RangeMapOptions | Assoc): Span {
    if (typeof optionsOrAssoc === "string") {
      return {
        start: this.mapPosition(span.start, optionsOrAssoc),
        end: this.mapPosition(span.end, optionsOrAssoc),
      };
    }
    return {
      start: this.mapPosition(span.start, optionsOrAssoc?.startAssoc ?? "before"),
      end: this.mapPosition(span.end, optionsOrAssoc?.endAssoc ?? "after"),
    };
  }
  mapPositionEffect(pos: number, assoc: Assoc = "after"): Effect.Effect<number, TextError> {
    return validatePos(pos, this.length).pipe(Effect.map(() => this.mapPosition(pos, assoc)));
  }
  mapPositionTrackedEffect(
    pos: number,
    assoc: Assoc = "after",
  ): Effect.Effect<MappedPos, TextError> {
    return validatePos(pos, this.length).pipe(
      Effect.map(() => this.mapPositionTracked(pos, assoc)),
    );
  }
  mapRangeEffect(
    span: Span,
    optionsOrAssoc?: RangeMapOptions | Assoc,
  ): Effect.Effect<Span, TextError> {
    const err = validateSpan(span, this.length);
    return err ? Effect.fail(err) : Effect.succeed(this.mapRange(span, optionsOrAssoc));
  }
  compose(other: ChangeDesc): Effect.Effect<ChangeDesc, TextError> {
    if (this.lengthAfter !== other.length) {
      return Effect.fail(
        new ComposeLengthMismatch({ leftLengthAfter: this.lengthAfter, rightLength: other.length }),
      );
    }
    return ChangeSet.fromOperations(
      composeOperations(
        operationsFromRanges(this.length, this.ranges),
        operationsFromRanges(other.length, other.ranges),
      ),
    ).pipe(Effect.map((changes) => changes.desc()));
  }
}
const validatePos = (pos: number, length: number): Effect.Effect<void, TextError> =>
  Number.isInteger(pos) && pos >= 0 && pos <= length
    ? Effect.void
    : Effect.fail(new PositionsOutOfBounds({ positions: [pos], length }));

const syntheticInsertedText = (range: ChangeRange): string => {
  const length = range.toB - range.fromB;
  const leading = Math.min(range.insertedLeadingWordLength, length);
  const trailing = Math.min(range.insertedTrailingWordLength, length - leading);
  return "a".repeat(leading) + "!".repeat(length - leading - trailing) + "a".repeat(trailing);
};

const operationsFromRanges = (
  length: number,
  ranges: readonly ChangeRange[],
): readonly Operation[] => {
  const ops: Operation[] = [];
  let pos = 0;
  for (const r of ranges) {
    if (r.fromA > pos) appendComposedOp(ops, { kind: "retain", length: r.fromA - pos });
    if (r.toA > r.fromA) appendComposedOp(ops, { kind: "delete", length: r.toA - r.fromA });
    const inserted = syntheticInsertedText(r);
    if (inserted.length > 0) appendComposedOp(ops, { kind: "insert", text: inserted });
    pos = r.toA;
  }
  if (pos < length) appendComposedOp(ops, { kind: "retain", length: length - pos });
  return ops;
};

const appendComposedOp = (out: Operation[], op: Operation): void => {
  if (op.kind === "insert" && op.text.length === 0) return;
  if ((op.kind === "retain" || op.kind === "delete") && op.length === 0) return;
  const last = out[out.length - 1];
  if (last?.kind === op.kind) {
    if (op.kind === "insert" && last.kind === "insert")
      out[out.length - 1] = { kind: "insert", text: last.text + op.text };
    else if (op.kind === "retain" && last.kind === "retain")
      out[out.length - 1] = { kind: "retain", length: last.length + op.length };
    else if (op.kind === "delete" && last.kind === "delete")
      out[out.length - 1] = { kind: "delete", length: last.length + op.length };
  } else out.push(op);
};

const composeOperations = (
  left: readonly Operation[],
  right: readonly Operation[],
): readonly Operation[] => {
  const out: Operation[] = [];
  let i = 0,
    j = 0;
  let a: Operation | undefined = left[i++];
  let b: Operation | undefined = right[j++];

  while (a || b) {
    // Deletions from the first transaction consume original document text and
    // never appear in the intermediate document. They always survive compose.
    if (a?.kind === "delete") {
      appendComposedOp(out, a);
      a = left[i++];
      continue;
    }

    // Insertions from the second transaction are in final coordinates only and
    // do not consume the intermediate output of the first transaction.
    if (b?.kind === "insert") {
      appendComposedOp(out, b);
      b = right[j++];
      continue;
    }

    if (!a && !b) break;
    if (!a || !b) throw new Error("malformed composed operation streams");

    if (a.kind === "retain" && b.kind === "retain") {
      const n = Math.min(a.length, b.length);
      appendComposedOp(out, { kind: "retain", length: n });
      a = a.length === n ? left[i++] : { kind: "retain", length: a.length - n };
      b = b.length === n ? right[j++] : { kind: "retain", length: b.length - n };
      continue;
    }

    if (a.kind === "retain" && b.kind === "delete") {
      const n = Math.min(a.length, b.length);
      appendComposedOp(out, { kind: "delete", length: n });
      a = a.length === n ? left[i++] : { kind: "retain", length: a.length - n };
      b = b.length === n ? right[j++] : { kind: "delete", length: b.length - n };
      continue;
    }

    if (a.kind === "insert" && b.kind === "retain") {
      const n = Math.min(a.text.length, b.length);
      appendComposedOp(out, { kind: "insert", text: a.text.slice(0, n) });
      a = a.text.length === n ? left[i++] : { kind: "insert", text: a.text.slice(n) };
      b = b.length === n ? right[j++] : { kind: "retain", length: b.length - n };
      continue;
    }

    if (a.kind === "insert" && b.kind === "delete") {
      // The second transaction deletes text inserted by the first. Drop that
      // inserted fragment from the composed output rather than deleting original text.
      const n = Math.min(a.text.length, b.length);
      a = a.text.length === n ? left[i++] : { kind: "insert", text: a.text.slice(n) };
      b = b.length === n ? right[j++] : { kind: "delete", length: b.length - n };
      continue;
    }

    throw new Error(`unsupported compose pair ${a.kind}/${b.kind}`);
  }

  return out;
};

export class ChangeSet {
  private constructor(
    readonly length: number,
    readonly lengthAfter: number,
    private readonly operations: readonly Operation[],
    private readonly ranges: readonly ChangeRange[],
    private readonly normalizedReplacements: readonly Replacement[],
  ) {}
  static empty(length: number): ChangeSet {
    return new ChangeSet(length, length, [], [], []);
  }
  static fromOperations(operations: Iterable<Operation>): Effect.Effect<ChangeSet, TextError> {
    return normalizeOperations(operations).pipe(
      Effect.map((ops) => {
        let length = 0;
        let lengthAfter = 0;
        let posA = 0;
        let posB = 0;
        const ranges: ChangeRange[] = [];
        const reps: Replacement[] = [];

        // Canonical operation semantics: adjacent delete+insert represents one
        // replacement of original input. Insert+delete remains two logical changes
        // because the delete consumes intermediate text, not the original span.
        for (let i = 0; i < ops.length; i++) {
          const op = ops[i]!;
          if (op.kind === "retain") {
            length += op.length;
            lengthAfter += op.length;
            posA += op.length;
            posB += op.length;
          } else if (op.kind === "delete") {
            const next = ops[i + 1];
            const insert = next?.kind === "insert" ? next.text : "";
            ranges.push(makeRange(posA, posA + op.length, posB, posB + insert.length, insert));
            reps.push({
              span: { start: posA, end: posA + op.length },
              text: insert.length ? insert : null,
            });
            length += op.length;
            posA += op.length;
            if (next?.kind === "insert") {
              lengthAfter += next.text.length;
              posB += next.text.length;
              i++;
            }
          } else {
            ranges.push(makeRange(posA, posA, posB, posB + op.text.length, op.text));
            reps.push({ span: { start: posA, end: posA }, text: op.text });
            lengthAfter += op.text.length;
            posB += op.text.length;
          }
        }
        return new ChangeSet(length, lengthAfter, ops, ranges, reps);
      }),
    );
  }
  static fromReplacements(
    length: number,
    replacements: Iterable<ReplacementInput>,
    options?: { readonly overlap?: ReplacementOverlapPolicy },
  ): Effect.Effect<ChangeSet, TextError> {
    const accepted: Array<{ r: Replacement; order: number }> = [];
    let order = 0;
    for (const input of replacements) {
      const r = Replacement.normalize(input);
      const err = validateSpan(r.span, length);
      if (err) return Effect.fail(err);
      accepted.push({ r, order: order++ });
    }
    const sorted = [...accepted].sort(
      (a, b) => a.r.span.start - b.r.span.start || a.r.span.end - b.r.span.end || a.order - b.order,
    );
    const final: Replacement[] = [];
    let previousEnd = 0;
    if (options?.overlap === "ignoreLater") {
      for (const item of accepted)
        if (!final.some((p) => item.r.span.start < p.span.end && item.r.span.end > p.span.start))
          final.push(item.r);
      final.sort((a, b) => a.span.start - b.span.start || a.span.end - b.span.end);
    } else {
      for (const item of sorted) {
        if (item.r.span.start < previousEnd)
          return Effect.fail(
            new OverlappingReplacement({
              previousEnd,
              start: item.r.span.start,
              end: item.r.span.end,
            }),
          );
        previousEnd = Math.max(previousEnd, item.r.span.end);
        final.push(item.r);
      }
    }
    const ops: Operation[] = [];
    let pos = 0;
    for (const r of final) {
      if (r.span.start > pos) ops.push({ kind: "retain", length: r.span.start - pos });
      if (r.span.end > r.span.start)
        ops.push({ kind: "delete", length: r.span.end - r.span.start });
      if (r.text !== null && r.text.length > 0) ops.push({ kind: "insert", text: r.text });
      pos = r.span.end;
    }
    if (pos < length) ops.push({ kind: "retain", length: length - pos });
    return ChangeSet.fromOperations(ops);
  }
  changes(): readonly Operation[] {
    return this.operations;
  }
  isEmpty(): boolean {
    return this.ranges.length === 0;
  }
  toLegacyReplacements(): readonly LegacyReplacement[] {
    return this.normalizedReplacements.map((r) => ({
      from: r.span.start,
      to: r.span.end,
      text: r.text ?? "",
    }));
  }
  /** @deprecated use toLegacyReplacements */
  get replacements(): readonly LegacyReplacement[] {
    return this.toLegacyReplacements();
  }
  *iterReplacements(): IterableIterator<Replacement> {
    yield* this.normalizedReplacements;
  }
  desc(): ChangeDesc {
    return ChangeDesc.unsafeMake(this.length, this.lengthAfter, this.ranges);
  }
  apply(doc: TextBuffer): TextBuffer {
    if (doc.length !== this.length)
      throw new LengthMismatch({ expected: this.length, actual: doc.length });
    return doc.apply({ replacements: this.toLegacyReplacements() });
  }
  applyTo(doc: TextBuffer): Effect.Effect<TextBuffer, TextError> {
    if (doc.length !== this.length)
      return Effect.fail(new LengthMismatch({ expected: this.length, actual: doc.length }));
    return Effect.try({
      try: () => this.apply(doc),
      catch: (error) =>
        error instanceof Error && "_tag" in error
          ? (error as TextError)
          : new InvalidChangeSet({
              message: error instanceof Error ? error.message : String(error),
            }),
    });
  }
  invert(original: TextBuffer): Effect.Effect<ChangeSet, TextError> {
    if (original.length !== this.length)
      return Effect.fail(new LengthMismatch({ expected: this.length, actual: original.length }));
    const inv: Replacement[] = [];
    for (let i = 0; i < this.normalizedReplacements.length; i++) {
      const r = this.normalizedReplacements[i]!;
      const range = this.ranges[i]!;
      inv.push({
        span: { start: range.fromB, end: range.toB },
        text: original.sliceString(r.span.start, r.span.end),
      });
    }
    return ChangeSet.fromReplacements(this.lengthAfter, inv);
  }
  mapPosition(pos: number, assoc: Assoc = "after"): number {
    return this.desc().mapPosition(pos, assoc);
  }
  mapPositionTracked(pos: number, assoc: Assoc = "after"): MappedPos {
    return this.desc().mapPositionTracked(pos, assoc);
  }
  mapRange(span: Span, optionsOrAssoc?: RangeMapOptions | Assoc): Span {
    return this.desc().mapRange(span, optionsOrAssoc);
  }
  mapPositionEffect(pos: number, assoc: Assoc = "after") {
    return this.desc().mapPositionEffect(pos, assoc);
  }
  mapPositionTrackedEffect(pos: number, assoc: Assoc = "after") {
    return this.desc().mapPositionTrackedEffect(pos, assoc);
  }
  mapRangeEffect(span: Span, optionsOrAssoc?: RangeMapOptions | Assoc) {
    return this.desc().mapRangeEffect(span, optionsOrAssoc);
  }
  compose(other: ChangeSet): Effect.Effect<ChangeSet, TextError> {
    if (this.lengthAfter !== other.length) {
      return Effect.fail(
        new ComposeLengthMismatch({ leftLengthAfter: this.lengthAfter, rightLength: other.length }),
      );
    }
    if (this.operations.length === 0) return Effect.succeed(other);
    if (other.operations.length === 0) return Effect.succeed(this);
    return Effect.try({
      try: () => composeOperations(this.operations, other.operations),
      catch: (error) =>
        new InvalidChangeSet({ message: error instanceof Error ? error.message : String(error) }),
    }).pipe(Effect.flatMap((ops) => ChangeSet.fromOperations(ops)));
  }
  updatePositions(
    items: Iterable<{ readonly position: MutablePosition; readonly assoc: Assoc }>,
  ): Effect.Effect<void, TextError> {
    const arr = [...items];
    if (arr.some((x) => isTrackingAssoc(x.assoc)))
      return Effect.fail(new TrackingAssocInBatchUpdate());
    const bad = arr
      .map((x) => x.position.value)
      .filter((p) => !Number.isInteger(p) || p < 0 || p > this.length);
    if (bad.length)
      return Effect.fail(new PositionsOutOfBounds({ positions: bad, length: this.length }));
    for (const x of arr) x.position.value = this.mapPosition(x.position.value, x.assoc);
    return Effect.void;
  }
}

export class Transaction {
  private constructor(readonly changes: ChangeSet) {}
  static empty(length: number): Transaction {
    return new Transaction(ChangeSet.empty(length));
  }
  static fromChangeSet(changes: ChangeSet): Transaction {
    return new Transaction(changes);
  }
  static fromReplacements(
    length: number,
    replacements: Iterable<ReplacementInput>,
    options?: { readonly overlap?: ReplacementOverlapPolicy },
  ): Effect.Effect<Transaction, TextError> {
    return ChangeSet.fromReplacements(length, replacements, options).pipe(
      Effect.map((c) => new Transaction(c)),
    );
  }
  static replace(length: number, span: Span, text: string): Effect.Effect<Transaction, TextError> {
    return Transaction.fromReplacements(length, [Replacement.replace(span, text)]);
  }
  static insertAt(length: number, at: number, text: string): Effect.Effect<Transaction, TextError> {
    return Transaction.fromReplacements(length, [Replacement.insert(at, text)]);
  }
  static delete(length: number, span: Span): Effect.Effect<Transaction, TextError> {
    return Transaction.fromReplacements(length, [Replacement.delete(span)]);
  }
  static deleteSpans(
    length: number,
    spans: Iterable<Span>,
    options?: { readonly overlap?: DeletionOverlapPolicy },
  ): Effect.Effect<Transaction, TextError> {
    const raw = [...spans];
    for (const s of raw) {
      const err = validateSpan(s, length);
      if (err) return Effect.fail(err);
    }
    let out: Span[] = [];
    if (options?.overlap === "merge") {
      for (const s of raw.sort((a, b) => a.start - b.start || a.end - b.end)) {
        const last = out[out.length - 1];
        if (last && s.start <= last.end)
          out[out.length - 1] = { start: last.start, end: Math.max(last.end, s.end) };
        else out.push(s);
      }
    } else if (options?.overlap === "ignoreLater") {
      for (const s of raw) if (!out.some((p) => s.start < p.end && s.end > p.start)) out.push(s);
      out.sort((a, b) => a.start - b.start || a.end - b.end);
    } else {
      let prev = 0;
      for (const s of raw.sort((a, b) => a.start - b.start || a.end - b.end)) {
        if (s.start < prev)
          return Effect.fail(
            new OverlappingReplacement({ previousEnd: prev, start: s.start, end: s.end }),
          );
        prev = s.end;
        out.push(s);
      }
    }
    return Transaction.fromReplacements(
      length,
      out.map((span) => Replacement.delete(span)),
    );
  }
  get length(): number {
    return this.changes.length;
  }
  get lengthAfter(): number {
    return this.changes.lengthAfter;
  }
  desc(): ChangeDesc {
    return this.changes.desc();
  }
  apply(doc: TextBuffer): TextBuffer {
    return this.changes.apply(doc);
  }
  applyTo(doc: TextBuffer): Effect.Effect<TextBuffer, TextError> {
    return this.changes.applyTo(doc);
  }
  invert(original: TextBuffer): Effect.Effect<Transaction, TextError> {
    return this.changes.invert(original).pipe(Effect.map((c) => new Transaction(c)));
  }
  compose(other: Transaction): Effect.Effect<Transaction, TextError> {
    return this.changes.compose(other.changes).pipe(Effect.map((c) => new Transaction(c)));
  }
  mapPosition(pos: number, assoc: Assoc = "after"): number {
    return this.changes.mapPosition(pos, assoc);
  }
  mapPositionTracked(pos: number, assoc: Assoc = "after"): MappedPos {
    return this.changes.mapPositionTracked(pos, assoc);
  }
  mapRange(span: Span, optionsOrAssoc?: RangeMapOptions | Assoc): Span {
    return this.changes.mapRange(span, optionsOrAssoc);
  }
  mapPositionEffect(pos: number, assoc: Assoc = "after") {
    return this.changes.mapPositionEffect(pos, assoc);
  }
  mapPositionTrackedEffect(pos: number, assoc: Assoc = "after") {
    return this.changes.mapPositionTrackedEffect(pos, assoc);
  }
  mapRangeEffect(span: Span, optionsOrAssoc?: RangeMapOptions | Assoc) {
    return this.changes.mapRangeEffect(span, optionsOrAssoc);
  }
}

export const changeSetFromOperations = (ops: readonly Operation[]): ChangeSet =>
  Effect.runSync(ChangeSet.fromOperations(ops));
export const mapPosition = (changes: ChangeSet, pos: number, assoc: Assoc = "after"): number =>
  changes.mapPosition(pos, assoc);
export const invert = (changes: ChangeSet, before: TextBuffer): ChangeSet =>
  Effect.runSync(changes.invert(before));
export const compose = (a: ChangeSet, b: ChangeSet): Effect.Effect<ChangeSet, TextError> =>
  a.compose(b);
