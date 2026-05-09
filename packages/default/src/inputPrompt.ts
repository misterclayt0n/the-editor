import { spawnSync } from "node:child_process";
import type { SurfaceChange, SurfaceEdit, SurfaceId, SurfaceSelectionRange } from "@the-editor/contracts/runtime";
import { commandId, roleId, roleImplementationId, type EditorModule } from "@the-editor/editor";
import { getDefaultState } from "./state.js";

type InputPromptKind =
  | "selectRegex"
  | "splitSelection"
  | "keepSelections"
  | "removeSelections"
  | "searchForward"
  | "searchBackward"
  | "shellPipe"
  | "shellPipeTo"
  | "shellInsertOutput"
  | "shellAppendOutput"
  | "shellKeepPipe";
type SearchDirection = "forward" | "backward";

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
  extend: boolean;
}

interface StoredInputPromptState {
  active?: ActiveInputPromptState;
}

const inputPromptRoleId = roleId("inputPrompt");
const inputPromptImplementationId = roleImplementationId("default.inputPrompt");
const inputPromptStateSlot = "default.inputPrompt.state";
type ShellBehavior = "replace" | "ignore" | "insert" | "append" | "keep";

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

    editor.commands.register({
      id: commandId("search"),
      title: "Search",
      async run() {
        await openInputPrompt(editor, "searchForward");
      },
    });

    editor.commands.register({
      id: commandId("rsearch"),
      title: "Search Backward",
      async run() {
        await openInputPrompt(editor, "searchBackward");
      },
    });

    editor.commands.register<{ count?: number }, void>({
      id: commandId("search_next"),
      title: "Search Next",
      async run(_ctx, args) {
        await repeatStoredSearch(editor, "forward", false, args?.count);
      },
    });

    editor.commands.register<{ count?: number }, void>({
      id: commandId("search_prev"),
      title: "Search Prev",
      async run(_ctx, args) {
        await repeatStoredSearch(editor, "backward", false, args?.count);
      },
    });

    editor.commands.register<{ count?: number }, void>({
      id: commandId("extend_search_next"),
      title: "Extend Search Next",
      async run(_ctx, args) {
        await repeatStoredSearch(editor, "forward", true, args?.count);
      },
    });

    editor.commands.register<{ count?: number }, void>({
      id: commandId("extend_search_prev"),
      title: "Extend Search Prev",
      async run(_ctx, args) {
        await repeatStoredSearch(editor, "backward", true, args?.count);
      },
    });

    editor.commands.register({ id: commandId("shell_pipe"), title: "Shell Pipe", async run() { await openInputPrompt(editor, "shellPipe"); } });
    editor.commands.register({ id: commandId("shell_pipe_to"), title: "Shell Pipe To", async run() { await openInputPrompt(editor, "shellPipeTo"); } });
    editor.commands.register({ id: commandId("shell_insert_output"), title: "Shell Insert Output", async run() { await openInputPrompt(editor, "shellInsertOutput"); } });
    editor.commands.register({ id: commandId("shell_append_output"), title: "Shell Append Output", async run() { await openInputPrompt(editor, "shellAppendOutput"); } });
    editor.commands.register({ id: commandId("shell_keep_pipe"), title: "Shell Keep Pipe", async run() { await openInputPrompt(editor, "shellKeepPipe"); } });

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
    extend: isSearchPromptKind(kind) && getDefaultState(editor).mode === "select",
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
    case "splitSelection":
      await updateSplitSelectionPreview(editor, active);
      return;
    case "keepSelections":
      await updateKeepOrRemoveSelectionsPreview(editor, active, false);
      return;
    case "removeSelections":
      await updateKeepOrRemoveSelectionsPreview(editor, active, true);
      return;
    case "searchForward":
      await updateSearchPreview(editor, active, "forward");
      return;
    case "searchBackward":
      await updateSearchPreview(editor, active, "backward");
      return;
    case "shellPipe":
    case "shellPipeTo":
    case "shellInsertOutput":
    case "shellAppendOutput":
    case "shellKeepPipe":
      active.error = undefined;
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
    case "splitSelection":
      return finalizeSplitSelection(editor, active);
    case "keepSelections":
      return finalizeKeepOrRemoveSelections(editor, active, false);
    case "removeSelections":
      return finalizeKeepOrRemoveSelections(editor, active, true);
    case "searchForward":
    case "searchBackward":
      return finalizeSearch(editor, active);
    case "shellPipe":
      return finalizeShellPrompt(editor, active, "replace");
    case "shellPipeTo":
      return finalizeShellPrompt(editor, active, "ignore");
    case "shellInsertOutput":
      return finalizeShellPrompt(editor, active, "insert");
    case "shellAppendOutput":
      return finalizeShellPrompt(editor, active, "append");
    case "shellKeepPipe":
      return finalizeShellPrompt(editor, active, "keep");
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

