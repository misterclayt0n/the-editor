/**
 * The single picker surface: one dialog, one windowed list, N sources.
 *
 * Every `Picker.open` request — first-party or user extension — renders here.
 * The host owns the chrome (dialog shell, windowed rows, preview pane, status
 * line, footer hints, focus restore); the request owns the data and the
 * semantics (source, choose/dismiss, live selection effects, actions). Rows
 * render from the request's data fields; `renderItem`/`renderPreview` are the
 * shell-shaped escape hatches (ReactNodes in this shell), mirroring how
 * `PanelRenderer.icon` stays `unknown` in the package and concrete in the host.
 */

import {
  useCallback,
  useMemo,
  useSyncExternalStore,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import type {
  PickerAction,
  PickerController,
  PickerItem,
  PickerRequest,
} from "@the-editor/picker";
import type { AppRuntime } from "../runtime";
import { FileIcon } from "../workbench/fileIcons";
import { cn } from "@/lib/utils";
import { CommandDialog } from "@/ui/command";
import { VirtualCommand, type VirtualCommandRow } from "@/ui/virtual-command";
import { FilePreview } from "./FilePreviewPanel";
import type { PreviewTarget } from "./filePreview";

const DEFAULT_ROW_HEIGHT = 26;
const DETAIL_ROW_HEIGHT = 36;

interface PickerRow extends VirtualCommandRow {
  readonly item: PickerItem;
}

/** One chord spec like "mod+enter" / "alt+m" against a live event ("mod" =
 *  meta on mac, ctrl elsewhere — accepts either, matching the keymap system's
 *  forgiving stance for the picker's two-key vocabulary). */
const actionMatches = (keys: string, event: KeyboardEvent): boolean => {
  const parts = keys.toLowerCase().split("+");
  const key = parts[parts.length - 1]!;
  const wantMod = parts.includes("mod");
  const wantAlt = parts.includes("alt");
  const wantShift = parts.includes("shift");
  if (wantMod !== (event.metaKey || event.ctrlKey)) return false;
  if (wantAlt !== event.altKey) return false;
  if (wantShift !== event.shiftKey) return false;
  // Alt composes characters on macOS (⌥M → µ), so match on `code` for letters.
  if (key.length === 1) return event.code.toLowerCase() === `key${key}`;
  return event.key.toLowerCase() === key;
};

const actionLabel = (action: PickerAction): string =>
  typeof action.label === "function" ? action.label() : action.label;

function defaultRow(item: PickerItem, selected: boolean): ReactNode {
  if (item.header === true) {
    return (
      <div className="global-search__group-row">
        <span className="global-search__group-heading">
          {item.icon !== undefined && <FileIcon fileName={item.icon.fileName} />}
          <span className="global-search__group-file">{item.label}</span>
          {item.detail !== undefined && (
            <span className="global-search__group-directory">{item.detail}</span>
          )}
          {item.hint !== undefined && (
            <span className="global-search__group-count">{item.hint}</span>
          )}
        </span>
      </div>
    );
  }

  const fileLike =
    item.icon?.kind === "file" || item.detail !== undefined || item.badge !== undefined;

  return (
    <div
      className={cn("command-palette__item command-item", fileLike && "file-finder__item")}
      data-slot="command-item"
      data-selected={selected ? "true" : undefined}
    >
      {item.icon !== undefined && <FileIcon fileName={item.icon.fileName} />}
      {item.segments !== undefined ? (
        <span className="command-palette__item-title">
          {item.segments.map((seg, i) =>
            seg.match === true ? (
              <mark key={i} className={cn("global-search__hit", seg.className)}>
                {seg.text}
              </mark>
            ) : (
              <span key={i} className={seg.className}>
                {seg.text}
              </span>
            ),
          )}
        </span>
      ) : item.detail !== undefined ? (
        <span className="file-finder__item-main">
          <span className="command-palette__item-title file-finder__name">{item.label}</span>
          <span className="file-finder__path">{item.detail}</span>
        </span>
      ) : (
        <span className="command-palette__item-title">{item.label}</span>
      )}
      {item.badge !== undefined && <span className="file-finder__git">{item.badge}</span>}
      {item.hint !== undefined && <span className="command-palette__shortcut">{item.hint}</span>}
    </div>
  );
}

export interface PickerHostProps {
  readonly controller: PickerController;
  readonly runtime: AppRuntime;
  /** Runs after any close (choose or dismiss) — the focus-restore hook. */
  readonly onClosed?: () => void;
}

export function PickerHost({ controller, runtime, onClosed }: PickerHostProps) {
  const state = useSyncExternalStore(
    useCallback((cb: () => void) => controller.subscribe(cb), [controller]),
    useCallback(() => controller.state(), [controller]),
  );

  const request = state.request as PickerRequest | null;
  const chrome = request?.chrome;
  const canPreview = runtime.fileSystem()?.readTextRange !== undefined;
  const previewOf = request?.preview;
  const showDataPreview = previewOf !== undefined && canPreview;
  const showPreview = showDataPreview || request?.renderPreview !== undefined;

  const rows = useMemo<ReadonlyArray<PickerRow>>(
    () =>
      state.items.map((item) => ({
        id: item.id,
        item,
        ...(item.header === true ? { selectable: false as const } : {}),
      })),
    [state.items],
  );

  const rowHeight =
    request?.rowHeight ??
    (state.items.some((item) => item.detail !== undefined || item.badge !== undefined)
      ? DETAIL_ROW_HEIGHT
      : DEFAULT_ROW_HEIGHT);

  const selectedItem = state.items.find((item) => item.id === state.selectedId);
  const previewTarget = useMemo<PreviewTarget | null>(() => {
    if (!showDataPreview || selectedItem === undefined) return null;
    const spec = previewOf(selectedItem);
    if (spec === null) return null;
    return {
      path: spec.path,
      size: spec.size,
      ...(spec.focusLine !== undefined ? { focusLine: spec.focusLine } : {}),
    };
  }, [showDataPreview, previewOf, selectedItem]);

  const close = (reason: "choose" | "dismiss" | "action") => {
    if (reason !== "choose") controller.dismiss();
    onClosed?.();
  };

  const runAction = (action: PickerAction) => {
    const closes = action.closes !== false;
    const item = selectedItem ?? null;
    const query = state.query;
    // Close first for closing actions (matches choose semantics: the action's
    // work — a multibuffer spawn — replaces the dialog).
    if (closes) close("action");
    action.run(item, query);
  };

  const onInputKeyDown = (event: KeyboardEvent) => {
    const actions = request?.actions;
    if (actions === undefined) return;
    for (const action of actions) {
      if (actionMatches(action.keys, event)) {
        event.preventDefault();
        runAction(action);
        return;
      }
    }
  };

  const status = state.error ?? (state.loading ? "Searching…" : state.note);
  const emptyText =
    state.error ??
    (state.loading ? "Searching…" : (request?.emptyText?.(state.query) ?? "No results."));
  const actions = request?.actions;

  return (
    <CommandDialog
      open={state.open}
      onOpenChange={(open) => {
        if (!open) close("dismiss");
      }}
      title={request?.title ?? "Picker"}
      description={request?.source.placeholder ?? "Pick an item"}
      className={cn(
        "command-palette-dialog",
        chrome?.dialogClassName,
        showDataPreview && "file-finder-dialog--preview",
      )}
    >
      <VirtualCommand<PickerRow>
        className={cn(
          "command-palette command-root",
          chrome?.rootClassName,
          showDataPreview && "file-finder",
        )}
        bodyClassName={cn(
          chrome?.bodyClassName,
          showDataPreview && "file-finder__body file-finder__body--with-preview",
        )}
        listColClassName={cn(chrome?.listColClassName, showDataPreview && "file-finder__list-col")}
        listClassName={cn(
          "command-palette__list command-list",
          chrome?.listClassName,
          showDataPreview && "file-finder__list",
        )}
        inputClassName="command-palette__input command-input"
        placeholder={request?.source.placeholder ?? "Type to search…"}
        query={state.query}
        onQueryChange={(query) => controller.setQuery(query)}
        onInputKeyDown={onInputKeyDown}
        rows={rows}
        rowHeight={rowHeight}
        value={state.selectedId ?? ""}
        onValueChange={(id) => controller.setSelected(id)}
        onChoose={(row) => {
          controller.choose(row.id);
          close("choose");
        }}
        belowInput={
          status !== undefined && actions === undefined ? (
            <div className="file-finder__meta">{status}</div>
          ) : undefined
        }
        empty={
          <div className="command-palette__empty command-empty" data-slot="command-empty">
            {emptyText}
          </div>
        }
        footer={
          actions !== undefined ? (
            <div className="global-search__footer">
              <span className="global-search__status">{status ?? ""}</span>
              <span className="global-search__hints">
                {actions.map((action) => (
                  <button
                    key={action.keys}
                    type="button"
                    className="global-search__mode"
                    onClick={() => runAction(action)}
                  >
                    {actionLabel(action)}
                  </button>
                ))}
                <span className="global-search__hint">↵ open</span>
              </span>
            </div>
          ) : undefined
        }
        aside={
          request?.renderPreview !== undefined ? (
            (request.renderPreview(selectedItem ?? null) as ReactNode)
          ) : showDataPreview ? (
            <FilePreview target={previewTarget} runtime={runtime} />
          ) : null
        }
        renderRow={({ item }, { selected, query }) =>
          request?.renderItem !== undefined
            ? (request.renderItem(item, { selected, query }) as ReactNode)
            : defaultRow(item, selected)
        }
      />
    </CommandDialog>
  );
}
