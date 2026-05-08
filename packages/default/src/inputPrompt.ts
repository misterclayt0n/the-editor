import type { SurfaceId, SurfaceSelectionRange } from "@the-editor/contracts/runtime";
import { commandId, roleId, roleImplementationId, type EditorModule } from "@the-editor/editor";
import { getDefaultState } from "./state.js";

type InputPromptKind = "selectRegex";

export interface InputPromptRoleState {
  visible: boolean;
  kind?: InputPromptKind;
  title?: string;
  placeholder?: string;
  query: string;
  error?: string;
}

interface ActiveInputPromptState {
  visible: true;
  kind: InputPromptKind;
  title: string;
  placeholder: string;
  query: string;
  error?: string;
  surfaceId: SurfaceId;
  originalSelection: SurfaceSelectionRange[];
}

interface StoredInputPromptState {
  active?: ActiveInputPromptState;
}

const inputPromptRoleId = roleId("inputPrompt");
const inputPromptImplementationId = roleImplementationId("default.inputPrompt");
const inputPromptStateSlot = "default.inputPrompt.state";

export function defaultInputPrompt(): EditorModule {
  return (editor) => {
    editor.roles.setFallback(inputPromptRoleId, inputPromptImplementationId);

    editor.roles.register<InputPromptRoleState>({
      role: inputPromptRoleId,
      implementationId: inputPromptImplementationId,
      title: "Default Input Prompt",
      async open(_ctx, args) {
        const kind = isInputPromptKind((args as { kind?: unknown } | undefined)?.kind)
          ? (args as { kind: InputPromptKind }).kind
          : "selectRegex";
        await openInputPrompt(editor, kind);
      },
      async close() {
        await closeInputPrompt(editor, true);
      },
      async focus() {
        editor.focus.setRole(inputPromptRoleId);
      },
      snapshot() {
        const active = inputPromptState(editor).active;
        return {
          visible: Boolean(active),
          kind: active?.kind,
          title: active?.title,
          placeholder: active?.placeholder,
          query: active?.query ?? "",
          error: active?.error,
        } satisfies InputPromptRoleState;
      },
    });

    editor.commands.register<{ kind: InputPromptKind }, void>({
      id: commandId("input_prompt.open"),
      title: "Open Input Prompt",
      hidden: true,
      async run(_ctx, args) {
        if (!isInputPromptKind(args?.kind)) return;
        await openInputPrompt(editor, args.kind);
      },
    });

    editor.commands.register<void, void>({
      id: commandId("input_prompt.close"),
      title: "Close Input Prompt",
      hidden: true,
      async run() {
        await closeInputPrompt(editor, true);
      },
    });

    editor.commands.register<{ query: string }, void>({
      id: commandId("input_prompt.setQuery"),
      title: "Set Input Prompt Query",
      hidden: true,
      async run(_ctx, args) {
        const active = inputPromptState(editor).active;
        if (!active) return;
        active.query = args.query;
        await updateInputPromptPreview(editor, active);
      },
    });

    editor.commands.register<void, void>({
      id: commandId("input_prompt.accept"),
      title: "Accept Input Prompt",
      hidden: true,
      async run() {
        const active = inputPromptState(editor).active;
        if (!active) return;
        const accepted = await finalizeInputPrompt(editor, active);
        if (!accepted) return;
        await closeInputPrompt(editor, false);
      },
    });
  };
}

function inputPromptState(editor: Parameters<EditorModule>[0]): StoredInputPromptState {
  return editor.stateSlot(inputPromptStateSlot, () => ({} satisfies StoredInputPromptState));
}

async function openInputPrompt(editor: Parameters<EditorModule>[0], kind: InputPromptKind): Promise<void> {
  const surfaceId = getDefaultState(editor).activeSurfaceId;
  if (!surfaceId) return;

  const state = await editor.runtime.surfaces.readState({ surfaceId });
  const active: ActiveInputPromptState = {
    visible: true,
    kind,
    title: promptTitle(kind),
    placeholder: promptPlaceholder(kind),
    query: "",
    error: undefined,
    surfaceId,
    originalSelection: state.selection.map(cloneRange),
  };

  inputPromptState(editor).active = active;
  editor.focus.setRole(inputPromptRoleId);
}

async function closeInputPrompt(editor: Parameters<EditorModule>[0], restoreSelection: boolean): Promise<void> {
  const active = inputPromptState(editor).active;
  if (!active) return;
  if (restoreSelection) {
    await editor.runtime.surfaces.applyEdit({
      surfaceId: active.surfaceId,
      edit: { selection: active.originalSelection.map(cloneRange) },
    });
  }
  inputPromptState(editor).active = undefined;
  editor.focus.restore();
}

async function updateInputPromptPreview(
  editor: Parameters<EditorModule>[0],
  active: ActiveInputPromptState,
): Promise<void> {
  switch (active.kind) {
    case "selectRegex":
      await updateSelectRegexPreview(editor, active);
      return;
  }
}

