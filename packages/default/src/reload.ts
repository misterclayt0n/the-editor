import { commandId, matchesCommandPaletteItem, type CommandArgumentItem, type EditorModule } from "@the-editor/editor";

export function defaultReload(): EditorModule {
  return (editor) => {
    editor.commands.register<{ specifier?: string } | undefined, void>({
      id: commandId("editor.reload"),
      title: "Reload Packages",
      aliases: ["reload"],
      args: [{ name: "specifier", source: "reloadTargets" }],
      category: "Workbench",
      palette: "topLevel",
      keywords: ["packages"],
      icon: { kind: "lucide", name: "refresh-cw" },
      async run(_ctx, args) {
        await editor.roles.close("commandPalette");
        await editor.packages.reload(args?.specifier);
      },
    });

    editor.commands.registerArgumentProvider({
      id: "reloadTargets",
      title: "Reload Targets",
      provide(ctx) {
        const packages = editor.packages.snapshot();
        const items: CommandArgumentItem<string>[] = [];
        for (const pkg of packages) {
          items.push({
            id: pkg.id,
            title: pkg.id,
            detail: pkg.loadedTargets.length ? `Targets: ${pkg.loadedTargets.join(", ")}` : "Registered package",
            value: pkg.id,
            keywords: [pkg.id, ...pkg.loadedTargets],
          });
          for (const target of pkg.loadedTargets) {
            const specifier = `${pkg.id}/${target}`;
            items.push({
              id: specifier,
              title: specifier,
              detail: `Reload ${target} target`,
              value: specifier,
              keywords: [specifier, pkg.id, target],
            });
          }
        }
        return items.filter((item) => matchesCommandPaletteItem(ctx.query, item));
      },
    });
  };
}
