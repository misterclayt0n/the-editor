// Dev orchestrator, simplified from t3code's dev flow:
// 1. start the web renderer's Vite dev server
// 2. bundle main/preload with tsdown
// 3. launch Electron pointed at the dev server
import { spawn, spawnSync } from "node:child_process";
import { connect } from "node:net";
import { chmodSync, existsSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const desktopDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const webDir = resolve(desktopDir, "..", "web");

/**
 * node-pty's `spawn-helper` is a Mach-O/ELF executable that pty.fork()
 * posix_spawn()s; the prebuilt tarball can extract it without the +x bit (a
 * pnpm quirk), and then every spawn fails with "posix_spawnp failed". Re-arm
 * the execute bit on every dev launch so a fresh install just works.
 */
function ensurePtyHelperExecutable() {
  try {
    const require = createRequire(import.meta.url);
    const ptyRoot = resolve(dirname(require.resolve("node-pty")), "..");
    const candidates = [resolve(ptyRoot, "build/Release/spawn-helper")];
    const prebuilds = resolve(ptyRoot, "prebuilds");
    if (existsSync(prebuilds)) {
      for (const entry of readdirSync(prebuilds)) {
        candidates.push(resolve(prebuilds, entry, "spawn-helper"));
      }
    }
    for (const candidate of candidates) {
      if (existsSync(candidate)) chmodSync(candidate, 0o755);
    }
  } catch {
    // node-pty is optional; the terminal panel just won't have a shell.
  }
}
ensurePtyHelperExecutable();

const port = Number(process.env.PORT ?? 5183);
const devServerUrl = `http://localhost:${port}`;

const require = createRequire(import.meta.url);
const electronPath = require("electron");

const children = [];
let shuttingDown = false;

function shutdown(code) {
  if (shuttingDown) return;
  shuttingDown = true;
  for (const child of children) child.kill("SIGTERM");
  process.exit(code);
}

process.once("SIGINT", () => shutdown(130));
process.once("SIGTERM", () => shutdown(143));

function bin(dir, name) {
  return join(dir, "node_modules", ".bin", name);
}

const bundle = spawnSync(bin(desktopDir, "tsdown"), { cwd: desktopDir, stdio: "inherit" });
if (bundle.status !== 0) {
  console.error("[dev-electron] tsdown bundle failed");
  process.exit(bundle.status ?? 1);
}

const vite = spawn(bin(webDir, "vite"), ["--port", String(port), "--strictPort"], {
  cwd: webDir,
  stdio: "inherit",
});
children.push(vite);
vite.once("exit", (code) => shutdown(code ?? 0));

function waitForPort(host, targetPort, timeoutMs = 30_000) {
  const startedAt = Date.now();
  return new Promise((resolvePromise, rejectPromise) => {
    const attempt = () => {
      const socket = connect({ host, port: targetPort }, () => {
        socket.destroy();
        resolvePromise();
      });
      socket.once("error", () => {
        socket.destroy();
        if (Date.now() - startedAt > timeoutMs) {
          rejectPromise(new Error(`timed out waiting for ${host}:${targetPort}`));
          return;
        }
        setTimeout(attempt, 200);
      });
    };
    attempt();
  });
}

await waitForPort("localhost", port);

const childEnv = { ...process.env, VITE_DEV_SERVER_URL: devServerUrl };
delete childEnv.ELECTRON_RUN_AS_NODE;

const electron = spawn(electronPath, ["dist-electron/main.cjs"], {
  cwd: desktopDir,
  env: childEnv,
  stdio: "inherit",
});
children.push(electron);
electron.once("exit", (code) => shutdown(code ?? 0));
