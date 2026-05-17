import * as React from "react";
import * as ReactJsxRuntime from "react/jsx-runtime";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import * as PierreDiffs from "@pierre/diffs";
import * as PierreTreesReact from "@pierre/trees/react";
import * as LucideReact from "lucide-react";
import ts from "typescript";
import type { HealthDiagnosticEntry, InteractiveSessionSnapshot, PackageSnapshot, ThemeSnapshot, ToolRootTarget } from "@the-editor/editor";
import * as editorCursorAnchorsModule from "@the-editor/editor/cursorAnchors";
import * as editorMarkdownModule from "@the-editor/editor/markdown";
import * as editorMarkdownLinksModule from "@the-editor/editor/markdownLinks";
import * as editorMarkdownViewModule from "@the-editor/editor/markdownView";
import * as editorTextSurfaceCanvasModule from "@the-editor/editor/textSurfaceCanvas";
import * as editorSurfaceViewportControllerModule from "@the-editor/editor/surfaceViewportController";
import * as editorThemesModule from "@the-editor/editor/themes";
import { themeCssVariables } from "@the-editor/editor/themes";
import * as editorUiModule from "@the-editor/editor/ui";
import * as editorUiCommandModule from "@the-editor/ui/command";
import {
  createUiHost,
  type EditorUiModule,
  type UiSessionClient,
  type WorkbenchSnapshot,
  type WorkbenchTabstripViewContext,
} from "@the-editor/editor/ui";
import { UI_SOURCE_ALLOWED_SPECIFIERS, validateUiModuleImportGraph } from "./uiModuleGraph.js";

type LoadedTarget = {
  moduleKey: string;
  contributionModuleId: string;
  dispose(): void;
  path: string;
  version: number;
  target: "ui";
};

export class RendererPackageRuntime {
  private readonly sessions = new RendererUiSessionClient();

  readonly host = createUiHost({
    dispatch(dispatch) {
      return window.theEditor.policyRequest("command.dispatch", dispatch);
    },
  }, this.sessions);

  private readonly listeners = new Set<() => void>();
  private readonly loaded = new Map<string, LoadedTarget>();
  private readonly localDiagnostics = new Map<string, HealthDiagnosticEntry>();
  private readonly appliedThemeVariables = new Set<string>();

  async update(snapshot: WorkbenchSnapshot): Promise<void> {
    const desired = new Map<string, { moduleKey: string; path: string; version: number; target: "ui" }>();
    for (const pkg of snapshot.packages) {
      for (const target of ["ui"] as const) {
        if (!pkg.loadedTargets.includes(target)) continue;
        const path = pkg.entrypointPaths[target];
        const version = pkg.loadVersions[target];
        if (!path || !version) continue;
        const moduleKey = `${pkg.id}/${target}`;
        desired.set(moduleKey, { moduleKey, path, version, target });
      }
    }

    let changed = false;

    for (const [moduleKey, loaded] of [...this.loaded.entries()]) {
      const next = desired.get(moduleKey);
      if (!next) {
        this.unload(moduleKey);
        this.clearUiDiagnostic(moduleKey);
        changed = true;
        continue;
      }
      if (next.path === loaded.path && next.version === loaded.version) continue;

      try {
        const replacement = await this.instantiate(next.moduleKey, next.path, next.version, next.target);
        this.loaded.set(moduleKey, replacement);
        loaded.dispose();
        this.clearUiDiagnostic(moduleKey);
        changed = true;
      } catch (error) {
        changed = this.reportUiDiagnostic(moduleKey, next, error) || changed;
      }
    }

    for (const [moduleKey, next] of desired.entries()) {
      if (this.loaded.has(moduleKey)) continue;
      try {
        const loaded = await this.instantiate(next.moduleKey, next.path, next.version, next.target);
        this.loaded.set(moduleKey, loaded);
        this.clearUiDiagnostic(moduleKey);
        changed = true;
      } catch (error) {
        changed = this.reportUiDiagnostic(moduleKey, next, error) || changed;
      }
    }

    this.sessions.reset(snapshot.sessions ?? []);
    this.host.update({
      ...snapshot,
      healthDiagnostics: [...(snapshot.healthDiagnostics ?? []), ...this.localDiagnostics.values()],
      sessions: this.sessions.all(),
    });
    this.applyTheme(snapshot);

    if (changed) this.emitChange();
  }

  updateSession(session: InteractiveSessionSnapshot): void {
    this.sessions.update(session);
  }

  removeSession(sessionId: string): void {
    this.sessions.remove(sessionId);
  }

  renderRole(role: string): React.ReactNode {
    return this.host.roles.render(role) as React.ReactNode;
  }

  renderToolRoot(root: ToolRootTarget): React.ReactNode {
    const nodes = this.host.tools.renderRoot(root) as React.ReactNode[];
    if (!nodes.length) return null;
    return nodes;
  }

