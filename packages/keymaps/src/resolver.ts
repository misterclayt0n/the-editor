/**
 * Pure key resolution. Given the bindings in registration order, the focused
 * panel, the pending chord prefix, and an incoming chord:
 *
 * - if any in-scope binding has [pending, chord] as a strict prefix, the
 *   sequence stays pending (prefixes win over exact matches, so a chord
 *   leader like ctrl+w is never consumed early);
 * - otherwise the best exact match wins: higher scope specificity first
 *   (panel > surface kind > global), later registration breaking ties — so
 *   user packages, activated last, shadow defaults for free;
 * - otherwise the chord is unhandled and the host may treat it as input.
 */

import {
  scopeApplies,
  scopeSpecificity,
  type PanelId,
  type SurfaceKind,
} from "@the-editor/workbench";
import { chordEquals, type Chord } from "./chord.js";
import type { KeyBindingContribution, KeyBindingFacts } from "./types.js";

export type FocusTarget = { readonly id: PanelId; readonly kind: SurfaceKind } | undefined;

export type KeyResolution =
  | { readonly _tag: "none"; readonly cancelled?: ReadonlyArray<Chord> }
  | { readonly _tag: "pending"; readonly chords: ReadonlyArray<Chord> }
  | { readonly _tag: "match"; readonly binding: KeyBindingContribution };

const NONE: KeyResolution = { _tag: "none" };

function factsMatch(required: KeyBindingFacts | undefined, current: KeyBindingFacts): boolean {
  if (required === undefined) return true;
  for (const [key, value] of Object.entries(required)) {
    if (current[key] !== value) return false;
  }
  return true;
}

function startsWith(chords: ReadonlyArray<Chord>, prefix: ReadonlyArray<Chord>): boolean {
  if (chords.length < prefix.length) return false;
  for (let i = 0; i < prefix.length; i++) {
    if (!chordEquals(chords[i]!, prefix[i]!)) return false;
  }
  return true;
}

export function resolveKey(args: {
  readonly bindings: ReadonlyArray<KeyBindingContribution>;
  readonly focus: FocusTarget;
  readonly pending: ReadonlyArray<Chord>;
  readonly chord: Chord;
  readonly facts?: KeyBindingFacts;
}): KeyResolution {
  const sequence = [...args.pending, args.chord];
  const facts = args.facts ?? {};
  let best: { readonly binding: KeyBindingContribution; readonly specificity: number } | undefined;
  let longerExists = false;

  for (const binding of args.bindings) {
    if (!scopeApplies(binding.scope, args.focus)) continue;
    if (!factsMatch(binding.when, facts)) continue;
    if (!startsWith(binding.chords, sequence)) continue;
    if (binding.chords.length > sequence.length) {
      longerExists = true;
      continue;
    }
    const specificity = scopeSpecificity(binding.scope);
    // >= keeps the later registration on ties.
    if (best === undefined || specificity >= best.specificity) {
      best = { binding, specificity };
    }
  }

  if (longerExists) return { _tag: "pending", chords: sequence };
  if (best) return { _tag: "match", binding: best.binding };
  return args.pending.length > 0 ? { _tag: "none", cancelled: sequence } : NONE;
}
