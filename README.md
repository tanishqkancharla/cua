# OpenSky Driver

The native backend for [OpenSky](https://github.com/tanishqkancharla/opensky), maintained in the [tanishqkancharla/cua fork](https://github.com/tanishqkancharla/cua). Native capability fixes belong here; SDK integration and consumer E2E tests belong in OpenSky.

Forked from [Cua](https://github.com/trycua/cua). The original license, contributor history, Rust crate names, and [upstream README](README.upstream.md) are retained. Implementation stays under `libs/cua-driver` to make upstream merges practical. Historical Cua documentation is not the OpenSky installation contract.

## Build and install

From this checkout, with Rust and platform build tools installed:

```sh
# macOS / Linux; --release is optional
bash libs/cua-driver/scripts/install.sh --release
```

```powershell
# Windows
.\libs\cua-driver\scripts\install.ps1
```

These entry points build this source tree; they never fetch upstream Cua releases. The installed executable is `opensky-driver` / `opensky-driver.exe`. The Cargo package and build output retain `cua-driver`; the installer gives the artifact its product name.

On macOS, `/Applications/OpenSkyDriver.app` displays **OpenSky Driver**, with bundle ID `com.opensky.driver`. It needs its own initial Accessibility and Screen Recording grants. Existing CuaDriverLocal grants do not transfer. Use `--require-stable-signing` with the installer's signing configuration for stable rebuilds.

```sh
opensky-driver --version
opensky-driver --opensky-driver-identity
opensky-driver permissions grant  # macOS, first setup
opensky doctor
```

Fork release artifacts are not assumed. OpenSky CI must set `OPENSKY_DRIVER_REF` to an exact fork commit after it has been pushed. Do not substitute upstream releases when setup fails.

## Product boundary

- Version: `opensky-driver <version>`. Offline identity: `{"product":"opensky-driver","protocolVersion":1,...}`; exits before logging, telemetry, permissions, or daemon startup.
- MCP name: `opensky-driver`. SDK/daemon contract: `0.7.0-opensky.1`, preventing tool calls through an explicit socket from accepting upstream's `0.7.0` daemon.
- State namespace: `opensky-driver`; install home: `~/.opensky-driver`. Separate sockets, PID files, Windows UIA pipe/task, Linux service, and macOS LaunchAgent.
- Upstream update downloads, announcements, and telemetry are disabled. Driver skill installation uses documentation embedded in this build.
- `CUA_*` configuration and Rust type/crate names remain compatibility details. The installer does not migrate or remove existing Cua applications, state, or permissions.

See [driver follow-ups](libs/cua-driver/docs/opensky-followups.md) and [validation](libs/cua-driver/docs/opensky-validation.md). Renaming and isolating the backend does not resolve the deferred desktop parity gaps.

Private `_upstream-*` scripts and `_install-rust.sh` retain upstream release tooling for merges and regression tests. OpenSky’s public entry points and runtime updater never call them.
