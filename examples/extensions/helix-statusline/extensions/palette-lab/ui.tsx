import { useEffect, useMemo, useRef } from "react";
import type { EditorUiModule } from "@the-editor/editor/ui";
import "./palette.css";

interface CommandPaletteItemView {
  id: string;
  title: string;
  detail?: string;
}

interface CommandPaletteViewState {
  visible: boolean;
  query: string;
  selectedIndex: number;
  placeholder?: string;
  items: CommandPaletteItemView[];
}

const uiModule: EditorUiModule = (host) => {
  host.roles.register<CommandPaletteViewState>({
    role: "commandPalette",
    implementationId: "default.commandPalette",
    render({ state, commands }) {
      if (!state?.visible) return null;
      return <HelixCommandBar state={state} dispatch={commands.dispatch} />;
    },
  });
};

export default uiModule;

function HelixCommandBar({
  state,
  dispatch,
}: {
  state: CommandPaletteViewState;
  dispatch: (dispatch: { id: string; args?: unknown }) => Promise<unknown>;
}) {
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const input = inputRef.current;
    if (!input) return;
    input.focus();
    const end = input.value.length;
    input.setSelectionRange(end, end);
  }, [state.visible]);

  const items = useMemo(() => {
    return state.items.map((item, index) => ({
      ...item,
      index,
      commandLabel: simplifyCommandLabel(item.title),
    }));
  }, [state.items]);

  return (
    <section className="helixCommandOverlay">
      <div className="helixCommandSheet">
        {items.length ? (
          <div className="helixCommandGrid" role="listbox" aria-label="Command suggestions">
            {items.map((item) => (
              <button
                key={item.id}
                type="button"
                className={item.index === state.selectedIndex ? "helixCommandItem active" : "helixCommandItem"}
                onMouseEnter={() => {
                  void dispatch({ id: "command_palette.selectIndex", args: { index: item.index } });
                }}
                onMouseDown={(event) => {
                  event.preventDefault();
                }}
                onClick={() => {
                  void dispatch({ id: "command_palette.selectIndex", args: { index: item.index } }).then(() => dispatch({ id: "command_palette.accept" }));
                }}
              >
                <span className="helixCommandItemLabel">{item.commandLabel}</span>
              </button>
            ))}
          </div>
        ) : (
          <div className="helixCommandEmpty">no matches</div>
        )}
      </div>

      <div className="helixCommandBar">
        <div className="helixCommandPrompt">:</div>
        <input
          ref={inputRef}
          className="helixCommandInput"
          placeholder={state.placeholder || "command"}
          spellCheck={false}
          value={stripLeadingColon(state.query)}
          onChange={(event) => {
            void dispatch({ id: "command_palette.setQuery", args: { query: normalizeCommandQuery(event.target.value) } });
          }}
          onKeyDown={(event) => {
            if (event.key === "ArrowUp") {
              event.preventDefault();
              void dispatch({ id: "command_palette.moveSelection", args: { delta: -1 } });
              return;
            }
            if (event.key === "ArrowDown") {
              event.preventDefault();
              void dispatch({ id: "command_palette.moveSelection", args: { delta: 1 } });
              return;
            }
            if (event.key === "Enter") {
              event.preventDefault();
              void dispatch({ id: "command_palette.accept" });
              return;
            }
            if (event.key === "Escape") {
              event.preventDefault();
              void dispatch({ id: "command_palette.close" });
            }
          }}
        />
      </div>
    </section>
  );
}

function simplifyCommandLabel(title: string): string {
  const trimmed = title.trim();
  const parts = trimmed.split(": ");
  return parts.length > 1 ? parts.slice(1).join(": ") : trimmed;
}

function stripLeadingColon(query: string): string {
  return query.startsWith(":") ? query.slice(1) : query;
}

function normalizeCommandQuery(query: string): string {
  const trimmed = stripLeadingColon(query);
  return `:${trimmed}`;
}
