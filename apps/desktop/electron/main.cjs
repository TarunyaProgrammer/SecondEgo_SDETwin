const { app, BrowserWindow } = require("electron");
const { randomBytes } = require("node:crypto");
const { spawn } = require("node:child_process");
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
  const python = process.env.SECONDEGO_PYTHON || path.join(repoRoot, ".venv", "bin", "python");
  const token = randomBytes(24).toString("base64url");
  gatewayProcess = spawn(python, ["-m", "SecondEgo.desktop.gateway", "--host", "127.0.0.1", "--port", String(gatewayPort)], {
    cwd: repoRoot,
    env: {
      ...process.env,
      PYTHONPATH: [path.join(repoRoot, "src"), process.env.PYTHONPATH].filter(Boolean).join(path.delimiter),
      SECONDEGO_UI_TOKEN: token,
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let announced = false;
  gatewayProcess.stdout.on("data", (data) => {
    if (!announced && data.toString().includes("SecondEgo UI:")) {
      announced = true;
      createWindow(token);
    }
  });
  gatewayProcess.stderr.on("data", (data) => console.error(data.toString().trim()));
  gatewayProcess.on("error", (error) => {
    console.error(`Could not start SecondEgo gateway: ${error.message}`);
    if (!announced) createWindow("");
  });
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
