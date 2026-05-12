import type { DiagnosticRecord, DiagnosticSummary } from "@the-editor/editor";

export const currentDiagnosticsPickerProviderId = "default.diagnostics.current";
export const workspaceDiagnosticsPickerProviderId = "default.diagnostics.workspace";

export function diagnosticSeverityLabel(severity: DiagnosticRecord["severity"]): string {
  switch (severity) {
    case "error":
      return "Error";
    case "warning":
      return "Warning";
    case "info":
      return "Info";
    case "hint":
      return "Hint";
  }
}

export function diagnosticSeverityLetter(severity: DiagnosticRecord["severity"]): string {
  switch (severity) {
    case "error":
      return "E";
    case "warning":
      return "W";
    case "info":
      return "I";
    case "hint":
      return "H";
  }
}

export function formatDiagnosticSummary(summary: DiagnosticSummary): string {
  const parts: string[] = [];
  if (summary.errorCount) parts.push(`E${summary.errorCount}`);
  if (summary.warningCount) parts.push(`W${summary.warningCount}`);
  if (summary.infoCount) parts.push(`I${summary.infoCount}`);
  if (summary.hintCount) parts.push(`H${summary.hintCount}`);
  return parts.join(" ");
}

export function diagnosticPrimaryLine(diagnostic: DiagnosticRecord): string {
  return firstLine(diagnostic.message) || diagnosticSeverityLabel(diagnostic.severity);
}

export function diagnosticLocationLabel(diagnostic: DiagnosticRecord): string {
  const location = `${diagnostic.range.start.line + 1}:${diagnostic.range.start.character + 1}`;
  return diagnostic.path ? `${diagnostic.path}:${location}` : location;
}

export function buildDiagnosticMarkdown(diagnostics: readonly DiagnosticRecord[]): string {
  return diagnostics.map((diagnostic) => buildSingleDiagnosticMarkdown(diagnostic)).filter(Boolean).join("\n\n---\n\n");
}

export function buildSingleDiagnosticMarkdown(diagnostic: DiagnosticRecord): string {
  const lines: string[] = [`### ${diagnosticSeverityLabel(diagnostic.severity)}`, "", diagnostic.message.trim()];
  const meta: string[] = [];
  if (diagnostic.source?.trim()) meta.push(`source: \`${diagnostic.source.trim()}\``);
  if (diagnostic.code != null) {
    const code = String(diagnostic.code);
    meta.push(diagnostic.codeHref ? `[${code}](${diagnostic.codeHref})` : `code: \`${code}\``);
  }
  if (diagnostic.tags.includes("deprecated")) meta.push("`deprecated`");
  if (diagnostic.tags.includes("unnecessary")) meta.push("`unnecessary`");
  if (meta.length) lines.push("", meta.join(" • "));

  if (diagnostic.relatedInformation.length) {
    lines.push("", "Related information", "");
    for (const info of diagnostic.relatedInformation) {
      const target = info.path
        ? `${info.path}:${info.range.start.line + 1}:${info.range.start.character + 1}`
        : `${info.range.start.line + 1}:${info.range.start.character + 1}`;
      lines.push(`- [${target}](${target}) — ${info.message.trim()}`);
    }
  }

  return lines.join("\n").trim();
}

function firstLine(value: string): string {
  return value.split(/\r?\n/, 1)[0]?.trim() ?? "";
}
