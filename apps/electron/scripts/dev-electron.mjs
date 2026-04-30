import { spawn } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";

const env = {
  ...process.env,
  THE_EDITOR_DEV_SERVER_URL: "http://127.0.0.1:5173",
};

setTimeout(() => {
  mkdirSync("dist/main", { recursive: true });
  copyFileSync("src/main/preload.cjs", "dist/main/preload.cjs");
  const child = spawn("electron", ["dist/main/main.cjs"], {
    stdio: "inherit",
    env,
  });
  child.on("exit", (code) => process.exit(code ?? 0));
}, 1500);
