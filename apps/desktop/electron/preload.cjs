const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("secondEgoWindow", {
  setExpanded: (expanded) => ipcRenderer.invoke("secondego:notch-expanded", Boolean(expanded)),
  setGestureEnabled: (enabled) => ipcRenderer.invoke("secondego:gesture-enabled", Boolean(enabled)),
  onNotchCollapse: (callback) => {
    const listener = () => callback();
    ipcRenderer.on("secondego:notch-collapse", listener);
    return () => ipcRenderer.removeListener("secondego:notch-collapse", listener);
  },
  onGesture: (callback) => {
    const listener = (_event, gesture) => callback(gesture);
    ipcRenderer.on("secondego:gesture", listener);
    return () => ipcRenderer.removeListener("secondego:gesture", listener);
  },
  onGestureStatus: (callback) => {
    const listener = (_event, status) => callback(status);
    ipcRenderer.on("secondego:gesture-status", listener);
    return () => ipcRenderer.removeListener("secondego:gesture-status", listener);
  },
});
