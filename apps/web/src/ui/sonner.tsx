"use client";

import * as React from "react";
import { useCallback, useSyncExternalStore } from "react";
import { Toaster as Sonner, type ToasterProps } from "sonner";
import type { AppRuntime } from "../runtime";

/**
 * Sonner, dressed in the editor's surface tokens (same language as the alert
 * dialog / command palette) and following the active theme's light/dark
 * appearance. The notification router drives it through `toast()`.
 */
export function Toaster({ runtime, ...props }: ToasterProps & { readonly runtime: AppRuntime }) {
  const appearance = useSyncExternalStore(
    useCallback((cb: () => void) => runtime.subscribeTheme(cb), [runtime]),
    useCallback(() => runtime.themeSnapshot().appearance, [runtime]),
  );

  return (
    <Sonner
      theme={appearance === "light" ? "light" : "dark"}
      position="bottom-right"
      className="toaster group"
      style={
        {
          "--normal-bg": "var(--bg-editor)",
          "--normal-text": "var(--fg)",
          "--normal-border": "var(--border)",
          "--border-radius": "var(--radius-lg)",
        } as React.CSSProperties
      }
      {...props}
    />
  );
}