async function updateSplitSelectionPreview(
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
  const nextSelection = splitSelectionMatches(state.text, active.originalSelection, regex);
  await editor.runtime.surfaces.applyEdit({
    surfaceId: active.surfaceId,
    edit: { selection: nextSelection },
  });
}

async function finalizeSplitSelection(
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
  const nextSelection = splitSelectionMatches(state.text, active.originalSelection, regex);
  active.error = undefined;
  await editor.runtime.surfaces.applyEdit({
    surfaceId: active.surfaceId,
    edit: { selection: nextSelection },
  });
  return true;
}

async function updateKeepOrRemoveSelectionsPreview(
  editor: Parameters<EditorModule>[0],
  active: ActiveInputPromptState,
  remove: boolean,
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
  const nextSelection = keepOrRemoveSelections(state.text, active.originalSelection, regex, remove);
  if (!nextSelection?.length) return;

  await editor.runtime.surfaces.applyEdit({
    surfaceId: active.surfaceId,
    edit: { selection: nextSelection },
  });
}

async function finalizeKeepOrRemoveSelections(
  editor: Parameters<EditorModule>[0],
  active: ActiveInputPromptState,
  remove: boolean,
): Promise<boolean> {
  if (!active.query) return true;

  const regex = buildSmartCaseRegex(active.query);
  if (regex instanceof Error) {
    active.error = regex.message;
    return false;
  }

  const state = await editor.runtime.surfaces.readState({ surfaceId: active.surfaceId });
  const nextSelection = keepOrRemoveSelections(state.text, active.originalSelection, regex, remove);
  if (!nextSelection?.length) {
    active.error = "no selections remaining";
    return false;
  }

  active.error = undefined;
  await editor.runtime.surfaces.applyEdit({
    surfaceId: active.surfaceId,
    edit: { selection: nextSelection },
  });
  return true;
}

async function updateSearchPreview(
  editor: Parameters<EditorModule>[0],
  active: ActiveInputPromptState,
  direction: SearchDirection,
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
  const nextSelection = searchRegexSelection(state.text, active.originalSelection, regex, direction, active.extend);
  if (!nextSelection) return;

  await editor.runtime.surfaces.applyEdit({
    surfaceId: active.surfaceId,
    edit: { selection: nextSelection },
  });
}

async function finalizeSearch(
  editor: Parameters<EditorModule>[0],
  active: ActiveInputPromptState,
): Promise<boolean> {
  if (!active.query) return true;

  const regex = buildSmartCaseRegex(active.query);
  if (regex instanceof Error) {
    active.error = regex.message;
    return false;
  }

  active.error = undefined;
  await editor.runtime.registers.write({ name: activeSearchRegister(editor), values: [active.query] });

  const state = await editor.runtime.surfaces.readState({ surfaceId: active.surfaceId });
  const nextSelection = searchRegexSelection(
    state.text,
    active.originalSelection,
    regex,
    searchDirectionForKind(active.kind) ?? "forward",
    active.extend,
  );
  if (!nextSelection) return true;

  await editor.runtime.surfaces.applyEdit({
    surfaceId: active.surfaceId,
    edit: { selection: nextSelection },
  });
  return true;
}

