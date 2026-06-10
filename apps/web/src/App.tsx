import { useCallback, useEffect, useState } from "react";
import type { EditorSnapshot } from "@the-editor/editor";
import { formatChords, type Chord } from "@the-editor/keymaps";
import { AppRuntime } from "./runtime";
import { EditorView } from "./editor/EditorView";
import { makeBenchmarkText } from "./benchmarkText";

export function App() {
  // The runtime is an external resource, so the owning effect creates AND
  // disposes it: under React StrictMode's mount -> cleanup -> remount cycle,
  // each mount gets a fresh runtime instead of reusing a disposed one.
  const [runtime, setRuntime] = useState<AppRuntime | null>(null);
  const [snapshot, setSnapshot] = useState<EditorSnapshot | null>(null);
  const [pending, setPending] = useState<ReadonlyArray<Chord>>([]);

  useEffect(() => {
    const booted = AppRuntime.boot(makeBenchmarkText());
    setRuntime(booted);
    setSnapshot(booted.snapshot());
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
      <EditorView runtime={runtime} onSnapshot={handleSnapshot} onPending={handlePending} />
      <div className="status-bar">
        <span>
          Ln {line.index + 1}, Col {head - line.from + 1}
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
