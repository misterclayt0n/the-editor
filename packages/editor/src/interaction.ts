import type { SurfaceId, SurfaceOperation } from "@the-editor/contracts/runtime";
import { commandId, type CommandDispatch } from "./commands.js";
import { contributionKind } from "./contributions.js";
import type { EditorHost } from "./editorHost.js";
import type { FocusTarget } from "./input.js";
import type { KeyInput, KeymapPendingState, KeymapResolution } from "./keymaps.js";
import { toDisposable, type Disposable } from "./disposable.js";
import { defineHostStore } from "./stores.js";

export type InteractionProfileId = string & { readonly __brand?: "InteractionProfileId" };
export type InteractionOverlayId = string & { readonly __brand?: "InteractionOverlayId" };

export interface InteractionContext<TState = unknown> {
  editor: EditorHost;
  profileId: InteractionProfileId;
  state: TState;
  previousSnapshot: InteractionSnapshot;
}

export interface InteractionModeSnapshot {
  id: string;
  title?: string;
  cursorShape?: "block" | "bar" | "underline";
  scopes: string[];
}

export interface InteractionPendingSnapshot {
  kind: string;
  label?: string;
  keys?: readonly string[];
  nextKeys?: readonly string[];
}

export interface InteractionProfileSnapshot {
  mode?: InteractionModeSnapshot;
  pending?: InteractionPendingSnapshot;
}

export interface InteractionSnapshot extends InteractionProfileSnapshot {
  profileId?: InteractionProfileId;
  overlayIds: InteractionOverlayId[];
}

export type InteractionEffect =
  | { kind: "surface.operation"; surfaceId: SurfaceId; operation: SurfaceOperation }
  | { kind: "surface.insertText"; surfaceId: SurfaceId; text: string }
  | { kind: "interaction.mode"; mode: string }
  | { kind: "focus.set"; target: FocusTarget }
  | { kind: "command.dispatch"; dispatch: CommandDispatch };

export interface InteractionTransition<TState = unknown> {
  state: TState;
  effects?: readonly InteractionEffect[];
  handled?: boolean;
}

export interface InteractionProfile<TState = unknown> {
  id: InteractionProfileId | string;
  title: string;
  description?: string;
  initialState(ctx: Omit<InteractionContext<TState>, "state">): TState;
  scopes(ctx: InteractionContext<TState>): readonly string[];
  reduceInput(ctx: InteractionContext<TState>, input: KeyInput): InteractionTransition<TState> | Promise<InteractionTransition<TState>>;
  snapshot(ctx: InteractionContext<TState>): InteractionProfileSnapshot;
}

export type InteractionProfileRegistration<TState = unknown> = InteractionProfile<TState>;

export interface InteractionOverlayRegistration {
  id: InteractionOverlayId | string;
  title: string;
  description?: string;
}

interface ProfileEntry<TState = unknown> {
  profile: InteractionProfile<TState>;
  state?: TState;
}

const keymapPendingStore = defineHostStore<InteractionKeymapPendingState | undefined>({
  id: "interaction.keymapPending",
  init: () => undefined,
});

export interface InteractionKeymapPendingState {
  scope: string;
  keys: string[];
}

export class InteractionRegistry {
  private readonly profiles = new Map<InteractionProfileId, ProfileEntry>();
  private readonly overlays = new Map<InteractionOverlayId, InteractionOverlayRegistration>();
  private readonly activeOverlays = new Set<InteractionOverlayId>();
  private activeProfileId: InteractionProfileId | undefined;
  private lastFocusSignature = "";

  constructor(private readonly editor: EditorHost) {}

  registerProfile<TState>(registration: InteractionProfileRegistration<TState>, moduleId = this.editor.currentModuleId()): Disposable {
    const id = interactionProfileId(registration.id);
    const profile: InteractionProfile<TState> = { ...registration, id };
    this.profiles.set(id, { profile });
    if (!this.activeProfileId) this.activeProfileId = id;

    const unregister = toDisposable(() => {
      const current = this.profiles.get(id);
      if (current?.profile === profile) this.profiles.delete(id);
      if (this.activeProfileId === id) {
        this.activeProfileId = this.profiles.keys().next().value as InteractionProfileId | undefined;
        this.clearPending();
      }
      this.editor.workbench.invalidate("interaction");
    });

    this.editor.workbench.invalidate("interaction");
    return this.editor.contributions.add({
      moduleId,
      kind: contributionKind("interactionProfile"),
      localId: id,
      label: registration.title,
      disposable: unregister,
    });
  }

