# OpenSky Driver rename validation — 2026-09-06

Source baseline: fork `14452f2e64c114e99e4bad3c6c8cec744c5e74ee`, which includes the previously installed query-context candidate. OpenSky baseline: `f190248ebf9198fd223578e150020d53f6c495f8`. Validation below applies to this rename/integration change, not to the deferred parity capabilities.

| Check | Result |
| --- | --- |
| OpenSky TypeScript build | Passed |
| OpenSky Node test suite, two workers | 336 passed |
| SDK E2E draft TypeScript check | Passed; desktop scenarios were not run |
| Rust driver binary unit suite | 192 passed |
| Complete installer-script pytest suite | 161 passed, 6 skipped; 3 subtests passed |
| Changed Unix installer shell syntax | Passed |
| Compiled macOS executable version/identity | Reported `opensky-driver 0.23.2` and identity protocol 1 |
| SDK `OpenSkyDriverClient.ensureHelper()` against the compiled fork | Accepted, with automatic installation/start disabled |
| Same SDK check against installed `CuaDriver.app` and `CuaDriverLocal.app` binaries | Both rejected after offline version checks |
| Git whitespace checks | Passed |

Rust commands use `cargo test --offline -p cua-driver --bin cua-driver` and `cargo build --offline -p cua-driver`. To reduce disk use, validation set `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, and `CARGO_PROFILE_TEST_DEBUG=0`, with a workspace target directory. The compiled artifact used for offline metadata checks was not installed or launched as a daemon.

The first SDK run hit resource/time limits during a concurrent Rust build; its successful rerun used two workers. The first Rust build required Swift cache access, and a later debug build exhausted available space. Only generated debug artifacts modified during this task were removed (3.29 GiB), then the smaller build completed. The linker emitted inherited Swift duplicate-symbol warnings; linking and tests succeeded.

Installer tests retain upstream release-tool coverage under private `_upstream-*` script names. Public entry points delegate only to the renamed source installer/uninstaller. A legacy autostart test needed an isolated empty process inventory so real host daemons could not affect its temporary fixture. One Linux-only running-binary test and five PowerShell-dependent tests were skipped on this Mac. Pytest also reported the repository's unused `asyncio_mode` setting because these synchronous tests did not load pytest-asyncio.

No installed app, CLI symlink, daemon, autostart setting, signing identity, or TCC grant was changed. Existing global `opensky` still points to the earlier checkout until the new SDK is linked/installed. The renamed app needs its first permission grants after installation. No GUI, Windows/Linux native runtime, hosted CI, release publication, or full desktop E2E acceptance is claimed. Inherited Cua release workflows have not been turned into an OpenSky release channel; distribution is source-build only, and OpenSky's real-driver CI requires an exact published fork SHA in `OPENSKY_DRIVER_REF`.
