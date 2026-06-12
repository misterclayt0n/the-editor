import { describe, expect, test } from "vitest";
import {
  createByteConnection,
  DocumentSyncState,
  globMatches,
  LspClient,
  offsetAtPosition,
  pathToUri,
  uriToPath,
  lsp,
  type ByteStreams,
} from "../src/index";

/** Two linked byte streams: bytes sent by one side arrive at the other. */
const streamPair = (): [ByteStreams, ByteStreams] => {
  const aChunk: Array<(c: Uint8Array) => void> = [];
  const bChunk: Array<(c: Uint8Array) => void> = [];
  const a: ByteStreams = {
    send: (chunk) => queueMicrotask(() => bChunk.forEach((l) => l(chunk))),
    onChunk: (l) => aChunk.push(l),
    onClose: () => {},
  };
  const b: ByteStreams = {
    send: (chunk) => queueMicrotask(() => aChunk.forEach((l) => l(chunk))),
    onChunk: (l) => bChunk.push(l),
    onClose: () => {},
  };
  return [a, b];
};

/** Apply LSP contentChanges sequentially (the server's view of didChange). */
const applyContentChanges = (
  text: string,
  changes: lsp.TextDocumentContentChangeEvent[],
): string => {
  for (const change of changes) {
    if (!("range" in change)) {
      text = change.text;
      continue;
    }
    const start = offsetAtPosition(text, change.range.start);
    const end = offsetAtPosition(text, change.range.end);
    text = text.slice(0, start) + change.text + text.slice(end);
  }
  return text;
};

describe("uri helpers", () => {
  test("roundtrip with spaces and unicode", () => {
    const path = "/tmp/my dir/héllo.rs";
    expect(uriToPath(pathToUri(path))).toBe(path);
  });
});

describe("DocumentSyncState", () => {
  test("single edit converts to the right range", () => {
    const state = new DocumentSyncState("hello\nworld\n");
    const changes = state.applyEdits([{ start: 8, end: 10, text: "XY" }]);
    expect(changes).toEqual([
      {
        range: { start: { line: 1, character: 2 }, end: { line: 1, character: 4 } },
        text: "XY",
      },
    ]);
  });

  test("random multi-edit transactions replay identically on the server", () => {
    let seed = 7;
    const rand = (n: number) => {
      seed = (seed * 1103515245 + 12345) & 0x7fffffff;
      return seed % n;
    };
    let editorText = Array.from({ length: 40 }, (_, i) => `line number ${i}`).join("\n");
    let serverText = editorText;
    const state = new DocumentSyncState(editorText);

    for (let round = 0; round < 60; round++) {
      // Build 1-3 ascending disjoint edits in OLD coordinates.
      const editCount = 1 + rand(3);
      const edits = [];
      let cursor = 0;
      for (let i = 0; i < editCount && cursor < editorText.length; i++) {
        const start = cursor + rand(Math.max(1, (editorText.length - cursor) >> 1));
        const end = Math.min(editorText.length, start + rand(8));
        const text = ["", "x", "two\nlines", "\n", "abc def"][rand(5)]!;
        edits.push({ start, end, text });
        cursor = end + 1;
      }
      // Editor applies simultaneously (descending application).
      let next = editorText;
      for (let i = edits.length - 1; i >= 0; i--) {
        const e = edits[i]!;
        next = next.slice(0, e.start) + e.text + next.slice(e.end);
      }
      editorText = next;
      // Server applies the converted sequential changes.
      serverText = applyContentChanges(serverText, state.applyEdits(edits));
      expect(serverText, `round ${round}`).toBe(editorText);
    }
  });
});

