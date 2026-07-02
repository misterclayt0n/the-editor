/**
 * The single picker surface: one dialog, one windowed list, N sources.
 *
 * Every `Picker.open` request — first-party or user extension — renders here.
 * The host owns the chrome (dialog shell, windowed rows, preview pane, status
 * line, focus restore); the request owns the data and the semantics (source,
 * choose/dismiss, live selection effects). Rows render from the request's
 * data fields; `renderItem` is the shell-shaped escape hatch (a ReactNode in
 * this shell), mirroring how `PanelRenderer.icon` stays `unknown` in the
 * package and concrete in the host.
 */

import { useCallback, useMemo, useSyncExternalStore, type ReactNode } from "react";
import type { PickerController, PickerItem, PickerRequest } from "@the-editor/picker";
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

function defaultRow(item: PickerItem, selected: boolean): ReactNode {
  const fileLike =
    item.icon?.kind === "file" || item.detail !== undefined || item.badge !== undefined;

  return (
    <div
      className={cn("command-palette__item command-item", fileLike && "file-finder__item")}
      data-slot="command-item"
      data-selected={selected ? "true" : undefined}
    >
      {item.icon !== undefined && <FileIcon fileName={item.icon.fileName} />}
      {item.detail !== undefined ? (
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
  const canPreview = runtime.fileSystem()?.readTextRange !== undefined;
  const previewOf = request?.preview;
  const showPreview = previewOf !== undefined && canPreview;

  const rows = useMemo<ReadonlyArray<PickerRow>>(
    () => state.items.map((item) => ({ id: item.id, item })),
    [state.items],
  );

  const rowHeight =
    request?.rowHeight ??
    (state.items.some((item) => item.detail !== undefined || item.badge !== undefined)
      ? DETAIL_ROW_HEIGHT
      : DEFAULT_ROW_HEIGHT);

  const selectedItem = state.items.find((item) => item.id === state.selectedId);
  const previewTarget = useMemo<PreviewTarget | null>(() => {
    if (!showPreview || selectedItem === undefined) return null;
    const spec = previewOf(selectedItem);
    if (spec === null) return null;
    return {
      path: spec.path,
      size: spec.size,
      ...(spec.focusLine !== undefined ? { focusLine: spec.focusLine } : {}),
    };
  }, [showPreview, previewOf, selectedItem]);

  const close = (reason: "choose" | "dismiss") => {
    if (reason === "dismiss") controller.dismiss();
    onClosed?.();
  };

  const status = state.error ?? (state.loading ? "Searching…" : state.note);

  return (
    <CommandDialog
      open={state.open}
      onOpenChange={(open) => {
        if (!open) close("dismiss");
      }}
      title={request?.title ?? "Picker"}
      description={request?.source.placeholder ?? "Pick an item"}
      className={cn("command-palette-dialog", showPreview && "file-finder-dialog--preview")}
    >
      <VirtualCommand<PickerRow>
        className={cn("command-palette command-root", showPreview && "file-finder")}
        bodyClassName={cn(showPreview && "file-finder__body file-finder__body--with-preview")}
        listColClassName={cn(showPreview && "file-finder__list-col")}
        listClassName={cn("command-palette__list command-list", showPreview && "file-finder__list")}
        inputClassName="command-palette__input command-input"
        placeholder={request?.source.placeholder ?? "Type to search…"}
        query={state.query}
        onQueryChange={(query) => controller.setQuery(query)}
        rows={rows}
        rowHeight={rowHeight}
        value={state.selectedId ?? ""}
        onValueChange={(id) => controller.setSelected(id)}
        onChoose={(row) => {
          controller.choose(row.id);
          close("choose");
        }}
        belowInput={
          status !== undefined ? <div className="file-finder__meta">{status}</div> : undefined
        }
        empty={
          <div className="command-palette__empty command-empty" data-slot="command-empty">
            {state.error ?? (state.loading ? "Searching…" : "No results.")}
          </div>
        }
        aside={showPreview ? <FilePreview target={previewTarget} runtime={runtime} /> : null}
        renderRow={({ item }, { selected, query }) =>
          request?.renderItem !== undefined
            ? (request.renderItem(item, { selected, query }) as ReactNode)
            : defaultRow(item, selected)
        }
      />
    </CommandDialog>
  );
}