async function finalizeInputPrompt(
  editor: Parameters<EditorModule>[0],
  active: ActiveInputPromptState,
): Promise<boolean> {
  switch (active.kind) {
    case "selectRegex":
      return finalizeSelectRegex(editor, active);
  }
}

async function updateSelectRegexPreview(
  editor: Parameters<EditorModule>[0],
  active: ActiveInputPromptState,
): Promise<void> {
  if (!active.query) {
    await editor.runtime.surfaces.applyEdit({
      surfaceId: active.surfaceId,
      edit: { selection: active.originalSelection.map(cloneRange) },
    });
    active.error = undefined;
    return;
  }

  const regex = buildSmartCaseRegex(active.query);
  if (regex instanceof Error) {
    active.error = regex.message;
    return;
  }

  active.error = undefined;
  const state = await editor.runtime.surfaces.readState({ surfaceId: active.surfaceId });
  const nextSelection = selectRegexMatches(state.text, active.originalSelection, regex);
  if (nextSelection instanceof Error) {
    active.error = nextSelection.message;
    return;
  }
  if (!nextSelection) return;

  await editor.runtime.surfaces.applyEdit({
    surfaceId: active.surfaceId,
    edit: { selection: nextSelection },
  });
}

async function finalizeSelectRegex(
  editor: Parameters<EditorModule>[0],
  active: ActiveInputPromptState,
): Promise<boolean> {
  if (!active.query) return true;

  const regex = buildSmartCaseRegex(active.query);
  if (regex instanceof Error) {
    active.error = regex.message;
    return false;
  }

  const state = await editor.runtime.surfaces.readState({ surfaceId: active.surfaceId });
  const nextSelection = selectRegexMatches(state.text, active.originalSelection, regex);
  if (nextSelection instanceof Error) {
    active.error = nextSelection.message;
    return false;
  }
  if (!nextSelection?.length) {
    active.error = "nothing selected";
    return false;
  }

  active.error = undefined;
  await editor.runtime.surfaces.applyEdit({
    surfaceId: active.surfaceId,
    edit: { selection: nextSelection },
  });
  return true;
}

function buildSmartCaseRegex(query: string): RegExp | Error {
  try {
    const flags = containsUppercase(query) ? "gmu" : "gimu";
    return new RegExp(query, flags);
  } catch (error) {
    return error instanceof Error ? error : new Error(String(error));
  }
}

function selectRegexMatches(
  text: string,
  selection: readonly SurfaceSelectionRange[],
  regex: RegExp,
): SurfaceSelectionRange[] | undefined | Error {
  const chars = Array.from(text);
  const ranges: SurfaceSelectionRange[] = [];
  let nextCursorId = selection.reduce((max, range) => Math.max(max, range.cursorId), 0) + 1;

  for (const cursor of selection) {
    const from = Math.min(cursor.anchor, cursor.head);
    const to = Math.max(cursor.anchor, cursor.head);
    const segment = chars.slice(from, to).join("");
    const startIndex = ranges.length;
    let assigned = false;

    for (const match of segment.matchAll(new RegExp(regex.source, regex.flags))) {
      const matchText = match[0] ?? "";
      const matchStart = from + charCount(segment.slice(0, match.index ?? 0));
      const matchEnd = matchStart + charCount(matchText);
      if (matchStart === to && matchEnd === to) continue;

      const range = orientRange(cursor, matchStart, matchEnd);
      const cursorId = !assigned && rangeContainsInclusive(range, cursor.head)
        ? (assigned = true, cursor.cursorId)
        : nextCursorId++;
      ranges.push({ cursorId, ...range });
    }

    if (ranges.length > startIndex && !assigned) {
      ranges[startIndex] = { ...ranges[startIndex]!, cursorId: cursor.cursorId };
    }
  }

  if (!ranges.length) return undefined;
  return ranges;
}

function orientRange(cursor: SurfaceSelectionRange, start: number, end: number): Pick<SurfaceSelectionRange, "anchor" | "head"> {
  return cursor.anchor <= cursor.head
    ? { anchor: start, head: end }
    : { anchor: end, head: start };
}

function rangeContainsInclusive(range: Pick<SurfaceSelectionRange, "anchor" | "head">, pos: number): boolean {
  const from = Math.min(range.anchor, range.head);
  const to = Math.max(range.anchor, range.head);
  return from <= pos && pos <= to;
}

function cloneRange(range: SurfaceSelectionRange): SurfaceSelectionRange {
  return { ...range };
}

function charCount(value: string): number {
  return Array.from(value).length;
}

function containsUppercase(value: string): boolean {
  return Array.from(value).some((ch) => ch.toLocaleLowerCase() !== ch.toLocaleUpperCase() && ch === ch.toLocaleUpperCase());
}

function promptTitle(kind: InputPromptKind): string {
  switch (kind) {
    case "selectRegex":
      return "SELECT";
  }
}

function promptPlaceholder(kind: InputPromptKind): string {
  switch (kind) {
    case "selectRegex":
      return "Regular expression";
  }
}

function isInputPromptKind(value: unknown): value is InputPromptKind {
  return value === "selectRegex";
}
