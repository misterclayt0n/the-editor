import { spawn, type ChildProcessByStdio } from "node:child_process";
import type { Writable, Readable } from "node:stream";
import { createInterface } from "node:readline";
import type {
  DocumentId,
  DocumentOpened,
  JsonRpcResponse,
  JsonValue,
  OkResult,
  RenderFrameDelta,
  RuntimeApi,
  RuntimeRpcError,
  SurfaceCreated,
  SurfaceId,
  Viewport,
} from "@the-editor/contracts/runtime";

export class RuntimeClientError extends Error {
  constructor(readonly rpcError: RuntimeRpcError) {
    super(rpcError.message);
    this.name = "RuntimeClientError";
  }
}

export interface RuntimeProcessClientOptions {
  command?: string;
  args?: string[];
  cwd?: string;
  stderr?: "inherit" | "pipe" | "ignore";
}

export class RuntimeProcessClient implements RuntimeApi {
  readonly server = {
    ping: (): Promise<OkResult> => this.request("server.ping", {}),
  };

  readonly documents = {
    openFile: (params: { path: string }): Promise<DocumentOpened> =>
      this.request("documents.openFile", params),
    createScratch: (params: { text?: string }): Promise<DocumentOpened> =>
      this.request("documents.createScratch", params),
  };

  readonly surfaces = {
    create: (params: { documentId: DocumentId; viewport?: Viewport }): Promise<SurfaceCreated> =>
      this.request("surfaces.create", params),
    setViewport: (params: { surfaceId: SurfaceId; viewport: Viewport }): Promise<OkResult> =>
      this.request("surfaces.setViewport", params),
    renderFrame: (params: {
      surfaceId: SurfaceId;
      viewport?: Viewport;
      sinceGeneration?: number | null;
    }): Promise<RenderFrameDelta> => this.request("surfaces.renderFrame", params),
  };

  readonly resources = {
    save: (params: { surfaceId: SurfaceId }): Promise<OkResult> =>
      this.request("resources.save", params),
  };

  private nextId = 1;
  private readonly pending = new Map<
    number,
    {
      resolve(value: unknown): void;
      reject(reason: unknown): void;
    }
  >();
  private readonly child: ChildProcessByStdio<Writable, Readable, Readable | null>;

  constructor(options: RuntimeProcessClientOptions = {}) {
    const command = options.command ?? "cargo";
    const args = options.args ?? ["run", "-q", "-p", "the-runtime", "--bin", "the-runtime"];
    this.child = spawn(command, args, {
      cwd: options.cwd,
      stdio: ["pipe", "pipe", options.stderr ?? "inherit"],
    }) as ChildProcessByStdio<Writable, Readable, Readable | null>;

    const lines = createInterface({ input: this.child.stdout });
    lines.on("line", (line) => this.handleLine(line));
    this.child.on("exit", (code, signal) => {
      const error = new Error(`runtime process exited code=${code ?? "null"} signal=${signal ?? "null"}`);
      for (const pending of this.pending.values()) {
        pending.reject(error);
      }
      this.pending.clear();
    });
  }

  dispose(): void {
    this.child.kill();
  }

  request<TResult>(method: string, params: JsonValue | object): Promise<TResult> {
    const id = this.nextId++;
    const payload = JSON.stringify({ id, method, params });
    return new Promise<TResult>((resolve, reject) => {
      this.pending.set(id, {
        resolve: (value) => resolve(value as TResult),
        reject,
      });
      this.child.stdin.write(`${payload}\n`, (error) => {
        if (!error) return;
        this.pending.delete(id);
        reject(error);
      });
    });
  }

  private handleLine(line: string): void {
    let response: JsonRpcResponse;
    try {
      response = JSON.parse(line) as JsonRpcResponse;
    } catch (error) {
      console.error("failed to parse runtime response", { line, error });
      return;
    }

    if (typeof response.id !== "number") {
      console.error("runtime response without numeric id", response);
      return;
    }

    const pending = this.pending.get(response.id);
    if (!pending) return;
    this.pending.delete(response.id);

    if ("error" in response) {
      pending.reject(new RuntimeClientError(response.error));
    } else {
      pending.resolve(response.result);
    }
  }
}

export function spawnRuntimeClient(options?: RuntimeProcessClientOptions): RuntimeProcessClient {
  return new RuntimeProcessClient(options);
}
