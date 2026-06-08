import { Effect } from "effect";
import { Selection, SelectionOutOfBoundsError } from "./selection";
import { TextBuffer, Transaction, type TextError } from "./text/index";

export type HistoryTimestamp = number;
export type UndoKind =
  | { readonly type: "steps"; readonly steps: number }
  | { readonly type: "time"; readonly durationMs: number };

export class RevisionId {
  readonly value: number;
  constructor(value: number) {
    if (!Number.isInteger(value) || value < 0) throw new InvalidRevisionIdError(value);
    this.value = value;
  }
  static root(): RevisionId {
    return new RevisionId(0);
  }
  static of(value: number): RevisionId {
    return new RevisionId(value);
  }
  get index(): number {
    return this.value;
  }
}

export class HistoryError extends Error {}
export class InvalidRevisionIdError extends HistoryError {
  constructor(readonly revision: number | RevisionId) {
    super(`invalid revision ${typeof revision === "number" ? revision : revision.index}`);
  }
}
export class RevisionOutOfBoundsError extends HistoryError {
  constructor(
    readonly revision: RevisionId,
    readonly max: RevisionId,
  ) {
    super(`revision ${revision.index} out of bounds, max ${max.index}`);
  }
}
export class StaleHistoryJumpError extends HistoryError {
  constructor(
    readonly expected: RevisionId,
    readonly got: RevisionId,
  ) {
    super(`stale history jump: expected ${expected.index}, got ${got.index}`);
  }
}
export class HistorySelectionOutOfBoundsError extends HistoryError {
  constructor(readonly documentLength: number) {
    super(`selection out of bounds for document length ${documentLength}`);
  }
}
export class CommitLengthMismatchError extends HistoryError {
  constructor(
    readonly expected: number,
    readonly got: number,
  ) {
    super(`commit length mismatch: expected ${expected}, got ${got}`);
  }
}
export class InvalidHistoryTimestampError extends HistoryError {
  constructor(readonly timestamp: number) {
    super(`invalid history timestamp ${timestamp}`);
  }
}
export class InvalidHistoryStepCountError extends HistoryError {
  constructor(readonly steps: number) {
    super(`invalid history step count ${steps}`);
  }
}
export class InvalidHistoryDurationError extends HistoryError {
  constructor(readonly durationMs: number) {
    super(`invalid history duration ${durationMs}`);
  }
}

type Revision = {
  readonly parent: RevisionId;
  lastChild: RevisionId | undefined;
  readonly transaction: Transaction;
  readonly inversion: Transaction;
  readonly timestamp: HistoryTimestamp;
  readonly selectionBefore: Selection;
  readonly selectionAfter: Selection;
};

export class HistoryJump {
  readonly source: RevisionId;
  readonly target: RevisionId;
  readonly transactions: readonly Transaction[];
  readonly selection: Selection;
  constructor(args: {
    readonly source: RevisionId;
    readonly target: RevisionId;
    readonly transactions: readonly Transaction[];
    readonly selection: Selection;
  }) {
    this.source = args.source;
    this.target = args.target;
    this.transactions = args.transactions;
    this.selection = args.selection;
  }
  get isEmpty(): boolean {
    return this.transactions.length === 0;
  }
  get length(): number {
    return this.transactions.length;
  }
}

const validateTimestamp = (timestamp: number): void => {
  if (!Number.isFinite(timestamp)) throw new InvalidHistoryTimestampError(timestamp);
};
const validateSteps = (steps: number): void => {
  if (!Number.isInteger(steps) || steps < 0) throw new InvalidHistoryStepCountError(steps);
};
const validateDuration = (durationMs: number): void => {
  if (!Number.isFinite(durationMs) || durationMs < 0)
    throw new InvalidHistoryDurationError(durationMs);
};
const validateSelection = (selection: Selection, length: number): void => {
  try {
    selection.validate(length);
  } catch (error) {
    if (error instanceof SelectionOutOfBoundsError)
      throw new HistorySelectionOutOfBoundsError(length);
    throw error;
  }
};

export class History {
  private readonly revisions: Revision[];
  private current: RevisionId;

