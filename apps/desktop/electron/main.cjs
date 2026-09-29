const { app, BrowserWindow, ipcMain, screen } = require("electron");
const { randomBytes } = require("node:crypto");
const { spawn } = require("node:child_process");
const fs = require("node:fs");
const net = require("node:net");
const path = require("node:path");

const devUrl = process.env.SECONDEGO_DESKTOP_URL;
const gatewayPort = Number(process.env.SECONDEGO_GATEWAY_PORT || 8787);
const repoRoot = path.join(__dirname, "..", "..", "..");
const allowedGestureNames = new Set(["thumbs_up", "thumbs_down", "open_palm"]);
const allowedGestureStates = new Set(["disabled", "starting", "active", "unavailable", "error"]);
const configuredGesturePort = Number(process.env.SECONDEGO_GESTURE_PORT || 52381);
const gesturePort = Number.isInteger(configuredGesturePort) && configuredGesturePort > 1023 && configuredGesturePort < 65536
  ? configuredGesturePort
  : 52381;
let gatewayProcess;
let mainWindow;
let gestureProcess;
let gestureSocket;
let gestureRetryTimer;
let gestureReadyTimer;
let gestureGeneration = 0;
let gestureEnabled = process.env.SECONDEGO_GESTURES_ENABLED === "true" || process.env.SECONDEGO_GESTURES_ENABLED === "1";
let gestureStatus = {
  schema_version: 1,
  state: "disabled",
  message: null,
  timestamp: new Date().toISOString(),
};
// Preserve the companion as the macOS default. A full desktop window remains
// explicitly available with SECONDEGO_DESKTOP_MODE=window.
const notchMode = process.platform === "darwin" && process.env.SECONDEGO_DESKTOP_MODE !== "window";

const NOTCH_SIZE = {
  closed: { width: 260, height: 46 },
  open: { width: 1180, height: 800 },
};

function notchBounds(expanded) {
  const display = screen.getPrimaryDisplay();
  const size = expanded ? NOTCH_SIZE.open : NOTCH_SIZE.closed;
  const width = Math.min(size.width, Math.max(236, display.workArea.width - 24));
  const height = Math.min(size.height, Math.max(38, display.workArea.height - 18));
  return {
    x: Math.round(display.bounds.x + (display.bounds.width - width) / 2),
    y: display.bounds.y,
    width,
    height,
  };
}

function sendGestureStatus() {
  if (!mainWindow || mainWindow.isDestroyed() || mainWindow.webContents.isDestroyed()) return;
  mainWindow.webContents.send("secondego:gesture-status", gestureStatus);
}

function setGestureStatus(state, message = null) {
  if (!allowedGestureStates.has(state)) return;
  gestureStatus = {
    schema_version: 1,
    state,
    message: typeof message === "string" && message.trim() ? message.trim().slice(0, 280) : null,
    timestamp: new Date().toISOString(),
  };
  if (state !== "starting" && gestureReadyTimer) {
    clearTimeout(gestureReadyTimer);
    gestureReadyTimer = undefined;
  }
  sendGestureStatus();
}

function safeGestureStatusMessage(status, message) {
  const detail = typeof message === "string" ? message : "";
  if (status === "unavailable" && /dependenc/i.test(detail)) {
    return "Landmark tracking is optional. From the repository root, run: .venv/bin/pip install -e '.[gesture]'";
  }
  if (status === "unavailable" && /camera/i.test(detail)) {
    return "The camera could not be opened. Check the app's macOS Camera permission, then try again.";
  }
  return detail || null;
}

