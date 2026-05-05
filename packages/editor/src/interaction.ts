import { contributionKind } from "./contributions.js";
import type { EditorHost } from "./editorHost.js";
import { toDisposable, type Disposable } from "./disposable.js";

export type InteractionProfileId = string & { readonly __brand?: "InteractionProfileId" };
export type InteractionOverlayId = string & { readonly __brand?: "InteractionOverlayId" };

export interface InteractionProfileRegistration {
  id: InteractionProfileId | string;
  title: string;
  description?: string;
}

export interface InteractionOverlayRegistration {
  id: InteractionOverlayId | string;
  title: string;
  description?: string;
}

export interface InteractionSnapshot {
  profileId?: InteractionProfileId;
  overlayIds: InteractionOverlayId[];
}

export class InteractionRegistry {
  private readonly profiles = new Map<InteractionProfileId, InteractionProfileRegistration>();
  private readonly overlays = new Map<InteractionOverlayId, InteractionOverlayRegistration>();
  private readonly activeOverlays = new Set<InteractionOverlayId>();
  private activeProfileId: InteractionProfileId | undefined;

  constructor(private readonly editor: EditorHost) {}

  registerProfile(registration: InteractionProfileRegistration, moduleId = this.editor.currentModuleId()): Disposable {
    const id = interactionProfileId(registration.id);
    this.profiles.set(id, { ...registration, id });
    if (!this.activeProfileId) this.activeProfileId = id;

    const unregister = toDisposable(() => {
      const current = this.profiles.get(id);
      if (current?.id === id) this.profiles.delete(id);
      if (this.activeProfileId === id) this.activeProfileId = this.profiles.keys().next().value as InteractionProfileId | undefined;
    });

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
    this.activeProfileId = normalized;
  }

  activateOverlay(id: InteractionOverlayId | string): void {
    const normalized = interactionOverlayId(id);
    if (!this.overlays.has(normalized)) throw new Error(`unknown interaction overlay: ${normalized}`);
    this.activeOverlays.add(normalized);
  }

  deactivateOverlay(id: InteractionOverlayId | string): void {
    this.activeOverlays.delete(interactionOverlayId(id));
  }

  snapshot(): InteractionSnapshot {
    return {
      profileId: this.activeProfileId,
      overlayIds: [...this.activeOverlays],
    };
  }
}

export function interactionProfileId(id: InteractionProfileId | string): InteractionProfileId {
  return id as InteractionProfileId;
}

export function interactionOverlayId(id: InteractionOverlayId | string): InteractionOverlayId {
  return id as InteractionOverlayId;
}
