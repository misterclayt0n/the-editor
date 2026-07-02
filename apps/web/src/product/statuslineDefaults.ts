/**
 * The default statusline, as registry segments. These read live host state
 * (VCS branch, focused file, cursor, diagnostics, pending chords, buffer
 * stats), so the host registers them in boot — the same "host wires its
 * defaults into a public registry" pattern the VCS gutter and LSP intel
 * providers use. An extension adds its own segment through the same
 * `StatuslineSegments` service. The shell renders the bar purely from the
 * registry.
 */

import { formatChords } from "@the-editor/keymaps";
import type { StatuslineSegment } from "@the-editor/workbench";
import type { AppRuntime, EditorPane } from "../runtime";

const fileLabel = (pane: EditorPane | undefined): string => {
  if (pane === undefined) return "";
  const info = pane.fileInfo();
  if (info === null) return "untitled";
  const name = info.path.split("/").pop() ?? info.path;
  if (info.status === "conflict") return `${name} · CONFLICT (file changed on disk)`;
  if (info.status === "missing") return `${name} · deleted on disk`;
  return info.status === "dirty" ? `${name} •` : name;
};

export function defaultStatuslineSegments(runtime: AppRuntime): readonly StatuslineSegment[] {
  return [
    {
      id: "the-editor/vcs-branch",
      side: "left",
      priority: 0,
      content: () => {
        const branch = runtime.vcs?.branch();
        return branch != null ? { text: `${branch} · `, className: "status-branch" } : null;
      },
    },
    {
      id: "the-editor/file-position",
      side: "left",
      priority: 1,
      content: () => {
        const pane = runtime.focusedPane();
        const snapshot = runtime.focusedPaneHost()?.snapshot() ?? null;
        const head = snapshot?.selection.primary.head;
        const line = snapshot !== null && head !== undefined ? snapshot.text.lineRangeAt(head) : null;
        const position =
          line !== null && head !== undefined ? ` · Ln ${line.index + 1}, Col ${head - line.from + 1}` : "";
        const label = fileLabel(pane);
        return label.length > 0 || position.length > 0 ? { text: `${label}${position}` } : null;
      },
    },
    {
      id: "the-editor/diagnostics",
      side: "left",
      priority: 2,
      content: () => {
        const host = runtime.focusedPaneHost();
        const head = host?.snapshot().selection.primary.head;
        const docId = head !== undefined ? (host?.resolveSource(head)?.docId ?? null) : null;
        const counts = docId !== null ? runtime.diagnosticsFor(docId) : null;
        if (counts === null || counts.errors + counts.warnings === 0) return null;
        const parts: string[] = [];
        if (counts.errors > 0) parts.push(`✗ ${counts.errors}`);
        if (counts.warnings > 0) parts.push(`⚠ ${counts.warnings}`);
        return { text: ` · ${parts.join(" ")}`, className: "status-diagnostics" };
      },
    },
    {
      id: "the-editor/pending-chords",
      side: "left",
      priority: 3,
      content: () => {
        const pending = runtime.pendingChords();
        return pending.length > 0 ? { text: ` · ${formatChords(pending)} …` } : null;
      },
    },
    {
      id: "the-editor/buffer-stats",
      side: "right",
      priority: 0,
      content: () => {
        const snapshot = runtime.focusedPaneHost()?.snapshot() ?? null;
        if (snapshot === null) return null;
        const dirty = snapshot.dirty ? " · modified" : "";
        return { text: `${snapshot.text.length} chars · v${snapshot.version}${dirty}` };
      },
    },
  ];
}
