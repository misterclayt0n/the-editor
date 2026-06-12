/**
 * MessageConnection over raw byte streams (a spawned server's stdio,
 * crossing the ProcessPort). vscode-jsonrpc owns framing, correlation,
 * cancellation ($/cancelRequest) and progress; we only adapt our chunk
 * callbacks to its RAL stream interfaces.
 */

import {
  createProtocolConnection,
  ReadableStreamMessageReader,
  WriteableStreamMessageWriter,
  type Disposable,
  type ProtocolConnection,
} from "vscode-languageserver-protocol/browser";

export type ByteStreams = {
  /** Write a chunk to the server's stdin. */
  readonly send: (chunk: Uint8Array) => void;
  /** Single-subscriber stdout chunks. */
  readonly onChunk: (listener: (chunk: Uint8Array) => void) => void;
  /** Process exited. */
  readonly onClose: (listener: () => void) => void;
};

const disposable = (dispose: () => void): Disposable => ({ dispose });

export function createByteConnection(streams: ByteStreams): ProtocolConnection {
  const dataListeners = new Set<(data: Uint8Array) => void>();
  const closeListeners = new Set<() => void>();
  streams.onChunk((chunk) => {
    for (const listener of dataListeners) listener(chunk);
  });
  streams.onClose(() => {
    for (const listener of closeListeners) listener();
  });

  const reader = new ReadableStreamMessageReader({
    onData: (listener: (data: Uint8Array) => void) => {
      dataListeners.add(listener);
      return disposable(() => dataListeners.delete(listener));
    },
    onClose: (listener: () => void) => {
      closeListeners.add(listener);
      return disposable(() => closeListeners.delete(listener));
    },
    onError: () => disposable(() => {}),
    onEnd: (listener: () => void) => {
      closeListeners.add(listener);
      return disposable(() => closeListeners.delete(listener));
    },
  });

  const encoder = new TextEncoder();
  const writer = new WriteableStreamMessageWriter({
    onClose: (listener: () => void) => {
      closeListeners.add(listener);
      return disposable(() => closeListeners.delete(listener));
    },
    onError: () => disposable(() => {}),
    onEnd: (listener: () => void) => {
      closeListeners.add(listener);
      return disposable(() => closeListeners.delete(listener));
    },
    write: (data: Uint8Array | string) => {
      streams.send(typeof data === "string" ? encoder.encode(data) : data);
      return Promise.resolve();
    },
    end: () => {},
  });

  const connection = createProtocolConnection(reader, writer);
  connection.listen();
  return connection;
}
