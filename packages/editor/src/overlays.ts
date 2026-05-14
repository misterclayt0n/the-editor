import type { EditorHost } from "./editorHost.js";
import type { FocusTarget } from "./input.js";
import { roleId, type RoleId } from "./roles.js";

interface OverlayEntry {
  role: RoleId;
  restoreTarget: FocusTarget;
  order: number;
  attachedToEditor: boolean;
  scope?: string;
}

const overlayStateSlot = "overlays.state";

interface OverlayState {
  nextOrder: number;
  entries: Map<RoleId, OverlayEntry>;
}

export class OverlayRegistry {
  constructor(private readonly editor: EditorHost) {}

  open(role: RoleId | string, options: { focus?: boolean; restoreTarget?: FocusTarget; attachedToEditor?: boolean; scope?: string } = {}): void {
    const normalizedRole = roleId(role);
    const state = this.state();
    const current = state.entries.get(normalizedRole);
    state.entries.set(normalizedRole, {
      role: normalizedRole,
      restoreTarget: options.restoreTarget ?? current?.restoreTarget ?? this.editor.input.current(),
      order: current?.order ?? ++state.nextOrder,
      attachedToEditor: options.attachedToEditor ?? current?.attachedToEditor ?? false,
      scope: options.scope ?? current?.scope,
    });
    if (options.focus ?? true) this.editor.input.focusRole(normalizedRole, { reason: "overlay.open" });
    this.editor.workbench.invalidate("focus", "roles");
  }

  close(role: RoleId | string, options: { restoreFocus?: boolean; fallback?: FocusTarget } = {}): void {
    const normalizedRole = roleId(role);
    const state = this.state();
    const entry = state.entries.get(normalizedRole);
    if (!entry) return;
    state.entries.delete(normalizedRole);
    if (options.restoreFocus ?? true) {
      this.editor.input.restoreTarget(entry.restoreTarget, { reason: "overlay.close", fallback: options.fallback ?? { kind: "editor" } });
    }
    this.editor.workbench.invalidate("focus", "roles");
  }

  closeAll(options: { restoreFocus?: boolean; fallback?: FocusTarget } = {}): void {
    const state = this.state();
    const entries = [...state.entries.values()].sort((a, b) => b.order - a.order);
    state.entries.clear();
    if ((options.restoreFocus ?? true) && entries[0]) {
      this.editor.input.restoreTarget(entries[0].restoreTarget, { reason: "overlay.closeAll", fallback: options.fallback ?? { kind: "editor" } });
    }
    this.editor.workbench.invalidate("focus", "roles");
  }

  isOpen(role: RoleId | string): boolean {
    return this.state().entries.has(roleId(role));
  }

  current(): RoleId | undefined {
    return [...this.state().entries.values()].sort((a, b) => b.order - a.order)[0]?.role;
  }

  attachedScopes(): string[] {
    return [...this.state().entries.values()]
      .filter((entry) => entry.attachedToEditor)
      .sort((a, b) => b.order - a.order)
      .map((entry) => entry.scope ?? `role.${entry.role}`);
  }

  closeAttachedToEditor(options: { restoreFocus?: boolean; fallback?: FocusTarget } = {}): void {
    const attached = [...this.state().entries.values()]
      .filter((entry) => entry.attachedToEditor)
      .sort((a, b) => b.order - a.order);
    for (const entry of attached) {
      this.close(entry.role, options);
    }
  }

  attachedRoles(): RoleId[] {
    return [...this.state().entries.values()]
      .filter((entry) => entry.attachedToEditor)
      .sort((a, b) => b.order - a.order)
      .map((entry) => entry.role);
  }

  async dismissAttachedToEditor(options: { restoreFocus?: boolean; fallback?: FocusTarget } = {}): Promise<void> {
    for (const role of this.attachedRoles()) {
      const implementation = this.editor.roles.activeImplementation(role);
      if (implementation?.close) await this.editor.roles.close(role);
      else this.close(role, options);
    }
  }

  restoreTarget(role: RoleId | string): FocusTarget | undefined {
    return this.state().entries.get(roleId(role))?.restoreTarget;
  }

  private state(): OverlayState {
    return this.editor.stateSlot(overlayStateSlot, () => ({
      nextOrder: 0,
      entries: new Map(),
    }));
  }
}
