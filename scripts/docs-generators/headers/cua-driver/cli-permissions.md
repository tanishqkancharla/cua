`opensky-driver permissions status` reports the selected daemon's native platform prerequisites: X11/AT-SPI on Linux, integrity/UIA on Windows, and attributed TCC grants on macOS. It is not proof that a target application action or capture will succeed.

- `permissions status --socket <path>` reads only that endpoint. An absent endpoint reports `status: "unknown"` and `daemon_running: false`; it never falls back to the default daemon. The command is read-only and never prompts. On macOS it does not run the ScreenCaptureKit probe (macOS Tahoe can show a separate dialog for it), so it reports `screen_recording_capturable: null` and `direct_capture_status: "not_checked"`, plus the last successful `permissions grant` verification when one is recorded for the running driver identity.
- `permissions grant` launches the installed `OpenSkyDriver.app` through LaunchServices so the prompts attribute to the app, then requires a successful live capture probe. It refuses `--socket` with exit64 before any grant UI. It never sends a prompt-capable request over the daemon socket.
- Embedded hosts, in-process `CuaDriver.create()` runtimes and `cua-driver mcp --direct` own their own prompts: there, `check_permissions` stays read-only even with `{"prompt": true}`.
- After a grant changes, quit and relaunch the responsible app (`OpenSkyDriver.app`, or the host of a direct runtime).

See [Permissions](/cua-driver/guides/permissions) for the grant flow and recovery.
