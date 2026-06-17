/**
 * "Find in Files" dialog: a query input that runs a global content search and
 * opens the results as a multibuffer. The multibuffer IS the result list, so
 * this dialog only collects the query (Enter submits) — capability-gated on the
 * native finder + an open folder, mirroring the quick-open dialog.
 */
import { useEffect, useRef, useState } from "react";
import type { AppRuntime } from "../runtime";
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/ui/command";

export interface SearchDialogProps {
  readonly open: boolean;
  readonly runtime: AppRuntime;
  readonly onOpenChange: (open: boolean) => void;
}

export function SearchDialog({ open, runtime, onOpenChange }: SearchDialogProps) {
  const [query, setQuery] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const hasFinder = runtime.fileFinderPort() !== undefined;
  const hasRoot = runtime.workspaceRoot !== null;

  useEffect(() => {
    if (!open) {
      setQuery("");
      return;
    }
    const id = window.requestAnimationFrame(() => inputRef.current?.focus());
    return () => window.cancelAnimationFrame(id);
  }, [open]);

  const trimmed = query.trim();
  const ready = trimmed.length > 0 && hasFinder && hasRoot;

  const runSearch = () => {
    if (!ready) return;
    onOpenChange(false);
    void runtime.searchProject(trimmed);
  };

  const status = !hasRoot
    ? "Open a folder to search."
    : !hasFinder
      ? "Project search is unavailable in this host."
      : "Press Enter to search the project.";

  return (
    <CommandDialog
      open={open}
      onOpenChange={onOpenChange}
      title="Find in Files"
      description="Search file contents across the workspace"
      className="command-palette-dialog"
    >
      <Command className="command-palette file-finder" shouldFilter={false} loop>
        <CommandInput
          ref={inputRef}
          className="command-palette__input"
          placeholder="Search in files…"
          value={query}
          onValueChange={setQuery}
        />
        <div className="file-finder__body">
          <div className="file-finder__meta">{status}</div>
          <CommandList className="command-palette__list file-finder__list">
            <CommandEmpty className="command-palette__empty">
              Type a query to search across the project.
            </CommandEmpty>
            {ready ? (
              <CommandGroup className="command-palette__group" heading="Search">
                <CommandItem
                  className="command-palette__item file-finder__item"
                  value={trimmed}
                  onSelect={runSearch}
                >
                  <span className="file-finder__item-main">
                    <span className="command-palette__item-title file-finder__name">
                      Search for “{trimmed}”
                    </span>
                  </span>
                </CommandItem>
              </CommandGroup>
            ) : null}
          </CommandList>
        </div>
      </Command>
    </CommandDialog>
  );
}
