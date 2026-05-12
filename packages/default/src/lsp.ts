import { toDisposable, type EditorHost, type EditorModule, type LspServerStatus } from "@the-editor/editor";
import { importedLspLanguages, importedLspServers } from "./lsp/generated.js";
import { getDefaultState } from "./state.js";

export interface LspStatuslineState {
  phase: "off" | "loading" | "ready" | "error";
  text?: string;
}

interface LspControllerState {
  spinnerIndex: number;
  pendingStarts: Map<string, string>;
  startFailures: Map<string, { path: string; message: string }>;
}

const lspStatuslineSlot = "default.lsp.statusline";
const lspControllerSlot = "default.lsp.controller";
const spinnerFrames = ["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];

export function defaultLsp(): EditorModule {
  return (editor) => {
    for (const server of buildRegisteredServerConfigs()) editor.lsp.registerServer(server);
    for (const language of importedLspLanguages) editor.lsp.registerLanguage(language);

    let disposed = false;

    void (async () => {
      let previousPath: string | undefined;
      while (!disposed) {
        const activePath = getDefaultState(editor).activePath;
        if (activePath && activePath !== previousPath) void ensureLspForPath(editor, activePath);
        previousPath = activePath;
        lspController(editor).spinnerIndex = (lspController(editor).spinnerIndex + 1) % spinnerFrames.length;
        await updateLspStatusline(editor);
        await delay(currentLspStatusline(editor).phase === "loading" ? 100 : 700);
      }
    })();

    return toDisposable(() => {
      disposed = true;
    });
  };
}

export async function ensureLspForPath(editor: EditorHost, path: string): Promise<void> {
  const serverIds = editor.lsp.serverIdsForPath(path);
  if (!serverIds.length) {
    await updateLspStatusline(editor);
    return;
  }

  const controller = lspController(editor);
  let statuses = await editor.lsp.status();

  for (const serverId of serverIds) {
    if (statuses.some((server) => server.id === serverId && isOperational(server))) continue;
    if (controller.pendingStarts.has(serverId)) continue;

    controller.pendingStarts.set(serverId, path);
    await updateLspStatusline(editor, statuses);
    try {
      await editor.lsp.whenServerRegistered(serverId);
      await editor.lsp.startServer({ id: serverId, rootUri: fileUri(editor.cwd.current()) });
      controller.startFailures.delete(`${serverId}\0${path}`);
    } catch (error) {
      controller.startFailures.set(`${serverId}\0${path}`, { path, message: normalizeLspError(error) });
    } finally {
      controller.pendingStarts.delete(serverId);
      statuses = await editor.lsp.status();
      await updateLspStatusline(editor, statuses);
    }
  }
}

export function currentLspStatusline(editor: EditorHost): LspStatuslineState {
  return editor.stateSlot<LspStatuslineState>(lspStatuslineSlot, () => ({ phase: "off" }));
}

export function fileUri(path: string): string {
  return `file://${path}`;
}

export function lspLanguageIdForPath(editor: EditorHost, path: string): string {
  return editor.lsp.languageForPath(path)?.languageId ?? fallbackLanguageIdForPath(path);
}

function lspController(editor: EditorHost): LspControllerState {
  return editor.stateSlot<LspControllerState>(lspControllerSlot, () => ({
    spinnerIndex: 0,
    pendingStarts: new Map(),
    startFailures: new Map(),
  }));
}

async function updateLspStatusline(editor: EditorHost, statuses?: LspServerStatus[]): Promise<void> {
  const next = buildLspStatusline(editor, statuses ?? await editor.lsp.status());
  const previous = currentLspStatusline(editor);
  if (previous.phase === next.phase && previous.text === next.text) return;
  editor.setStateSlot(lspStatuslineSlot, next);
  editor.workbench.invalidate("roles");
}

function buildLspStatusline(editor: EditorHost, statuses: readonly LspServerStatus[]): LspStatuslineState {
  const path = getDefaultState(editor).activePath;
  if (!path) return { phase: "off" };

  const language = editor.lsp.languageForPath(path);
  if (!language) return { phase: "off", text: "lsp: off" };

  const serverIds = editor.lsp.serverIdsForPath(path);
  if (!serverIds.length) return { phase: "off", text: `lsp: no server for ${language.languageId ?? language.id}` };

  const controller = lspController(editor);
  const pending = serverIds.filter((serverId) => controller.pendingStarts.get(serverId) === path);
  if (pending.length) {
    return {
      phase: "loading",
      text: clampStatuslineText(`lsp: ${spinnerFrames[controller.spinnerIndex]} starting ${pending.join(",")}`),
    };
  }

  const running = statuses.filter((server) => serverIds.includes(server.id) && isOperational(server));
  if (running.length) {
    return { phase: "ready", text: clampStatuslineText(`lsp: ready (${running.map((server) => server.id).join(",")})`) };
  }

  const statusError = statuses.find((server) => serverIds.includes(server.id) && server.state === "error");
  if (statusError) {
    return { phase: "error", text: clampStatuslineText(`lsp: error (${normalizeLspError(statusError.message ?? statusError.id)})`) };
  }

  const failure = serverIds.map((serverId) => controller.startFailures.get(`${serverId}\0${path}`)).find(Boolean);
  if (failure) {
    return { phase: "error", text: clampStatuslineText(`lsp: error (${failure.message})`) };
  }

  return { phase: "off", text: `lsp: off (${serverIds.join(",")})` };
}

function buildRegisteredServerConfigs() {
  return importedLspServers.map((server) => {
    const languages = importedLspLanguages
      .filter((language) => language.serverIds?.includes(server.id))
      .map((language) => language.languageId ?? language.id);
    const fileTypes = importedLspLanguages
      .filter((language) => language.serverIds?.includes(server.id))
      .flatMap((language) => (language.fileTypes ?? []).filter((fileType): fileType is string => typeof fileType === "string"));
    return {
      ...server,
      languages: [...new Set(languages)],
      fileTypes: [...new Set(fileTypes)],
    };
  });
}

function fallbackLanguageIdForPath(path: string): string {
  const name = path.split(/[\\/]/).at(-1) ?? path;
  const extension = name.includes(".") ? name.slice(name.lastIndexOf(".") + 1) : "";
  return extension || name || "plaintext";
}

function isOperational(server: LspServerStatus): boolean {
  return server.state === "running" || server.state === "initialized";
}

function normalizeLspError(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error ?? "error");
  if (message.includes("No such file or directory") || message.includes("os error 2")) return "command not found";
  return message.replace(/\s+/g, " ").trim();
}

function clampStatuslineText(text: string, max = 36): string {
  return text.length <= max ? text : `${text.slice(0, max - 1)}…`;
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
