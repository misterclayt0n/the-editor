import { roleImplementationId, type EditorModule } from "@the-editor/editor";

export interface ConfirmPromptState {
  sessionId?: string;
}

export interface ConfirmPromptSessionState {
  level: "warning" | "info";
  message: string;
  actions: string[];
}

const confirmPromptImplementationId = roleImplementationId("default.confirmPrompt");
let activeSessionId: string | undefined;

export function defaultConfirmPrompt(): EditorModule {
  return (editor) => {
    editor.roles.setFallback("confirmPrompt", confirmPromptImplementationId);
    editor.roles.register<ConfirmPromptState>({
      role: "confirmPrompt",
      implementationId: confirmPromptImplementationId,
      title: "Confirm Prompt",
      snapshot() {
        return { sessionId: activeSessionId };
      },
      close() {
        if (activeSessionId) editor.sessions.dispose(activeSessionId);
        activeSessionId = undefined;
        editor.workbench.invalidate("roles");
      },
    });
  };
}

export async function confirmPrompt(
  editor: Parameters<EditorModule>[0],
  input: ConfirmPromptSessionState,
): Promise<string | undefined> {
  if (editor.nativeDialogs) {
    return editor.nativeDialogs.prompt({
      level: input.level === "info" ? "info" : "warning",
      message: input.message,
      actions: input.actions,
    });
  }

  if (activeSessionId) editor.sessions.dispose(activeSessionId);

  return new Promise((resolve) => {
    const session = editor.sessions.create<ConfirmPromptSessionState>({
      kind: "confirmPrompt",
      role: "confirmPrompt",
      title: "Confirm",
      state: input,
      methods: {
        choose(_ctx, args) {
          const action = typeof (args as { action?: unknown } | undefined)?.action === "string"
            ? (args as { action: string }).action
            : undefined;
          cleanup(action);
        },
        close() {
          cleanup(undefined);
        },
      },
      dispose() {
        if (activeSessionId === session.id) activeSessionId = undefined;
      },
    });

    activeSessionId = session.id;
    editor.workbench.invalidate("roles");

    function cleanup(action: string | undefined): void {
      if (activeSessionId === session.id) activeSessionId = undefined;
      editor.sessions.dispose(session.id);
      editor.workbench.invalidate("roles");
      resolve(action);
    }
  });
}
