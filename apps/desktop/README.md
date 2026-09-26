# SecondEgo desktop observer

This is the optional Electron/React presentation shell. It does not contain
agent logic, model credentials, shell access, or direct repository writes.

The dependency-free browser observer can be started from the repository root:

```bash
make setup
make desktop
```

The Electron shell starts its own gateway. From this directory, install and build
the frontend, then run:

```bash
npm install
npm run desktop
```

The Electron main process starts the loopback Python gateway from the repository
`.venv` and passes its per-process token to the renderer. For development, run
the Vite server with `npm run dev`, then launch Electron with
`SECONDEGO_DESKTOP_URL=http://127.0.0.1:5173 npm run electron`.

The renderer consumes the versioned `EngineEvent` contract and polls bounded run
state. It must remain an observer: the Python engine owns planning, permissions,
tools, verification, recovery, and termination.
