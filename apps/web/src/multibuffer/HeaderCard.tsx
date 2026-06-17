/**
 * Chrome painted into an excerpt's reserved block rows. A file's first excerpt
 * gets the full header card (fold chevron + name + path + expand/Open File);
 * subsequent excerpts of the same file get a thin divider with expand chevrons.
 * React portals this into the renderer-owned, correctly-sized container.
 */

import { ChevronDownIcon, ChevronRightIcon, ChevronUpIcon } from "lucide-react";

import { FileIcon } from "../workbench/fileIcons";
import type { ExcerptMeta } from "./builder";

export type HeaderActions = {
  readonly onOpen: () => void;
  readonly onFold: () => void;
  readonly onExpandUp: () => void;
  readonly onExpandDown: () => void;
};

function ExpandChevrons({ meta, onExpandUp, onExpandDown }: { meta: ExcerptMeta } & Pick<HeaderActions, "onExpandUp" | "onExpandDown">) {
  if (meta.folded) return null;
  return (
    <>
      {meta.canExpandUp && (
        <button type="button" className="mb-expand" title="Show more above" onClick={onExpandUp}>
          <ChevronUpIcon aria-hidden="true" />
        </button>
      )}
      {meta.canExpandDown && (
        <button type="button" className="mb-expand" title="Show more below" onClick={onExpandDown}>
          <ChevronDownIcon aria-hidden="true" />
        </button>
      )}
    </>
  );
}

export function HeaderCard({ meta, onOpen, onFold, onExpandUp, onExpandDown }: { meta: ExcerptMeta } & HeaderActions) {
  if (!meta.isFirstOfFile) {
    return (
      <div className="mb-divider">
        <ExpandChevrons meta={meta} onExpandUp={onExpandUp} onExpandDown={onExpandDown} />
      </div>
    );
  }
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
        <ExpandChevrons meta={meta} onExpandUp={onExpandUp} onExpandDown={onExpandDown} />
        <button type="button" className="mb-header-open" onClick={onOpen}>
          Open File
        </button>
      </div>
    </div>
  );
}
