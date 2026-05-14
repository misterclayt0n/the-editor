import type { KeyInput } from "./keymaps.js";
import type { EditorHost } from "./editorHost.js";
import { roleId, type RoleId } from "./roles.js";
import { defineHostStore } from "./stores.js";
import { toolInstanceId, type ToolInstanceId } from "./tools.js";

export type FocusTarget =
  | { kind: "editor" }
  | { kind: "role"; role: RoleId }
  | { kind: "tool"; toolId: ToolInstanceId };

export interface InputFocusOptions {
  reason: string;
  source?: string;
}

export interface InputRestoreOptions {
  reason: string;
  fallback?: FocusTarget;
}

export interface InputRouteResult {
  handled: boolean;
}

export interface InputSnapshot {
  focus: FocusTarget;
  scopes: string[];
}

interface InputRouterState {
  focus: FocusTarget;
  history: FocusTarget[];
}

const inputRouterStateStore = defineHostStore<InputRouterState>({
  id: "input.router.state",
  init: () => ({ focus: { kind: "editor" }, history: [] }),
});
const historyLimit = 16;

export class InputRouter {
  constructor(private readonly editor: EditorHost) {}

  current(): FocusTarget {
    return this.state().focus;
  }

  focus(target: FocusTarget, _options: InputFocusOptions): FocusTarget {
    const normalized = normalizeFocusTarget(target);
    const state = this.state();
    const next = this.isValid(normalized) ? normalized : { kind: "editor" } as FocusTarget;
    if (sameTarget(state.focus, next)) return state.focus;

    if (this.isValid(state.focus)) {
      state.history.push(state.focus);
      if (state.history.length > historyLimit) state.history.splice(0, state.history.length - historyLimit);
    }

    state.focus = next;
    if (state.focus.kind !== "editor") {
      this.editor.overlays.closeAttachedToEditor({ restoreFocus: false });
    }
    this.editor.interaction.clearPending();
    this.editor.workbench.invalidate("focus", "interaction");
    return state.focus;
  }

  focusEditor(options: InputFocusOptions = { reason: "input.focusEditor" }): FocusTarget {
    return this.focus({ kind: "editor" }, options);
  }

  focusRole(role: RoleId | string, options: InputFocusOptions): FocusTarget {
    return this.focus({ kind: "role", role: roleId(role) }, options);
  }

  focusTool(toolId: ToolInstanceId | string, options: InputFocusOptions): FocusTarget {
    return this.focus({ kind: "tool", toolId: toolInstanceId(toolId) }, options);
  }

  restore(options: InputRestoreOptions): FocusTarget {
    const state = this.state();
    while (state.history.length) {
      const target = state.history.pop()!;
      if (!this.isValid(target)) continue;
      state.focus = target;
      this.editor.interaction.clearPending();
      this.editor.workbench.invalidate("focus", "interaction");
      return state.focus;
    }
    state.focus = this.isValid(options.fallback) ? normalizeFocusTarget(options.fallback!) : { kind: "editor" };
    this.editor.interaction.clearPending();
    this.editor.workbench.invalidate("focus", "interaction");
    return state.focus;
  }

  restoreTarget(target: FocusTarget | undefined, options: InputRestoreOptions): FocusTarget {
    if (target && this.isValid(target)) return this.focus(target, { reason: options.reason });
    return this.restore(options);
  }

  scopes(profileModeScope?: string): string[] {
    const focus = this.repairCurrentFocus();
    switch (focus.kind) {
      case "role":
        return dedupeScopes([
          `role.${focus.role}`,
          focus.role === roleId("commandPalette") ? "palette" : undefined,
          "global",
        ]);
      case "tool": {
        const tool = this.editor.tools.allInstances().find((candidate) => candidate.id === focus.toolId);
        return dedupeScopes([
          `tool.${focus.toolId}`,
          tool ? `tool-kind.${tool.typeId}` : undefined,
          "global",
        ]);
      }
      case "editor":
      default:
        return dedupeScopes([...this.editor.overlays.attachedScopes(), profileModeScope, "global"]);
    }
  }

  async routeKey(input: KeyInput): Promise<InputRouteResult> {
    const transition = await this.editor.interaction.handleInput(input);
    return { handled: transition.handled ?? Boolean(transition.effects?.length) };
  }

  snapshot(): InputSnapshot {
    return {
      focus: this.repairCurrentFocus(),
      scopes: this.scopes(this.editor.interaction.snapshot().mode?.scopes[0]),
    };
  }

  private state(): InputRouterState {
    return this.editor.stores.host(inputRouterStateStore);
  }

  private repairCurrentFocus(): FocusTarget {
    const state = this.state();
    if (this.isValid(state.focus)) return state.focus;
    state.focus = { kind: "editor" };
    this.editor.workbench.invalidate("focus", "interaction");
    return state.focus;
  }

  private isValid(target: FocusTarget | undefined): boolean {
    if (!target) return false;
    switch (target.kind) {
      case "editor":
        return true;
      case "role":
        return Boolean(this.editor.roles.snapshot(target.role)?.implementationId);
      case "tool":
        return this.editor.tools.allInstances().some((tool) => tool.id === target.toolId);
    }
  }
}

function normalizeFocusTarget(target: FocusTarget): FocusTarget {
  switch (target.kind) {
    case "editor":
      return { kind: "editor" };
    case "role":
      return { kind: "role", role: roleId(target.role) };
    case "tool":
      return { kind: "tool", toolId: toolInstanceId(target.toolId) };
  }
}

function sameTarget(left: FocusTarget, right: FocusTarget): boolean {
  if (left.kind !== right.kind) return false;
  switch (left.kind) {
    case "editor":
      return true;
    case "role":
      return left.role === (right as Extract<FocusTarget, { kind: "role" }>).role;
    case "tool":
      return left.toolId === (right as Extract<FocusTarget, { kind: "tool" }>).toolId;
  }
}

function dedupeScopes(scopes: Array<string | undefined>): string[] {
  return [...new Set(scopes.filter((scope): scope is string => Boolean(scope && scope.trim())))];
}
