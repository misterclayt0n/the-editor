import { Command as CommandPrimitive } from "cmdk";
import type { ComponentPropsWithoutRef, ElementRef, ForwardedRef } from "react";
import { forwardRef } from "react";

function cx(...classes: Array<string | false | null | undefined>): string {
  return classes.filter(Boolean).join(" ");
}

export const Command = forwardRef(function Command(
  { className, ...props }: ComponentPropsWithoutRef<typeof CommandPrimitive>,
  ref: ForwardedRef<ElementRef<typeof CommandPrimitive>>,
) {
  return <CommandPrimitive ref={ref} className={cx("commandPrimitive", className)} {...props} />;
});

export const CommandInput = forwardRef(function CommandInput(
  { className, ...props }: ComponentPropsWithoutRef<typeof CommandPrimitive.Input>,
  ref: ForwardedRef<ElementRef<typeof CommandPrimitive.Input>>,
) {
  return <CommandPrimitive.Input ref={ref} className={cx("commandPrimitiveInput", className)} {...props} />;
});

export const CommandList = forwardRef(function CommandList(
  { className, ...props }: ComponentPropsWithoutRef<typeof CommandPrimitive.List>,
  ref: ForwardedRef<ElementRef<typeof CommandPrimitive.List>>,
) {
  return <CommandPrimitive.List ref={ref} className={cx("commandPrimitiveList", className)} {...props} />;
});

export const CommandEmpty = forwardRef(function CommandEmpty(
  { className, ...props }: ComponentPropsWithoutRef<typeof CommandPrimitive.Empty>,
  ref: ForwardedRef<ElementRef<typeof CommandPrimitive.Empty>>,
) {
  return <CommandPrimitive.Empty ref={ref} className={cx("commandPrimitiveEmpty", className)} {...props} />;
});

export const CommandGroup = forwardRef(function CommandGroup(
  { className, ...props }: ComponentPropsWithoutRef<typeof CommandPrimitive.Group>,
  ref: ForwardedRef<ElementRef<typeof CommandPrimitive.Group>>,
) {
  return <CommandPrimitive.Group ref={ref} className={cx("commandPrimitiveGroup", className)} {...props} />;
});

export const CommandItem = forwardRef(function CommandItem(
  { className, ...props }: ComponentPropsWithoutRef<typeof CommandPrimitive.Item>,
  ref: ForwardedRef<ElementRef<typeof CommandPrimitive.Item>>,
) {
  return <CommandPrimitive.Item ref={ref} className={cx("commandPrimitiveItem", className)} {...props} />;
});

export const CommandSeparator = forwardRef(function CommandSeparator(
  { className, ...props }: ComponentPropsWithoutRef<typeof CommandPrimitive.Separator>,
  ref: ForwardedRef<ElementRef<typeof CommandPrimitive.Separator>>,
) {
  return <CommandPrimitive.Separator ref={ref} className={cx("commandPrimitiveSeparator", className)} {...props} />;
});

export function CommandShortcut({ className, ...props }: ComponentPropsWithoutRef<"span">) {
  return <span className={cx("commandPrimitiveShortcut", className)} {...props} />;
}
