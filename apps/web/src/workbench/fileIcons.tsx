/**
 * File-type icons for the workbench chrome, riding @pierre/trees' bundled
 * vscode-icons sprite (the same set the file tree renders, so tabs and
 * tree rows agree). The sprite sheet is injected into the document once;
 * icons are <use> references into it.
 */

import { createFileTreeIconResolver, getBuiltInSpriteSheet } from "@pierre/trees";

const SPRITE_HOST_ID = "the-editor-file-icon-sprite";

const { resolveIcon } = createFileTreeIconResolver("complete");

function ensureSpriteSheet(): void {
  if (document.getElementById(SPRITE_HOST_ID) !== null) return;
  const host = document.createElement("div");
  host.id = SPRITE_HOST_ID;
  host.style.display = "none";
  host.innerHTML = getBuiltInSpriteSheet("complete");
  document.body.append(host);
}

export function FileIcon({ fileName }: { readonly fileName: string }) {
  ensureSpriteSheet();
  const icon = resolveIcon("file-tree-icon-file", fileName);
  return (
    <svg
      className="wb-file-icon"
      viewBox={icon.viewBox ?? "0 0 16 16"}
      aria-hidden="true"
    >
      <use href={`#${icon.name}`} />
    </svg>
  );
}