  registerOverlay(registration: InteractionOverlayRegistration, moduleId = this.editor.currentModuleId()): Disposable {
    const id = interactionOverlayId(registration.id);
    this.overlays.set(id, { ...registration, id });

    const unregister = toDisposable(() => {
      const current = this.overlays.get(id);
      if (current?.id === id) this.overlays.delete(id);
      this.activeOverlays.delete(id);
      this.clearPending();
      this.editor.workbench.invalidate("interaction");
    });

    return this.editor.contributions.add({
      moduleId,
      kind: contributionKind("interactionOverlay"),
      localId: id,
      label: registration.title,
      disposable: unregister,
    });
  }

  useProfile(id: InteractionProfileId | string): void {
    const normalized = interactionProfileId(id);
    if (!this.profiles.has(normalized)) throw new Error(`unknown interaction profile: ${normalized}`);
    if (this.activeProfileId === normalized) return;
    this.activeProfileId = normalized;
    this.clearPending();
    this.editor.workbench.invalidate("interaction");
  }

  activateOverlay(id: InteractionOverlayId | string): void {
    const normalized = interactionOverlayId(id);
    if (!this.overlays.has(normalized)) throw new Error(`unknown interaction overlay: ${normalized}`);
    this.activeOverlays.add(normalized);
    this.clearPending();
    this.editor.workbench.invalidate("interaction");
  }

  deactivateOverlay(id: InteractionOverlayId | string): void {
    if (!this.activeOverlays.delete(interactionOverlayId(id))) return;
    this.clearPending();
    this.editor.workbench.invalidate("interaction");
  }

  async handleInput(input: KeyInput): Promise<InteractionTransition> {
    this.clearPendingIfFocusChanged();
    const entry = this.activeEntry();
    if (!entry) return { state: undefined, handled: false };
    const ctx = this.context(entry);
    const transition = await entry.profile.reduceInput(ctx, input);
    entry.state = transition.state;
    await this.applyEffects(transition.effects ?? []);
    this.editor.workbench.invalidate("interaction");
    return transition;
  }

  setMode(mode: string): void {
    this.setProfileMode(mode);
    this.clearPending();
  }

  async applyEffects(effects: readonly InteractionEffect[]): Promise<void> {
    for (const effect of effects) {
      switch (effect.kind) {
        case "surface.operation":
          await this.editor.runtime.surfaces.applyOperation({ surfaceId: effect.surfaceId, operation: effect.operation });
          break;
        case "surface.insertText":
          await this.editor.runtime.surfaces.applyOperation({ surfaceId: effect.surfaceId, operation: { name: "insert_text", text: effect.text } as SurfaceOperation });
          break;
        case "interaction.mode":
          this.setProfileMode(effect.mode);
          break;
        case "focus.set":
          this.editor.input.focus(effect.target, { reason: "interaction.effect" });
          this.clearPending();
          break;
        case "command.dispatch":
          await this.editor.commands.dispatch(effect.dispatch);
          break;
      }
    }
  }

  snapshot(): InteractionSnapshot {
    this.clearPendingIfFocusChanged();
    const entry = this.activeEntry();
    const profileSnapshot = entry ? entry.profile.snapshot(this.context(entry)) : {};
    return {
      ...profileSnapshot,
      pending: profileSnapshot.pending ?? this.keymapPendingSnapshot(),
      profileId: this.activeProfileId,
      overlayIds: [...this.activeOverlays],
    };
  }

  keymapPending(): InteractionKeymapPendingState | undefined {
    return this.editor.stores.get(keymapPendingStore);
  }

  setKeymapPending(pending: InteractionKeymapPendingState | undefined): void {
    if (pending) this.editor.stores.set(keymapPendingStore, { scope: pending.scope, keys: [...pending.keys] });
    else this.editor.stores.delete(keymapPendingStore);
    this.editor.workbench.invalidate("interaction");
  }

  clearPending(): void {
    this.editor.stores.delete(keymapPendingStore);
  }

  resolveKeymap(scopes: readonly string[], input: KeyInput): KeymapResolution {
    const pending = this.keymapPending();
    const result = this.editor.keymaps.resolveScopes(scopes, input, pending);
    if (result.kind === "pending") this.setKeymapPending({ scope: result.pending.scope, keys: [...result.pending.keys] });
    else this.setKeymapPending(undefined);
    return result;
  }

