/**
 * Chrome painted into an excerpt's reserved block rows. A file's first excerpt
 * gets the full header card (fold chevron + name + path + Open File);
 * subsequent excerpts of the same file get a thin divider.
 */

import { ChevronDownIcon, ChevronRightIcon } from "lucide-react";

import { FileIcon } from "../workbench/fileIcons";
import type { ExcerptMeta } from "./builder";

export type HeaderActions = {
  readonly onOpen: () => void;
  readonly onFold: () => void;
};


export function HeaderCard({ meta, onOpen, onFold }: { meta: ExcerptMeta } & HeaderActions) {
  if (!meta.isFirstOfFile) return <div className="mb-divider" />;
  return (
    <div className="mb-header" data-diffs-header="default">
      <div className="mb-header-content" data-header-content="">
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
        <FileIcon fileName={meta.fileName} />
        <button type="button" className="mb-header-path" title={`Open ${meta.path}`} onClick={onOpen}>
          <span className="mb-header-name" data-title="">
            <bdi>{meta.fileName}</bdi>
          </span>
          {meta.dir ? (
            <span className="mb-header-dir" data-prev-name="">
              <bdi>{meta.dir}</bdi>
            </span>
          ) : null}
        </button>
      </div>
      <div className="mb-header-metadata" data-metadata="">
        {meta.folded ? (
          <span className="mb-header-count">
            {meta.fileMatchCount} {meta.fileMatchCount === 1 ? "match" : "matches"}
          </span>
        ) : null}
        <button type="button" className="mb-header-open" onClick={onOpen}>
          Open File
        </button>
      </div>
    </div>
  );
}
