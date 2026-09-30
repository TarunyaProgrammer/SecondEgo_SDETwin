const { spawn } = require("node:child_process");

const electronBinary = require("electron");
const child = spawn(electronBinary, [require.resolve("./main.cjs")], {
  env: { ...process.env, ELECTRON_RUN_AS_NODE: undefined },
  stdio: "inherit",
});

let terminalShutdown = false;

function stopFromTerminal(signal) {
  terminalShutdown = true;
  if (!child.killed) child.kill(signal === "SIGINT" ? "SIGINT" : "SIGTERM");
}

process.once("SIGINT", () => stopFromTerminal("SIGINT"));
process.once("SIGTERM", () => stopFromTerminal("SIGTERM"));

child.once("error", (error) => {
  console.error(`Could not launch SecondEgo desktop: ${error.message}`);
  process.exitCode = 1;
});

child.once("exit", (code, signal) => {
  const expectedSignal = signal === "SIGINT" || signal === "SIGTERM";
  process.exitCode = terminalShutdown || expectedSignal ? 0 : (code ?? 1);
});
