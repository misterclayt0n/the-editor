import type { CursorRangeSpec, SelectionSpec, SurfaceId, SurfaceSnapshot } from "@the-editor/contracts/runtime";
import { commandId, type EditorHost, type EditorModule } from "@the-editor/editor";
import { getDefaultState, setEditorMode } from "./state.js";

export function defaultEditing(): EditorModule {
  return (editor) => {
    editor.commands.register({ id: commandId("mode.normal"), title: "Normal Mode", run: () => setEditorMode(editor, "normal") });
    editor.commands.register({ id: commandId("mode.insert"), title: "Insert Mode", run: () => setEditorMode(editor, "insert") });

    registerSelectionCommand(editor, "move.charLeft", (snapshot) => movePrimary(snapshot, -1));
    registerSelectionCommand(editor, "move.charRight", (snapshot) => movePrimary(snapshot, 1));
    registerSelectionCommand(editor, "move.visualLineUp", (snapshot) => moveLine(snapshot, -1));
    registerSelectionCommand(editor, "move.visualLineDown", (snapshot) => moveLine(snapshot, 1));
    registerSelectionCommand(editor, "goto.lineStart", lineStart);
    registerSelectionCommand(editor, "goto.lineEnd", lineEnd);
    registerSelectionCommand(editor, "goto.documentStart", documentStart);
    registerSelectionCommand(editor, "goto.documentEnd", documentEnd);

    editor.commands.register<{ text: string }, void>({
      id: commandId("edit.insertText"),
      title: "Insert Text",
      async run(_ctx, args) {
        await applyTextReplacement(editor, args.text);
      },
    });

    editor.commands.register({
      id: commandId("edit.insertNewline"),
      title: "Insert Newline",
      async run() {
        await applyTextReplacement(editor, "\n");
      },
    });

    editor.commands.register({
      id: commandId("edit.deleteBackward"),
      title: "Delete Backward",
      async run() {
        await deleteRelative(editor, -1);
      },
    });

    editor.commands.register({
      id: commandId("edit.deleteForward"),
      title: "Delete Forward",
      async run() {
        await deleteRelative(editor, 1);
      },
    });
  };
}

function registerSelectionCommand(
  editor: EditorHost,
  id: string,
  select: (snapshot: SurfaceSnapshot) => SelectionSpec,
): void {
  editor.commands.register({
    id: commandId(id),
    title: id,
    async run() {
      const { surfaceId, snapshot } = await activeSnapshot(editor);
      await editor.runtime.surfaces.setSelection({ surfaceId, selection: select(snapshot) });
    },
  });
}

async function applyTextReplacement(editor: EditorHost, text: string): Promise<void> {
  const { surfaceId, snapshot } = await activeSnapshot(editor);
  const range = primaryRange(snapshot);
  const start = Math.min(range.anchor, range.head);
  const end = Math.max(range.anchor, range.head);
  const next = start + [...text].length;
  await editor.runtime.surfaces.applyTransaction({
    surfaceId,
    transaction: { replacements: [{ start, end, text }] },
    selection: selectionWithPrimary(snapshot, { ...range, anchor: next, head: next }),
  });
}

async function deleteRelative(editor: EditorHost, direction: -1 | 1): Promise<void> {
  const { surfaceId, snapshot } = await activeSnapshot(editor);
  const range = primaryRange(snapshot);
  let start = Math.min(range.anchor, range.head);
  let end = Math.max(range.anchor, range.head);
  if (start === end) {
    if (direction < 0) start = Math.max(0, start - 1);
    else end = Math.min([...snapshot.text].length, end + 1);
  }
  if (start === end) return;
  await editor.runtime.surfaces.applyTransaction({
    surfaceId,
    transaction: { replacements: [{ start, end }] },
    selection: selectionWithPrimary(snapshot, { ...range, anchor: start, head: start }),
  });
}

async function activeSnapshot(editor: EditorHost): Promise<{ surfaceId: SurfaceId; snapshot: SurfaceSnapshot }> {
  const state = getDefaultState(editor);
  if (!state.activeSurfaceId) throw new Error("no active surface");
  return {
    surfaceId: state.activeSurfaceId,
    snapshot: await editor.runtime.surfaces.snapshot({ surfaceId: state.activeSurfaceId }),
  };
}

function primaryRange(snapshot: SurfaceSnapshot): CursorRangeSpec {
  return snapshot.selection.ranges.find((range) => range.id === snapshot.activeCursorId) ?? snapshot.selection.ranges[0]!;
}

function selectionWithPrimary(snapshot: SurfaceSnapshot, primary: CursorRangeSpec): SelectionSpec {
  const current = primaryRange(snapshot);
  return {
    ranges: snapshot.selection.ranges.map((range) => (range === current ? primary : range)),
  };
}

function movePrimary(snapshot: SurfaceSnapshot, delta: number): SelectionSpec {
  const range = primaryRange(snapshot);
  const pos = clamp(range.head + delta, 0, [...snapshot.text].length);
  return selectionWithPrimary(snapshot, { ...range, anchor: pos, head: pos });
}

function lineStart(snapshot: SurfaceSnapshot): SelectionSpec {
  const range = primaryRange(snapshot);
  const pos = snapshot.text.lastIndexOf("\n", Math.max(0, range.head - 1)) + 1;
  return selectionWithPrimary(snapshot, { ...range, anchor: pos, head: pos });
}

function lineEnd(snapshot: SurfaceSnapshot): SelectionSpec {
  const range = primaryRange(snapshot);
  const next = snapshot.text.indexOf("\n", range.head);
  const pos = next < 0 ? [...snapshot.text].length : next;
  return selectionWithPrimary(snapshot, { ...range, anchor: pos, head: pos });
}

function documentStart(snapshot: SurfaceSnapshot): SelectionSpec {
  const range = primaryRange(snapshot);
  return selectionWithPrimary(snapshot, { ...range, anchor: 0, head: 0 });
}

function documentEnd(snapshot: SurfaceSnapshot): SelectionSpec {
  const range = primaryRange(snapshot);
  const pos = [...snapshot.text].length;
  return selectionWithPrimary(snapshot, { ...range, anchor: pos, head: pos });
}

function moveLine(snapshot: SurfaceSnapshot, delta: -1 | 1): SelectionSpec {
  const range = primaryRange(snapshot);
  const lines = snapshot.text.split("\n");
  let remaining = range.head;
  let line = 0;
  while (line < lines.length && remaining > lines[line]!.length) {
    remaining -= lines[line]!.length + 1;
    line++;
  }
  const col = remaining;
  const nextLine = clamp(line + delta, 0, lines.length - 1);
  const pos = lines.slice(0, nextLine).reduce((sum, text) => sum + text.length + 1, 0) + Math.min(col, lines[nextLine]!.length);
  return selectionWithPrimary(snapshot, { ...range, anchor: pos, head: pos });
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}
