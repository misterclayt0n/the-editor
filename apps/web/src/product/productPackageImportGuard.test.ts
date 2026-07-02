import { describe, expect, test } from "vitest";
import * as ts from "typescript";
import { buildExtensionVirtualModules } from "./userExtensions";

const productSources = import.meta.glob<string>("./*.ts", {
  query: "?raw",
  import: "default",
  eager: true,
});

const INFRASTRUCTURE_DEFINE_PACKAGE_FILES = new Set(["./userExtensions.ts"]);

/**
 * G1's current dogfooding-debt ledger. Keep this list sorted.
 *
 * A line here means a first-party product package reaches something a loaded
 * user extension cannot reach through `buildExtensionVirtualModules()` or asks
 * the app for a host-only `the-editor/web/Host*` service. Adding new debt must
 * be explicit in this baseline; removing a line is the mechanical proof that a
 * seam was published or a host coupling was retired.
 */
const KNOWN_PRODUCT_PACKAGE_SEAM_DEBT = [
  "host-tag diffView.ts -> the-editor/web/HostDiffView",
  "host-tag fileTree.ts -> the-editor/web/HostFileTree",
  "host-tag terminal.ts -> the-editor/web/HostTerminal",
  "import editorDefaults.ts -> ../editor/commands",
  "import files.ts -> ./editorDefaults",
  "import intelActions.ts -> ./editorDefaults",
  "import lineNumbers.ts -> ../editor/gutter",
  "import lsp.ts -> ../generated/languageServers",
  "import lspActions.ts -> ./editorDefaults",
  "import terminal.ts -> ../terminal/menu",
  "import vcs.ts -> ../vcs/git",
  "import view.ts -> ./editorDefaults",
] as const;

const sourceOf = (file: string): string => {
  const source = productSources[file];
  if (source === undefined) throw new Error(`missing product source ${file}`);
  return source;
};

const displayFile = (file: string): string => (file.startsWith("./") ? file.slice(2) : file);

const resolveLocalTs = (specifier: string): string =>
  specifier.endsWith(".ts") ? specifier : `${specifier}.ts`;

const literalText = (node: ts.Node | undefined): string | undefined => {
  if (node === undefined) return undefined;
  return ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)
    ? node.text
    : undefined;
};

const findModuleSpecifiers = (file: string, source: string): readonly string[] => {
  const sourceFile = ts.createSourceFile(
    file,
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TS,
  );
  const specifiers = new Set<string>();

  for (const statement of sourceFile.statements) {
    if (ts.isImportDeclaration(statement) || ts.isExportDeclaration(statement)) {
      const text = literalText(statement.moduleSpecifier);
      if (text !== undefined) specifiers.add(text);
    }
  }

  const visit = (node: ts.Node): void => {
    if (ts.isCallExpression(node)) {
      const expression = node.expression;
      const [firstArg] = node.arguments;
      const text = literalText(firstArg);
      if (text !== undefined) {
        if (expression.kind === ts.SyntaxKind.ImportKeyword) specifiers.add(text);
        if (ts.isIdentifier(expression) && expression.text === "require") specifiers.add(text);
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);

  return [...specifiers].sort();
};

const manifestPackageFiles = (): readonly string[] =>
  findModuleSpecifiers("manifest.ts", sourceOf("./manifest.ts"))
    .filter((specifier) => specifier.startsWith("./"))
    .map(resolveLocalTs);

const definePackageFiles = (): readonly string[] =>
  Object.entries(productSources)
    .filter(([file]) => !file.endsWith(".test.ts"))
    .filter(([file]) => !INFRASTRUCTURE_DEFINE_PACKAGE_FILES.has(file))
    .filter(([, source]) => source.includes("definePackage("))
    .map(([file]) => file);

const productPackageFiles = (): readonly string[] =>
  [...new Set([...manifestPackageFiles(), ...definePackageFiles()])].sort();

const importDebt = (file: string, allowedImports: ReadonlySet<string>): readonly string[] =>
  findModuleSpecifiers(file, sourceOf(file))
    .filter((specifier) => !allowedImports.has(specifier))
    .map((specifier) => `import ${displayFile(file)} -> ${specifier}`);

const hostTagDebt = (file: string): readonly string[] => {
  const source = sourceOf(file);
  const pattern = /Context\.Tag\(\s*["'](the-editor\/web\/Host[^"']+)["']/g;
  const debt: string[] = [];
  for (const match of source.matchAll(pattern)) {
    const tag = match[1];
    if (tag !== undefined) debt.push(`host-tag ${displayFile(file)} -> ${tag}`);
  }
  return debt;
};

const currentProductPackageSeamDebt = (): readonly string[] => {
  const allowedImports = new Set(Object.keys(buildExtensionVirtualModules()));
  return productPackageFiles()
    .flatMap((file) => [...importDebt(file, allowedImports), ...hostTagDebt(file)])
    .sort();
};

describe("first-party product package extension surface", () => {
  test("has only the baselined G1 seam debt", () => {
    expect(currentProductPackageSeamDebt()).toEqual(KNOWN_PRODUCT_PACKAGE_SEAM_DEBT);
  });
});
