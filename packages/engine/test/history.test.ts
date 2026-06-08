import { Effect } from "effect";
import { describe, expect, test } from "vitest";
import {
  CommitLengthMismatchError,
  History,
  HistorySelectionOutOfBoundsError,
  RevisionId,
  RevisionOutOfBoundsError,
  Selection,
  SelectionRange,
  StaleHistoryJumpError,
  TextBuffer,
  Transaction,
  type HistoryJump,
} from "../src/index";

const run = <A, E = never>(effect: Effect.Effect<A, E>) => Effect.runSync(effect);
const applyJump = (buffer: TextBuffer, history: History, jump: HistoryJump) => {
  let next = buffer;
  for (const tx of jump.transactions) next = tx.apply(next);
  history.applyJump(jump);
  return { buffer: next, selection: jump.selection };
};

describe("History", () => {
  test("creates root state", () => {
    const history = History.create(5, Selection.point(0), { timestamp: 10 });
    expect(history.currentRevision.index).toBe(0);
    expect(history.currentLength).toBe(5);
    expect(history.currentSelection).toEqual(Selection.point(0));
    expect(history.undo()).toBeUndefined();
    expect(() => History.create(1, Selection.point(2))).toThrow(HistorySelectionOutOfBoundsError);
  });

  test("commit updates revision, length, selection, and timestamp", () => {
    const history = History.create(5, Selection.point(0), { timestamp: 0 });
    const before = TextBuffer.fromString("hello");
    const tx = run(Transaction.insertAt(before.length, 5, "!"));
    const after = tx.apply(before);
    const id = run(
      history.commit({
        before,
        transaction: tx,
        selectionBefore: Selection.point(0),
        selectionAfter: Selection.point(6),
        timestamp: 100,
      }),
    );
    expect(id.index).toBe(1);
    expect(history.currentRevision).toEqual(id);
    expect(history.currentLength).toBe(after.length);
    expect(history.currentSelection).toEqual(Selection.point(6));
    expect(history.revisionTimestamp(id)).toBe(100);
    expect(history.selection(id)).toEqual(Selection.point(6));
  });

  test("commit validation", () => {
    const history = History.create(5, Selection.point(0));
    const wrongBefore = TextBuffer.fromString("no");
    const tx = run(Transaction.insertAt(5, 5, "!"));
    const first = Effect.runSync(
      Effect.either(
        history.commit({
          before: wrongBefore,
          transaction: tx,
          selectionBefore: Selection.point(0),
          selectionAfter: Selection.point(6),
        }),
      ),
    );
    expect(first._tag === "Left" && first.left instanceof CommitLengthMismatchError).toBe(true);
    const badTx = run(Transaction.insertAt(2, 2, "!"));
    const second = Effect.runSync(
      Effect.either(
        history.commit({
          before: TextBuffer.fromString("hello"),
          transaction: badTx,
          selectionBefore: Selection.point(0),
          selectionAfter: Selection.point(3),
        }),
      ),
    );
    expect(second._tag === "Left" && second.left instanceof CommitLengthMismatchError).toBe(true);
    const third = Effect.runSync(
      Effect.either(
        history.commit({
          before: TextBuffer.fromString("hello"),
          transaction: tx,
          selectionBefore: Selection.point(9),
          selectionAfter: Selection.point(6),
        }),
      ),
    );
    expect(third._tag === "Left" && third.left instanceof HistorySelectionOutOfBoundsError).toBe(
      true,
    );
  });

  test("undo and redo restore text and selections", () => {
    const history = History.create(5, Selection.point(0));
    let buffer = TextBuffer.fromString("hello");
    const tx = run(Transaction.insertAt(buffer.length, 5, "!"));
    const before = buffer;
    buffer = tx.apply(buffer);
    run(
      history.commit({
        before,
        transaction: tx,
        selectionBefore: Selection.point(0),
        selectionAfter: Selection.point(6),
      }),
    );
    const undone = applyJump(buffer, history, history.undo()!);
    expect(undone.buffer.toString()).toBe("hello");
    expect(undone.selection).toEqual(Selection.point(0));
    const redone = applyJump(undone.buffer, history, history.redo()!);
    expect(redone.buffer.toString()).toBe("hello!");
    expect(redone.selection).toEqual(Selection.point(6));
  });

  test("branching and cross-branch jump", () => {
    const history = History.create(1, Selection.point(0));
    let buffer = TextBuffer.fromString("a");
    const tx1 = run(Transaction.insertAt(1, 1, "b"));
    const before1 = buffer;
    buffer = tx1.apply(buffer);
    const rev1 = run(
      history.commit({
        before: before1,
        transaction: tx1,
        selectionBefore: Selection.point(0),
        selectionAfter: Selection.point(2),
        timestamp: 1,
      }),
    );
    const txOld = run(Transaction.insertAt(2, 2, "c"));
    const beforeOld = buffer;
    const oldText = txOld.apply(buffer);
    const oldBranch = run(
      history.commit({
        before: beforeOld,
        transaction: txOld,
        selectionBefore: Selection.point(2),
        selectionAfter: Selection.point(3),
        timestamp: 2,
      }),
    );
    buffer = applyJump(oldText, history, history.jumpTo(rev1)).buffer;
    const txNew = run(Transaction.insertAt(2, 2, "d"));
    const beforeNew = buffer;
    const newText = txNew.apply(buffer);
    const newBranch = run(
      history.commit({
        before: beforeNew,
        transaction: txNew,
        selectionBefore: Selection.point(2),
        selectionAfter: Selection.point(3),
        timestamp: 3,
      }),
    );
    buffer = applyJump(newText, history, history.jumpTo(rev1)).buffer;
    expect(applyJump(buffer, history, history.redo()!).buffer.toString()).toBe("abd");
    const jumpOld = history.jumpTo(oldBranch);
    const result = applyJump(newText, history, jumpOld);
    expect(result.buffer.toString()).toBe("abc");
    expect(history.currentRevision).toEqual(oldBranch);
    expect(newBranch.index).toBeGreaterThan(oldBranch.index);
  });

  test("changesSince composes ancestor and branch paths", () => {
    const history = History.create(1, Selection.point(0));
    let buffer = TextBuffer.fromString("a");
    const tx1 = run(Transaction.insertAt(1, 1, "b"));
    const before1 = buffer;
    buffer = tx1.apply(buffer);
    const rev1 = run(
      history.commit({
        before: before1,
        transaction: tx1,
        selectionBefore: Selection.point(0),
        selectionAfter: Selection.point(2),
      }),
    );
    const tx2 = run(Transaction.insertAt(2, 2, "c"));
    const before2 = buffer;
    buffer = tx2.apply(buffer);
    run(
      history.commit({
        before: before2,
        transaction: tx2,
        selectionBefore: Selection.point(2),
        selectionAfter: Selection.point(3),
      }),
    );
    expect(run(history.changesSince(history.currentRevision))).toBeUndefined();
    expect(run(history.changesSince(rev1))!.apply(TextBuffer.fromString("ab")).toString()).toBe(
      "abc",
    );
  });

  test("stale jump check", () => {
    const history = History.create(1, Selection.point(0));
    let buffer = TextBuffer.fromString("a");
    const tx = run(Transaction.insertAt(1, 1, "b"));
    const before = buffer;
    buffer = tx.apply(buffer);
    run(
      history.commit({
        before,
        transaction: tx,
        selectionBefore: Selection.point(0),
        selectionAfter: Selection.point(2),
      }),
    );
    const jump = history.undo()!;
    applyJump(buffer, history, jump);
    expect(() => history.applyJump(jump)).toThrow(StaleHistoryJumpError);
    expect(history.currentRevision.index).toBe(0);
  });

  test("time navigation", () => {
    const history = History.create(1, Selection.point(0), { timestamp: 0 });
    let buffer = TextBuffer.fromString("a");
    for (const [ch, ts] of [
      ["b", 10],
      ["c", 20],
      ["d", 30],
    ] as const) {
      const tx = run(Transaction.insertAt(buffer.length, buffer.length, ch));
      const before = buffer;
      buffer = tx.apply(buffer);
      run(
        history.commit({
          before,
          transaction: tx,
          selectionBefore: Selection.point(before.length),
          selectionAfter: Selection.point(buffer.length),
          timestamp: ts,
        }),
      );
    }
    expect(history.jumpDurationBackward(15).target.index).toBe(1);
    applyJump(buffer, history, history.jumpDurationBackward(15));
    expect(history.jumpDurationForward(15).target.index).toBe(3);
    expect(history.earlier({ type: "steps", steps: 1 }).target.index).toBe(0);
    expect(history.later({ type: "time", durationMs: 20 }).target.index).toBe(3);
  });

  test("invalid revision ids", () => {
    const history = History.create(0, Selection.point(0));
    expect(() => history.selection(RevisionId.of(99))).toThrow(RevisionOutOfBoundsError);
    expect(() => history.revisionLength(RevisionId.of(99))).toThrow(RevisionOutOfBoundsError);
    expect(() => history.revisionTimestamp(RevisionId.of(99))).toThrow(RevisionOutOfBoundsError);
    expect(() => history.jumpTo(RevisionId.of(99))).toThrow(RevisionOutOfBoundsError);
  });

  test("deletion undo restores deleted text", () => {
    const history = History.create(5, Selection.point(0));
    let buffer = TextBuffer.fromString("hello");
    const tx = run(Transaction.delete(buffer.length, { start: 1, end: 4 }));
    const before = buffer;
    buffer = tx.apply(buffer);
    run(
      history.commit({
        before,
        transaction: tx,
        selectionBefore: Selection.point(0),
        selectionAfter: Selection.point(1),
      }),
    );
    expect(buffer.toString()).toBe("ho");
    expect(applyJump(buffer, history, history.undo()!).buffer.toString()).toBe("hello");
  });
});
