# OpenSky Driver 0.35.2 integration candidate

Imports immutable upstream `ad694f82aaaa12e67257e4510837fc1f6cfc9484` with original commit ancestry and retains OpenSky branding, source-only installers, custom advertised Mac actions, exact window/document identity, native paste, viewport observations, input uncertainty and X11 Unicode delivery corrections.

The new native compact observation default requires **OpenSky SDK 0.2.2 or newer**. The driver advertises `windowStateViewCompatibility: "compact-v1"`; SDK transports request the complete structured response while preserving explicit output choices. Older SDK versions should be upgraded before this driver.

Native HTML options do not select through synthetic `.click()`. The DOM-event route now refuses them with `browser_action_unavailable` before dispatching a click. HTML tag and namespace are checked in the same live-node JavaScript turn; ordinary buttons and ARIA options keep their synthetic route. Trusted foreground input is not replaced or replayed. This addresses the false-success mechanism reported in [SDK issue54](https://github.com/tanishqkancharla/opensky/issues/54); actual new-driver controls remain pending.

## Evidence at source reconciliation

- 67 contract, 1,034 core and 605 Mac unit tests pass; six desktop tests are ignored. These are unit/contract evidence.
- Canonical Python/TypeScript bindings regenerated from the Rust library.
- Native Mac CLI/tool registry regenerated. Linux/Windows snapshots retain explicit derivation provenance and require native dump verification.
- The whole Cargo lock owner checks16 lockfiles with zero failures/drift. Edited workflow expressions pass actionlint.
- Mac release driver/cursor build passes. Both the source installer and canonical binding generator keep compiler-plugin symbols intact; SDK/driver optimization remains unchanged.
- The original installed Driver0.34.1 reproduces native HTML option no-op in real isolated Chrome: selection stays Standard after a dispatched click; ARIA option succeeds. Exact native activation before a trusted parent-select click and native ArrowDown/Enter changes selection to Express. Earlier failed controls are retained privately.

This candidate has not yet replaced the installed driver. New-driver Mac application effects and TigerVNC/XFWM4 with sandboxed Chrome remain to be verified. [Issue53](https://github.com/tanishqkancharla/opensky/issues/53), covered GTK captures, remains unresolved. No VM, sandbox bypass, TCC reset, unsigned driver release or production Linux acceptance is claimed.
