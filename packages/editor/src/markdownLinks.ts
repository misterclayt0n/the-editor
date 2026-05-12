export interface MarkdownFileLinkTarget {
  href: string;
  path: string;
  line?: number;
  column?: number;
}

const WINDOWS_DRIVE_PATH_PATTERN = /^[A-Za-z]:[\\/]/;
const WINDOWS_UNC_PATH_PATTERN = /^\\\\/;
const EXTERNAL_SCHEME_PATTERN = /^([A-Za-z][A-Za-z0-9+.-]*):(.*)$/;
const RELATIVE_PATH_PREFIX_PATTERN = /^(~\/|\.{1,2}\/)/;
const RELATIVE_FILE_PATH_PATTERN = /^[A-Za-z0-9._-]+(?:\/[A-Za-z0-9._-]+)+(?::\d+){0,2}$/;
const RELATIVE_FILE_NAME_PATTERN = /^[A-Za-z0-9._-]+\.[A-Za-z0-9_-]+(?::\d+){0,2}$/;
const POSITION_SUFFIX_PATTERN = /:\d+(?::\d+)?$/;
const POSITION_ONLY_PATTERN = /^\d+(?::\d+)?$/;
const POSIX_FILE_ROOT_PREFIXES = [
  "/Users/",
  "/home/",
  "/tmp/",
  "/var/",
  "/etc/",
  "/opt/",
  "/mnt/",
  "/Volumes/",
  "/private/",
  "/root/",
] as const;

export function resolveMarkdownFileLinkTarget(href: string | undefined, cwd?: string): MarkdownFileLinkTarget | null {
  if (!href) return null;
  const rawHref = href.trim();
  if (!rawHref || rawHref.startsWith("#")) return null;

  const fileUrlTarget = rawHref.toLowerCase().startsWith("file:")
    ? parseFileUrlHref(rawHref)
    : null;
  const source = fileUrlTarget ?? stripSearchAndHash(rawHref);
  const decodedPath = fileUrlTarget ? source.path.trim() : safeDecode(source.path.trim());
  const decodedHash = safeDecode(source.hash.trim());

  if (!decodedPath) return null;
  if (!WINDOWS_DRIVE_PATH_PATTERN.test(decodedPath) && !WINDOWS_UNC_PATH_PATTERN.test(decodedPath) && hasExternalScheme(decodedPath)) {
    return null;
  }
  if (!isLikelyPathCandidate(decodedPath)) return null;

  const pathWithPosition = appendLineColumnFromHash(decodedPath, decodedHash);
  const resolved = isRelativePath(pathWithPosition)
    ? (cwd ? resolvePathLinkTarget(pathWithPosition, cwd) : null)
    : pathWithPosition;
  if (!resolved) return null;

  const { path, line, column } = splitPathAndPosition(resolved);
  return {
    href: rawHref,
    path,
    line: line ? Number.parseInt(line, 10) : undefined,
    column: column ? Number.parseInt(column, 10) : undefined,
  };
}

export function isMarkdownExternalLink(href: string | undefined): boolean {
  if (!href) return false;
  const trimmed = href.trim();
  if (!trimmed || trimmed.startsWith("#")) return false;
  const match = trimmed.match(EXTERNAL_SCHEME_PATTERN);
  if (!match) return /^https?:\/\//i.test(trimmed);
  const rest = match[2] ?? "";
  return rest.startsWith("//") || !POSITION_ONLY_PATTERN.test(rest);
}

