#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import sys
import threading
import time
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
  parser = argparse.ArgumentParser(
    description="Mock stdio LSP server for deterministic the-term integration tests.",
  )
  parser.add_argument(
    "--mode",
    choices=("normal", "timeout", "crash-init", "ignore-init"),
    default="normal",
    help="Server behavior mode.",
  )
  parser.add_argument(
    "--log",
    default="/tmp/the-editor-mock-lsp.log",
    help="JSONL log path for incoming/outgoing protocol messages.",
  )
  parser.add_argument(
    "--hover-delay",
    type=float,
    default=1.5,
    help="Delay for hover responses in normal mode (seconds).",
  )
  parser.add_argument(
    "--timeout-delay",
    type=float,
    default=12.0,
    help="Delay for hover responses in timeout mode (seconds).",
  )
  parser.add_argument(
    "--startup-progress",
    action="store_true",
    help="Emit a work-done progress sequence right after initialized.",
  )
  return parser.parse_args()


class MockLsp:
  def __init__(self, args: argparse.Namespace):
    self.args = args
    self.log_path = Path(args.log)
    self.log_path.parent.mkdir(parents=True, exist_ok=True)
    self.log_file = self.log_path.open("a", encoding="utf-8", buffering=1)

    self.stdout_lock = threading.Lock()
    self.state_lock = threading.Lock()
    self.cancelled_request_ids: set[int] = set()
    self.exit_requested = False
    self.sent_startup_progress = False
    self.next_server_request_id = 9000

  def log(self, direction: str, payload: Any, note: str | None = None) -> None:
    entry: dict[str, Any] = {
      "ts": time.time(),
      "dir": direction,
      "payload": payload,
    }
    if note is not None:
      entry["note"] = note
    self.log_file.write(json.dumps(entry, ensure_ascii=False) + "\n")

  def read_message(self) -> dict[str, Any] | None:
    content_length: int | None = None
    while True:
      header_line = sys.stdin.buffer.readline()
      if header_line == b"":
        return None
      if header_line in (b"\r\n", b"\n"):
        break
      decoded = header_line.decode("utf-8", errors="replace").strip()
      if decoded.lower().startswith("content-length:"):
        _, value = decoded.split(":", 1)
        content_length = int(value.strip())

    if content_length is None:
      raise ValueError("missing Content-Length header")

    body = sys.stdin.buffer.read(content_length)
    if len(body) < content_length:
      return None
    message = json.loads(body.decode("utf-8"))
    self.log("in", message)
    return message

  def send(self, message: dict[str, Any], *, note: str | None = None) -> None:
    encoded = json.dumps(message, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    header = f"Content-Length: {len(encoded)}\r\n\r\n".encode("ascii")
    with self.stdout_lock:
      sys.stdout.buffer.write(header)
      sys.stdout.buffer.write(encoded)
      sys.stdout.buffer.flush()
    self.log("out", message, note=note)

  def send_response(self, request_id: Any, result: Any = None, error: Any = None) -> None:
    response: dict[str, Any] = {
      "jsonrpc": "2.0",
      "id": request_id,
    }
    if error is None:
      response["result"] = result
    else:
      response["error"] = error
    self.send(response)

  def send_notification(self, method: str, params: Any | None = None) -> None:
    notification: dict[str, Any] = {
      "jsonrpc": "2.0",
      "method": method,
    }
    if params is not None:
      notification["params"] = params
    self.send(notification)

  def send_server_request(self, method: str, params: Any | None = None) -> int:
    request_id = self.next_server_request_id
    self.next_server_request_id += 1
    request: dict[str, Any] = {
      "jsonrpc": "2.0",
      "id": request_id,
      "method": method,
    }
    if params is not None:
      request["params"] = params
    self.send(request)
    return request_id

  def handle_request(self, message: dict[str, Any]) -> None:
    method = str(message.get("method"))
    request_id = message.get("id")
    params = message.get("params")

    if method == "initialize":
      if self.args.mode == "crash-init":
        self.log("meta", {"mode": self.args.mode}, note="exiting on initialize")
        sys.exit(1)
      if self.args.mode == "ignore-init":
        self.log("meta", {"mode": self.args.mode}, note="ignoring initialize")
        return

      capabilities = {
        "textDocumentSync": {
          "openClose": True,
          "change": 2,
          "save": {"includeText": True},
        },
        "hoverProvider": True,
        "definitionProvider": True,
        "referencesProvider": True,
        "documentSymbolProvider": True,
        "workspaceSymbolProvider": True,
        "completionProvider": {},
        "signatureHelpProvider": {},
        "codeActionProvider": True,
        "renameProvider": True,
        "documentFormattingProvider": True,
      }
      self.send_response(
        request_id,
        result={
          "capabilities": capabilities,
          "serverInfo": {"name": "the-editor-mock-lsp", "version": "0.1.0"},
        },
      )
      return

    if method == "shutdown":
      self.send_response(request_id, result=None)
      return

    if method == "textDocument/definition":
      uri = None
      if isinstance(params, dict):
        uri = params.get("textDocument", {}).get("uri")
      self.send_response(
        request_id,
        result=[
          {
            "uri": uri or "file:///tmp/mock.rs",
            "range": {
              "start": {"line": 0, "character": 0},
              "end": {"line": 0, "character": 5},
            },
          }
        ],
      )
      return

    if method == "textDocument/references":
      self.send_response(request_id, result=[])
      return

    if method == "textDocument/documentSymbol":
      self.send_response(request_id, result=[])
      return

    if method == "workspace/symbol":
      self.send_response(request_id, result=[])
      return

    if method == "textDocument/hover":
      delay = self.args.timeout_delay if self.args.mode == "timeout" else self.args.hover_delay
      self.schedule_hover_response(request_id, delay)
      return

    self.send_response(request_id, result=None)

  def schedule_hover_response(self, request_id: Any, delay: float) -> None:
    token = f"hover-{request_id}"
    self.send_notification(
      "$/progress",
      {
        "token": token,
        "value": {
          "kind": "begin",
          "title": "hover",
          "message": "mock hover started",
          "percentage": 0,
        },
      },
    )

    def worker() -> None:
      time.sleep(delay)
      canceled = False
      if isinstance(request_id, int):
        with self.state_lock:
          canceled = request_id in self.cancelled_request_ids

      if canceled:
        self.log("meta", {"id": request_id}, note="hover canceled")
        self.send_notification(
          "$/progress",
          {
            "token": token,
            "value": {
              "kind": "end",
              "message": "mock hover canceled",
            },
          },
        )
        return

      self.send_response(
        request_id,
        result={
          "contents": {
            "kind": "markdown",
            "value": "Mock hover documentation from `mock_lsp.py`.",
          }
        },
      )
      self.send_notification(
        "$/progress",
        {
          "token": token,
          "value": {
            "kind": "end",
            "message": "mock hover completed",
          },
        },
      )

    threading.Thread(target=worker, daemon=True).start()

  def maybe_emit_startup_progress(self) -> None:
    if self.sent_startup_progress or not self.args.startup_progress:
      return
    self.sent_startup_progress = True
    token = "startup-index"
    self.send_server_request("window/workDoneProgress/create", {"token": token})
    self.send_notification(
      "$/progress",
      {
        "token": token,
        "value": {
          "kind": "begin",
          "title": "startup",
          "message": "mock indexing started",
          "percentage": 5,
        },
      },
    )
    self.send_notification(
      "$/progress",
      {
        "token": token,
        "value": {
          "kind": "end",
          "message": "mock indexing complete",
        },
      },
    )

  def handle_notification(self, message: dict[str, Any]) -> None:
    method = str(message.get("method"))
    params = message.get("params")

    if method == "$/cancelRequest":
      request_id = None
      if isinstance(params, dict):
        request_id = params.get("id")
      if isinstance(request_id, int):
        with self.state_lock:
          self.cancelled_request_ids.add(request_id)
        self.log("meta", {"id": request_id}, note="received cancel request")
      return

    if method == "initialized":
      self.maybe_emit_startup_progress()
      return

    if method == "exit":
      self.exit_requested = True
      return

  def run(self) -> int:
    try:
      while not self.exit_requested:
        message = self.read_message()
        if message is None:
          break

        if "method" in message and "id" in message:
          self.handle_request(message)
          continue

        if "method" in message:
          self.handle_notification(message)
          continue

        self.log("in-response", message)
    except KeyboardInterrupt:
      return 0
    except SystemExit:
      raise
    except Exception as exc:  # pragma: no cover
      self.log("error", {"error": str(exc)})
      return 1
    return 0


def main() -> int:
  args = parse_args()
  server = MockLsp(args)
  return server.run()


if __name__ == "__main__":
  raise SystemExit(main())
