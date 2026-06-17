/**
 * Server-root selection, kept dependency-free so it stays unit-testable
 * (the manager itself pulls in browser-only LSP transport).
 */

/**
 * The root a language server is keyed and rooted at for a file. Files inside
 * the open workspace all collapse to ONE root per language (the workspace
 * root — VS Code's model), so a multibuffer spanning every package never
 * spawns a server farm. Out-of-workspace files (a go-to-def into a
 * dependency) fall back to the marker-walk root, keyed independently.
 */
export function chooseServerRoot(
  path: string,
  workspaceRoot: string | null,
  markerRoot: string,
): string {
  if (workspaceRoot === null) return markerRoot;
  const prefix = workspaceRoot.endsWith("/") ? workspaceRoot : `${workspaceRoot}/`;
  return path === workspaceRoot || path.startsWith(prefix) ? workspaceRoot : markerRoot;
}