function handleGestureFrame(line) {
  let frame;
  try {
    frame = JSON.parse(line);
  } catch {
    return;
  }
  if (!frame || typeof frame !== "object" || frame.schema_version !== 1) return;

  if (frame.event_type === "gesture.status" && typeof frame.status === "string" && allowedGestureStates.has(frame.status)) {
    setGestureStatus(frame.status, safeGestureStatusMessage(frame.status, frame.message));
    return;
  }

  if (
    frame.event_type !== "gesture.action"
    || !gestureEnabled
    || typeof frame.gesture !== "string"
    || !allowedGestureNames.has(frame.gesture)
    || !mainWindow
    || mainWindow.isDestroyed()
    || mainWindow.webContents.isDestroyed()
  ) {
    return;
  }

  const timestamp = typeof frame.timestamp === "string" && Number.isFinite(Date.parse(frame.timestamp))
    ? frame.timestamp
    : new Date().toISOString();
  mainWindow.webContents.send("secondego:gesture", {
    schema_version: 1,
    gesture: frame.gesture,
    hand: frame.hand === "Left" || frame.hand === "Right" ? frame.hand : "Right",
    confidence: 1,
    timestamp,
  });
}

function disconnectGestureBroker() {
  if (gestureSocket) {
    gestureSocket.destroy();
    gestureSocket = undefined;
  }
}

function scheduleGestureBrokerConnection(generation, delay = 250) {
  if (gestureRetryTimer) clearTimeout(gestureRetryTimer);
  gestureRetryTimer = setTimeout(() => {
    gestureRetryTimer = undefined;
    connectGestureBroker(generation);
  }, delay);
}

function connectGestureBroker(generation) {
  if (!gestureEnabled || generation !== gestureGeneration || !gestureProcess || gestureSocket) return;

  const socket = net.createConnection({ host: "127.0.0.1", port: gesturePort });
  let buffer = "";

  socket.setNoDelay(true);
  socket.on("data", (chunk) => {
    buffer = `${buffer}${chunk.toString("utf8")}`;
    if (buffer.length > 16_384) buffer = "";
    let newline = buffer.indexOf("\n");
    while (newline >= 0) {
      const line = buffer.slice(0, newline);
      buffer = buffer.slice(newline + 1);
      if (line.trim()) handleGestureFrame(line);
      newline = buffer.indexOf("\n");
    }
  });
  socket.on("error", () => {});
  socket.on("close", () => {
    if (gestureSocket === socket) gestureSocket = undefined;
    if (gestureEnabled && generation === gestureGeneration && gestureProcess) {
      scheduleGestureBrokerConnection(generation, 500);
    }
  });
  socket.on("connect", () => {
    if (!gestureEnabled || generation !== gestureGeneration || !gestureProcess) {
      socket.destroy();
      return;
    }
    gestureSocket = socket;
  });
}

function gesturePython() {
  if (process.env.SECONDEGO_PYTHON) return process.env.SECONDEGO_PYTHON;
  const localPython = path.join(repoRoot, ".venv", "bin", "python");
  return fs.existsSync(localPython) ? localPython : null;
}

function recordGestureWatcherError(output) {
  const text = output.toString();
  console.error(text.trim());
  if (/requires 'opencv-python' and 'mediapipe'/i.test(text)) {
    setGestureStatus("unavailable", "Landmark tracking is optional. From the repository root, run: .venv/bin/pip install -e '.[gesture]'");
  } else if (/cannot open camera/i.test(text)) {
    setGestureStatus("unavailable", "The camera could not be opened. Check the app's macOS Camera permission, then try again.");
  } else if (/address already in use/i.test(text)) {
    setGestureStatus("error", "Another local gesture tracker is using the companion port.");
  }
}

