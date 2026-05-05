import { existsSync, readFileSync, statSync } from "node:fs";
import { builtinModules, createRequire } from "node:module";
import path from "node:path";
import ts from "typescript";

const nodeRequire = createRequire(import.meta.url);
const sourceExtensions = [".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs"] as const;
const builtins = new Set([...builtinModules, ...builtinModules.map((name) => `node:${name}`)]);

interface CachedModule {
  exports: unknown;
}

export class SourceModuleLoader {
  private readonly cache = new Map<string, CachedModule>();

  constructor(private readonly virtualModules: Record<string, unknown> = {}) {}

  loadDefault<T>(entryPath: string): T {
    const module = this.loadModule(entryPath) as { default?: T } | T;
    if (typeof module === "function") return module as T;
    if (module && typeof module === "object" && "default" in module) return module.default as T;
    throw new Error(`module does not export a default value: ${entryPath}`);
  }

  loadModule(entryPath: string): unknown {
    return this.evaluate(resolveLocalModule(entryPath, process.cwd())).exports;
  }

  private evaluate(filePath: string): CachedModule {
    const normalizedPath = path.resolve(filePath);
    const cached = this.cache.get(normalizedPath);
    if (cached) return cached;

    if (normalizedPath.endsWith(".json")) {
      const jsonModule: CachedModule = { exports: JSON.parse(readFileSync(normalizedPath, "utf8")) as unknown };
      this.cache.set(normalizedPath, jsonModule);
      return jsonModule;
    }

    const source = readFileSync(normalizedPath, "utf8");
    const transpiled = ts.transpileModule(source, {
      fileName: normalizedPath,
      compilerOptions: {
        module: ts.ModuleKind.CommonJS,
        target: ts.ScriptTarget.ES2024,
        jsx: ts.JsxEmit.ReactJSX,
        esModuleInterop: true,
        allowJs: true,
      },
    }).outputText;

    const module: CachedModule = { exports: {} };
    this.cache.set(normalizedPath, module);

    const wrapper = new Function("require", "module", "exports", "__filename", "__dirname", transpiled);
    wrapper(
      (specifier: string) => this.requireFrom(specifier, normalizedPath),
      module,
      module.exports,
      normalizedPath,
      path.dirname(normalizedPath),
    );
    return module;
  }

  private requireFrom(specifier: string, importerPath: string): unknown {
    const virtual = this.virtualModules[specifier];
    if (virtual !== undefined) return virtual;
    if (builtins.has(specifier)) return nodeRequire(specifier);
    if (isLocalSpecifier(specifier)) return this.evaluate(resolveLocalModule(specifier, path.dirname(importerPath))).exports;
    return nodeRequire(specifier);
  }
}

function isLocalSpecifier(specifier: string): boolean {
  return specifier.startsWith(".") || specifier.startsWith("/");
}

function resolveLocalModule(specifier: string, baseDir: string): string {
  const absolute = path.resolve(baseDir, specifier);
  const resolved = tryResolveFile(absolute);
  if (resolved) return resolved;
  throw new Error(`unable to resolve local module ${specifier} from ${baseDir}`);
}

function tryResolveFile(candidate: string): string | undefined {
  if (existsSync(candidate)) {
    const stats = statSync(candidate);
    if (stats.isFile()) return candidate;
    if (stats.isDirectory()) {
      for (const extension of sourceExtensions) {
        const indexCandidate = path.join(candidate, `index${extension}`);
        if (existsSync(indexCandidate) && statSync(indexCandidate).isFile()) return indexCandidate;
      }
      const jsonIndex = path.join(candidate, "index.json");
      if (existsSync(jsonIndex) && statSync(jsonIndex).isFile()) return jsonIndex;
    }
  }

  const parsed = path.parse(candidate);
  if (parsed.ext) {
    if (parsed.ext === ".js" || parsed.ext === ".jsx" || parsed.ext === ".mjs" || parsed.ext === ".cjs") {
      for (const extension of sourceExtensions) {
        const nextCandidate = path.join(parsed.dir, `${parsed.name}${extension}`);
        if (existsSync(nextCandidate) && statSync(nextCandidate).isFile()) return nextCandidate;
      }
    }
    return undefined;
  }

  for (const extension of sourceExtensions) {
    const nextCandidate = `${candidate}${extension}`;
    if (existsSync(nextCandidate) && statSync(nextCandidate).isFile()) return nextCandidate;
  }

  const jsonCandidate = `${candidate}.json`;
  if (existsSync(jsonCandidate) && statSync(jsonCandidate).isFile()) return jsonCandidate;
  return undefined;
}
