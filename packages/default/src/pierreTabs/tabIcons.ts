import {
  getBuiltInFileIconColor,
  getBuiltInFileIconName,
  getBuiltInSpriteSheet,
  resolveBuiltInFileIconToken,
} from "./builtInIcons.js";

export interface PierreTabResolvedIcon {
  name: string;
  color?: string;
}

export function pierreTabIconSpriteMarkup(): string {
  return getBuiltInSpriteSheet("complete");
}

export function resolvePierreTabIcon(input: { kind: string; path?: string; title: string }): PierreTabResolvedIcon {
  const fileName = fileNameForTab(input);
  const token = resolveBuiltInFileIconToken("complete", fileName, extensionCandidates(fileName));
  if (!token) return { name: "file-tree-icon-file" };
  return {
    name: getBuiltInFileIconName(token),
    color: getBuiltInFileIconColor(token),
  };
}

export function tabLabel(path: string | undefined, title: string): string {
  if (!path) return title;
  const trimmed = path.endsWith("/") ? path.slice(0, -1) : path;
  const index = trimmed.lastIndexOf("/");
  return index < 0 ? trimmed : trimmed.slice(index + 1);
}

function fileNameForTab(input: { kind: string; path?: string; title: string }): string {
  if (input.kind === "surface") return tabLabel(input.path, input.title);
  return `${input.title}.txt`;
}

function extensionCandidates(fileName: string): string[] {
  const parts = fileName.toLowerCase().split(".");
  const candidates: string[] = [];
  for (let index = 1; index < parts.length; index += 1) {
    candidates.push(parts.slice(index).join("."));
  }
  return candidates;
}
