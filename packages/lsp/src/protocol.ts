/** URI helpers; protocol types come from vscode-languageserver-protocol. */

export function pathToUri(path: string): string {
  return `file://${path.split("/").map(encodeURIComponent).join("/")}`;
}

export function uriToPath(uri: string): string | null {
  if (!uri.startsWith("file://")) return null;
  return uri
    .slice("file://".length)
    .split("/")
    .map(decodeURIComponent)
    .join("/");
}