async function repeatStoredSearch(
  editor: Parameters<EditorModule>[0],
  direction: SearchDirection,
  extend: boolean,
  count?: number,
): Promise<void> {
  const surfaceId = getDefaultState(editor).activeSurfaceId;
  if (!surfaceId) return;

  const { values } = await editor.runtime.registers.read({ name: activeSearchRegister(editor) });
  const query = values?.[0];
  if (!query) return;

  const regex = buildSmartCaseRegex(query);
  if (regex instanceof Error) return;

  for (let remaining = Math.max(1, count ?? 1); remaining > 0; remaining -= 1) {
    const state = await editor.runtime.surfaces.readState({ surfaceId });
    const nextSelection = searchRegexSelection(state.text, state.selection, regex, direction, extend);
    if (!nextSelection) return;
    await editor.runtime.surfaces.applyEdit({
      surfaceId,
      edit: { selection: nextSelection },
    });
  }
}

async function finalizeShellPrompt(
  editor: Parameters<EditorModule>[0],
  active: ActiveInputPromptState,
  behavior: ShellBehavior,
): Promise<boolean> {
  const command = active.query.trim();
  if (!command) {
    active.error = "shell command cannot be empty";
    return false;
  }

  try {
    await applyShellBehavior(editor, active.surfaceId, command, behavior);
    active.error = undefined;
    return true;
  } catch (error) {
    active.error = error instanceof Error ? error.message : String(error);
    return false;
  }
}

async function applyShellBehavior(
  editor: Parameters<EditorModule>[0],
  surfaceId: SurfaceId,
  command: string,
  behavior: ShellBehavior,
): Promise<void> {
  const state = await editor.runtime.surfaces.readState({ surfaceId });

  if (behavior === "keep") {
    const selection = await keepPipeSelections(state, command);
    await editor.runtime.surfaces.applyEdit({ surfaceId, edit: { selection } });
    return;
  }

  const chars = Array.from(state.text);
  const pipe = behavior === "replace" || behavior === "ignore";
  const changes: SurfaceChange[] = [];
  const selection: SurfaceSelectionRange[] = [];
  let sharedOutput: string | undefined;
  let offset = 0;

  for (const range of state.selection) {
    const from = Math.min(range.anchor, range.head);
    const to = Math.max(range.anchor, range.head);
    const inputText = chars.slice(from, to).join("");
    const output = pipe
      ? trimPipeTrailingNewline(inputText, runShellCommand(command, inputText))
      : (sharedOutput ??= runShellCommand(command));

    if (behavior === "ignore") continue;

    const outputLength = charCount(output);
    const changeStart = behavior === "insert" ? from : behavior === "append" ? to : from;
    const changeEnd = behavior === "replace" ? to : changeStart;
    const deletedLength = changeEnd - changeStart;
    const anchor = changeEnd + offset - deletedLength;

    changes.push({ start: changeStart, end: changeEnd, text: output });
    selection.push({ cursorId: range.cursorId, ...orientRange(range, anchor, anchor + outputLength) });
    offset += outputLength - deletedLength;
  }

  if (behavior === "ignore") return;
  if (!changes.length) return;

  const edit: SurfaceEdit = { changes, selection };
  await editor.runtime.surfaces.applyEdit({ surfaceId, edit });
  await editor.runtime.surfaces.commitUndoCheckpoint({ surfaceId });
}

async function keepPipeSelections(
  state: { text: string; selection: SurfaceSelectionRange[] },
  command: string,
): Promise<SurfaceSelectionRange[]> {
  const chars = Array.from(state.text);
  const selection = state.selection.filter((range) => {
    const from = Math.min(range.anchor, range.head);
    const to = Math.max(range.anchor, range.head);
    try {
      runShellCommand(command, chars.slice(from, to).join(""));
      return true;
    } catch {
      return false;
    }
  }).map(cloneRange);

  if (!selection.length) throw new Error("No selections remaining");
  return selection;
}

function trimPipeTrailingNewline(inputText: string, output: string): string {
  if (inputText.endsWith("\n") || !output.endsWith("\n")) return output;
  const trimmed = output.slice(0, -1);
  return trimmed.endsWith("\r") ? trimmed.slice(0, -1) : trimmed;
}

