import { contributionKind } from "./contributions.js";
import type { EditorHost } from "./editorHost.js";
import { toDisposable, type Disposable } from "./disposable.js";
import type { JsonValue, LspDiagnosticEntry, LspLogEntry, LspServerConfig, LspServerId, LspServerStatus } from "@the-editor/contracts/runtime";

export interface LspLanguageDefinition {
  id: string;
  languageId?: string;
  fileTypes?: Array<string | { glob: string }>;
  serverIds?: string[];
}

export type { LspDiagnosticEntry, LspLogEntry, LspServerConfig, LspServerId, LspServerStatus };

interface RegisteredLspLanguage {
  definition: LspLanguageDefinition;
  disposable: Disposable;
}

interface RegisteredLspServer {
  config: LspServerConfig;
  registration: Promise<void>;
}

export class LspRegistry {
  private readonly languagesById = new Map<string, RegisteredLspLanguage>();
  private readonly serversById = new Map<string, RegisteredLspServer>();

  constructor(private readonly editor: EditorHost) {}

  registerServer(config: LspServerConfig): Disposable {
    const registration = this.editor.runtime.lsp.registerServer(config).then(() => undefined).catch((error) => {
      this.editor.notifications.error("lsp", `failed to register LSP server ${config.id}: ${error instanceof Error ? error.message : String(error)}`);
      throw error;
    });
    this.serversById.set(config.id, { config, registration });
    const disposable = toDisposable(() => {
      const current = this.serversById.get(config.id);
      if (current?.config === config) this.serversById.delete(config.id);
    });
    return this.editor.contributions.add({
      moduleId: this.editor.currentModuleId(),
      kind: contributionKind("lspServer"),
      localId: config.id,
      label: config.id,
      disposable,
    });
  }

  registerLanguage(definition: LspLanguageDefinition): Disposable {
    const previous = this.languagesById.get(definition.id);
    previous?.disposable.dispose();
    const disposable = toDisposable(() => {
      const current = this.languagesById.get(definition.id);
      if (current?.definition === definition) this.languagesById.delete(definition.id);
    });
    this.languagesById.set(definition.id, { definition, disposable });
    return this.editor.contributions.add({
      moduleId: this.editor.currentModuleId(),
      kind: contributionKind("lspLanguage"),
      localId: definition.id,
      label: definition.id,
      disposable,
    });
  }

  languages(): LspLanguageDefinition[] {
    return [...this.languagesById.values()].map((entry) => entry.definition);
  }

  languageForPath(path: string): LspLanguageDefinition | undefined {
    const candidates = this.languages().filter((definition) => matchesLspLanguagePath(definition, path));
    return candidates.at(-1);
  }

  serverIdsForPath(path: string): string[] {
    const language = this.languageForPath(path);
    return [...new Set(language?.serverIds ?? [])];
  }

  configuredServers(): Promise<LspServerConfig[]> {
    return Promise.resolve([...this.serversById.values()].map((entry) => entry.config));
  }

  server(id: string): LspServerConfig | undefined {
    return this.serversById.get(id)?.config;
  }

  async whenServerRegistered(id: string): Promise<void> {
    await this.serversById.get(id)?.registration;
  }

  startServer(params: { id: string; rootUri?: string }): Promise<{ serverId: LspServerId }> {
    return this.editor.runtime.lsp.startServer(params);
  }

  shutdownServer(serverId: LspServerId): Promise<void> {
    return this.editor.runtime.lsp.shutdownServer({ serverId }).then(() => undefined);
  }

  status(): Promise<LspServerStatus[]> {
    return this.editor.runtime.lsp.status();
  }

  diagnostics(): Promise<LspDiagnosticEntry[]> {
    return this.editor.runtime.lsp.diagnostics();
  }

  logs(serverId: LspServerId): Promise<LspLogEntry[]> {
    return this.editor.runtime.lsp.logs({ serverId });
  }

  request(params: { serverId: LspServerId; method: string; params?: JsonValue; timeoutMs?: number }): Promise<JsonValue> {
    return this.editor.runtime.lsp.request(params);
  }

  notify(params: { serverId: LspServerId; method: string; params?: JsonValue }): Promise<void> {
    return this.editor.runtime.lsp.notify(params).then(() => undefined);
  }
}

export function matchesLspLanguagePath(definition: LspLanguageDefinition, path: string): boolean {
  const fileTypes = definition.fileTypes ?? [];
  if (!fileTypes.length) return false;
  const name = path.split(/[\\/]/).at(-1) ?? path;
  const extension = name.includes(".") ? name.slice(name.lastIndexOf(".") + 1) : "";
  return fileTypes.some((fileType) => typeof fileType === "string"
    ? fileType === extension || fileType === name
    : typeof fileType.glob === "string" && matchGlob(fileType.glob, name));
}

function matchGlob(glob: string, name: string): boolean {
  if (!glob.includes("*") && !glob.includes("?")) return glob === name || name.endsWith(glob);
  let pattern = "^";
  for (const char of glob) {
    if (char === "*") pattern += ".*";
    else if (char === "?") pattern += ".";
    else pattern += /[|\\{}()[\]^$+?.]/.test(char) ? `\\${char}` : char;
  }
  pattern += "$";
  return new RegExp(pattern).test(name);
}