  renderWorkbenchTabstrip(input: Omit<WorkbenchTabstripViewContext, "commands" | "sessions" | "workbench">): React.ReactNode {
    return this.host.tabstrips.render(input) as React.ReactNode;
  }

  hasToolRoot(root: ToolRootTarget): boolean {
    return this.host.tools.toolsForRoot(root).length > 0;
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  private async instantiate(moduleKey: string, entryPath: string, version: number, target: "ui"): Promise<LoadedTarget> {
    const contributionModuleId = `${moduleKey}@${version}`;
    const loader = new RendererSourceModuleLoader(contributionModuleId, version);
    loader.validate(entryPath);
    const activate = loader.loadDefault<EditorUiModule>(entryPath);
    let moduleDisposable: { dispose(): void } | undefined;
    await this.host.withModule(contributionModuleId, async () => {
      const result = await activate(this.host);
      if (result) moduleDisposable = result;
    });

    return {
      moduleKey,
      contributionModuleId,
      path: entryPath,
      version,
      target,
      dispose: () => {
        moduleDisposable?.dispose();
        this.host.unloadModule(contributionModuleId);
        loader.dispose();
      },
    };
  }

  private unload(moduleKey: string): void {
    const loaded = this.loaded.get(moduleKey);
    if (!loaded) return;
    this.loaded.delete(moduleKey);
    loaded.dispose();
  }

  private reportUiDiagnostic(
    moduleKey: string,
    next: { moduleKey: string; path: string; version: number; target: "ui" },
    error: unknown,
  ): boolean {
    const id = `ui:${moduleKey}`;
    const previous = this.localDiagnostics.get(id);
    const entry: HealthDiagnosticEntry = {
      id,
      severity: "error",
      source: "ui-runtime",
      packageId: moduleKey.split("/")[0],
      target: next.target,
      message: `failed to load ${moduleKey}`,
      details: error instanceof Error ? error.stack ?? error.message : String(error),
    };
    this.localDiagnostics.set(id, entry);
    console.error(`failed to load ${moduleKey}`, error);
    return JSON.stringify(previous) !== JSON.stringify(entry);
  }

  private clearUiDiagnostic(moduleKey: string): void {
    this.localDiagnostics.delete(`ui:${moduleKey}`);
  }

  private applyTheme(snapshot: WorkbenchSnapshot): void {
    this.setThemeVariables(snapshot.theme);
    const root = document.documentElement;
    const activeThemeId = snapshot.theme?.active?.id ?? "";
    if (activeThemeId) root.setAttribute("data-theme-id", activeThemeId);
    else root.removeAttribute("data-theme-id");
  }

  private setThemeVariables(theme?: ThemeSnapshot): void {
    const rootStyle = document.documentElement.style;
    for (const name of this.appliedThemeVariables) {
      rootStyle.removeProperty(name);
    }
    this.appliedThemeVariables.clear();

    const variables = themeCssVariables(theme?.active);
    for (const [name, value] of Object.entries(variables)) {
      rootStyle.setProperty(name, value);
      this.appliedThemeVariables.add(name);
    }
  }

  private emitChange(): void {
    for (const listener of this.listeners) listener();
  }
}

class RendererUiSessionClient implements UiSessionClient {
  private readonly sessions = new Map<string, InteractiveSessionSnapshot>();
  private readonly listeners = new Map<string, Set<() => void>>();

  all(): InteractiveSessionSnapshot[] {
    return [...this.sessions.values()];
  }

  get<TState = unknown>(id: string | undefined): InteractiveSessionSnapshot<TState> | undefined {
    if (!id) return undefined;
    return this.sessions.get(id) as InteractiveSessionSnapshot<TState> | undefined;
  }

  subscribe(id: string, listener: () => void): () => void {
    const list = this.listeners.get(id) ?? new Set<() => void>();
    list.add(listener);
    this.listeners.set(id, list);
    return () => {
      const current = this.listeners.get(id);
      if (!current) return;
      current.delete(listener);
      if (!current.size) this.listeners.delete(id);
    };
  }

  dispatch(id: string, method: string, args?: unknown): Promise<unknown> {
    return window.theEditor.policyRequest("session.dispatch", { sessionId: id, method, args });
  }

  reset(snapshots: readonly InteractiveSessionSnapshot[]): void {
    for (const snapshot of snapshots) this.update(snapshot);
  }

  update(snapshot: InteractiveSessionSnapshot): void {
    const previous = this.sessions.get(snapshot.id);
    if (previous && previous.revision >= snapshot.revision) return;
    this.sessions.set(snapshot.id, snapshot);
    this.emit(snapshot.id);
  }

  remove(sessionId: string): void {
    if (!this.sessions.delete(sessionId)) return;
    this.emit(sessionId);
  }

  private emit(sessionId: string): void {
    const listeners = this.listeners.get(sessionId);
    if (!listeners) return;
    for (const listener of listeners) listener();
  }
}

class RendererSourceModuleLoader {
  private readonly cache = new Map<string, { exports: unknown }>();
  private readonly styleElements = new Map<string, HTMLStyleElement>();

