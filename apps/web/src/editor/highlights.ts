/**
 * Per-document bridge to the synchronous syntax engine.
 *
 * There is no async cache here anymore — no coverage windows, no run
 * mapping, no round trips. The renderer calls `ensureSynced` at the top of
 * every render pass (feeding the exact change sets it is about to paint),
 * the engine interpolates + reparses within a keystroke budget, and
 * `runsBetween` then answers from the engine's own tree: the frame that
 * shows an edit or a scroll jump is the frame that has its colors.
 *
 * The document bus subscription is the catch-all for edits no renderer
 * paints (hot reload into a background tab); version gating makes the two
 * paths idempotent. Big files parse in pumped slices (the engine's parser
 * resumes timed-out parses), so the UI thread never blocks on open.
 */

import { Context, Effect, Fiber, Stream } from "effect";
import {
  DocumentInvalidated,
  DocumentNotifications,
  payloadsOf,
  type DocumentChangeRange,
  type DocumentScope,
} from "@the-editor/editor";
import { TextDocument } from "@the-editor/engine";
import type { HighlightRun, SyntaxEdit, SyntaxPort } from "../platform/syntax";

const PUMP_BUDGET_MS = 8;

export class DocumentHighlights {
  private version: number;
  private active: boolean;
  private closed = false;
  private pumping = false;
  private readonly listeners = new Set<() => void>();
  private readonly busFiber: Fiber.RuntimeFiber<unknown, unknown>;
  private readonly text: TextDocument.Service;

  constructor(
    private readonly port: SyntaxPort,
    readonly docId: string,
    document: DocumentScope,
    private readonly path: string | undefined,
  ) {
    const context = document.context as Context.Context<TextDocument | DocumentNotifications>;
    this.text = Context.get(context, TextDocument);
    const bus = Context.get(context, DocumentNotifications);

    const snapshot = Effect.runSync(this.text.snapshot);
    this.version = snapshot.version;
    const opened = this.port.open(
      docId,
      this.path,
      snapshot.buffer.sliceString(0, snapshot.buffer.length),
    );
    this.active = opened !== null;
    if (this.active) this.schedulePump();

    // Catch-all for writers no renderer is painting (background hot
    // reload); foreground edits arrive earlier via ensureSynced.
    this.busFiber = Effect.runFork(
      Stream.runForEach(bus.notifications, (notification) =>
        Effect.sync(() => {
          for (const payload of payloadsOf(notification, DocumentInvalidated)) {
            this.applyChanges(payload.version, payload.changes ?? null);
          }
        }),
      ),
    );
  }

  /**
   * Renderer pre-layout hook: bring the engine to `version` SYNCHRONOUSLY
   * using the change sets the renderer is about to paint (null = unknown,
   * full resync). Idempotent across panes and the bus subscription.
   */
  ensureSynced(version: number, changeSets: ReadonlyArray<ReadonlyArray<DocumentChangeRange>> | null): void {
    if (!this.active || this.closed || version === this.version) return;
    if (changeSets === null || this.version !== version - changeSets.length) {
      this.resync(version);
      return;
    }
    for (const changes of changeSets) {
      this.applyChanges(this.version + 1, changes);
    }
  }

  /** Synchronous windowed runs straight from the engine's tree. */
  runsBetween(from: number, to: number): readonly HighlightRun[] {
    if (!this.active || this.closed) return [];
    return this.port.highlights(this.docId, from, to);
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  close(): void {
    if (this.closed) return;
    this.closed = true;
    Effect.runFork(Fiber.interrupt(this.busFiber));
    this.listeners.clear();
    this.port.close(this.docId);
  }

  private applyChanges(
    version: number,
    changes: ReadonlyArray<DocumentChangeRange> | null,
  ): void {
    if (!this.active || this.closed || version <= this.version) return;
    if (changes === null || version !== this.version + 1) {
      this.resync(version);
      return;
    }
    const buffer = Effect.runSync(this.text.get);
    const edits: SyntaxEdit[] = changes.map((change) => ({
      start: change.fromA,
      end: change.toA,
      text: buffer.sliceString(change.fromB, change.toB),
    }));
    this.version = version;
    if (this.port.edit(this.docId, edits) === "pending") this.schedulePump();
  }

  /** Version chain broken (missed notifications): rebuild from the buffer. */
  private resync(version: number): void {
    const buffer = Effect.runSync(this.text.get);
    this.version = version;
    const opened = this.port.open(this.docId, this.path, buffer.sliceString(0, buffer.length));
    this.active = opened !== null;
    if (this.active) this.schedulePump();
  }

  /** Big parses complete in budgeted slices off the critical path. */
  private schedulePump(): void {
    if (this.pumping || this.closed) return;
    this.pumping = true;
    const step = () => {
      this.pumping = false;
      if (this.closed || !this.active) return;
      const status = this.port.pump(this.docId, PUMP_BUDGET_MS);
      if (status === "pending") {
        this.pumping = true;
        window.setTimeout(step, 0);
        return;
      }
      // Accurate now: anyone displaying this document repaints.
      for (const listener of this.listeners) listener();
    };
    window.setTimeout(step, 0);
  }
}
