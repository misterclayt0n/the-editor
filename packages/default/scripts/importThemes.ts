import { mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import path from "node:path";

interface ImportedTheme {
  id: string;
  title: string;
  palette?: Record<string, string>;
  styles?: Record<string, unknown>;
  ghostty?: {
    background?: string;
    foreground?: string;
    cursorColor?: string;
    selectionBackground?: string;
    selectionForeground?: string;
    palette?: Record<string, string>;
  };
}

const sourceDir = process.argv[2]
  ? path.resolve(process.argv[2])
  : path.join(homedir(), ".config/the-editor/runtime/themes");
const outputDir = path.resolve(import.meta.dir, "../src/themes/generated");

const rawCache = new Map<string, Record<string, unknown>>();
const resolvedCache = new Map<string, ImportedTheme>();
const moduleNames = new Map<string, string>();

await rm(outputDir, { recursive: true, force: true });
await mkdir(outputDir, { recursive: true });

const entries = await readdir(sourceDir, { withFileTypes: true });
const themeFiles = entries
  .filter((entry) => entry.isFile() && entry.name.endsWith(".toml"))
  .map((entry) => entry.name)
  .sort((a, b) => a.localeCompare(b));

const imported = [] as Array<{ id: string; moduleName: string; fileName: string }>;
for (const file of themeFiles) {
  const themeId = path.basename(file, ".toml");
  const moduleName = uniqueModuleName(themeId);
  const fileName = `${moduleName}.ts`;
  const theme = await resolveTheme(themeId, new Set());
  await writeFile(path.join(outputDir, fileName), renderThemeModule(theme));
  imported.push({ id: themeId, moduleName, fileName });
}

await writeFile(path.join(outputDir, "index.ts"), renderIndexModule(imported));

console.log(`imported ${imported.length} themes from ${sourceDir} -> ${outputDir}`);

async function resolveTheme(themeId: string, stack: Set<string>): Promise<ImportedTheme> {
  const cached = resolvedCache.get(themeId);
  if (cached) return cached;
  if (stack.has(themeId)) throw new Error(`circular theme inheritance: ${[...stack, themeId].join(" -> ")}`);

  stack.add(themeId);
  const raw = await readRawTheme(themeId);
  const inheritedId = typeof raw.inherits === "string" ? raw.inherits : undefined;
  const base = inheritedId ? await resolveTheme(inheritedId, stack) : undefined;
  const current = extractTheme(raw, themeId);
  const merged = mergeThemes(base, current);
  resolvedCache.set(themeId, merged);
  stack.delete(themeId);
  return merged;
}

async function readRawTheme(themeId: string): Promise<Record<string, unknown>> {
  const cached = rawCache.get(themeId);
  if (cached) return cached;

  const filePath = path.join(sourceDir, `${themeId}.toml`);
  const text = await readFile(filePath, "utf8");
  const parsed = Bun.TOML.parse(text) as Record<string, unknown>;
  rawCache.set(themeId, parsed);
  return parsed;
}

function extractTheme(raw: Record<string, unknown>, themeId: string): ImportedTheme {
  const styles: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(raw)) {
    if (key === "inherits" || key === "palette" || key === "ghostty") continue;
    flattenStyleEntry(styles, key, value);
  }

  const palette = stringRecord(raw.palette);
  const ghostty = extractGhostty(raw.ghostty);

  return {
    id: themeId,
    title: titleFromThemeId(themeId),
    palette: Object.keys(palette).length ? palette : undefined,
    styles: Object.keys(styles).length ? styles : undefined,
    ghostty,
  };
}

function extractGhostty(value: unknown): ImportedTheme["ghostty"] | undefined {
  if (!isPlainObject(value)) return undefined;
  const palette = stringRecord(value.palette);
  const ghostty = {
    background: stringValue(value.background),
    foreground: stringValue(value.foreground),
    cursorColor: stringValue(value.cursorColor),
    selectionBackground: stringValue(value.selectionBackground),
    selectionForeground: stringValue(value.selectionForeground),
    palette: Object.keys(palette).length ? palette : undefined,
  };
  return Object.values(ghostty).some((entry) => entry !== undefined)
    ? ghostty
    : undefined;
}

function flattenStyleEntry(target: Record<string, unknown>, key: string, value: unknown): void {
  if (typeof value === "string") {
    target[key] = value;
    return;
  }

  if (!isPlainObject(value)) return;
  if (looksLikeStyle(value)) {
    target[key] = cloneJson(value);
    return;
  }

  for (const [childKey, childValue] of Object.entries(value)) {
    flattenStyleEntry(target, `${key}.${childKey}`, childValue);
  }
}

function looksLikeStyle(value: Record<string, unknown>): boolean {
  return ["fg", "bg", "modifiers", "underline"].some((key) => key in value);
}

function mergeThemes(base: ImportedTheme | undefined, current: ImportedTheme): ImportedTheme {
  if (!base) return current;
  return {
    id: current.id,
    title: current.title,
    palette: {
      ...(base.palette ?? {}),
      ...(current.palette ?? {}),
    },
    styles: {
      ...(base.styles ?? {}),
      ...(current.styles ?? {}),
    },
    ghostty: mergeGhostty(base.ghostty, current.ghostty),
  };
}

function mergeGhostty(
  base: ImportedTheme["ghostty"] | undefined,
  current: ImportedTheme["ghostty"] | undefined,
): ImportedTheme["ghostty"] | undefined {
  if (!base) return current;
  if (!current) return base;
  return {
    ...base,
    ...current,
    palette: {
      ...(base.palette ?? {}),
      ...(current.palette ?? {}),
    },
  };
}

function renderThemeModule(theme: ImportedTheme): string {
  return [
    'import { defineTheme } from "@the-editor/editor";',
    "",
    `export default defineTheme(${serialize(theme)});`,
    "",
  ].join("\n");
}

function renderIndexModule(imported: Array<{ id: string; moduleName: string; fileName: string }>): string {
  const imports = imported.map(({ moduleName, fileName }) => `import ${moduleName} from "./${fileName.slice(0, -3)}.js";`);
  const exports = imported.map(({ moduleName }) => `  ${moduleName},`);
  return [
    ...imports,
    "",
    "export const importedThemes = [",
    ...exports,
    "] as const;",
    "",
  ].join("\n");
}

function uniqueModuleName(themeId: string): string {
  const existing = moduleNames.get(themeId);
  if (existing) return existing;
  const base = sanitizeIdentifier(themeId);
  let next = base;
  let index = 2;
  while ([...moduleNames.values()].includes(next)) {
    next = `${base}_${index++}`;
  }
  moduleNames.set(themeId, next);
  return next;
}

function sanitizeIdentifier(value: string): string {
  const sanitized = value.replace(/[^a-zA-Z0-9_$]+/g, "_").replace(/^([0-9])/, "_$1");
  return sanitized || "theme";
}

function titleFromThemeId(themeId: string): string {
  return themeId
    .replace(/[_-]+/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/\b\w/g, (match) => match.toUpperCase());
}

function stringRecord(value: unknown): Record<string, string> {
  if (!isPlainObject(value)) return {};
  const entries = Object.entries(value)
    .filter(([, entry]) => typeof entry === "string")
    .map(([key, entry]) => [key, entry as string]);
  return Object.fromEntries(entries);
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function cloneJson<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function serialize(value: unknown): string {
  return JSON.stringify(value, null, 2);
}
