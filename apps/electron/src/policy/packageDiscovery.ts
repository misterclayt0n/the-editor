import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import type { EditorHost, EditorPackageManifest, PackageTargetKind } from "@the-editor/editor";

interface RootPackageJson {
  workspaces?: { packages?: string[] } | string[];
}

interface EditorPackageJson {
  name?: string;
  displayName?: string;
  editor?: {
    id?: string;
    title?: string;
    runtime?: string;
    interaction?: string;
    ui?: string;
    theme?: string;
    defaultTargets?: PackageTargetKind[];
  };
}

export class ElectronPackageDiscovery {
  private discoveredIds = new Set<string>();

  constructor(
    private readonly rootDir: string,
    private readonly cwd: string | (() => string),
  ) {}

  async sync(editor: EditorHost): Promise<void> {
    editor.diagnostics.clearPrefix("packages:discover:");

    const result = discoverEditorPackages(this.rootDir, typeof this.cwd === "function" ? this.cwd() : this.cwd);
    const nextIds = new Set<string>();
    for (const diagnostic of result.diagnostics) {
      editor.diagnostics.report(diagnostic);
    }

    for (const manifest of result.manifests) {
      nextIds.add(manifest.id);
      editor.packages.register(manifest);
    }

    for (const id of this.discoveredIds) {
      if (!nextIds.has(id)) await editor.packages.unregister(id);
    }
    this.discoveredIds = nextIds;
  }
}

export function discoverEditorPackages(rootDir: string, cwd: string): {
  manifests: EditorPackageManifest[];
  diagnostics: Array<{ id: string; severity: "warn" | "error"; source: string; message: string; details?: string; packageId?: string }>;
} {
  const manifests: EditorPackageManifest[] = [];
  const diagnostics: Array<{ id: string; severity: "warn" | "error"; source: string; message: string; details?: string; packageId?: string }> = [];
  const seen = new Set<string>();
  for (const packageDir of candidatePackageDirectories(rootDir, cwd)) {
    const result = readEditorPackageManifest(packageDir);
    if (result.diagnostic) {
      diagnostics.push(result.diagnostic);
      continue;
    }

    const manifest = result.manifest;
    if (!manifest) continue;
    if (seen.has(manifest.id)) {
      diagnostics.push({
        id: `packages:discover:duplicate:${manifest.id}`,
        severity: "warn",
        source: "packages.discover",
        packageId: manifest.id,
        message: `duplicate editor package id ${manifest.id} discovered; first package wins`,
        details: packageDir,
      });
      continue;
    }
    seen.add(manifest.id);
    manifests.push(manifest);
  }
  manifests.sort((a, b) => String(a.id).localeCompare(String(b.id)));
  return { manifests, diagnostics };
}

function candidatePackageDirectories(rootDir: string, cwd: string): string[] {
  const dirs = new Set<string>();
  for (const dir of workspacePackageDirectories(rootDir)) dirs.add(dir);
  for (const dir of globalExtensionDirectories()) dirs.add(dir);
  for (const dir of localExtensionDirectories(rootDir, cwd)) dirs.add(dir);
  return [...dirs];
}

function workspacePackageDirectories(rootDir: string): string[] {
  const packageJsonPath = path.join(rootDir, "package.json");
  if (!existsSync(packageJsonPath)) return [];

  const rootPackage = JSON.parse(readFileSync(packageJsonPath, "utf8")) as RootPackageJson;
  const patterns = Array.isArray(rootPackage.workspaces)
    ? rootPackage.workspaces
    : (rootPackage.workspaces?.packages ?? []);

  const dirs: string[] = [];
  for (const pattern of patterns) {
    dirs.push(...expandWorkspacePattern(rootDir, pattern));
  }
  return dirs;
}

function globalExtensionDirectories(): string[] {
  return extensionDirectories(path.join(os.homedir(), ".the-editor", "extensions"));
}

function localExtensionDirectories(rootDir: string, cwd: string): string[] {
  const dirs = new Set<string>();
  for (const baseDir of [rootDir, cwd]) {
    for (const dir of extensionDirectories(path.join(baseDir, ".the-editor", "extensions"))) {
      dirs.add(dir);
    }
  }
  return [...dirs];
}

function extensionDirectories(baseDir: string): string[] {
  if (!existsSync(baseDir) || !statSync(baseDir).isDirectory()) return [];
  return readdirSync(baseDir)
    .map((name) => path.join(baseDir, name))
    .filter((entry) => existsSync(path.join(entry, "package.json")));
}

function expandWorkspacePattern(rootDir: string, pattern: string): string[] {
  if (!pattern.endsWith("/*")) {
    const candidate = path.join(rootDir, pattern);
    return existsSync(path.join(candidate, "package.json")) ? [candidate] : [];
  }

  const baseDir = path.join(rootDir, pattern.slice(0, -2));
  if (!existsSync(baseDir) || !statSync(baseDir).isDirectory()) return [];
  return readdirSync(baseDir)
    .map((name) => path.join(baseDir, name))
    .filter((entry) => existsSync(path.join(entry, "package.json")));
}

function readEditorPackageManifest(packageDir: string): {
  manifest?: EditorPackageManifest;
  diagnostic?: { id: string; severity: "error"; source: string; message: string; details?: string };
} {
  const packageJsonPath = path.join(packageDir, "package.json");
  if (!existsSync(packageJsonPath)) return {};

  let packageJson: EditorPackageJson;
  try {
    packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8")) as EditorPackageJson;
  } catch (error) {
    return {
      diagnostic: {
        id: `packages:discover:parse:${packageJsonPath}`,
        severity: "error",
        source: "packages.discover",
        message: `failed to parse ${packageJsonPath}`,
        details: error instanceof Error ? error.message : String(error),
      },
    };
  }

  const editor = packageJson.editor;
  if (!editor) return {};

  const id = editor.id ?? packageJson.name ?? path.basename(packageDir);
  const title = editor.title ?? packageJson.displayName ?? packageJson.name ?? id;
  const entrypoints = {
    runtime: resolveEntrypoint(packageDir, editor.runtime),
    interaction: resolveEntrypoint(packageDir, editor.interaction),
    ui: resolveEntrypoint(packageDir, editor.ui),
    theme: resolveEntrypoint(packageDir, editor.theme),
  };

  return {
    manifest: {
      id,
      title,
      entrypoints,
      defaultTargets: editor.defaultTargets,
    },
  };
}

function resolveEntrypoint(packageDir: string, entrypoint: string | undefined): string | undefined {
  if (!entrypoint) return undefined;
  return path.resolve(packageDir, entrypoint);
}
