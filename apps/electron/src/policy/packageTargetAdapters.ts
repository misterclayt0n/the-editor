import type { PackageTargetAdapterMap } from "@the-editor/editor";
import { SourceModuleLoader } from "./sourceModuleLoader.js";

export function createPolicyPackageTargetAdapters(): PackageTargetAdapterMap {
  return {
    runtime: {
      async activate(ctx) {
        if (typeof ctx.entrypoint !== "string") return;
        const loader = new SourceModuleLoader();
        const activate = loader.loadDefault<(editor: typeof ctx.editor) => Promise<void> | void>(ctx.entrypoint);
        await ctx.editor.modules.load({ id: ctx.moduleId, activate });
      },
      deactivate(ctx) {
        ctx.editor.modules.unload(ctx.moduleId);
      },
    },
    interaction: {
      async activate(ctx) {
        if (typeof ctx.entrypoint !== "string") return;
        const loader = new SourceModuleLoader();
        const activate = loader.loadDefault<(editor: typeof ctx.editor) => Promise<void> | void>(ctx.entrypoint);
        await ctx.editor.modules.load({ id: ctx.moduleId, activate });
      },
      deactivate(ctx) {
        ctx.editor.modules.unload(ctx.moduleId);
      },
    },
    ui: {
      activate() {},
      deactivate() {},
    },
    theme: {
      async activate(ctx) {
        if (typeof ctx.entrypoint !== "string") return;
        const loader = new SourceModuleLoader();
        const activate = loader.loadDefault<(editor: typeof ctx.editor) => Promise<void> | void>(ctx.entrypoint);
        await ctx.editor.modules.load({ id: ctx.moduleId, activate });
      },
      deactivate(ctx) {
        ctx.editor.modules.unload(ctx.moduleId);
      },
    },
  };
}
