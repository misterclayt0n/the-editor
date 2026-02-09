# Mock LSP deterministic testing

This document validates the TODO 8 LSP behavior in `the-term` against a controllable stdio server.

## Prereqs

- `python3` available.
- Run from repo root.
- Use the mock server at `scripts/mock_lsp.py`.

## Shared setup

```bash
rm -f /tmp/the-editor-mock-lsp.log
export THE_EDITOR_LSP_COMMAND=python3
```

## 1) Progress notifications

Start editor with startup progress enabled:

```bash
export THE_EDITOR_LSP_ARGS="-u scripts/mock_lsp.py --mode normal --startup-progress --log /tmp/the-editor-mock-lsp.log"
cargo run -p the-term -- flake.nix
```

Expected:

- On startup, statusline/message stream shows progress begin/end text from mock server.
- Log includes `$\/progress` notifications and `window/workDoneProgress/create`.

Check:

```bash
jq -c 'select(.dir=="out" and .payload.method=="$/progress")' /tmp/the-editor-mock-lsp.log
jq -c 'select(.dir=="out" and .payload.method=="window/workDoneProgress/create")' /tmp/the-editor-mock-lsp.log
```

## 2) Request cancellation (`$/cancelRequest`)

With the same `normal` mode session running:

- Press `space k` (hover) repeatedly in quick succession.

Expected:

- Previous hover requests are canceled when new hover requests are dispatched.

Check:

```bash
jq -c 'select(.dir=="in" and .payload.method=="$/cancelRequest")' /tmp/the-editor-mock-lsp.log
```

## 3) File watch (`workspace/didChangeWatchedFiles`)

With editor still open:

```bash
touch flake.nix
```

Expected:

- `workspace/didChangeWatchedFiles` is sent for the active file.

Check:

```bash
jq -c 'select(.dir=="in" and .payload.method=="workspace/didChangeWatchedFiles")' /tmp/the-editor-mock-lsp.log
```

## 4) Timeout + single retry

Restart editor in timeout mode:

```bash
rm -f /tmp/the-editor-mock-lsp.log
export THE_EDITOR_LSP_ARGS="-u scripts/mock_lsp.py --mode timeout --timeout-delay 12 --log /tmp/the-editor-mock-lsp.log"
cargo run -p the-term -- flake.nix
```

Then press `space k` once and wait ~18s.

Expected:

- First hover request times out at ~8s and is retried once.
- Second timeout produces timeout message (no more retries).
- Mock log shows the same hover request id arriving twice.

Check duplicate request ids:

```bash
jq -r 'select(.dir=="in" and .payload.method=="textDocument/hover") | .payload.id' /tmp/the-editor-mock-lsp.log | sort | uniq -c
```

You should see at least one id with count `2`.

## 5) Restart burst limit (6 in 30s)

Restart editor with crashing initialize:

```bash
rm -f /tmp/the-editor-mock-lsp.log
export THE_EDITOR_LSP_ARGS="-u scripts/mock_lsp.py --mode crash-init --log /tmp/the-editor-mock-lsp.log"
cargo run -p the-term -- flake.nix
```

Wait ~4 seconds, then stop editor.

Expected:

- Runtime restarts quickly, then stops restarting after burst limit.
- Mock log contains a bounded count of `initialize` requests (initial + limited restarts), not unbounded growth.

Check:

```bash
jq -r 'select(.dir=="in" and .payload.method=="initialize") | .payload.method' /tmp/the-editor-mock-lsp.log | wc -l
```

The count should stabilize quickly (no continuous increase).
