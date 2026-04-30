const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("theEditor", {
  runtimeRequest(method, params) {
    return ipcRenderer.invoke("runtime:request", { method, params });
  },
  policyRequest(method, params) {
    return ipcRenderer.invoke("policy:request", { method, params });
  },
  onPolicyEvent(handler) {
    const listener = (_event, payload) => handler(payload);
    ipcRenderer.on("policy:event", listener);
    return () => ipcRenderer.off("policy:event", listener);
  },
});
