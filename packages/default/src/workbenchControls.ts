import { commandId, matchesCommandPaletteItem, roleImplementationId, toolTypeId, type CommandPaletteItem, type EditorModule } from "@the-editor/editor";
import { getDefaultState } from "./state.js";

const inspectorToolTypeId = toolTypeId("default.inspector");
const statuslineImplementationId = roleImplementationId("default.statusline");
const tabstripImplementationId = roleImplementationId("default.tabstrip");

export function defaultWorkbenchControls(): EditorModule {
  return (editor) => {
    editor.roles.setFallback("statusline", statuslineImplementationId);
    editor.roles.setFallback("tabstrip", tabstripImplementationId);

    editor.roles.register({
      role: "statusline",
      implementationId: statuslineImplementationId,
      title: "Default Statusline",
      snapshot() {
        const state = getDefaultState(editor);
        const commandPalette = editor.roles.snapshot("commandPalette");
        return {
          mode: editor.interaction.snapshot().mode?.id,
          activePath: state.activePaneId ? state.surfacePanes.get(state.activePaneId)?.path : undefined,
          activeRoleImplementation: commandPalette?.implementationId,
          selectedCommandPalette: commandPalette?.selectedImplementationId,
        };
      },
    });

    editor.roles.register({
      role: "tabstrip",
      implementationId: tabstripImplementationId,
      title: "Default Tabstrip",
      snapshot: () => ({}),
    });

    editor.tools.registerType({
      id: inspectorToolTypeId,
      title: "Workbench Inspector",
      root: "bottomPanel",
      instancePolicy: "singleton",
      closeable: true,
      persistenceKey: "default.inspector",
    });

    editor.commands.register<{ role: string; implementationId: string }, void>({
      id: commandId("role.activate"),
      title: "Activate Role Implementation",
      category: "Workbench",
      hidden: true,
      run(_ctx, args) {
        editor.roles.activate(args.role, args.implementationId);
      },
    });

    editor.commands.register<{ role: string }, void>({
      id: commandId("role.clearSelection"),
      title: "Clear Role Selection",
      category: "Workbench",
      hidden: true,
      run(_ctx, args) {
        editor.roles.clearSelection(args.role);
      },
    });

    editor.commands.register<{ typeId: string; title?: string; input?: unknown; placement?: unknown }, string>({
      id: commandId("tool.open"),
      title: "Open Tool",
      category: "Workbench",
      hidden: true,
      run(_ctx, args) {
        const instance = editor.workbench.openTool(args.typeId, {
          title: args.title,
          input: args.input,
          placement: args.placement as never,
        });
        return instance.id;
      },
    });

    editor.commands.register<{ toolId: string }, void>({
      id: commandId("tool.close"),
      title: "Close Tool",
      category: "Workbench",
      hidden: true,
      run(_ctx, args) {
        editor.tools.close(args.toolId);
      },
    });

    editor.commands.register<void, void>({
      id: commandId("workbench.toggleInspector"),
      title: "Toggle Workbench Inspector",
      category: "Workbench",
      keywords: ["inspector", "diagnostics", "extensions", "tools"],
      run() {
        const existing = editor.tools.allInstances().find((instance) => instance.typeId === inspectorToolTypeId);
        if (existing) {
          editor.tools.close(existing.id);
          return;
        }
        editor.workbench.openTool(inspectorToolTypeId);
      },
    });

    editor.palette.registerProvider({
      id: "default.roles",
      title: "Roles",
      priority: 20,
      provide(ctx, query) {
        const items: CommandPaletteItem[] = [];
        const snapshots = ctx.editor.roles.snapshotAll();
        for (const [role, snapshot] of Object.entries(snapshots)) {
          for (const available of snapshot.available) {
            items.push({
              id: `role:${role}:${available.implementationId}`,
              title: `Role: ${role} → ${available.packageId ?? available.implementationId}`,
              detail: available.implementationId,
              group: "Roles",
              keywords: ["role", role, available.implementationId, available.packageId ?? ""],
              command: {
                id: commandId("role.activate"),
                args: { role, implementationId: available.implementationId },
              },
            });
          }
          if (snapshot.selectedImplementationId) {
            items.push({
              id: `role:${role}:clear`,
              title: `Role: ${role} → automatic`,
              detail: snapshot.implementationId ? `Current: ${snapshot.implementationId}` : "Automatic selection",
              group: "Roles",
              keywords: ["role", role, "automatic", "default", "clear"],
              command: {
                id: commandId("role.clearSelection"),
                args: { role },
              },
            });
          }
        }
        return items.filter((item) => matchesCommandPaletteItem(query, item));
      },
    });

    editor.palette.registerProvider({
      id: "default.tools",
      title: "Tools",
      priority: 10,
      provide(_ctx, query) {
        return [
          {
            id: "tool:inspector",
            title: "Workbench: Toggle Inspector",
            detail: "workbench.toggleInspector",
            group: "Tools",
            keywords: ["tool", "inspector", "diagnostics", "extensions"],
            command: { id: commandId("workbench.toggleInspector") },
          },
        ].filter((item) => matchesCommandPaletteItem(query, item));
      },
    });
  };
}

export { inspectorToolTypeId, statuslineImplementationId, tabstripImplementationId };