function startGestureService() {
  if (!gestureEnabled || gestureProcess) return gestureStatus;

  const python = gesturePython();
  if (!python) {
    setGestureStatus("unavailable", "The local Python environment was not found. Run make setup, then install the optional gesture dependencies.");
    return gestureStatus;
  }

  const generation = ++gestureGeneration;
  const camera = Number(process.env.SECONDEGO_GESTURE_CAMERA || 0);
  const cameraIndex = Number.isInteger(camera) && camera >= 0 ? camera : 0;
  setGestureStatus("starting");

  try {
    gestureProcess = spawn(python, [
      "-m", "SecondEgo.gesture.watcher",
      "--headless",
      "--port", String(gesturePort),
      "--camera", String(cameraIndex),
    ], {
      cwd: repoRoot,
      env: {
        ...process.env,
        PYTHONPATH: [path.join(repoRoot, "src"), process.env.PYTHONPATH].filter(Boolean).join(path.delimiter),
        PYTHONUNBUFFERED: "1",
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
  } catch (error) {
    setGestureStatus("error", `Could not start landmark tracking: ${error.message}`);
    return gestureStatus;
  }

  const child = gestureProcess;
  child.stdout.on("data", (data) => {
    const output = data.toString().trim();
    if (output) console.log(`[gestures] ${output}`);
  });
  child.stderr.on("data", recordGestureWatcherError);
  child.on("error", (error) => {
    if (gestureProcess === child) gestureProcess = undefined;
    if (gestureEnabled && generation === gestureGeneration) {
      setGestureStatus("error", `Could not start landmark tracking: ${error.message}`);
    }
  });
  child.on("exit", (code, signal) => {
    if (gestureProcess === child) gestureProcess = undefined;
    disconnectGestureBroker();
    if (!gestureEnabled || generation !== gestureGeneration) return;
    if (gestureStatus.state === "starting" || gestureStatus.state === "active") {
      setGestureStatus("error", code === 0
        ? "Landmark tracking stopped before a gesture was received."
        : `Landmark tracking exited unexpectedly${signal ? ` (${signal})` : ""}.`);
    }
  });

  connectGestureBroker(generation);
  gestureReadyTimer = setTimeout(() => {
    if (gestureEnabled && generation === gestureGeneration && gestureStatus.state === "starting") {
      setGestureStatus("error", "Landmark tracking did not become ready. Check camera permission and try again.");
    }
  }, 12_000);
  return gestureStatus;
}

function stopGestureService() {
  gestureGeneration += 1;
  if (gestureRetryTimer) clearTimeout(gestureRetryTimer);
  if (gestureReadyTimer) clearTimeout(gestureReadyTimer);
  gestureRetryTimer = undefined;
  gestureReadyTimer = undefined;
  disconnectGestureBroker();
  const child = gestureProcess;
  gestureProcess = undefined;
  if (child && !child.killed) child.kill();
  setGestureStatus("disabled");
}

function createWindow(token) {
  const initialBounds = notchMode ? notchBounds(false) : { width: 1440, height: 960 };
  mainWindow = new BrowserWindow({
    ...initialBounds,
    ...(notchMode
      ? {
          frame: false,
          transparent: true,
          backgroundColor: "#00000000",
          hasShadow: false,
          resizable: false,
          movable: false,
          minimizable: false,
          maximizable: false,
          fullscreenable: false,
          skipTaskbar: true,
          alwaysOnTop: true,
          visibleOnAllWorkspaces: true,
          titleBarStyle: "hidden",
        }
      : {
          minWidth: 980,
          minHeight: 680,
          backgroundColor: "#0b0f14",
        }),
    webPreferences: {
      preload: path.join(__dirname, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });

  if (notchMode) {
    mainWindow.setAlwaysOnTop(true, "floating");
    mainWindow.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true });
    mainWindow.on("blur", () => {
      if (!mainWindow || mainWindow.isDestroyed()) return;
      if (!mainWindow.webContents.isDestroyed()) {
        mainWindow.webContents.send("secondego:notch-collapse");
      }
      // Collapse at the host boundary as well as in React. This prevents a
      // renderer timing issue from leaving a large transparent hit target over
      // the desktop after the user clicks elsewhere.
      setTimeout(() => {
        if (!mainWindow || mainWindow.isDestroyed() || mainWindow.isFocused()) return;
        mainWindow.setBounds(notchBounds(false), false);
        mainWindow.showInactive();
      }, 40);
    });
  }

  const query = new URLSearchParams({
    ...(token ? { token } : {}),
    ...(notchMode ? { notch: "1" } : {}),
    ...(process.env.SECONDEGO_GESTURES_ENABLED
      ? { gestures: process.env.SECONDEGO_GESTURES_ENABLED === "true" || process.env.SECONDEGO_GESTURES_ENABLED === "1" ? "1" : "0" }
      : {}),
  }).toString();
  if (devUrl) {
    mainWindow.loadURL(`${devUrl}${query ? `?${query}` : ""}`);
  } else {
    mainWindow.loadFile(path.join(__dirname, "..", "dist", "index.html"), { search: query ? `?${query}` : "" });
  }

  mainWindow.webContents.once("did-finish-load", sendGestureStatus);

  mainWindow.once("ready-to-show", () => {
    if (notchMode) mainWindow.showInactive();
    else mainWindow.show();
  });
}

ipcMain.handle("secondego:notch-expanded", (_event, expanded) => {
  if (!notchMode || !mainWindow || mainWindow.isDestroyed()) return;
  mainWindow.setBounds(notchBounds(Boolean(expanded)), true);
  if (expanded) {
    mainWindow.show();
    mainWindow.focus();
  } else {
    mainWindow.showInactive();
  }
});

ipcMain.handle("secondego:gesture-enabled", (_event, enabled) => {
  gestureEnabled = Boolean(enabled);
  return gestureEnabled ? startGestureService() : (stopGestureService(), gestureStatus);
});

function startGateway() {
  const token = randomBytes(24).toString("base64url");
  const rustCandidates = [
    process.env.SECONDEGO_GATEWAY_BINARY,
    path.join(repoRoot, "engine-rs", "target", "release", "secondego-gateway"),
    path.join(repoRoot, "engine-rs", "target", "debug", "secondego-gateway"),
  ].filter(Boolean);
  const rustGateway = rustCandidates.find((candidate) => fs.existsSync(candidate));
  const python = process.env.SECONDEGO_PYTHON || path.join(repoRoot, ".venv", "bin", "python");
  const command = rustGateway || python;
  const args = rustGateway ? [] : ["-m", "SecondEgo.desktop.gateway", "--host", "127.0.0.1", "--port", String(gatewayPort)];
  gatewayProcess = spawn(command, args, {
    cwd: repoRoot,
    env: {
      ...process.env,
      PYTHONPATH: [path.join(repoRoot, "src"), process.env.PYTHONPATH].filter(Boolean).join(path.delimiter),
      SECONDEGO_UI_TOKEN: token,
      SECONDEGO_GATEWAY_PORT: String(gatewayPort),
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let announced = false;
  const announce = () => {
    if (!announced) {
      announced = true;
      createWindow(token);
    }
  };
  let portInUse = false;
  gatewayProcess.stdout.on("data", (data) => {
    if (data.toString().includes("SecondEgo UI:")) announce();
  });
  gatewayProcess.stderr.on("data", (data) => {
    const text = data.toString();
    console.error(text.trim());
    if (text.includes("Address already in use") || text.includes("in use")) {
      portInUse = true;
    }
  });
  gatewayProcess.on("error", (error) => {
    console.error(`Could not start SecondEgo gateway: ${error.message}`);
    announce();
  });
  // Rust reports readiness on stderr and Python reports it on stdout. The window
  // is observational and can safely retry health/run requests while either
  // process finishes binding its loopback socket.
  setTimeout(announce, 250);
  gatewayProcess.on("exit", () => {
    if (portInUse) {
      console.log("Gateway is already running externally; keeping UI open.");
      return;
    }
    if (app.isReady() && mainWindow && !mainWindow.isDestroyed()) mainWindow.close();
  });
}

app.whenReady().then(() => {
  if (notchMode && app.dock) app.dock.hide();
  startGateway();
  if (gestureEnabled) startGestureService();
  if (notchMode) {
    screen.on("display-metrics-changed", () => {
      if (mainWindow && !mainWindow.isDestroyed()) mainWindow.setBounds(notchBounds(false));
    });
  }
  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow("");
  });
});

app.on("window-all-closed", () => {
  if (gatewayProcess) gatewayProcess.kill();
  stopGestureService();
  if (process.platform !== "darwin") app.quit();
});

app.on("before-quit", () => {
  if (gatewayProcess) gatewayProcess.kill();
  stopGestureService();
});
