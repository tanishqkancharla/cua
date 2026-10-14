# OpenSky Driver0.35.3 source integration

Immutable upstream `ae2c117cd96673bb2dc0c2b3bff023ad70586b92` is merged with original ancestry at `40c404d3c602c4ff5fcda9bc131be31e7b588720`. This includes background Mac menu/save/Finder corrections, preservation of a user's keystrokes, explicit foreground desktop pointer requirements, minimal cursor badges and the first Chromium AT-SPI traversal wake. OpenSky exact browser/headless navigation contracts, Mac native plain/rich paste and set-value delivery ladder, Linux edit-dispatch uncertainty and X11 Unicode/capture fixes remain retained.

## Validation and cleanup

The four complete affected Rust crates pass2,213 unit/contract cases, with26 desktop cases ignored by their existing opt-in boundary. This is model-free unit/contract evidence, not native GUI acceptance. The additional proxy cleanup owner passes on an actual isolated daemon; reverting to leader-only cleanup makes it fail, and the mutation child is exactly reaped. The original two control-loss/session-expiry owners pass unchanged.

The initial local protocol gate failed before runtime startup: the disclaimed child was observed in dyld opening its executable under Documents, while its parent waited. A byte-identical executable in Library/Caches passes the original compatibility binary's four cases. Use the existing `CUA_TEST_DRIVER_BIN` seam for this controlled relocation; no permission or responsibility bypass is added. Protocol recovery now uses that same testkit binary selection and transfers its child to the canonical `ChildReaper`, which kills the owned Unix process group or Windows job. The original leader-only fixture left six exact test daemon children; all were recovered, with installed driver preserved.

Native Mac CLI/MCP references are regenerated from the candidate executable and pass the generator's drift check. Linux and Windows snapshots are explicitly reconciled and require native dump verification. Installer `install.ps1` remains the OpenSky source wrapper; `_upstream-install.ps1` exactly mirrors this immutable upstream downloader.

## Distribution boundary

At inspection on2026-10-14, upstream main contains the0.35.3 release commit but `cua-driver-rs-v0.35.3` release assets return404. The preserved upstream downloader still bakes0.35.2. This record describes a newer source candidate, not a published signed binary release. The installed stable-signed Mac driver remains0.35.2/source01d4 until separate installation and affected real GUI validation. Full canonical Mac/Windows/Wayland desktop acceptance remains separate; no VM, privileged container or Chrome sandbox bypass is used.


## Actual current Mac installation

Source1e5f6a146273763a0efa1cd96276d43b997684f3 is now installed as0.35.3, binary SHA25665ff7e6a1f6c294d1dc0ccb472f4dc1d22a478e1ac4c312b4cbf66cad08c0baf. The same certificate-backed com.opensky.driver requirement and both existing TCC grants are verified; actual launch-observation settings remain250/50ms. All26 required affected current Mac SDK desktop contracts pass with owned app/clipboard cleanup. Original clipboard watcher process-registry failure remains preserved; exact saved payload/cleanup passed, unchanged keeper and fresh full paste suite pass.

[OpenSky SDK0.2.7](https://github.com/tanishqkancharla/opensky/releases/tag/v0.2.7) is published with a verified public tarball checksum and installed-package Node22/24 read-only smokes against this exact signed driver. SDK runtime sourcea328614a5e592085568ce308d4b6cd2a4d25ff90; release sourced1a8ad5e793a23d5eb06e73329588a5f409ac95b. This subsequent maintenance-record commit changes no driver code or installed binary. Linux0353 native GUI, sandboxed Chrome and complete canonical desktop acceptance remain separate.
