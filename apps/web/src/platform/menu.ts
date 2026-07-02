/**
 * The OS context-menu bridge (host-only). On desktop it pops the real native
 * menu via Electron; in a plain browser there's no OS menu API, so a tiny
 * plain-DOM popup stands in (no component library). Either way the shell never
 * builds a menu widget — the platform draws it.
 */

export type NativeMenuItem =
  | {
      readonly id: string;
      readonly label: string;
      readonly enabled?: boolean;
      readonly accelerator?: string;
    }
  | { readonly separator: true };

export type MenuPosition = { readonly x: number; readonly y: number };

export interface MenuPort {
  popup(items: ReadonlyArray<NativeMenuItem>, position: MenuPosition): Promise<string | null>;
}

/** The Electron native menu, when the desktop bridge is present. */
export function createMenuPort(): MenuPort | undefined {
  const bridge = (window as unknown as { theEditorMenu?: MenuPort }).theEditorMenu;
  return bridge ?? undefined;
}

/**
 * Plain-DOM fallback popup for the web: a positioned list, dismissed on pick,
 * Escape, scroll, or outside pointer-down. Deliberately minimal — the desktop
 * path is the real one; this just keeps the web build functional.
 */
export function popupDomMenu(
  items: ReadonlyArray<NativeMenuItem>,
  position: MenuPosition,
): Promise<string | null> {
  return new Promise((resolve) => {
    const menu = document.createElement("div");
    menu.className = "os-menu";
    menu.setAttribute("role", "menu");
    menu.style.position = "fixed";
    menu.style.left = `${position.x}px`;
    menu.style.top = `${position.y}px`;
    menu.style.zIndex = "2147483647";

    let settled = false;
    const finish = (id: string | null) => {
      if (settled) return;
      settled = true;
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("pointerdown", onOutside, true);
      window.removeEventListener("blur", onBlur, true);
      menu.remove();
      resolve(id);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        finish(null);
      }
    };
    const onOutside = (event: PointerEvent) => {
      if (!menu.contains(event.target as Node)) finish(null);
    };
    const onBlur = () => finish(null);

    for (const item of items) {
      if ("separator" in item) {
        const sep = document.createElement("div");
        sep.className = "os-menu__separator";
        sep.setAttribute("role", "separator");
        menu.append(sep);
        continue;
      }
      const row = document.createElement("button");
      row.type = "button";
      row.className = "os-menu__item";
      row.setAttribute("role", "menuitem");
      row.disabled = item.enabled === false;
      const label = document.createElement("span");
      label.className = "os-menu__label";
      label.textContent = item.label;
      row.append(label);
      if (item.accelerator !== undefined) {
        const accel = document.createElement("span");
        accel.className = "os-menu__accelerator";
        accel.textContent = item.accelerator;
        row.append(accel);
      }
      if (item.enabled !== false) {
        row.addEventListener("click", () => finish(item.id));
      }
      menu.append(row);
    }

    document.body.append(menu);
    // Keep the menu on-screen: nudge left/up if it overflows the viewport.
    const rect = menu.getBoundingClientRect();
    if (rect.right > window.innerWidth) menu.style.left = `${Math.max(0, window.innerWidth - rect.width)}px`;
    if (rect.bottom > window.innerHeight) menu.style.top = `${Math.max(0, window.innerHeight - rect.height)}px`;

    window.addEventListener("keydown", onKey, true);
    window.addEventListener("pointerdown", onOutside, true);
    window.addEventListener("blur", onBlur, true);
  });
}