  private activeEntry(): ProfileEntry | undefined {
    return this.activeProfileId ? this.profiles.get(this.activeProfileId) : undefined;
  }

  private context<TState>(entry: ProfileEntry<TState>): InteractionContext<TState> {
    if (entry.state === undefined) {
      entry.state = entry.profile.initialState({
        editor: this.editor,
        profileId: interactionProfileId(entry.profile.id),
        previousSnapshot: this.snapshotWithoutProfile(),
      });
    }
    return {
      editor: this.editor,
      profileId: interactionProfileId(entry.profile.id),
      state: entry.state,
      previousSnapshot: this.snapshotWithoutProfile(),
    };
  }

  private snapshotWithoutProfile(): InteractionSnapshot {
    return {
      profileId: this.activeProfileId,
      overlayIds: [...this.activeOverlays],
      pending: this.keymapPendingSnapshot(),
    };
  }

  private keymapPendingSnapshot(): InteractionPendingSnapshot | undefined {
    const pending = this.keymapPending();
    if (!pending) return undefined;
    const keymapPending = this.editor.keymaps.pending(pending);
    if (!keymapPending) return undefined;
    return {
      kind: "keymap",
      label: keymapPending.name,
      keys: keymapPending.keys,
      nextKeys: keymapPending.nextKeys,
    };
  }

  private setProfileMode(mode: string): void {
    const entry = this.activeEntry();
    if (!entry) return;
    const ctx = this.context(entry);
    if (!ctx.state || typeof ctx.state !== "object") return;
    entry.state = { ...(ctx.state as Record<string, unknown>), mode };
    this.editor.workbench.invalidate("interaction");
  }

  private clearPendingIfFocusChanged(): void {
    const signature = JSON.stringify({ focus: this.editor.input.current(), active: this.editor.workbench.activePane()?.id });
    if (!this.lastFocusSignature) {
      this.lastFocusSignature = signature;
      return;
    }
    if (signature === this.lastFocusSignature) return;
    this.lastFocusSignature = signature;
    this.clearPending();
  }
}

export function keymapProfile<TState extends { mode: string }>(options: {
  id: string;
  title: string;
  description?: string;
  initialMode: TState["mode"];
  initialState?: () => Omit<TState, "mode">;
  modeSnapshots: Record<string, Omit<InteractionModeSnapshot, "id">>;
  reduceMatched?: (ctx: InteractionContext<TState>, dispatches: CommandDispatch[], input: KeyInput) => InteractionTransition<TState> | Promise<InteractionTransition<TState>>;
  reduceUnmatched?: (ctx: InteractionContext<TState>, input: KeyInput) => InteractionTransition<TState> | Promise<InteractionTransition<TState>>;
}): InteractionProfile<TState> {
  return {
    id: options.id,
    title: options.title,
    description: options.description,
    initialState() {
      return { ...(options.initialState?.() ?? {}), mode: options.initialMode } as TState;
    },
    scopes(ctx) {
      return ctx.editor.input.scopes(ctx.state.mode);
    },
    async reduceInput(ctx, input) {
      const resolution = ctx.editor.interaction.resolveKeymap(this.scopes(ctx), input);
      if (resolution.kind === "matched") {
        if (options.reduceMatched) return options.reduceMatched(ctx, resolution.dispatches, input);
        return { state: ctx.state, handled: true, effects: resolution.dispatches.map((dispatch) => ({ kind: "command.dispatch", dispatch }) satisfies InteractionEffect) };
      }
      if (resolution.kind === "pending") return { state: ctx.state, handled: true };
      if (resolution.kind === "cancelled") return { state: ctx.state, handled: true };
      return options.reduceUnmatched?.(ctx, input) ?? { state: ctx.state, handled: false };
    },
    snapshot(ctx) {
      const mode = String(ctx.state.mode);
      const modeInfo = options.modeSnapshots[mode] ?? { title: mode, scopes: [`editor.${mode}`] };
      return {
        mode: { id: mode, ...modeInfo, scopes: [...modeInfo.scopes] },
      };
    },
  };
}

export function modeCommand(mode: string): CommandDispatch {
  return { id: commandId(`${mode}_mode`) };
}

export function interactionProfileId(id: InteractionProfileId | string): InteractionProfileId {
  return id as InteractionProfileId;
}

export function interactionOverlayId(id: InteractionOverlayId | string): InteractionOverlayId {
  return id as InteractionOverlayId;
}
