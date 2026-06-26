// OSC 52 — "Manipulate Selection Data". xterm.js does not handle this itself;
// terminals register it so TUIs can copy through SSH/tmux without owning the
// host clipboard API directly.

export type Osc52ParseResult =
  | { readonly kind: "write"; readonly selections: string; readonly text: string }
  | { readonly kind: "query" }
  | { readonly kind: "invalid"; readonly reason: string };

export type Osc52ClipboardRequestOptions = {
  readonly allowClipboardWrite: boolean;
  readonly writeClipboardText: (text: string) => Promise<void>;
  readonly writeSelectionText?: (text: string) => Promise<void>;
  readonly onBlockedWrite?: () => void;
};

const MAX_OSC52_BYTES = 128 * 1024;

export function handleOsc52ClipboardRequest(
  data: string,
  options: Osc52ClipboardRequestOptions,
): boolean {
  const parsed = parseOsc52(data);
  if (parsed.kind !== "write") return true;

  if (!options.allowClipboardWrite) {
    options.onBlockedWrite?.();
    return true;
  }

  if (parsed.selections.includes("c")) {
    void options.writeClipboardText(parsed.text).catch(() => {
      // Clipboard writes are best-effort and should never block parser progress.
    });
  }
  if (parsed.selections.includes("p") || parsed.selections.includes("s")) {
    void options.writeSelectionText?.(parsed.text).catch(() => {
      // Primary/selection clipboard is optional across platforms.
    });
  }
  return true;
}

export function parseOsc52(data: string): Osc52ParseResult {
  const semi = data.indexOf(";");
  if (semi === -1) return { kind: "invalid", reason: "missing selection/data separator" };

  const selections = data.slice(0, semi);
  const payload = data.slice(semi + 1);
  if (selections.length === 0) return { kind: "invalid", reason: "empty selection list" };
  if (!/^[cpqs0-7]+$/.test(selections)) return { kind: "invalid", reason: "unknown selection kind" };
  if (payload === "?") return { kind: "query" };
  if (payload.length > MAX_OSC52_BYTES) return { kind: "invalid", reason: "payload exceeds size limit" };

  const decoded = decodeBase64Utf8(payload);
  if (decoded === null) return { kind: "invalid", reason: "payload is not valid base64" };
  return { kind: "write", selections, text: decoded };
}

function decodeBase64Utf8(base64: string): string | null {
  const normalized = normalizeBase64Payload(base64);
  if (normalized === null) return null;
  try {
    const binary = atob(normalized);
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
    return new TextDecoder("utf-8", { fatal: false }).decode(bytes);
  } catch {
    return null;
  }
}

function normalizeBase64Payload(value: string): string | null {
  let stripped = "";
  let sawWhitespace = false;
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (isWhitespaceCode(code)) {
      if (!sawWhitespace) {
        stripped = value.slice(0, index);
        sawWhitespace = true;
      }
      continue;
    }
    if (!isBase64Code(code)) return null;
    if (sawWhitespace) stripped += value[index];
  }
  return sawWhitespace ? stripped : value;
}

function isBase64Code(code: number): boolean {
  return (
    (code >= 65 && code <= 90) ||
    (code >= 97 && code <= 122) ||
    (code >= 48 && code <= 57) ||
    code === 43 ||
    code === 47 ||
    code === 61
  );
}

function isWhitespaceCode(code: number): boolean {
  return code === 32 || (code >= 9 && code <= 13);
}
