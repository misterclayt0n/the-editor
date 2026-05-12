import type { EditorHost } from "./editorHost.js";
import { roleId, type RoleId } from "./roles.js";
import { toolInstanceId, type ToolInstanceId } from "./tools.js";

export type FocusTarget =
  | { kind: "editor" }
  | { kind: "role"; role: RoleId }
  | { kind: "tool"; toolId: ToolInstanceId };

interface FocusState {
  current: FocusTarget;
  history: FocusTarget[];
}

const focusStateSlot = "focus.state";
const historyLimit = 16;

export class FocusRegistry {
  constructor(private readonly editor: EditorHost) {}

  current(): FocusTarget {
    return this.state().current;
  }

  set(target: FocusTarget): FocusTarget {
    const normalized = normalizeFocusTarget(target);
    const state = this.state();
    if (sameTarget(state.current, normalized)) return state.current;
    if (this.isValid(state.current)) {
      state.history.push(state.current);
      if (state.history.length > historyLimit) state.history.splice(0, state.history.length - historyLimit);
    }
    state.current = this.isValid(normalized) ? normalized : { kind: "editor" };
    if (state.current.kind !== "editor") {
      this.editor.overlays.closeAttachedToEditor({ restoreFocus: false });
    }
    return state.current;
  }

  setEditor(): FocusTarget {
    return this.set({ kind: "editor" });
  }

  setRole(role: RoleId | string): FocusTarget {
    return this.set({ kind: "role", role: roleId(role) });
  }

  setTool(toolId: ToolInstanceId | string): FocusTarget {
    return this.set({ kind: "tool", toolId: toolInstanceId(toolId) });
  }

  restore(fallback: FocusTarget = { kind: "editor" }): FocusTarget {
    const state = this.state();
    while (state.history.length) {
      const target = state.history.pop()!;
      if (!this.isValid(target)) continue;
      state.current = target;
      return state.current;
    }
    state.current = this.isValid(fallback) ? normalizeFocusTarget(fallback) : { kind: "editor" };
    return state.current;
  }

  restoreTarget(target: FocusTarget | undefined, fallback: FocusTarget = { kind: "editor" }): FocusTarget {
    if (target && this.isValid(target)) return this.set(target);
    return this.restore(fallback);
  }

  scopes(editorMode?: string): string[] {
    const current = this.current();
    switch (current.kind) {
      case "role":
        return dedupeScopes([
          `role.${current.role}`,
          current.role === roleId("commandPalette") ? "palette" : undefined,
          "global",
        ]);
      case "tool": {
        const tool = this.editor.tools.allInstances().find((candidate) => candidate.id === current.toolId);
        return dedupeScopes([
          `tool.${current.toolId}`,
          tool ? `tool-kind.${tool.typeId}` : undefined,
          "global",
        ]);
      }
      case "editor":
      default:
        return dedupeScopes([...this.editor.overlays.attachedScopes(), editorMode, "global"]);
    }
  }

  private state(): FocusState {
    return this.editor.stateSlot(focusStateSlot, () => ({
      current: { kind: "editor" } as FocusTarget,
      history: [],
    }));
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
      default:
        return false;
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
