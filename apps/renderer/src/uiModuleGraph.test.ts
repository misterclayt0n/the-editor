import { describe, expect, it } from "vitest";
import { validateUiModuleImportGraph } from "./uiModuleGraph.js";

describe("validateUiModuleImportGraph", () => {
  it("rejects transitive renderer-unsafe value imports", () => {
    const files = new Map<string, string>([
      ["/ui.tsx", 'import "./shared.ts";\nexport default function ui() {}\n'],
      ["/shared.ts", 'import { commandId } from "@the-editor/editor";\nvoid commandId;\n'],
    ]);

    expect(() => validateUiModuleImportGraph(
      "/ui.tsx",
      (path) => requireFile(files, path),
      (specifier, importer) => resolveLocal(files, specifier, importer),
    )).toThrow(/unsupported ui module import @the-editor\/editor/);
  });

  it("allows type-only imports from renderer-unsafe modules", () => {
    const files = new Map<string, string>([
      ["/ui.tsx", 'import type { EditorModule } from "@the-editor/editor";\nconst _x: EditorModule | undefined = undefined;\nexport default function ui() { return _x; }\n'],
    ]);

    expect(() => validateUiModuleImportGraph(
      "/ui.tsx",
      (path) => requireFile(files, path),
      (specifier, importer) => resolveLocal(files, specifier, importer),
    )).not.toThrow();
  });
});

function requireFile(files: Map<string, string>, path: string): string {
  const source = files.get(path);
  if (source === undefined) throw new Error(`missing file ${path}`);
  return source;
}

function resolveLocal(files: Map<string, string>, specifier: string, importer: string): string | undefined {
  if (!specifier.startsWith(".") && !specifier.startsWith("/")) return undefined;
  const base = specifier.startsWith("/")
    ? specifier
    : `${dirname(importer)}/${specifier}`;
  const normalized = normalize(base);
  if (files.has(normalized)) return normalized;
  return undefined;
}

function dirname(path: string): string {
  const normalized = normalize(path);
  const index = normalized.lastIndexOf("/");
  return index > 0 ? normalized.slice(0, index) : "/";
}

function normalize(path: string): string {
  const parts: string[] = [];
  for (const part of path.split("/")) {
    if (!part || part === ".") continue;
    if (part === "..") {
      parts.pop();
      continue;
    }
    parts.push(part);
  }
  return `/${parts.join("/")}`;
}
