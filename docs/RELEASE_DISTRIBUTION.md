# SecondEgo release and distribution

This document defines how a build becomes a user-downloadable desktop release.
The website is the only public download surface. GitHub Releases stores the
artifacts for the release pipeline; the website exposes exact artifact URLs
only after they have passed the release checks.

## Artifact contract

Each platform release produces an installer, a SHA-256 checksum file, and a
platform-scoped JSON manifest:

- macOS: signed and notarized `.dmg` files for Apple Silicon and Intel.
- Windows: signed NSIS `.exe` installer for x64.
- Linux: `.AppImage` and `.deb` packages for x64. Linux artifacts are not
  treated as trusted until the project adds a signing and repository strategy.

The manifest records the product version, platform, architecture, signing
state, notarization state, artifact names, byte sizes, and SHA-256 digests.
Never publish an artifact by filename alone; promote the checksum and manifest
with it.

## Release flow

1. Create a version tag such as `v0.2.0` after the Rust workspace, desktop
   build, and website checks pass.
2. The release workflow builds the Rust gateway for each target and stages it
   inside the Electron package. The renderer does not contain the gateway or
   provider secrets.
3. macOS and Windows tagged builds fail closed when signing credentials are
   absent. macOS also fails when notarization credentials are absent.
4. The workflow publishes the installer, checksum, and manifest together to
   the GitHub release and keeps per-platform artifact names separate.
5. Review the manifest and install the artifact on a clean machine for each
   supported platform before enabling website links.
6. Deploy the website with the exact release URLs in the build environment:

   ```bash
   VITE_DOWNLOAD_MAC_URL="https://.../SecondEgo-0.2.0-arm64.dmg" \
   VITE_DOWNLOAD_WINDOWS_URL="https://.../SecondEgo-0.2.0-x64.exe" \
   VITE_DOWNLOAD_LINUX_URL="https://.../SecondEgo-0.2.0-x64.AppImage" \
   npm run build
   ```

The website intentionally leaves a platform unavailable when its URL is
missing. Do not add a fallback to an unverified build, a source archive, or a
third-party mirror.

## Installer behavior

The macOS DMG uses the conventional Finder flow: a branded window places the
SecondEgo application beside an Applications shortcut. Users drag the app to
Applications, eject the disk image, and launch the signed/notarized app. The
DMG artwork is static for predictable Finder behavior; motion belongs in the
first-launch product experience, not in the installer window.

The Windows NSIS installer should install per machine only when elevated
permissions are granted, preserve the user profile on upgrade, and create an
uninstaller. The Linux AppImage is portable; the `.deb` package is the
integration path for Debian-family systems. Neither installer should request a
provider key. Keys are entered after first launch and stored in the platform
credential vault.

## Promotion and rollback

Keep release URLs immutable. To roll back the website, redeploy the previous
known-good URL set rather than replacing an existing artifact. A release is
not considered promoted until:

- the installer opens on a clean supported OS;
- the bundled gateway starts and the first-run UI reaches provider setup;
- a provider preflight succeeds with a test key supplied by the tester;
- the download page highlights the detected platform and shows the other
  platform states accurately;
- the checksum and manifest are available beside the installer.

Updates are manual until a signed update channel is implemented. Do not add
silent update behavior before signing, downgrade protection, staged rollout,
and rollback telemetry are specified.

