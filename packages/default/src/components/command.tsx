import type { ComponentPropsWithoutRef, ForwardedRef } from "react";
import { forwardRef } from "react";

function cx(...classes: Array<string | false | null | undefined>): string {
  return classes.filter(Boolean).join(" ");
}

export const Command = forwardRef(function Command(
  { className, shouldFilter: _shouldFilter, value: _value, ...props }: ComponentPropsWithoutRef<"div"> & { shouldFilter?: boolean; value?: string },
  ref: ForwardedRef<HTMLDivElement>,
) {
  return <div ref={ref} className={cx("commandPrimitive", className)} {...props} />;
});

export const CommandInput = forwardRef(function CommandInput(
  { className, onValueChange, ...props }: Omit<ComponentPropsWithoutRef<"input">, "onChange"> & { onValueChange?: (value: string) => void },
  ref: ForwardedRef<HTMLInputElement>,
) {
  return <input ref={ref} className={cx("commandPrimitiveInput", className)} onChange={(event) => onValueChange?.(event.target.value)} {...props} />;
});

export const CommandList = forwardRef(function CommandList(
  { className, ...props }: ComponentPropsWithoutRef<"div">,
  ref: ForwardedRef<HTMLDivElement>,
) {
  return <div ref={ref} className={cx("commandPrimitiveList", className)} {...props} />;
});

export const CommandEmpty = forwardRef(function CommandEmpty(
  { className, ...props }: ComponentPropsWithoutRef<"div">,
  ref: ForwardedRef<HTMLDivElement>,
) {
  return <div ref={ref} className={cx("commandPrimitiveEmpty", className)} {...props} />;
});

export const CommandGroup = forwardRef(function CommandGroup(
  { className, ...props }: ComponentPropsWithoutRef<"div">,
  ref: ForwardedRef<HTMLDivElement>,
) {
  return <div ref={ref} className={cx("commandPrimitiveGroup", className)} {...props} />;
});

export const CommandItem = forwardRef(function CommandItem(
  { className, onSelect, onClick, value: _value, ...props }: Omit<ComponentPropsWithoutRef<"button">, "value"> & { value?: string; onSelect?: () => void },
  ref: ForwardedRef<HTMLButtonElement>,
) {
  return <button ref={ref} type="button" className={cx("commandPrimitiveItem", className)} onClick={(event) => { onClick?.(event); if (!event.defaultPrevented) onSelect?.(); }} {...props} />;
});

export const CommandSeparator = forwardRef(function CommandSeparator(
  { className, ...props }: ComponentPropsWithoutRef<"div">,
  ref: ForwardedRef<HTMLDivElement>,
) {
  return <div ref={ref} role="separator" className={cx("commandPrimitiveSeparator", className)} {...props} />;
});

export function CommandShortcut({ className, ...props }: ComponentPropsWithoutRef<"span">) {
  return <span className={cx("commandPrimitiveShortcut", className)} {...props} />;
}
