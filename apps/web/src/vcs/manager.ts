/**
 * The VCS manager: workspace state in, badges and gutters out — the same
 * consumer shape as the LSP manager. One active provider per workspace
 * root (first registered provider whose detect() answers true). Repo
 * status feeds the file tree's badges and the status bar; per-document
 * HEAD baselines feed diff gutters as an ordinary EditorVisuals
 * contribution (gutter bands), recomputed debounced off the document bus.
 */

import { Context, Effect, Exit, Fiber, Scope, Stream } from "effect";
import {
  DocumentInvalidated,
  DocumentNotifications,
  EditorVisuals,
  payloadsOf,
  type DocumentScope,
  type EditorVisualSet,
  type FileSystemPort,
  type VisualsHandle,
} from "@the-editor/editor";
import { TextDocument, type TextBuffer } from "@the-editor/engine";
import { diffLines, type LineDiff } from "./diff";
import type { VcsProvider, VcsProviders, VcsStatusEntry } from "../product/vcs";

const STATUS_DEBOUNCE_MS = 300;
/**
 * Recompute is pure memory (the HEAD baseline is cached at attach); the
 * only reason to wait at all is coalescing keystroke bursts within a frame.
 */
const GUTTER_DEBOUNCE_MS = 16;
const MAX_BASELINE_CHARS = 2_000_000;

type GutterAttachment = {
  readonly docId: string;
  readonly path: string;
  readonly document: DocumentScope;
  readonly visualsScope: Scope.CloseableScope;
  readonly visuals: VisualsHandle;
  readonly busFiber: Fiber.RuntimeFiber<unknown, unknown>;
  baseline: string | null;
  timer: number | null;
};

const gutterSet = (diff: LineDiff): EditorVisualSet => ({
  gutterBands: [
    {
      id: "vcs-diff",
      side: "left",
      widthCh: 0,
      // A thin colored strip beside the line numbers (Zed/VSCode), not a
      // padded text column.
      widthPx: 3,
      priority: 1,
      cell: (line) => {
        const mark = diff.lines.get(line);
        if (mark === "added") return { text: "", className: "vcs-gutter-added" };
        if (mark === "modified") return { text: "", className: "vcs-gutter-modified" };
        if (diff.deletedAbove.has(line)) return { text: "", className: "vcs-gutter-deleted" };
        return null;
      },
    },
  ],
});

export class VcsManager {
  private root: string | null = null;
  private provider: VcsProvider | null = null;
  private statusEntries: ReadonlyArray<VcsStatusEntry> = [];
  private branchName: string | null = null;
  private readonly listeners = new Set<() => void>();
  private readonly gutters = new Map<string, GutterAttachment>();
  private refreshTimer: number | null = null;
  private unwatchRoot: (() => void) | null = null;
  private generation = 0;
  private disposed = false;

