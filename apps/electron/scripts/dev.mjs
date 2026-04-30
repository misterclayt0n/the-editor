import { spawn } from "node:child_process";

const children = [];

function run(name, command, args, env = {}) {
  const child = spawn(command, args, {
    stdio: "inherit",
    env: { ...process.env, ...env },
  });
  children.push(child);
  child.on("exit", (code, signal) => {
    if (shuttingDown) return;
    console.error(`${name} exited code=${code ?? "null"} signal=${signal ?? "null"}`);
    shutdown(code ?? 1);
  });
}

let shuttingDown = false;
function shutdown(code = 0) {
  shuttingDown = true;
  for (const child of children) child.kill();
  process.exit(code);
}

process.on("SIGINT", () => shutdown(0));
process.on("SIGTERM", () => shutdown(0));

run("main", "bun", ["run", "dev:main"]);
run("renderer", "bun", ["run", "dev:renderer"]);
run("electron", "bun", ["run", "dev:electron"], {
  THE_EDITOR_DEV_SERVER_URL: "http://127.0.0.1:5173",
});
