import { useCallback, useEffect, useState } from "react";
import type { EditorSnapshot } from "@the-editor/editor";
import { formatChords, type Chord } from "@the-editor/keymaps";
import { AppRuntime } from "./runtime";
import { EditorView } from "./editor/EditorView";

const fileLabel = (runtime: AppRuntime): string => {
  const info = runtime.fileInfo();
  if (info === null) return "untitled";
  const name = info.path.split("/").pop() ?? info.path;
  if (info.status === "conflict") return `${name} · CONFLICT (file changed on disk)`;
  if (info.status === "missing") return `${name} · deleted on disk`;
  return info.status === "dirty" ? `${name} •` : name;
};

export function App() {
  // The runtime is an external resource, so the owning effect creates AND
  // disposes it: under React StrictMode's mount -> cleanup -> remount cycle,
  // each mount gets a fresh runtime instead of reusing a disposed one.
  const [runtime, setRuntime] = useState<AppRuntime | null>(null);
  const [snapshot, setSnapshot] = useState<EditorSnapshot | null>(null);
  const [pending, setPending] = useState<ReadonlyArray<Chord>>([]);
  // Bumped when the active document changes: remounts the editor view.
  const [viewEpoch, setViewEpoch] = useState(0);

  useEffect(() => {
    const booted = AppRuntime.boot("");
    booted.onDocumentSwitched = () => {
      setSnapshot(booted.snapshot());
      setViewEpoch((epoch) => epoch + 1);
    };
    booted.onFileStateChanged = () => setSnapshot(booted.snapshot());
    setRuntime(booted);
    setSnapshot(booted.snapshot());
    const isDev = location.hostname === "localhost" || location.hostname === "127.0.0.1";
    if (isDev) {
      (window as unknown as Record<string, unknown>).__editorHost = {
        open: (path: string) => booted.openPath(path),
        save: () => booted.saveActiveFile(),
        info: () => booted.fileInfo(),
      };
    }
    return () => {
      setRuntime(null);
      booted.dispose();
    };
  }, []);

  const handleSnapshot = useCallback((next: EditorSnapshot) => setSnapshot(next), []);
  const handlePending = useCallback((chords: ReadonlyArray<Chord>) => setPending(chords), []);

  if (runtime === null || snapshot === null) return <div className="app" />;

  const head = snapshot.selection.primary.head;
  const line = snapshot.text.lineRangeAt(head);

  return (
    <div className="app">
      <EditorView
        key={viewEpoch}
        runtime={runtime}
        onSnapshot={handleSnapshot}
        onPending={handlePending}
      />
      <div className="status-bar">
        <span>
          {fileLabel(runtime)} · Ln {line.index + 1}, Col {head - line.from + 1}
          {pending.length > 0 ? ` · ${formatChords(pending)} …` : ""}
        </span>
        <span>
          {snapshot.text.length} chars · v{snapshot.version}
          {snapshot.dirty ? " · modified" : ""}
        </span>
      </div>
    </div>
  );
}
