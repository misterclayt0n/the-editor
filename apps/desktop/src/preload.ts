import { contextBridge } from "electron";

// No privileged capabilities yet. The bridge exists so the renderer can detect
// it runs inside the desktop shell, and as the seam for future host modules.
contextBridge.exposeInMainWorld("theEditorDesktop", {
  platform: process.platform,
});
