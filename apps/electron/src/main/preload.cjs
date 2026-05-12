const fs = require("node:fs");
const path = require("node:path");
const { contextBridge, ipcRenderer } = require("electron");

const sourceExtensions = [".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs"];

contextBridge.exposeInMainWorld("theEditor", {
  openFileDialog() {
    return ipcRenderer.invoke("app:open-file-dialog");
  },
  policyRequest(method, params) {
    return ipcRenderer.invoke("policy:request", { method, params });
  },
  openExternal(url) {
    return ipcRenderer.invoke("app:open-external", url);
  },
  onPolicyEvent(handler) {
    const listener = (_event, payload) => handler(payload);
    ipcRenderer.on("policy:event", listener);
    return () => ipcRenderer.off("policy:event", listener);
  },
  readExtensionFile(filePath) {
    return fs.readFileSync(filePath, "utf8");
  },
  resolveExtensionPath(specifier, importerPath) {
    if (!specifier.startsWith(".") && !specifier.startsWith("/")) return null;
    return resolveLocalModule(specifier, path.dirname(importerPath));
  },
});

function resolveLocalModule(specifier, baseDir) {
  const absolute = path.resolve(baseDir, specifier);
  const resolved = tryResolveFile(absolute);
  if (!resolved) throw new Error(`unable to resolve local module ${specifier} from ${baseDir}`);
  return resolved;
}

function tryResolveFile(candidate) {
  if (fs.existsSync(candidate)) {
    const stats = fs.statSync(candidate);
    if (stats.isFile()) return candidate;
    if (stats.isDirectory()) {
      for (const extension of sourceExtensions) {
        const indexCandidate = path.join(candidate, `index${extension}`);
        if (fs.existsSync(indexCandidate) && fs.statSync(indexCandidate).isFile()) return indexCandidate;
      }
      const jsonIndex = path.join(candidate, "index.json");
      if (fs.existsSync(jsonIndex) && fs.statSync(jsonIndex).isFile()) return jsonIndex;
    }
  }

  const parsed = path.parse(candidate);
  if (parsed.ext) {
    if (parsed.ext === ".js" || parsed.ext === ".jsx" || parsed.ext === ".mjs" || parsed.ext === ".cjs") {
      for (const extension of sourceExtensions) {
        const nextCandidate = path.join(parsed.dir, `${parsed.name}${extension}`);
        if (fs.existsSync(nextCandidate) && fs.statSync(nextCandidate).isFile()) return nextCandidate;
      }
    }
    return null;
  }

  for (const extension of sourceExtensions) {
    const nextCandidate = `${candidate}${extension}`;
    if (fs.existsSync(nextCandidate) && fs.statSync(nextCandidate).isFile()) return nextCandidate;
  }

  const jsonCandidate = `${candidate}.json`;
  if (fs.existsSync(jsonCandidate) && fs.statSync(jsonCandidate).isFile()) return jsonCandidate;
  return null;
}