function runShellCommand(command: string, input?: string): string {
  const [shell, ...args] = shellProgram();
  const output = spawnSync(shell, [...args, command], {
    input,
    encoding: "utf8",
    maxBuffer: 10 * 1024 * 1024,
  });

  if (output.error) throw new Error(`Failed to start shell: ${output.error.message}`);
  if ((output.status ?? 0) !== 0) {
    const stderr = (output.stderr ?? "").trimEnd();
    if (stderr) throw new Error(stderr);
    throw new Error(output.status === null ? "Shell command failed" : `Shell command failed: status ${output.status}`);
  }

  const stderr = output.stderr ?? "";
  return stderr.length ? stderr : (output.stdout ?? "");
}

function shellProgram(): string[] {
  if (process.platform === "win32") {
    const shell = process.env.COMSPEC?.trim() || "cmd.exe";
    return [shell, "/C"];
  }
  const shell = process.env.SHELL?.trim() || "/bin/sh";
  return [shell, "-c"];
}

function activeSearchRegister(editor: Parameters<EditorModule>[0]): string {
  return getDefaultState(editor).currentRegister || "/";
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

function splitSelectionMatches(
  text: string,
  selection: readonly SurfaceSelectionRange[],
  regex: RegExp,
): SurfaceSelectionRange[] {
  const chars = Array.from(text);
  const ranges: SurfaceSelectionRange[] = [];
  let nextId = nextCursorId(selection);

  for (const cursor of selection) {
    if (cursor.anchor === cursor.head) {
      ranges.push(cloneRange(cursor));
      continue;
    }

    const from = Math.min(cursor.anchor, cursor.head);
    const to = Math.max(cursor.anchor, cursor.head);
    const segment = chars.slice(from, to).join("");
    const startIndex = ranges.length;
    let assigned = false;
    let start = from;

    for (const match of segment.matchAll(new RegExp(regex.source, regex.flags))) {
      const matchText = match[0] ?? "";
      const split = from + charCount(segment.slice(0, match.index ?? 0));
      if (start !== split) {
        const piece = orientRange(cursor, start, split);
        const cursorId = !assigned && rangeContainsInclusive(piece, cursor.head)
          ? (assigned = true, cursor.cursorId)
          : nextId++;
        ranges.push({ cursorId, ...piece });
      }
      start = split + charCount(matchText);
    }

    if (start < to) {
      const piece = orientRange(cursor, start, to);
      const cursorId = !assigned && rangeContainsInclusive(piece, cursor.head)
        ? (assigned = true, cursor.cursorId)
        : nextId++;
      ranges.push({ cursorId, ...piece });
    }

    if (ranges.length === startIndex) {
      ranges.push({ cursorId: cursor.cursorId, anchor: cursor.head, head: cursor.head });
    } else if (!assigned) {
      ranges[startIndex] = { ...ranges[startIndex]!, cursorId: cursor.cursorId };
    }
  }

  return ranges;
}

function keepOrRemoveSelections(
  text: string,
  selection: readonly SurfaceSelectionRange[],
  regex: RegExp,
  remove: boolean,
): SurfaceSelectionRange[] | undefined {
  const chars = Array.from(text);
  const filtered = selection.filter((range) => {
    const from = Math.min(range.anchor, range.head);
    const to = Math.max(range.anchor, range.head);
    const segment = chars.slice(from, to).join("");
    return new RegExp(regex.source, regex.flags).test(segment) !== remove;
  });

  return filtered.length ? filtered.map(cloneRange) : undefined;
}

function searchRegexSelection(
  text: string,
  selection: readonly SurfaceSelectionRange[],
  regex: RegExp,
  direction: SearchDirection,
  extend: boolean,
): SurfaceSelectionRange[] | undefined {
  if (!selection.length) return undefined;

  const matches = collectRegexMatches(text, regex);
  if (!matches.length) return undefined;

  const pickedIndex = extend
    ? (direction === "forward" ? selection.length - 1 : 0)
    : 0;
  const picked = selection[pickedIndex];
  if (!picked) return undefined;

  const from = Math.min(picked.anchor, picked.head);
  const to = Math.max(picked.anchor, picked.head);
  const match = direction === "forward"
    ? findNextMatch(matches, to) ?? matches[0]
    : findPrevMatch(matches, from) ?? matches.at(-1);
  if (!match) return undefined;

  if (extend) {
    return [...selection.map(cloneRange), { cursorId: nextCursorId(selection), anchor: match.start, head: match.end }];
  }

  return selection.map((range, index) => index === pickedIndex
    ? { cursorId: range.cursorId, anchor: match.start, head: match.end }
    : cloneRange(range));
}

function collectRegexMatches(text: string, regex: RegExp): Array<{ start: number; end: number }> {
  const matches: Array<{ start: number; end: number }> = [];
  let previousCodeUnitIndex = 0;
  let previousCharIndex = 0;
  const globalRegex = new RegExp(regex.source, regex.flags.includes("g") ? regex.flags : `${regex.flags}g`);

  for (const match of text.matchAll(globalRegex)) {
    const matchText = match[0] ?? "";
    if (!matchText) continue;

    const codeUnitIndex = match.index ?? 0;
    const start = previousCharIndex + charCount(text.slice(previousCodeUnitIndex, codeUnitIndex));
    const end = start + charCount(matchText);
    matches.push({ start, end });
    previousCodeUnitIndex = codeUnitIndex + matchText.length;
    previousCharIndex = end;
  }

  return matches;
}

function findNextMatch(
  matches: Array<{ start: number; end: number }>,
  start: number,
): { start: number; end: number } | undefined {
  return matches.find((match) => match.start >= start);
}

function findPrevMatch(
  matches: Array<{ start: number; end: number }>,
  start: number,
): { start: number; end: number } | undefined {
  for (let index = matches.length - 1; index >= 0; index -= 1) {
    const match = matches[index];
    if (match && match.end <= start) return match;
  }
  return undefined;
}

function nextCursorId(selection: readonly SurfaceSelectionRange[]): number {
  return selection.reduce((max, range) => Math.max(max, range.cursorId), 0) + 1;
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
    case "splitSelection":
      return "SPLIT";
    case "keepSelections":
      return "KEEP";
    case "removeSelections":
      return "REMOVE";
    case "searchForward":
      return "/";
    case "searchBackward":
      return "?";
    case "shellPipe":
      return "PIPE";
    case "shellPipeTo":
      return "PIPE TO";
    case "shellInsertOutput":
      return "INSERT";
    case "shellAppendOutput":
      return "APPEND";
    case "shellKeepPipe":
      return "KEEP PIPE";
  }
}