function safeDecode(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

function stripSearchAndHash(value: string): { path: string; hash: string } {
  const hashIndex = value.indexOf("#");
  const pathWithSearch = hashIndex >= 0 ? value.slice(0, hashIndex) : value;
  const rawHash = hashIndex >= 0 ? value.slice(hashIndex) : "";
  const queryIndex = pathWithSearch.indexOf("?");
  return {
    path: queryIndex >= 0 ? pathWithSearch.slice(0, queryIndex) : pathWithSearch,
    hash: rawHash,
  };
}

function parseFileUrlHref(href: string): { path: string; hash: string } | null {
  try {
    const parsed = new URL(href);
    if (parsed.protocol.toLowerCase() !== "file:") return null;
    const decodedPath = safeDecode(parsed.pathname);
    if (!decodedPath) return null;
    return {
      path: /^\/[A-Za-z]:[\\/]/.test(decodedPath) ? decodedPath.slice(1) : decodedPath,
      hash: parsed.hash,
    };
  } catch {
    return null;
  }
}

function hasExternalScheme(path: string): boolean {
  const match = path.match(EXTERNAL_SCHEME_PATTERN);
  if (!match) return false;
  const rest = match[2] ?? "";
  return rest.startsWith("//") || !POSITION_ONLY_PATTERN.test(rest);
}

function isLikelyPathCandidate(path: string): boolean {
  if (WINDOWS_DRIVE_PATH_PATTERN.test(path) || WINDOWS_UNC_PATH_PATTERN.test(path)) return true;
  if (RELATIVE_PATH_PREFIX_PATTERN.test(path)) return true;
  if (path.startsWith("/")) return looksLikePosixFilesystemPath(path);
  return RELATIVE_FILE_PATH_PATTERN.test(path) || RELATIVE_FILE_NAME_PATTERN.test(path);
}

function looksLikePosixFilesystemPath(path: string): boolean {
  if (!path.startsWith("/")) return false;
  if (POSIX_FILE_ROOT_PREFIXES.some((prefix) => path.startsWith(prefix))) return true;
  if (POSITION_SUFFIX_PATTERN.test(path)) return true;
  const basename = path.slice(path.lastIndexOf("/") + 1);
  return /\.[A-Za-z0-9_-]+$/.test(basename);
}

function appendLineColumnFromHash(path: string, hash: string): string {
  if (!hash || POSITION_SUFFIX_PATTERN.test(path)) return path;
  const match = hash.match(/^#L(\d+)(?:C(\d+))?$/i);
  if (!match?.[1]) return path;
  return `${path}:${match[1]}${match[2] ? `:${match[2]}` : ""}`;
}

function isRelativePath(path: string): boolean {
  return RELATIVE_PATH_PREFIX_PATTERN.test(path)
    || (!path.startsWith("/") && !WINDOWS_DRIVE_PATH_PATTERN.test(path) && !WINDOWS_UNC_PATH_PATTERN.test(path));
}

function resolvePathLinkTarget(rawPath: string, cwd: string): string {
  const { path, line, column } = splitPathAndPosition(rawPath);
  let resolvedPath = path;
  if (path.startsWith("~/")) {
    const home = inferHomeFromCwd(cwd);
    if (home) {
      const separator: "/" | "\\" = isWindowsPathStyle(home) ? "\\" : "/";
      resolvedPath = joinPath(home, path.slice(2), separator);
    }
  } else if (!isAbsolutePath(path)) {
    const separator: "/" | "\\" = isWindowsPathStyle(cwd) ? "\\" : "/";
    resolvedPath = joinPath(cwd, path, separator);
  }
  return line ? `${resolvedPath}:${line}${column ? `:${column}` : ""}` : resolvedPath;
}

function splitPathAndPosition(value: string): { path: string; line?: string; column?: string } {
  let path = value;
  let column: string | undefined;
  let line: string | undefined;
  const columnMatch = path.match(/:(\d+)$/);
  if (!columnMatch?.[1]) return { path };
  column = columnMatch[1];
  path = path.slice(0, -columnMatch[0].length);
  const lineMatch = path.match(/:(\d+)$/);
  if (lineMatch?.[1]) {
    line = lineMatch[1];
    path = path.slice(0, -lineMatch[0].length);
  } else {
    line = column;
    column = undefined;
  }
  return { path, line, column };
}

function inferHomeFromCwd(cwd: string): string | undefined {
  const posixUser = cwd.match(/^\/Users\/([^/]+)/);
  if (posixUser?.[1]) return `/Users/${posixUser[1]}`;
  const posixHome = cwd.match(/^\/home\/([^/]+)/);
  if (posixHome?.[1]) return `/home/${posixHome[1]}`;
  const windowsUser = cwd.match(/^([A-Za-z]:\\Users\\[^\\]+)/);
  if (windowsUser?.[1]) return windowsUser[1];
  return undefined;
}

function isWindowsAbsolutePath(value: string): boolean {
  return /^[A-Za-z]:[\\/]/.test(value) || value.startsWith("\\\\");
}

function isWindowsPathStyle(value: string): boolean {
  return isWindowsAbsolutePath(value) || /[A-Za-z]:\\/.test(value);
}

function isAbsolutePath(value: string): boolean {
  return value.startsWith("/") || isWindowsAbsolutePath(value);
}

function joinPath(base: string, next: string, separator: "/" | "\\"): string {
  const cleanBase = base.replace(/[\\/]+$/, "");
  return separator === "\\"
    ? `${cleanBase}\\${next.replaceAll("/", "\\")}`
    : `${cleanBase}/${next.replace(/^\/+/, "")}`;
}
