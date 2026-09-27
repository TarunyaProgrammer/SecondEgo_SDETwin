# SecondEgo desktop observer

This is the optional Electron/React presentation shell. It does not contain
agent logic, model credentials, shell access, or direct repository writes.

The dependency-free Python compatibility browser observer can be started from the repository root:

```bash
make setup
make desktop
```

The Rust-backed Electron shell can be started with:

```bash
make desktop-electron
```

On macOS, `make desktop-electron` builds and starts a small native AppKit/WebKit
shell as a notch companion: it appears as a floating capsule centered at the
top edge, expands downward when clicked, and stays above other windows. Camera
gestures are optional shortcuts controlled from the companion settings. It is
assembled as a native `.app` bundle, avoiding Electron/Chromium startup
differences across macOS releases. On non-macOS systems, the same target uses
Electron. The browser observer started by `make desktop` remains a normal page.

The Electron shell starts its own gateway. From this directory, install and build
the frontend, then run:

```bash
npm install
npm run desktop
```

The Electron main process prefers the compiled Rust loopback gateway from
`engine-rs/target/{release,debug}` and falls back to the Python compatibility
gateway only when no Rust binary is available. It passes a per-process token to
the renderer. For development, run
the Vite server with `npm run dev`, then launch Electron with
`SECONDEGO_DESKTOP_URL=http://127.0.0.1:5173 npm run electron`.

The launcher explicitly removes `ELECTRON_RUN_AS_NODE`; if that variable is set
in the parent shell, Electron silently runs as Node.js and no window can open.

The renderer consumes the versioned `EngineEvent` contract and polls bounded run
state. It must remain an observer: the engine owns planning, permissions, tools,
verification, recovery, and termination.
