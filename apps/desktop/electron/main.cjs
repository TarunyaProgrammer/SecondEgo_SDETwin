const { app, BrowserWindow, ipcMain, screen } = require("electron");
const { randomBytes } = require("node:crypto");
const { spawn } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const devUrl = process.env.SECONDEGO_DESKTOP_URL;
const gatewayPort = Number(process.env.SECONDEGO_GATEWAY_PORT || 8787);
let gatewayProcess;
let mainWindow;
const notchMode = process.platform === "darwin" && process.env.SECONDEGO_DESKTOP_MODE !== "window";

const NOTCH_SIZE = {
  closed: { width: 236, height: 38 },
  open: { width: 760, height: 620 },
};

function notchBounds(expanded) {
  const display = screen.getPrimaryDisplay();
  const size = expanded ? NOTCH_SIZE.open : NOTCH_SIZE.closed;
  return {
    x: Math.round(display.bounds.x + (display.bounds.width - size.width) / 2),
    y: display.bounds.y,
    width: size.width,
    height: size.height,
  };
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
  }

  const query = new URLSearchParams({
    ...(token ? { token } : {}),
    ...(notchMode ? { notch: "1" } : {}),
  }).toString();
  if (devUrl) {
    mainWindow.loadURL(`${devUrl}${query ? `?${query}` : ""}`);
  } else {
    mainWindow.loadFile(path.join(__dirname, "..", "dist", "index.html"), { search: query ? `?${query}` : "" });
  }

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

function startGateway() {
  const repoRoot = path.join(__dirname, "..", "..", "..");
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
  gatewayProcess.stdout.on("data", (data) => {
    if (data.toString().includes("SecondEgo UI:")) announce();
  });
  gatewayProcess.stderr.on("data", (data) => console.error(data.toString().trim()));
  gatewayProcess.on("error", (error) => {
    console.error(`Could not start SecondEgo gateway: ${error.message}`);
    announce();
  });
  // Rust reports readiness on stderr and Python reports it on stdout. The window
  // is observational and can safely retry health/run requests while either
  // process finishes binding its loopback socket.
  setTimeout(announce, 250);
  gatewayProcess.on("exit", () => {
    if (app.isReady() && mainWindow && !mainWindow.isDestroyed()) mainWindow.close();
  });
}

app.whenReady().then(() => {
  if (notchMode && app.dock) app.dock.hide();
  startGateway();
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
  if (process.platform !== "darwin") app.quit();
});

app.on("before-quit", () => {
  if (gatewayProcess) gatewayProcess.kill();
});
