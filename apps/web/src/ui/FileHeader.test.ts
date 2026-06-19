import { describe, expect, test } from "vitest";
import { pathBaseName, pathDirName, workspaceRelativePath } from "./FileHeader";

describe("FileHeader path helpers", () => {
  test("displays paths relative to the active workspace root", () => {
    expect(
      workspaceRelativePath(
        "/Users/misterclayt0n/code/the-editor/apps/web/src/ui/FileFinder.tsx",
        "/Users/misterclayt0n/code/the-editor",
      ),
    ).toBe("apps/web/src/ui/FileFinder.tsx");
  });

  test("leaves out-of-workspace paths unchanged", () => {
    expect(workspaceRelativePath("/tmp/outside/file.ts", "/Users/misterclayt0n/code/the-editor")).toBe(
      "/tmp/outside/file.ts",
    );
  });

  test("splits display path into file name and directory", () => {
    const path = "apps/web/src/ui/FileFinder.tsx";
    expect(pathBaseName(path)).toBe("FileFinder.tsx");
    expect(pathDirName(path)).toBe("apps/web/src/ui");
  });
});
