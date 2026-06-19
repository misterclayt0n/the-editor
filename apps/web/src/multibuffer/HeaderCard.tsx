/**
 * Chrome painted into an excerpt's reserved block rows. A file's first excerpt
 * gets the full header card (fold chevron + name + path + Open File);
 * subsequent excerpts of the same file get a thin divider.
 */

import type { ReactNode } from "react";
import { ChevronDownIcon, ChevronRightIcon } from "lucide-react";

import { FileHeader, workspaceRelativePath } from "../ui/FileHeader";
import type { ExcerptMeta } from "./builder";

export type HeaderActions = {
  readonly onOpen: () => void;
  readonly onFold: () => void;
};


export function HeaderCard({
  meta,
  workspaceRoot,
  onOpen,
  onFold,
}: { meta: ExcerptMeta; workspaceRoot: string | null } & HeaderActions) {
  if (!meta.isFirstOfFile) return <div className="mb-divider" />;
  const prefix = (
    <button
      type="button"
      className="mb-fold"
      title={meta.folded ? "Unfold file" : "Fold file"}
      aria-label={meta.folded ? "Unfold file" : "Fold file"}
      aria-pressed={meta.folded}
      onClick={onFold}
    >
      {meta.folded ? <ChevronRightIcon aria-hidden="true" /> : <ChevronDownIcon aria-hidden="true" />}
    </button>
  );
  const metadata: ReactNode = meta.folded ? (
    <span className="mb-header-count">
      {meta.fileMatchCount} {meta.fileMatchCount === 1 ? "match" : "matches"}
    </span>
  ) : null;
  const displayPath = workspaceRelativePath(meta.path, workspaceRoot);
  return (
    <FileHeader
      displayPath={displayPath}
      prefix={prefix}
      metadata={metadata}
      onOpen={onOpen}
      dataDiffsHeader
    />
  );
}