  constructor(
    private readonly moduleId: string,
    private readonly version: number,
  ) {}

  validate(entryPath: string): void {
    validateUiModuleImportGraph(
      entryPath,
      (path) => window.theEditor.readExtensionFile(path),
      (specifier, importerPath) => window.theEditor.resolveExtensionPath(specifier, importerPath) ?? undefined,
    );
  }

  loadDefault<T>(entryPath: string): T {
    const module = this.evaluate(entryPath).exports as { default?: T } | T;
    if (typeof module === "function") return module as T;
    if (module && typeof module === "object" && "default" in module) return module.default as T;
    throw new Error(`ui module does not export a default value: ${entryPath}`);
  }

  dispose(): void {
    for (const style of this.styleElements.values()) style.remove();
    this.styleElements.clear();
    this.cache.clear();
  }

  private evaluate(filePath: string): { exports: unknown } {
    const cached = this.cache.get(filePath);
    if (cached) return cached;

    if (filePath.endsWith(".css")) {
      const cssModule = { exports: this.loadCss(filePath) };
      this.cache.set(filePath, cssModule);
      return cssModule;
    }

    if (filePath.endsWith(".json")) {
      const jsonModule = { exports: JSON.parse(window.theEditor.readExtensionFile(filePath)) as unknown };
      this.cache.set(filePath, jsonModule);
      return jsonModule;
    }

    const source = window.theEditor.readExtensionFile(filePath);
    const transpiled = ts.transpileModule(source, {
      fileName: filePath,
      compilerOptions: {
        module: ts.ModuleKind.CommonJS,
        target: ts.ScriptTarget.ES2024,
        jsx: ts.JsxEmit.ReactJSX,
        esModuleInterop: true,
        allowJs: true,
      },
    }).outputText;

    const module = { exports: {} as unknown };
    this.cache.set(filePath, module);

    const wrapper = new Function("require", "module", "exports", "__filename", "__dirname", transpiled);
    wrapper(
      (specifier: string) => this.requireFrom(specifier, filePath),
      module,
      module.exports,
      filePath,
      dirname(filePath),
    );
    return module;
  }

  private requireFrom(specifier: string, importerPath: string): unknown {
    if (specifier === "react") return React;
    if (specifier === "react/jsx-runtime") return ReactJsxRuntime;
    if (specifier === "react-markdown") return ReactMarkdown;
    if (specifier === "remark-gfm") return remarkGfm;
    if (specifier === "@pierre/diffs") return PierreDiffs;
    if (specifier === "@pierre/trees/react") return PierreTreesReact;
    if (specifier === "lucide-react") return LucideReact;
    if (specifier === "@the-editor/contracts/runtime") return {};
    if (specifier === "@the-editor/editor/ui") return editorUiModule;
    if (specifier === "@the-editor/editor/themes") return editorThemesModule;
    if (specifier === "@the-editor/editor/textSurfaceCanvas") return editorTextSurfaceCanvasModule;
    if (specifier === "@the-editor/editor/surfaceViewportController") return editorSurfaceViewportControllerModule;
    if (specifier === "@the-editor/editor/cursorAnchors") return editorCursorAnchorsModule;
    if (specifier === "@the-editor/editor/markdown") return editorMarkdownModule;
    if (specifier === "@the-editor/editor/markdownLinks") return editorMarkdownLinksModule;
    if (specifier === "@the-editor/editor/markdownView") return editorMarkdownViewModule;
    if (specifier === "@the-editor/ui/command") return editorUiCommandModule;
    if (!specifier.startsWith(".") && !specifier.startsWith("/")) {
      throw new Error(
        `unsupported ui module import ${specifier} from ${importerPath}; `
        + `allowed: ${[...UI_SOURCE_ALLOWED_SPECIFIERS].join(", ")}`,
      );
    }
    const resolved = window.theEditor.resolveExtensionPath(specifier, importerPath);
    if (!resolved) throw new Error(`unable to resolve ${specifier} from ${importerPath}`);
    return this.evaluate(resolved).exports;
  }

  private loadCss(filePath: string): string {
    const existing = this.styleElements.get(filePath);
    if (existing) return existing.textContent ?? "";

    const cssText = window.theEditor.readExtensionFile(filePath);
    const style = document.createElement("style");
    style.dataset.theEditorModule = `${this.moduleId}@${this.version}`;
    style.dataset.theEditorPath = filePath;
    style.textContent = cssText;
    document.head.appendChild(style);
    this.styleElements.set(filePath, style);
    return cssText;
  }
}

function dirname(filePath: string): string {
  const normalized = filePath.replace(/\\/g, "/");
  const index = normalized.lastIndexOf("/");
  return index >= 0 ? normalized.slice(0, index) : normalized;
}

export type { WorkbenchSnapshot, PackageSnapshot };
