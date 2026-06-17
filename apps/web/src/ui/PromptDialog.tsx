/**
 * Minimal single-input prompt dialog. Used by rename, generic string prompts,
 * and the live project-search query box.
 */

import { useEffect, useRef, useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/ui/dialog";

export interface PromptDialogProps {
  readonly open: boolean;
  readonly title: string;
  readonly description?: string;
  readonly initialValue: string;
  readonly placeholder?: string;
  readonly onOpenChange: (open: boolean) => void;
  /** Fired on every keystroke while the dialog is open. */
  readonly onChange?: (value: string) => void;
  /** Fired when the user presses Enter. */
  readonly onSubmit?: (value: string) => void;
}

export function PromptDialog({
  open,
  title,
  description,
  initialValue,
  placeholder,
  onOpenChange,
  onChange,
  onSubmit,
}: PromptDialogProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [value, setValue] = useState(initialValue);

  useEffect(() => {
    if (!open) {
      setValue(initialValue);
      return;
    }
    setValue(initialValue);
    const frame = window.requestAnimationFrame(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    });
    return () => window.cancelAnimationFrame(frame);
  }, [open, initialValue]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="prompt-dialog" showCloseButton={false}>
        <DialogHeader className="sr-only">
          <DialogTitle>{title}</DialogTitle>
          {description ? <DialogDescription>{description}</DialogDescription> : null}
        </DialogHeader>
        <input
          ref={inputRef}
          className="prompt-dialog__input"
          value={value}
          placeholder={placeholder}
          onChange={(event) => {
            const next = event.target.value;
            setValue(next);
            onChange?.(next);
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              onSubmit?.(value.trim());
              onOpenChange(false);
            } else if (event.key === "Escape") {
              event.preventDefault();
              onOpenChange(false);
            }
          }}
        />
      </DialogContent>
    </Dialog>
  );
}