function promptPlaceholder(kind: InputPromptKind): string {
  switch (kind) {
    case "selectRegex":
    case "splitSelection":
    case "keepSelections":
    case "removeSelections":
      return "Regular expression";
    case "searchForward":
    case "searchBackward":
      return "Search";
    case "shellPipe":
    case "shellPipeTo":
    case "shellInsertOutput":
    case "shellAppendOutput":
    case "shellKeepPipe":
      return "Shell command";
  }
}

function isInputPromptKind(value: unknown): value is InputPromptKind {
  return value === "selectRegex"
    || value === "splitSelection"
    || value === "keepSelections"
    || value === "removeSelections"
    || value === "searchForward"
    || value === "searchBackward"
    || value === "shellPipe"
    || value === "shellPipeTo"
    || value === "shellInsertOutput"
    || value === "shellAppendOutput"
    || value === "shellKeepPipe";
}

function isSearchPromptKind(kind: InputPromptKind): kind is "searchForward" | "searchBackward" {
  return kind === "searchForward" || kind === "searchBackward";
}

function searchDirectionForKind(kind: InputPromptKind): SearchDirection | undefined {
  switch (kind) {
    case "searchForward":
      return "forward";
    case "searchBackward":
      return "backward";
    case "selectRegex":
    case "splitSelection":
    case "keepSelections":
    case "removeSelections":
    case "shellPipe":
    case "shellPipeTo":
    case "shellInsertOutput":
    case "shellAppendOutput":
    case "shellKeepPipe":
      return undefined;
  }
}
