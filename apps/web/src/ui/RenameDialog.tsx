/**
 * Minimal single-input prompt, palette-styled: rename's name collector.
 * (The same primitive will serve future "ask for a string" commands.)
 */

import { useEffect, useRef, useState } from "react";

export function RenameDialog({
  open,
  initialValue,
  onSubmit,
  onClose,
  placeholder = "New name",
  ariaLabel = "Rename Symbol",
}: {
  readonly open: boolean;
  readonly initialValue: string;
  readonly onSubmit: (value: string) => void;
  readonly onClose: () => void;
  readonly placeholder?: string;
  readonly ariaLabel?: string;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [value, setValue] = useState(initialValue);

  useEffect(() => {
    if (!open) return;
    setValue(initialValue);
    const frame = window.requestAnimationFrame(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    });
    return () => window.cancelAnimationFrame(frame);
  }, [open, initialValue]);

  if (!open) return null;

  return (
    <div className="rename-dialog" role="dialog" aria-label={ariaLabel}>
      <input
        ref={inputRef}
        className="rename-dialog__input"
        value={value}
        placeholder={placeholder}
        onChange={(event) => setValue(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            onSubmit(value.trim());
            onClose();
          } else if (event.key === "Escape") {
            event.preventDefault();
            onClose();
          }
        }}
        onBlur={onClose}
      />
    </div>
  );
}
