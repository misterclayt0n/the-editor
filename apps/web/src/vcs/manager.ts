/**
 * The VCS manager: workspace state in, badges and gutters out. One active
 * provider per workspace root (first registered provider whose detect()
 * answers true). Repo status feeds the file tree's badges and the status bar;
 * per-document HEAD baselines feed the diff gutter — but the gutter itself is a
 * `DecorationProvider` (see `vcs/gutter.ts`), so this manager only owns repo
 * state and answers `baselineFor(path)`. It never touches a document's
 * EditorVisuals or lifecycle.
 */

import { Effect } from "effect";
import { type FileSystemPort } from "@the-editor/editor";
import type { VcsProvider, VcsProviders, VcsStatusEntry } from "@the-editor/vcs";

const STATUS_DEBOUNCE_MS = 300;
const MAX_BASELINE_CHARS = 2_000_000;

/** A document's HEAD baseline plus whether it lives under the active repo (so a
 *  tracked-but-new file diffs against empty rather than showing nothing). */
export type VcsBaseline = {
  readonly baseline: string | null;
  readonly inRepo: boolean;
};

export class VcsManager {
  private root: string | null = null;
  private provider: VcsProvider | null = null;
  private statusEntries: ReadonlyArray<VcsStatusEntry> = [];
  private branchName: string | null = null;
  private readonly listeners = new Set<() => void>();
  private refreshTimer: number | null = null;
  private unwatchRoot: (() => void) | null = null;
  private generation = 0;
  private disposed = false;

  constructor(
    private readonly providers: VcsProviders.Service,
    private readonly fileSystem: FileSystemPort.Service | undefined,
  ) {}

  /** Fires on any repo-state change (root re-anchor, status/HEAD refresh) — the
   *  gutter provider re-baselines off this, the tree/status bar re-render. */
  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => void this.listeners.delete(listener);
  }

  branch(): string | null {
    return this.branchName;
  }

  /** Dev diagnostics. */
  debugInfo(): Record<string, unknown> {
    return { root: this.root, provider: this.provider?.id ?? null };
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

  /** Unified diff of the working tree against its baseline (null = no provider / unsupported). */
  async patch(): Promise<string | null> {
    const root = this.root;
    const provider = this.provider;
    if (root === null || provider === null || this.disposed) return null;
    const generation = this.generation;
    const patch = await provider.patch(root).catch(() => null);
    return generation === this.generation && !this.disposed ? patch : null;
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
    // HEAD/root moved: let gutter providers re-baseline.
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
  }

  /** Saves refresh immediately-ish; the watcher catches everything else. */
  documentSaved(): void {
    this.scheduleRefresh();
  }

  private relativePath(path: string): string | null {
    if (this.root === null) return null;
    const prefix = this.root.endsWith("/") ? this.root : `${this.root}/`;
    return path.startsWith(prefix) ? path.slice(prefix.length) : null;
  }

  /** The HEAD baseline for a working-tree path (used by the gutter provider).
   *  `inRepo` distinguishes an untracked file (no gutter) from a new tracked
   *  one (diff against empty). Baselines over the size cap are dropped. */
  async baselineFor(path: string): Promise<VcsBaseline> {
    const provider = this.provider;
    const root = this.root;
    const relative = this.relativePath(path);
    if (provider === null || root === null || relative === null) {
      return { baseline: null, inRepo: false };
    }
    const content = await provider.headContent(root, relative).catch(() => null);
    return {
      baseline: content !== null && content.length <= MAX_BASELINE_CHARS ? content : null,
      inRepo: true,
    };
  }

  dispose(): void {
    this.disposed = true;
    this.generation++;
    this.unwatchRoot?.();
    if (this.refreshTimer !== null) window.clearTimeout(this.refreshTimer);
  }

  private notify(): void {
    for (const listener of this.listeners) listener();
  }
}
