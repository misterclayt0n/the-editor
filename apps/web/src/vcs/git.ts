/**
 * The git provider: plain `git` subprocesses over the ProcessPort (the
 * same seam language servers ride — no native library, no extra surface).
 * Porcelain -z output is NUL-delimited and stable across locales.
 */

import type { ProcessPort } from "../platform/process";
import type { VcsFileStatus, VcsProvider, VcsStatusEntry } from "@the-editor/vcs";

type RunResult = { code: number | null; stdout: string };

const decoder = new TextDecoder();

async function run(
  port: ProcessPort,
  cwd: string,
  args: readonly string[],
): Promise<RunResult | null> {
  const id = await port.spawn({ command: "git", args: [...args], cwd });
  if (id === null) return null;
  return new Promise((resolve) => {
    const chunks: Uint8Array[] = [];
    port.onData(id, (chunk, stream) => {
      if (stream === "stdout") chunks.push(chunk);
    });
    port.onExit(id, (code) => {
      const total = chunks.reduce((n, c) => n + c.length, 0);
      const merged = new Uint8Array(total);
      let at = 0;
      for (const chunk of chunks) {
        merged.set(chunk, at);
        at += chunk.length;
      }
      resolve({ code, stdout: decoder.decode(merged) });
    });
  });
}

/** Porcelain v1 XY → tree badge status (worktree column wins). */
function statusOf(xy: string): VcsFileStatus | null {
  if (xy === "??") return "untracked";
  if (xy === "!!") return "ignored";
  const x = xy[0] ?? " ";
  const y = xy[1] ?? " ";
  if (y === "D" || x === "D") return "deleted";
  if (x === "R" || y === "R") return "renamed";
  if (x === "A" && y === " ") return "added";
  if (x === "A" || y === "A") return "added";
  if (x === "M" || y === "M" || x === "T" || y === "T") return "modified";
  return null;
}

export function createGitProvider(port: ProcessPort): VcsProvider {
  return {
    id: "git",

    async detect(root) {
      const result = await run(port, root, ["rev-parse", "--is-inside-work-tree"]);
      return result?.code === 0 && result.stdout.trim() === "true";
    },

    async status(root) {
      const result = await run(port, root, [
        "status",
        "--porcelain",
        "-z",
        "--untracked-files=all",
      ]);
      if (result === null || result.code !== 0) return [];
      const fields = result.stdout.split("\0");
      const entries: VcsStatusEntry[] = [];
      for (let i = 0; i < fields.length; i++) {
        const field = fields[i]!;
        if (field.length < 4) continue;
        const xy = field.slice(0, 2);
        const path = field.slice(3);
        const status = statusOf(xy);
        // Renames carry the ORIGINAL path in the next NUL field.
        if (xy[0] === "R" || xy[0] === "C") i++;
        if (status !== null && path.length > 0) entries.push({ path, status });
      }
      return entries;
    },

    async headContent(root, relPath) {
      const result = await run(port, root, ["show", `HEAD:${relPath}`]);
      return result !== null && result.code === 0 ? result.stdout : null;
    },

    async listFiles(root) {
      const result = await run(port, root, [
        "ls-files",
        "--cached",
        "--others",
        "--exclude-standard",
        "-z",
      ]);
      if (result === null || result.code !== 0) return null;
      return result.stdout.split("\0").filter((path) => path.length > 0);
    },

    async patch(root) {
      const tracked = await run(port, root, ["diff", "HEAD"]);
      if (tracked === null || tracked.code !== 0) return null;
      const untracked = await run(port, root, ["ls-files", "--others", "--exclude-standard", "-z"]);
      if (untracked === null || untracked.code !== 0) return tracked.stdout;
      const chunks = [tracked.stdout];
      for (const path of untracked.stdout.split("\0")) {
        if (path.length === 0) continue;
        const added = await run(port, root, ["diff", "--no-index", "--", "/dev/null", path]);
        // `git diff --no-index` exits 1 when it successfully found differences.
        if (added !== null && (added.code === 0 || added.code === 1)) chunks.push(added.stdout);
      }
      return chunks.join("");
    },

    async info(root) {
      const result = await run(port, root, ["rev-parse", "--abbrev-ref", "HEAD"]);
      if (result === null || result.code !== 0) return null;
      return { branch: result.stdout.trim() };
    },
  };
}
