const { app, BrowserWindow } = require("electron");
const { randomBytes } = require("node:crypto");
const { spawn } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const devUrl = process.env.SECONDEGO_DESKTOP_URL;
const gatewayPort = Number(process.env.SECONDEGO_GATEWAY_PORT || 8787);
let gatewayProcess;
let mainWindow;

function createWindow(token) {
  mainWindow = new BrowserWindow({
    width: 1440,
    height: 960,
    minWidth: 980,
    minHeight: 680,
    backgroundColor: "#0b0f14",
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });

  const query = token ? `?token=${encodeURIComponent(token)}` : "";
  if (devUrl) {
    mainWindow.loadURL(`${devUrl}${query}`);
  } else {
    mainWindow.loadFile(path.join(__dirname, "..", "dist", "index.html"), { search: query });
  }
}

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
  startGateway();
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
