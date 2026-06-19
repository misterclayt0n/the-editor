/**
 * The terminal engine. THIS is the one-line swap: comment one line, uncomment
 * the other. Both backends expose the same surface (Terminal, FitAddon,
 * ensureReady, attachAutoFit) — both speak the xterm.js API.
 */

export * from "./backend-ghostty";
// export * from "./backend-xterm";
