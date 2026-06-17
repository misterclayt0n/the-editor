/**
 * Chrome painted into an excerpt's reserved block rows. A file's first excerpt
 * gets the full header card (fold chevron + name + path + expand/Open File);
 * subsequent excerpts of the same file get a thin divider with expand chevrons.
 * React portals this into the renderer-owned, correctly-sized container.
 */

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
          ⌃
        </button>
      )}
      {meta.canExpandDown && (
        <button type="button" className="mb-expand" title="Show more below" onClick={onExpandDown}>
          ⌄
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
    <div className="mb-header">
      <button
        type="button"
        className="mb-fold"
        title={meta.folded ? "Unfold file" : "Fold file"}
        onClick={onFold}
      >
        {meta.folded ? "▸" : "▾"}
      </button>
      <button type="button" className="mb-header-path" title={`Open ${meta.path}`} onClick={onOpen}>
        <span className="mb-header-name">{meta.fileName}</span>
        {meta.dir ? <span className="mb-header-dir">{meta.dir}</span> : null}
        {meta.folded ? (
          <span className="mb-header-count">
            {meta.fileMatchCount} {meta.fileMatchCount === 1 ? "match" : "matches"}
          </span>
        ) : null}
      </button>
      <div className="mb-header-actions">
        <ExpandChevrons meta={meta} onExpandUp={onExpandUp} onExpandDown={onExpandDown} />
        <button type="button" className="mb-header-open" onClick={onOpen}>
          Open File
        </button>
      </div>
    </div>
  );
}
