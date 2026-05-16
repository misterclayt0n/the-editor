import type { ComponentPropsWithoutRef, ForwardedRef } from "react";
import { forwardRef } from "react";

function cn(...classes: Array<string | false | null | undefined>): string {
  return classes.filter(Boolean).join(" ");
}

export const Command = forwardRef(function Command(
  { className, shouldFilter: _shouldFilter, value: _value, ...props }: ComponentPropsWithoutRef<"div"> & { shouldFilter?: boolean; value?: string },
  ref: ForwardedRef<HTMLDivElement>,
) {
  return <div ref={ref} className={cn("flex min-h-0 flex-col overflow-hidden text-popover-foreground", className)} {...props} />;
});

export const CommandInput = forwardRef(function CommandInput(
  { className, onValueChange, ...props }: Omit<ComponentPropsWithoutRef<"input">, "onChange"> & { onValueChange?: (value: string) => void },
  ref: ForwardedRef<HTMLInputElement>,
) {
  return (
    <input
      ref={ref}
      className={cn(
        "flex h-11 w-full rounded-md bg-transparent py-3 text-sm outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
      onChange={(event) => onValueChange?.(event.target.value)}
      {...props}
    />
  );
});

export const CommandList = forwardRef(function CommandList(
  { className, ...props }: ComponentPropsWithoutRef<"div">,
  ref: ForwardedRef<HTMLDivElement>,
) {
  return <div ref={ref} className={cn("max-h-[min(420px,calc(70vh-68px))] overflow-y-auto overflow-x-hidden p-1", className)} {...props} />;
});

export const CommandEmpty = forwardRef(function CommandEmpty(
  { className, ...props }: ComponentPropsWithoutRef<"div">,
  ref: ForwardedRef<HTMLDivElement>,
) {
  return <div ref={ref} className={cn("py-6 text-center text-sm text-muted-foreground", className)} {...props} />;
});

export const CommandGroup = forwardRef(function CommandGroup(
  { className, ...props }: ComponentPropsWithoutRef<"div">,
  ref: ForwardedRef<HTMLDivElement>,
) {
  return <div ref={ref} className={cn("overflow-hidden p-1 text-foreground", className)} {...props} />;
});

export const CommandItem = forwardRef(function CommandItem(
  { className, onSelect, onClick, value: _value, ...props }: Omit<ComponentPropsWithoutRef<"button">, "value"> & { value?: string; onSelect?: () => void },
  ref: ForwardedRef<HTMLButtonElement>,
) {
  return (
    <button
      ref={ref}
      type="button"
      className={cn(
        "relative flex w-full cursor-default select-none items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm outline-none data-[active=true]:bg-accent data-[active=true]:text-accent-foreground disabled:pointer-events-none disabled:opacity-50",
        className,
      )}
      onClick={(event) => {
        onClick?.(event);
        if (!event.defaultPrevented) onSelect?.();
      }}
      {...props}
    />
  );
});

export const CommandSeparator = forwardRef(function CommandSeparator(
  { className, ...props }: ComponentPropsWithoutRef<"div">,
  ref: ForwardedRef<HTMLDivElement>,
) {
  return <div ref={ref} role="separator" className={cn("-mx-1 h-px bg-border", className)} {...props} />;
});

export function CommandShortcut({ className, ...props }: ComponentPropsWithoutRef<"span">) {
  return <span className={cn("ml-auto text-xs tracking-widest text-muted-foreground", className)} {...props} />;
}