describe("globMatches", () => {
  test("common watcher patterns", () => {
    expect(globMatches("**/*.rs", "/proj/src/main.rs")).toBe(true);
    expect(globMatches("**/*.rs", "/proj/src/main.ts")).toBe(false);
    expect(globMatches("**/Cargo.toml", "/proj/Cargo.toml")).toBe(true);
    expect(globMatches("**/*.{ts,tsx}", "/a/b/c.tsx")).toBe(true);
    expect(globMatches("*.rs", "main.rs")).toBe(true);
    expect(globMatches("*.rs", "src/main.rs")).toBe(false);
  });
});

describe("client over the real wire", () => {
  /** Fake server speaking through the actual framed connection. */
  const makeServer = (streams: ByteStreams) => {
    const connection = createByteConnection(streams);
    const documents = new Map<string, string>();
    const log: string[] = [];
    connection.onRequest(
      lsp.InitializeRequest.type,
      (): lsp.InitializeResult => ({
        capabilities: {
          textDocumentSync: { openClose: true, change: lsp.TextDocumentSyncKind.Incremental },
          definitionProvider: true,
          documentFormattingProvider: true,
          renameProvider: true,
          inlayHintProvider: true,
        },
        serverInfo: { name: "fake" },
      }),
    );
    connection.onNotification(lsp.InitializedNotification.type, () => {
      log.push("initialized");
    });
    connection.onNotification(lsp.DidOpenTextDocumentNotification.type, (p) => {
      documents.set(p.textDocument.uri, p.textDocument.text);
    });
    connection.onNotification(lsp.DidChangeTextDocumentNotification.type, (p) => {
      const text = applyContentChanges(
        documents.get(p.textDocument.uri) ?? "",
        p.contentChanges as lsp.TextDocumentContentChangeEvent[],
      );
      documents.set(p.textDocument.uri, text);
      void connection.sendNotification(lsp.PublishDiagnosticsNotification.type, {
        uri: p.textDocument.uri,
        version: p.textDocument.version,
        diagnostics: text.includes("badword")
          ? [
              {
                range: { start: { line: 0, character: 0 }, end: { line: 0, character: 1 } },
                message: "bad",
                severity: 1,
              },
            ]
          : [],
      });
    });
    connection.onRequest(lsp.DefinitionRequest.type, () => ({
      uri: "file:///proj/def.ts",
      range: { start: { line: 3, character: 2 }, end: { line: 3, character: 8 } },
    }));
    connection.onRequest(lsp.DocumentFormattingRequest.type, () => [
      {
        range: { start: { line: 0, character: 0 }, end: { line: 0, character: 0 } },
        newText: "// formatted\n",
      },
    ]);
    connection.onRequest(lsp.RenameRequest.type, (p) => ({
      changes: {
        [p.textDocument.uri]: [
          {
            range: { start: { line: 0, character: 0 }, end: { line: 0, character: 3 } },
            newText: p.newName,
          },
        ],
      },
    }));
    connection.onRequest(lsp.InlayHintRequest.type, () => [
      { position: { line: 0, character: 5 }, label: ": number" },
    ]);
    return { connection, documents, log };
  };

  test("initialize, incremental sync, diagnostics, features", async () => {
    const [clientSide, serverSide] = streamPair();
    const server = makeServer(serverSide);
    const published: lsp.PublishDiagnosticsParams[] = [];
    const client = new LspClient(createByteConnection(clientSide), {
      rootUri: "file:///proj",
      onDiagnostics: (p) => published.push(p),
    });
    const init = await client.initialize();
    expect(init.serverInfo?.name).toBe("fake");
    expect(client.syncKind()).toBe(lsp.TextDocumentSyncKind.Incremental);
    expect(client.supports(lsp.DefinitionRequest.method)).toBe(true);
    expect(client.supports(lsp.CompletionRequest.method)).toBe(false);

    const uri = "file:///proj/a.ts";
    const editorBefore = "const x = 1;\n";
    client.openDocument(uri, "typescript", 0, editorBefore);

    // Incremental change: insert "badword " at offset 6.
    const sync = new DocumentSyncState(editorBefore);
    client.changeDocumentIncremental(uri, 1, sync.applyEdits([{ start: 6, end: 6, text: "badword " }]));
    await new Promise((r) => setTimeout(r, 20));
    expect(server.documents.get(uri)).toBe("const badword x = 1;\n");
    expect(published.at(-1)?.diagnostics).toHaveLength(1);

    const definitions = await client.definition(uri, { line: 0, character: 6 });
    expect(definitions[0]?.uri).toBe("file:///proj/def.ts");
    const edits = await client.formatting(uri, { tabSize: 2, insertSpaces: true });
    expect(edits[0]?.newText).toBe("// formatted\n");
    const workspaceEdit = await client.rename(uri, { line: 0, character: 6 }, "renamed");
    expect(workspaceEdit?.changes?.[uri]?.[0]?.newText).toBe("renamed");
    const hints = await client.inlayHints(uri, {
      start: { line: 0, character: 0 },
      end: { line: 1, character: 0 },
    });
    expect(hints[0]?.label).toBe(": number");
    await client.shutdown(100);
  });

  test("dynamic registration drives supports() and watched patterns", async () => {
    const [clientSide, serverSide] = streamPair();
    const serverConnection = createByteConnection(serverSide);
    serverConnection.onRequest(lsp.InitializeRequest.type, (): lsp.InitializeResult => ({ capabilities: {} }));
    const client = new LspClient(createByteConnection(clientSide), { rootUri: null });
    await client.initialize();
    expect(client.supports(lsp.DefinitionRequest.method)).toBe(false);

    await serverConnection.sendRequest(lsp.RegistrationRequest.type, {
      registrations: [
        { id: "r1", method: lsp.DefinitionRequest.method },
        {
          id: "r2",
          method: lsp.DidChangeWatchedFilesNotification.method,
          registerOptions: { watchers: [{ globPattern: "**/Cargo.toml" }] },
        },
      ],
    });
    expect(client.supports(lsp.DefinitionRequest.method)).toBe(true);
    expect(client.watchedFilePatterns()).toEqual(["**/Cargo.toml"]);

    await serverConnection.sendRequest(lsp.UnregistrationRequest.type, {
      unregisterations: [{ id: "r1", method: lsp.DefinitionRequest.method }],
    });
    expect(client.supports(lsp.DefinitionRequest.method)).toBe(false);
  });

  test("workspace/configuration answers from settings by section", async () => {
    const [clientSide, serverSide] = streamPair();
    const serverConnection = createByteConnection(serverSide);
    serverConnection.onRequest(lsp.InitializeRequest.type, (): lsp.InitializeResult => ({ capabilities: {} }));
    const client = new LspClient(createByteConnection(clientSide), {
      rootUri: null,
      settings: () => ({ "rust-analyzer": { checkOnSave: true } }),
    });
    await client.initialize();
    const result = await serverConnection.sendRequest(lsp.ConfigurationRequest.type, {
      items: [{ section: "rust-analyzer.checkOnSave" }, { section: "missing.thing" }],
    });
    expect(result).toEqual([true, null]);
  });

  test("workspace/applyEdit reaches the host hook", async () => {
    const [clientSide, serverSide] = streamPair();
    const serverConnection = createByteConnection(serverSide);
    serverConnection.onRequest(lsp.InitializeRequest.type, (): lsp.InitializeResult => ({ capabilities: {} }));
    const applied: lsp.ApplyWorkspaceEditParams[] = [];
    const client = new LspClient(createByteConnection(clientSide), {
      rootUri: null,
      onApplyEdit: (params) => {
        applied.push(params);
        return { applied: true };
      },
    });
    await client.initialize();
    const result = await serverConnection.sendRequest(lsp.ApplyWorkspaceEditRequest.type, {
      edit: { changes: {} },
    });
    expect(result).toEqual({ applied: true });
    expect(applied).toHaveLength(1);
  });
});
