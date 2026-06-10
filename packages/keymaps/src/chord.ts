/**
 * A chord is one keypress with modifiers. A key sequence is one or more
 * chords ("ctrl+w h" = ctrl+w, then h — Emacs/vim style prefixes).
 *
 * Keys use DOM `KeyboardEvent.key` names, lowercased ("arrowleft", "enter",
 * "escape", single characters as themselves). "space" and "plus" are spelled
 * out because " " and "+" collide with the chord syntax.
 */

import { InvalidKeysError } from "./errors.js";

export type Chord = {
  readonly key: string;
  readonly ctrl: boolean;
  readonly alt: boolean;
  readonly shift: boolean;
  readonly meta: boolean;
};

const MODIFIER_ALIASES: Record<string, keyof Omit<Chord, "key">> = {
  ctrl: "ctrl",
  control: "ctrl",
  alt: "alt",
  option: "alt",
  shift: "shift",
  meta: "meta",
  cmd: "meta",
  super: "meta",
};

const KEY_ALIASES: Record<string, string> = {
  esc: "escape",
  return: "enter",
  space: " ",
  plus: "+",
  left: "arrowleft",
  right: "arrowright",
  up: "arrowup",
  down: "arrowdown",
};

const MODIFIER_KEY_NAMES = new Set(["control", "shift", "alt", "meta"]);

function normalizeKeyName(raw: string): string {
  const lower = raw.toLowerCase();
  return KEY_ALIASES[lower] ?? lower;
}

export function parseChord(input: string): Chord {
  const tokens = input.split("+").filter((token) => token.length > 0);
  // "ctrl++" means ctrl plus the "+" key; prefer the "plus" spelling but cope.
  if (input.endsWith("+") && !input.endsWith("++")) {
    throw new InvalidKeysError({ keys: input, reason: "trailing +" });
  }
  if (input.includes("++")) tokens.push("+");
  if (tokens.length === 0) throw new InvalidKeysError({ keys: input, reason: "empty chord" });

  const chord = { key: "", ctrl: false, alt: false, shift: false, meta: false };
  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i]!;
    const isLast = i === tokens.length - 1;
    const modifier = MODIFIER_ALIASES[token.toLowerCase()];
    if (!isLast) {
      if (!modifier) throw new InvalidKeysError({ keys: input, reason: `unknown modifier "${token}"` });
      chord[modifier] = true;
    } else if (modifier && tokens.length === 1) {
      throw new InvalidKeysError({ keys: input, reason: "chord has no key" });
    } else {
      chord.key = normalizeKeyName(token);
    }
  }
  if (chord.key.length === 0) throw new InvalidKeysError({ keys: input, reason: "chord has no key" });
  return chord;
}

/** Parse a whitespace-separated key sequence: "ctrl+w h" → [ctrl+w, h]. */
export function parseKeys(input: string): ReadonlyArray<Chord> {
  const parts = input.trim().split(/\s+/).filter((part) => part.length > 0);
  if (parts.length === 0) throw new InvalidKeysError({ keys: input, reason: "empty sequence" });
  return parts.map(parseChord);
}

/** Canonical form used for equality and map keys. */
export function chordKey(chord: Chord): string {
  let out = "";
  if (chord.ctrl) out += "ctrl+";
  if (chord.alt) out += "alt+";
  if (chord.shift) out += "shift+";
  if (chord.meta) out += "meta+";
  return out + chord.key;
}

export function chordEquals(a: Chord, b: Chord): boolean {
  return chordKey(a) === chordKey(b);
}

export function formatChord(chord: Chord): string {
  const key = chord.key === " " ? "space" : chord.key === "+" ? "plus" : chord.key;
  return chordKey({ ...chord, key });
}

export function formatChords(chords: ReadonlyArray<Chord>): string {
  return chords.map(formatChord).join(" ");
}

export type KeyboardEventLike = {
  readonly key: string;
  readonly ctrlKey: boolean;
  readonly altKey: boolean;
  readonly shiftKey: boolean;
  readonly metaKey: boolean;
};

/** Undefined for modifier-only keydowns (pressing just Shift is not a chord). */
export function chordFromKeyboardEvent(event: KeyboardEventLike): Chord | undefined {
  if (MODIFIER_KEY_NAMES.has(event.key.toLowerCase())) return undefined;
  return {
    key: event.key.length === 1 ? event.key.toLowerCase() : normalizeKeyName(event.key),
    ctrl: event.ctrlKey,
    alt: event.altKey,
    shift: event.shiftKey,
    meta: event.metaKey,
  };
}

/**
 * Hosts use this for the self-insert fallback: an unbound printable keypress
 * (no ctrl/alt/meta) should become text input rather than a dead key.
 */
export function isTextInputChord(chord: Chord): boolean {
  return chord.key.length === 1 && !chord.ctrl && !chord.alt && !chord.meta;
}
