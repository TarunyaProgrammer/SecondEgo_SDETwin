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

For the full terminal preflight and surface selector, use the repository-root
launcher instead:

```bash
make run
```

Choose `Desktop demo` or set `make run PROFILE=desktop` to launch a desktop-led
mission. The terminal launcher collects missing model/TTS keys without echoing
them and passes them only to the local gateway/engine process. Keys are never
sent to the renderer.

On macOS, `make desktop-electron` keeps the existing floating notch companion
as its default. It expands downward when clicked, collapses when clicked again
or when focus leaves the window, and stays above other windows. The optional
native AppKit/WebKit shell is available with `USE_MACOS_NATIVE=1`; it follows
the same interaction contract. Camera gestures are optional shortcuts
controlled from the companion settings. Set
`SECONDEGO_DESKTOP_MODE=window npm run electron` to open the full desktop
observer instead.

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

When `SECONDEGO_GATEWAY_PORT` is not explicitly set, Electron reserves an
ephemeral loopback port and passes the exact gateway URL to the renderer. This
avoids collisions with another local process and prevents the app from
silently attaching its UI to an unrelated gateway. Set the variable only when
an explicit stable port is required for development or an integration test.

The launcher explicitly removes `ELECTRON_RUN_AS_NODE`; if that variable is set
in the parent shell, Electron silently runs as Node.js and no window can open.

The renderer consumes the versioned `EngineEvent` contract and polls bounded run
state. It must remain an observer: the engine owns planning, permissions, tools,
verification, recovery, and termination. When gestures are enabled, the
renderer uses the configured camera path and plays a short local chime only
after a debounced gesture is accepted.

## Release packaging

The Electron Builder configuration produces a DMG on macOS, an NSIS installer
on Windows, and AppImage/deb artifacts on Linux. The Rust gateway is staged with
the platform-native executable name (`secondego-gateway` or
`secondego-gateway.exe`); the macOS credential helper is included only in the
macOS target. Release jobs write platform-specific manifests and checksums so
parallel jobs do not overwrite each other's metadata. Manual dispatch can build
unsigned candidates for QA; tagged macOS and Windows releases fail closed unless
their signing credentials are present (macOS also requires notarization).

The installer contract stays conventional: on macOS, open the branded DMG and
drag SecondEgo to the provided Applications shortcut; on Windows, run the
signed NSIS installer; on Linux, launch the AppImage after granting execute
permission or install the `.deb` package. The DMG artwork is intentionally
static so Finder remains predictable and accessible; motion belongs in the
first-run experience. First launch performs gateway startup and the
provider-vault setup inside the app rather than asking users to copy binaries or
edit release files by hand.
