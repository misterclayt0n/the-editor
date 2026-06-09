import { useCallback, useEffect, useMemo, useState } from "react";
import type { EditorSnapshot } from "@the-editor/editor";
import { EditorSession } from "./editor/session";
import { EditorView } from "./editor/EditorView";

const SAMPLE = `// The Editor — first vertical slice
//
// Type printable characters, Enter, Backspace, Delete, Tab.
// Move with arrow keys, Home/End. Shift extends the selection.
// Click to place the caret.

function hello(name: string): string {
\treturn \`hello, \${name}\`;
}

hello("world");
`;

export function App() {
  const session = useMemo(() => EditorSession.create(SAMPLE), []);
  const [snapshot, setSnapshot] = useState<EditorSnapshot>(() => session.snapshot());

  useEffect(() => () => session.dispose(), [session]);

  const handleSnapshot = useCallback((next: EditorSnapshot) => setSnapshot(next), []);

  const head = snapshot.selection.primary.head;
  const line = snapshot.text.lineRangeAt(head);

  return (
    <div className="app">
      <EditorView session={session} onSnapshot={handleSnapshot} />
      <div className="status-bar">
        <span>
          Ln {line.index + 1}, Col {head - line.from + 1}
        </span>
        <span>
          {snapshot.text.length} chars · v{snapshot.version}
          {snapshot.dirty ? " · modified" : ""}
        </span>
      </div>
    </div>
  );
}