  private constructor(root: Revision) {
    this.revisions = [root];
    this.current = RevisionId.root();
  }

  static create(
    initialLength: number,
    initialSelection: Selection,
    options?: { readonly timestamp?: HistoryTimestamp },
  ): History {
    if (!Number.isInteger(initialLength) || initialLength < 0)
      throw new CommitLengthMismatchError(0, initialLength);
    validateSelection(initialSelection, initialLength);
    const timestamp = options?.timestamp ?? Date.now();
    validateTimestamp(timestamp);
    const empty = Transaction.empty(initialLength);
    return new History({
      parent: RevisionId.root(),
      lastChild: undefined,
      transaction: empty,
      inversion: empty,
      timestamp,
      selectionBefore: initialSelection,
      selectionAfter: initialSelection,
    });
  }

  get currentRevision(): RevisionId {
    return this.current;
  }
  get atRoot(): boolean {
    return this.current.index === 0;
  }
  get length(): number {
    return this.revisions.length;
  }
  get isEmpty(): boolean {
    return this.revisions.length === 1;
  }
  get currentSelection(): Selection {
    return this.revision(this.current).selectionAfter;
  }
  get currentLength(): number {
    return this.revisionLength(this.current);
  }
  get currentTimestamp(): HistoryTimestamp {
    return this.revisionTimestamp(this.current);
  }

  selection(revision: RevisionId): Selection {
    return this.revision(revision).selectionAfter;
  }
  revisionLength(revision: RevisionId): number {
    return this.revision(revision).transaction.lengthAfter;
  }
  revisionTimestamp(revision: RevisionId): HistoryTimestamp {
    return this.revision(revision).timestamp;
  }

  commit(args: {
    readonly before: TextBuffer;
    readonly transaction: Transaction;
    readonly selectionBefore: Selection;
    readonly selectionAfter: Selection;
    readonly timestamp?: HistoryTimestamp;
  }): Effect.Effect<RevisionId, TextError | HistoryError> {
    let timestamp: number;
    try {
      const expected = this.currentLength;
      if (args.before.length !== expected)
        throw new CommitLengthMismatchError(expected, args.before.length);
      const desc = args.transaction.desc();
      if (desc.length !== expected) throw new CommitLengthMismatchError(expected, desc.length);
      validateSelection(args.selectionBefore, desc.length);
      validateSelection(args.selectionAfter, desc.lengthAfter);
      timestamp = args.timestamp ?? Date.now();
      validateTimestamp(timestamp);
    } catch (error) {
      if (error instanceof HistoryError) return Effect.fail(error);
      throw error;
    }

    return Effect.gen(this, function* () {
      const inversion = yield* args.transaction.invert(args.before);
      const id = RevisionId.of(this.revisions.length);
      this.revision(this.current).lastChild = id;
      this.revisions.push({
        parent: this.current,
        lastChild: undefined,
        transaction: args.transaction,
        inversion,
        timestamp,
        selectionBefore: args.selectionBefore,
        selectionAfter: args.selectionAfter,
      });
      this.current = id;
      return id;
    });
  }

  changesSince(
    revision: RevisionId,
  ): Effect.Effect<Transaction | undefined, TextError | HistoryError> {
    return Effect.gen(this, function* () {
      this.revision(revision);
      if (revision.index === this.current.index) return undefined;
      const jump = this.jumpBetween(revision, this.current);
      if (jump.transactions.length === 0) return undefined;
      let composed = jump.transactions[0]!;
      for (let i = 1; i < jump.transactions.length; i++)
        composed = yield* composed.compose(jump.transactions[i]!);
      return composed;
    });
  }

  undo(): HistoryJump | undefined {
    if (this.atRoot) return undefined;
    const current = this.revision(this.current);
    return new HistoryJump({
      source: this.current,
      target: current.parent,
      transactions: [current.inversion],
      selection: current.selectionBefore,
    });
  }

  redo(): HistoryJump | undefined {
    const child = this.revision(this.current).lastChild;
    if (child === undefined) return undefined;
    const revision = this.revision(child);
    return new HistoryJump({
      source: this.current,
      target: child,
      transactions: [revision.transaction],
      selection: revision.selectionAfter,
    });
  }

