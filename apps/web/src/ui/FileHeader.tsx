import type { ReactNode } from "react";

import { FileIcon } from "../workbench/fileIcons";

export function pathBaseName(path: string): string {
  const slash = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return slash === -1 ? path : path.slice(slash + 1);
}

export function pathDirName(path: string): string {
  const slash = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return slash === -1 ? "" : path.slice(0, slash);
}

function trimTrailingSlashes(path: string): string {
  let end = path.length;
  while (end > 1 && path.charCodeAt(end - 1) === 47) end--;
  return end === path.length ? path : path.slice(0, end);
}

export function workspaceRelativePath(path: string, workspaceRoot: string | null): string {
  if (workspaceRoot === null) return path;
  const root = trimTrailingSlashes(workspaceRoot) || "/";
  if (root === "/") return path.startsWith("/") ? path.slice(1) : path;
  const prefix = `${root}/`;
  return path.startsWith(prefix) ? path.slice(prefix.length) : path;
}

export type FileHeaderProps = {
  readonly displayPath: string;
  readonly previousDisplayPath?: string;
  readonly className?: string;
  readonly prefix?: ReactNode;
  readonly metadata?: ReactNode;
  readonly onOpen?: () => void;
  readonly openLabel?: string;
  readonly showOpenButton?: boolean;
  readonly title?: string;
  readonly dataDiffsHeader?: boolean;
};

const classNames = (...parts: readonly (string | undefined | false)[]): string => parts.filter(Boolean).join(" ");

export function FileHeader({
  displayPath,
  previousDisplayPath,
  className,
  prefix,
  metadata,
  onOpen,
  openLabel = "Open File",
  showOpenButton = true,
  title,
  dataDiffsHeader = false,
}: FileHeaderProps) {
  const fileName = pathBaseName(displayPath);
  const dir = pathDirName(displayPath);
  const pathTitle = title ?? (previousDisplayPath === undefined ? displayPath : `${previousDisplayPath} → ${displayPath}`);
  const pathLabel = previousDisplayPath === undefined ? displayPath : `${previousDisplayPath} renamed to ${displayPath}`;
  const pathContent = (
    <>
      {previousDisplayPath === undefined ? null : (
        <span className="file-header-prev diff-view-header-prev">
          <bdi>{previousDisplayPath}</bdi>
          <span className="file-header-prev-arrow diff-view-header-prev-arrow" aria-hidden="true">
            →
          </span>
        </span>
      )}
      <span className="file-header-name mb-header-name" data-title="">
        <bdi>{fileName}</bdi>
      </span>
      {dir.length > 0 ? (
        <span className="file-header-dir mb-header-dir" data-prev-name="">
          <bdi>{dir}</bdi>
        </span>
      ) : null}
    </>
  );

  return (
    <div
      className={classNames("file-header mb-header", className)}
      {...(dataDiffsHeader ? { "data-diffs-header": "default" } : {})}
    >
      <div className="file-header-content mb-header-content" data-header-content="">
        {prefix}
        <FileIcon fileName={fileName} />
        {onOpen === undefined ? (
          <div className="file-header-path mb-header-path" title={pathTitle} aria-label={pathLabel}>
            {pathContent}
          </div>
        ) : (
          <button type="button" className="file-header-path mb-header-path" title={`Open ${pathTitle}`} onClick={onOpen}>
            {pathContent}
          </button>
        )}
      </div>
      <div className="file-header-metadata mb-header-metadata" data-metadata="">
        {metadata}
        {onOpen !== undefined && showOpenButton ? (
          <button type="button" className="file-header-open mb-header-open" onClick={onOpen}>
            {openLabel}
          </button>
        ) : null}
      </div>
    </div>
  );
}
