import { describe, expect, test } from "vitest";
import { Effect, Layer } from "effect";
import { definePackage, toPackageDescriptor } from "@the-editor/extensions";
import { makePackageRuntime } from "@the-editor/kernel";
import {
  FONT_SLOT_BUFFER,
  FONT_SLOT_TERMINAL,
  FONT_SLOT_UI,
  Fonts,
  makeFonts,
  resolveFontSystem,
  type FontConfigDefinition,
} from "../src/index.js";

const customConfig = {
  id: "test/fonts",
  slots: {
    [FONT_SLOT_BUFFER]: {
      family: "Iosevka",
      fallbacks: ["Symbols Nerd Font Mono", "monospace"],
      size: 14,
      features: { liga: false, calt: true },
    },
  },
} as const satisfies FontConfigDefinition;

const defaultMonospaceCssFamily = [
  "Iosevka",
  "\"SF Mono\"",
  "Menlo",
  "Monaco",
  "\"Cascadia Code\"",
  "\"Symbols Nerd Font Mono\"",
  "\"MesloLGS Nerd Font\"",
  "\"JetBrainsMono Nerd Font\"",
  "\"Hack Nerd Font\"",
  "monospace",
].join(", ");

describe("resolveFontSystem", () => {
  test("defaults to Zed Sans UI and Iosevka code fonts", () => {
    const system = resolveFontSystem([], []);

    expect(system.slots[FONT_SLOT_UI]?.cssFamily).toBe(
      "\"Zed Sans\", \"IBM Plex Sans\", -apple-system, BlinkMacSystemFont, \"Segoe UI\", system-ui, sans-serif",
    );
    expect(system.slots[FONT_SLOT_BUFFER]?.cssFamily).toBe(defaultMonospaceCssFamily);
    expect(system.slots[FONT_SLOT_TERMINAL]?.cssFamily).toBe(defaultMonospaceCssFamily);
  });

  test("inherits terminal font fields from buffer", () => {
    const system = resolveFontSystem([], [customConfig]);
    expect(system.slots[FONT_SLOT_BUFFER]?.cssFamily).toBe(
      "Iosevka, \"Symbols Nerd Font Mono\", monospace",
    );
    expect(system.slots[FONT_SLOT_TERMINAL]?.cssFamily).toBe(
      "Iosevka, \"Symbols Nerd Font Mono\", monospace",
    );
    expect(system.slots[FONT_SLOT_TERMINAL]?.size).toBe(12.5);
  });

  test("allows terminal to override buffer", () => {
    const system = resolveFontSystem([], [
      customConfig,
      {
        id: "test/terminal",
        slots: {
          [FONT_SLOT_TERMINAL]: {
            family: "CommitMono",
            size: 13,
          },
        },
      },
    ]);

    expect(system.slots[FONT_SLOT_TERMINAL]?.cssFamily).toBe(
      "CommitMono, \"Symbols Nerd Font Mono\", monospace",
    );
    expect(system.slots[FONT_SLOT_TERMINAL]?.size).toBe(13);
  });
});

describe("Fonts", () => {
  test("removes package font config on disposal", () =>
    Effect.runSync(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePackageRuntime;
          const fonts = yield* makeFonts;
          const package_ = definePackage("test/font-package", () =>
            Effect.gen(function* () {
              const service = yield* Fonts;
              yield* service.configure({
                id: "test/transient-font",
                slots: {
                  [FONT_SLOT_BUFFER]: {
                    family: "CommitMono",
                  },
                },
              });
            }),
          );

          yield* runtime.activateGraph([toPackageDescriptor(package_, Layer.succeed(Fonts, fonts))]);
          expect((yield* fonts.resolveSlot(FONT_SLOT_BUFFER)).family).toBe("CommitMono");

          yield* runtime.disposePackage(package_.id);
          expect((yield* fonts.resolveSlot(FONT_SLOT_BUFFER)).family).toBe("Iosevka");
        }),
      ),
    ));
});