  jumpTo(revision: RevisionId): HistoryJump {
    this.revision(revision);
    return this.jumpBetween(this.current, revision);
  }
  jumpBackward(steps: number): HistoryJump {
    validateSteps(steps);
    let target = this.current;
    for (let i = 0; i < steps && target.index !== 0; i++) target = this.revision(target).parent;
    return this.jumpTo(target);
  }
  jumpForward(steps: number): HistoryJump {
    validateSteps(steps);
    let target = this.current;
    for (let i = 0; i < steps; i++) {
      const child = this.revision(target).lastChild;
      if (child === undefined) break;
      target = child;
    }
    return this.jumpTo(target);
  }

  jumpDurationBackward(durationMs: number): HistoryJump {
    validateDuration(durationMs);
    return this.jumpTo(this.nearestTimestamp(this.currentTimestamp - durationMs, "backward"));
  }
  jumpDurationForward(durationMs: number): HistoryJump {
    validateDuration(durationMs);
    return this.jumpTo(this.nearestTimestamp(this.currentTimestamp + durationMs, "forward"));
  }
  earlier(kind: UndoKind): HistoryJump {
    return kind.type === "steps"
      ? this.jumpBackward(kind.steps)
      : this.jumpDurationBackward(kind.durationMs);
  }
  later(kind: UndoKind): HistoryJump {
    return kind.type === "steps"
      ? this.jumpForward(kind.steps)
      : this.jumpDurationForward(kind.durationMs);
  }

  applyJump(jump: HistoryJump): void {
    if (jump.source.index !== this.current.index)
      throw new StaleHistoryJumpError(this.current, jump.source);
    this.revision(jump.target);
    this.current = jump.target;
  }

  private revision(id: RevisionId): Revision {
    if (!Number.isInteger(id.index) || id.index < 0) throw new InvalidRevisionIdError(id);
    const revision = this.revisions[id.index];
    if (revision === undefined)
      throw new RevisionOutOfBoundsError(id, RevisionId.of(this.revisions.length - 1));
    return revision;
  }

  private pathUp(from: RevisionId, to: RevisionId): RevisionId[] {
    const path: RevisionId[] = [];
    let current = from;
    while (current.index !== to.index) {
      path.push(current);
      current = this.revision(current).parent;
    }
    return path;
  }

  private lowestCommonAncestor(a: RevisionId, b: RevisionId): RevisionId {
    const seenA = new Set<number>();
    const seenB = new Set<number>();
    let left = a;
    let right = b;
    for (;;) {
      seenA.add(left.index);
      seenB.add(right.index);
      if (seenA.has(right.index)) return right;
      if (seenB.has(left.index)) return left;
      left = this.revision(left).parent;
      right = this.revision(right).parent;
    }
  }

  private jumpBetween(source: RevisionId, target: RevisionId): HistoryJump {
    if (source.index === target.index)
      return new HistoryJump({
        source,
        target,
        transactions: [],
        selection: this.selection(target),
      });
    const lca = this.lowestCommonAncestor(source, target);
    const up = this.pathUp(source, lca);
    const down = this.pathUp(target, lca);
    const transactions = [
      ...up.map((id) => this.revision(id).inversion),
      ...down.reverse().map((id) => this.revision(id).transaction),
    ];
    const selection = this.selection(target);
    return new HistoryJump({ source, target, transactions, selection });
  }

  private nearestTimestamp(target: number, direction: "backward" | "forward"): RevisionId {
    let best = 0;
    let bestDiff = Number.POSITIVE_INFINITY;
    let bestTie = 1;
    for (let i = 0; i < this.revisions.length; i++) {
      const timestamp = this.revisions[i]!.timestamp;
      const diff = Math.abs(timestamp - target);
      const isAfter = timestamp >= target;
      const tie = direction === "backward" ? (isAfter ? 1 : 0) : !isAfter ? 1 : 0;
      if (diff < bestDiff || (diff === bestDiff && tie < bestTie)) {
        best = i;
        bestDiff = diff;
        bestTie = tie;
      }
    }
    return RevisionId.of(best);
  }
}
