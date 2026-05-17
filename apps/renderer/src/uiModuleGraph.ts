import ts from "typescript";

export const UI_SOURCE_ALLOWED_SPECIFIERS = new Set([
  "react",
  "react/jsx-runtime",
  "react-markdown",
  "remark-gfm",
  "@pierre/diffs",
  "@pierre/trees",
  "@pierre/trees/react",
  "lucide-react",
  "@the-editor/contracts/runtime",
  "@the-editor/editor/ui",
  "@the-editor/editor/themes",
  "@the-editor/editor/textSurfaceCanvas",
  "@the-editor/editor/surfaceViewportController",
  "@the-editor/editor/cursorAnchors",
  "@the-editor/editor/markdown",
  "@the-editor/editor/markdownLinks",
  "@the-editor/editor/markdownView",
  "@the-editor/ui/command",
]);

export function validateUiModuleImportGraph(
  entryPath: string,
  readFile: (path: string) => string,
  resolveLocal: (specifier: string, importerPath: string) => string | undefined,
): void {
  const seen = new Set<string>();

  const visit = (filePath: string) => {
    if (seen.has(filePath)) return;
    seen.add(filePath);
    if (filePath.endsWith(".css") || filePath.endsWith(".json")) return;

    const source = readFile(filePath);
    const sourceFile = ts.createSourceFile(filePath, source, ts.ScriptTarget.ES2024, true, scriptKind(filePath));
    for (const specifier of runtimeImportSpecifiers(sourceFile)) {
      if (isLocalSpecifier(specifier)) {
        const resolved = resolveLocal(specifier, filePath);
        if (!resolved) throw new Error(`unable to resolve ${specifier} from ${filePath}`);
        visit(resolved);
        continue;
      }
      if (!UI_SOURCE_ALLOWED_SPECIFIERS.has(specifier)) {
        throw new Error(
          `unsupported ui module import ${specifier} from ${filePath}; `
          + "ui targets may only import renderer-safe modules",
        );
      }
    }
  };

  visit(entryPath);
}

function runtimeImportSpecifiers(sourceFile: ts.SourceFile): string[] {
  const specifiers: string[] = [];

  for (const statement of sourceFile.statements) {
    if (ts.isImportDeclaration(statement)) {
      if (statement.importClause?.isTypeOnly) continue;
      const specifier = stringLiteralText(statement.moduleSpecifier);
      if (specifier) specifiers.push(specifier);
      continue;
    }

    if (ts.isExportDeclaration(statement)) {
      if (statement.isTypeOnly) continue;
      const specifier = statement.moduleSpecifier ? stringLiteralText(statement.moduleSpecifier) : undefined;
      if (specifier) specifiers.push(specifier);
      continue;
    }

    if (ts.isImportEqualsDeclaration(statement)) {
      if (statement.isTypeOnly) continue;
      if (ts.isExternalModuleReference(statement.moduleReference)) {
        const specifier = stringLiteralText(statement.moduleReference.expression);
        if (specifier) specifiers.push(specifier);
      }
    }
  }

  return specifiers;
}

function stringLiteralText(node: ts.Node | undefined): string | undefined {
  return node && ts.isStringLiteral(node) ? node.text : undefined;
}

function isLocalSpecifier(specifier: string): boolean {
  return specifier.startsWith(".") || specifier.startsWith("/");
}

function scriptKind(filePath: string): ts.ScriptKind {
  if (filePath.endsWith(".tsx")) return ts.ScriptKind.TSX;
  if (filePath.endsWith(".ts")) return ts.ScriptKind.TS;
  if (filePath.endsWith(".jsx")) return ts.ScriptKind.JSX;
  if (filePath.endsWith(".json")) return ts.ScriptKind.JSON;
  return ts.ScriptKind.JS;
}
