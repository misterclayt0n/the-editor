// Launch Electron against the built renderer (apps/web/dist).
import { spawn } from "node:child_process";
import { chmodSync, existsSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const desktopDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const webDist = resolve(desktopDir, "..", "web", "dist", "index.html");

// node-pty's spawn-helper can lose its +x bit on install (pnpm + prebuilt
// tarball); re-arm it so pty.fork() doesn't fail with "posix_spawnp failed".
try {
  const ptyRoot = resolve(dirname(createRequire(import.meta.url).resolve("node-pty")), "..");
  const candidates = [resolve(ptyRoot, "build/Release/spawn-helper")];
  const prebuilds = resolve(ptyRoot, "prebuilds");
  if (existsSync(prebuilds)) {
    for (const entry of readdirSync(prebuilds)) candidates.push(resolve(prebuilds, entry, "spawn-helper"));
  }
  for (const candidate of candidates) if (existsSync(candidate)) chmodSync(candidate, 0o755);
} catch {
  // node-pty optional; terminal just won't have a shell.
}

if (!existsSync(join(desktopDir, "dist-electron", "main.cjs"))) {
  console.error("[start-electron] missing dist-electron/main.cjs — run `pnpm -s build` first");
  process.exit(1);
}
if (!existsSync(webDist)) {
  console.error("[start-electron] missing apps/web/dist — run `pnpm -s build` first");
  process.exit(1);
}

const require = createRequire(import.meta.url);
const electronPath = require("electron");

const childEnv = { ...process.env };
delete childEnv.ELECTRON_RUN_AS_NODE;
delete childEnv.VITE_DEV_SERVER_URL;

const electron = spawn(electronPath, ["dist-electron/main.cjs"], {
  cwd: desktopDir,
  env: childEnv,
  stdio: "inherit",
});
electron.once("exit", (code) => process.exit(code ?? 0));
