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
  onGatewayStatus: (callback) => {
    const listener = (_event, status) => callback(status);
    ipcRenderer.on("secondego:gateway-status", listener);
    return () => ipcRenderer.removeListener("secondego:gateway-status", listener);
  },
  listProviderProfiles: () => ipcRenderer.invoke("secondego:providers:list"),
  saveProviderProfile: (profile) => ipcRenderer.invoke("secondego:providers:save", profile),
  setActiveProviderProfile: (profile) => ipcRenderer.invoke("secondego:providers:activate", profile),
  deleteProviderProfile: (profile) => ipcRenderer.invoke("secondego:providers:delete", profile),
});
