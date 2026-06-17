/**
 * Backward-compatible rename prompt. New code should prefer PromptDialog.
 */

import { PromptDialog, type PromptDialogProps } from "./PromptDialog";

export interface RenameDialogProps {
  readonly open: boolean;
  readonly initialValue: string;
  readonly onSubmit: (value: string) => void;
  readonly onClose: () => void;
  readonly placeholder?: string;
  readonly ariaLabel?: string;
}

export function RenameDialog({
  open,
  initialValue,
  onSubmit,
  onClose,
  placeholder = "New name",
  ariaLabel = "Rename Symbol",
}: RenameDialogProps) {
  return (
    <PromptDialog
      open={open}
      title={ariaLabel}
      initialValue={initialValue}
      placeholder={placeholder}
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
      onSubmit={onSubmit}
    />
  );
}

export type { PromptDialogProps };
