/**
 * "Find in Files" prompt: a single query input. Press Enter to run the search.
 * No list, no command palette chrome.
 */
import type { AppRuntime } from "../runtime";
import { PromptDialog } from "./PromptDialog";

export interface SearchDialogProps {
  readonly open: boolean;
  readonly runtime: AppRuntime;
  readonly onOpenChange: (open: boolean) => void;
}

export function SearchDialog({ open, runtime, onOpenChange }: SearchDialogProps) {
  const hasFinder = runtime.fileFinderPort() !== undefined;
  const hasRoot = runtime.workspaceRoot !== null;
  const canSearch = hasFinder && hasRoot;

  const placeholder = !hasRoot
    ? "Open a folder to search"
    : !hasFinder
      ? "Project search unavailable"
      : "Search in files…";

  return (
    <PromptDialog
      open={open}
      title="Find in Files"
      description="Search file contents across the workspace"
      initialValue=""
      placeholder={placeholder}
      onOpenChange={onOpenChange}
      onSubmit={(value) => {
        const trimmed = value.trim();
        if (trimmed.length === 0 || !canSearch) return;
        onOpenChange(false);
        void runtime.searchProject(trimmed);
      }}
    />
  );
}
