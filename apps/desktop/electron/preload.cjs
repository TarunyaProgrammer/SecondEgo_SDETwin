const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("secondEgoWindow", {
  setExpanded: (expanded) => ipcRenderer.invoke("secondego:notch-expanded", Boolean(expanded)),
});