  constructor(
    private readonly providers: VcsProviders.Service,
    private readonly fileSystem: FileSystemPort.Service | undefined,
  ) {}

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => void this.listeners.delete(listener);
  }

  branch(): string | null {
    return this.branchName;
  }

  /** Dev diagnostics. */
  debugInfo(): Record<string, unknown> {
    return {
      root: this.root,
      provider: this.provider?.id ?? null,
      gutters: [...this.gutters.values()].map((g) => ({
        path: g.path,
        baseline: g.baseline === null ? null : g.baseline.length,
      })),
    };
  }

  /** Root-relative entries in @pierre/trees badge vocabulary. */
  treeStatusEntries(): ReadonlyArray<VcsStatusEntry> {
    return this.statusEntries;
  }

  /** Tracked file listing for the tree (null = no provider / unsupported). */
  async listFiles(root: string): Promise<ReadonlyArray<string> | null> {
    const provider = await this.providerFor(root);
    return provider === null ? null : provider.listFiles(root);
  }

  /** Re-anchor to a workspace root (or none). */
  async setRoot(root: string | null): Promise<void> {
    const generation = ++this.generation;
    this.unwatchRoot?.();
    this.unwatchRoot = null;
    this.root = root;
    this.provider = null;
    this.statusEntries = [];
    this.branchName = null;
    if (root !== null) {
      this.provider = await this.providerFor(root);
      if (generation !== this.generation) return;
      await this.refresh();
      if (generation !== this.generation) return;
      // Any change under the root re-checks status (edits, checkouts,
      // commits — .git/index changes ride the same recursive watcher).
      if (this.fileSystem?.watchDir !== undefined) {
        try {
          this.unwatchRoot = await Effect.runPromise(
            this.fileSystem.watchDir(root, (event) => {
              // Our own `git status` runs churn lock files — refreshing on
              // those is a self-trigger loop. Real state changes surface
              // through working files, HEAD, refs, and the index proper.
              if (event.path.endsWith(".lock")) return;
              this.scheduleRefresh();
            }),
          );
        } catch {
          // Watching is best-effort.
        }
      }
    }
    for (const attachment of this.gutters.values()) {
      void this.reloadBaseline(attachment);
    }
    this.notify();
  }

  private async providerFor(root: string): Promise<VcsProvider | null> {
    for (const provider of Effect.runSync(this.providers.list)) {
      try {
        if (await provider.detect(root)) return provider;
      } catch {
        // Provider probe failures mean "not mine".
      }
    }
    return null;
  }

  private scheduleRefresh(): void {
    if (this.refreshTimer !== null) window.clearTimeout(this.refreshTimer);
    this.refreshTimer = window.setTimeout(() => {
      this.refreshTimer = null;
      void this.refresh().then(() => this.notify());
    }, STATUS_DEBOUNCE_MS);
  }

  private async refresh(): Promise<void> {
    const root = this.root;
    const provider = this.provider;
    if (root === null || provider === null || this.disposed) return;
    const generation = this.generation;
    const [entries, info] = await Promise.all([
      provider.status(root).catch(() => [] as VcsStatusEntry[]),
      provider.info(root).catch(() => null),
    ]);
    if (generation !== this.generation || this.disposed) return;
    this.statusEntries = entries;
    this.branchName = info?.branch ?? null;
    // HEAD may have moved (commit, checkout): refresh gutter baselines.
    for (const attachment of this.gutters.values()) {
      void this.reloadBaseline(attachment);
    }
  }

  /** Saves refresh immediately-ish; the watcher catches everything else. */
  documentSaved(): void {
    this.scheduleRefresh();
  }

  // ------------------------------------------------------------ diff gutters

  attach(document: DocumentScope, path: string): void {
    const docId = String(document.id);
    if (this.gutters.has(docId)) return;
    const context = document.context as Context.Context<
      TextDocument | DocumentNotifications | EditorVisuals
    >;
    const visualsService = Context.get(context, EditorVisuals);
    const bus = Context.get(context, DocumentNotifications);
    const visualsScope = Effect.runSync(Scope.make());
    const visuals = Effect.runSync(
      visualsService.contribute("vcs-diff", {}).pipe(Scope.extend(visualsScope)),
    );
    const attachment: GutterAttachment = {
      docId,
      path,
      document,
      visualsScope,
      visuals,
      busFiber: Effect.runFork(
        Stream.runForEach(bus.notifications, (notification) =>
          Effect.sync(() => {
            if (payloadsOf(notification, DocumentInvalidated).length === 0) return;
            this.scheduleGutter(attachment);
          }),
        ),
      ),
      baseline: null,
      timer: null,
    };
    this.gutters.set(docId, attachment);
    void this.reloadBaseline(attachment);
  }

  detach(docId: string): void {
    const attachment = this.gutters.get(docId);
    if (attachment === undefined) return;
    this.gutters.delete(docId);
    if (attachment.timer !== null) window.clearTimeout(attachment.timer);
    Effect.runFork(Fiber.interrupt(attachment.busFiber));
    Effect.runFork(Scope.close(attachment.visualsScope, Exit.void));
  }

  private relativePath(path: string): string | null {
    if (this.root === null) return null;
    const prefix = this.root.endsWith("/") ? this.root : `${this.root}/`;
    return path.startsWith(prefix) ? path.slice(prefix.length) : null;
  }

  private async reloadBaseline(attachment: GutterAttachment): Promise<void> {
    const provider = this.provider;
    const root = this.root;
    const relative = this.relativePath(attachment.path);
    if (provider === null || root === null || relative === null) {
      attachment.baseline = null;
      this.publishGutter(attachment);
      return;
    }
    const content = await provider.headContent(root, relative).catch(() => null);
    if (!this.gutters.has(attachment.docId)) return;
    attachment.baseline =
      content !== null && content.length <= MAX_BASELINE_CHARS ? content : null;
    this.publishGutter(attachment);
  }

  private scheduleGutter(attachment: GutterAttachment): void {
    if (attachment.timer !== null) window.clearTimeout(attachment.timer);
    attachment.timer = window.setTimeout(() => {
      attachment.timer = null;
      this.publishGutter(attachment);
    }, GUTTER_DEBOUNCE_MS);
  }

  private publishGutter(attachment: GutterAttachment): void {
    if (this.disposed || !this.gutters.has(attachment.docId)) return;
    const buffer = this.bufferOf(attachment.document);
    const text = buffer.sliceString(0, buffer.length);
    const isNewFile =
      attachment.baseline === null &&
      this.provider !== null &&
      this.relativePath(attachment.path) !== null;
    const diff =
      attachment.baseline !== null
        ? diffLines(attachment.baseline, text)
        : isNewFile
          ? diffLines("", text)
          : { lines: new Map<number, "added" | "modified">(), deletedAbove: new Set<number>() };
    Effect.runSync(attachment.visuals.update(gutterSet(diff)));
  }

  private bufferOf(document: DocumentScope): TextBuffer {
    const text = Context.get(document.context as Context.Context<TextDocument>, TextDocument);
    return Effect.runSync(text.get);
  }

  dispose(): void {
    this.disposed = true;
    this.generation++;
    this.unwatchRoot?.();
    if (this.refreshTimer !== null) window.clearTimeout(this.refreshTimer);
    for (const docId of [...this.gutters.keys()]) this.detach(docId);
  }

  private notify(): void {
    for (const listener of this.listeners) listener();
  }
}
