import type {
  FontFaceDefinition,
  FontFeatureSettings,
  ResolvedFontSystem,
} from "@the-editor/fonts";
import { cssFontFeatureSettings } from "@the-editor/fonts";

export type FontFileLoader = (path: string) => Promise<ArrayBuffer>;

type LoadedFace = {
  readonly face: FontFace;
};

type LoadingFace = {
  readonly key: string;
};

export type FontDomDriver = {
  readonly apply: (system: ResolvedFontSystem) => void;
  readonly dispose: () => void;
};

const fontFaceKey = (face: FontFaceDefinition): string =>
  JSON.stringify({
    id: face.id,
    family: face.family,
    source: face.source,
    weight: face.weight,
    style: face.style,
    display: face.display,
    unicodeRange: face.unicodeRange,
    featureSettings: face.featureSettings,
  });

const descriptorFor = (face: FontFaceDefinition): FontFaceDescriptors => {
  const descriptor: FontFaceDescriptors = {};
  if (face.weight !== undefined) descriptor.weight = String(face.weight);
  if (face.style !== undefined) descriptor.style = face.style;
  if (face.display !== undefined) descriptor.display = face.display;
  if (face.unicodeRange !== undefined) descriptor.unicodeRange = face.unicodeRange;
  if (face.featureSettings !== undefined) {
    descriptor.featureSettings = cssFontFeatureSettings(face.featureSettings as FontFeatureSettings);
  }
  return descriptor;
};

const sourceFor = async (
  face: FontFaceDefinition,
  loadFile: FontFileLoader | undefined,
): Promise<string | ArrayBuffer> => {
  switch (face.source.kind) {
    case "url":
      return `url(${JSON.stringify(face.source.url)})`;
    case "file":
      if (loadFile === undefined) throw new Error("No font file loader is available");
      return loadFile(face.source.path);
  }
};

const applyCssVars = (system: ResolvedFontSystem): void => {
  const root = document.documentElement;
  for (const [name, value] of Object.entries(system.cssVars)) root.style.setProperty(name, value);
};

export const createFontDomDriver = (loadFile?: FontFileLoader): FontDomDriver => {
  const loaded = new Map<string, LoadedFace>();
  const loading = new Map<string, LoadingFace>();
  let disposed = false;

  const unload = (key: string, entry: LoadedFace): void => {
    document.fonts.delete(entry.face);
    loaded.delete(key);
  };

  const load = (key: string, face: FontFaceDefinition): void => {
    if (typeof FontFace === "undefined") return;
    const loadingEntry: LoadingFace = { key };
    loading.set(key, loadingEntry);
    void (async () => {
      try {
        const source = await sourceFor(face, loadFile);
        const fontFace = new FontFace(face.family, source, descriptorFor(face));
        await fontFace.load();
        if (disposed || loading.get(key) !== loadingEntry) return;
        document.fonts.add(fontFace);
        loaded.set(key, { face: fontFace });
      } catch (error) {
        console.error(`[fonts] failed to load ${face.id}:`, error);
      } finally {
        if (loading.get(key) === loadingEntry) loading.delete(key);
      }
    })();
  };

  return {
    apply: (system) => {
      applyCssVars(system);
      const desired = new Set(system.faces.map(fontFaceKey));
      for (const [key, entry] of loaded) {
        if (!desired.has(key)) unload(key, entry);
      }
      for (const key of loading.keys()) {
        if (!desired.has(key)) loading.delete(key);
      }
      for (const face of system.faces) {
        const key = fontFaceKey(face);
        if (loaded.has(key) || loading.has(key)) continue;
        load(key, face);
      }
    },
    dispose: () => {
      disposed = true;
      loading.clear();
      for (const [key, entry] of loaded) unload(key, entry);
    },
  };
};
