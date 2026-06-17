import { describe, expect, test } from "vitest";
import { chooseServerRoot } from "./roots";

/**
 * The server-root keying decides how many language servers a multibuffer
 * spawns. Files inside the open workspace MUST collapse to the single
 * workspace root (one server per language), or a search across every package
 * spawns a server farm and freezes the machine.
 */
describe("chooseServerRoot", () => {
  const ws = "/home/me/repo";

  test("files in different packages of the workspace share its root", () => {
    expect(chooseServerRoot(`${ws}/packages/engine/src/a.ts`, ws, `${ws}/packages/engine`)).toBe(ws);
    expect(chooseServerRoot(`${ws}/packages/editor/src/b.ts`, ws, `${ws}/packages/editor`)).toBe(ws);
    expect(chooseServerRoot(`${ws}/apps/web/src/c.ts`, ws, `${ws}/apps/web`)).toBe(ws);
  });

  test("the workspace root file itself maps to the workspace root", () => {
    expect(chooseServerRoot(`${ws}/index.ts`, ws, ws)).toBe(ws);
  });

  test("out-of-workspace files keep their own marker root, keyed independently", () => {
    expect(chooseServerRoot("/usr/lib/node_modules/x/d.ts", ws, "/usr/lib/node_modules/x")).toBe(
      "/usr/lib/node_modules/x",
    );
  });

  test("a sibling whose name shares a prefix is NOT inside the workspace", () => {
    // "/home/me/repo-tools" must not be mistaken for being under "/home/me/repo".
    expect(chooseServerRoot("/home/me/repo-tools/a.ts", ws, "/home/me/repo-tools")).toBe(
      "/home/me/repo-tools",
    );
  });

  test("no workspace open falls back to the marker root", () => {
    expect(chooseServerRoot(`${ws}/packages/engine/a.ts`, null, `${ws}/packages/engine`)).toBe(
      `${ws}/packages/engine`,
    );
  });

  test("a filesystem-root workspace contains every absolute path", () => {
    expect(chooseServerRoot("/anywhere/a.ts", "/", "/anywhere")).toBe("/");
  });
});
