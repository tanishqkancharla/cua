# OpenSky Driver0.35.3 source integration

Immutable upstream `ae2c117cd96673bb2dc0c2b3bff023ad70586b92` is merged with original ancestry at `40c404d3c602c4ff5fcda9bc131be31e7b588720`. This includes background Mac menu/save/Finder corrections, preservation of a user's keystrokes, explicit foreground desktop pointer requirements, minimal cursor badges and the first Chromium AT-SPI traversal wake. OpenSky exact browser/headless navigation contracts, Mac native plain/rich paste and set-value delivery ladder, Linux edit-dispatch uncertainty and X11 Unicode/capture fixes remain retained.

## Validation and cleanup

The four complete affected Rust crates pass2,213 unit/contract cases, with26 desktop cases ignored by their existing opt-in boundary. This is model-free unit/contract evidence, not native GUI acceptance. The additional proxy cleanup owner passes on an actual isolated daemon; reverting to leader-only cleanup makes it fail, and the mutation child is exactly reaped. The original two control-loss/session-expiry owners pass unchanged.

The initial local protocol gate failed before runtime startup: the disclaimed child was observed in dyld opening its executable under Documents, while its parent waited. A byte-identical executable in Library/Caches passes the original compatibility binary's four cases. Use the existing `CUA_TEST_DRIVER_BIN` seam for this controlled relocation; no permission or responsibility bypass is added. Protocol recovery now uses that same testkit binary selection and transfers its child to the canonical `ChildReaper`, which kills the owned Unix process group or Windows job. The original leader-only fixture left six exact test daemon children; all were recovered, with installed driver preserved.

Native Mac CLI/MCP references are regenerated from the candidate executable and pass the generator's drift check. Linux and Windows snapshots are explicitly reconciled and require native dump verification. Installer `install.ps1` remains the OpenSky source wrapper; `_upstream-install.ps1` exactly mirrors this immutable upstream downloader.

## Distribution boundary

At inspection on2026-10-14, upstream main contains the0.35.3 release commit but `cua-driver-rs-v0.35.3` release assets return404. The preserved upstream downloader still bakes0.35.2. This record describes a newer source candidate, not a published signed binary release. The installed stable-signed Mac driver remains0.35.2/source01d4 until separate installation and affected real GUI validation. Full canonical Mac/Windows/Wayland desktop acceptance remains separate; no VM, privileged container or Chrome sandbox bypass is used.
