import type { FontConfigDefinition, FontFeatureSettings, FontSlotDefinition } from "./types.js";

export const FONT_SLOT_UI = "ui";
export const FONT_SLOT_BUFFER = "buffer";
export const FONT_SLOT_TERMINAL = "terminal";
export const FONT_SLOT_AGENT_PROSE = "agent.prose";
export const FONT_SLOT_AGENT_CODE = "agent.code";

export const DEFAULT_UI_FONT: FontSlotDefinition = {
  family: "Zed Sans",
  fallbacks: [
    "IBM Plex Sans",
    "-apple-system",
    "BlinkMacSystemFont",
    "Segoe UI",
    "system-ui",
    "sans-serif",
  ],
  size: 13,
  weight: 400,
  style: "normal",
  lineHeight: 18,
  features: {},
};

export const DEFAULT_BUFFER_FEATURES: FontFeatureSettings = {
  liga: false,
  calt: true,
};

export const DEFAULT_BUFFER_FONT: FontSlotDefinition = {
  family: "Iosevka",
  fallbacks: ["SF Mono", "Menlo", "Monaco", "Cascadia Code", "monospace"],
  size: 13,
  weight: 400,
  style: "normal",
  lineHeight: 20,
  features: DEFAULT_BUFFER_FEATURES,
};

export const defaultFontConfig: FontConfigDefinition = {
  id: "the-editor/fonts/default-slots",
  slots: {
    [FONT_SLOT_UI]: DEFAULT_UI_FONT,
    [FONT_SLOT_BUFFER]: DEFAULT_BUFFER_FONT,
    [FONT_SLOT_TERMINAL]: {
      extends: FONT_SLOT_BUFFER,
      size: 12.5,
    },
    [FONT_SLOT_AGENT_PROSE]: {
      extends: FONT_SLOT_UI,
    },
    [FONT_SLOT_AGENT_CODE]: {
      extends: FONT_SLOT_BUFFER,
      size: 12.5,
    },
  },
};
