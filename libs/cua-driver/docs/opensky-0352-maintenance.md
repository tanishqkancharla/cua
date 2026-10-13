# OpenSky Driver 0.35.2 integration candidate

Imports immutable upstream `ad694f82aaaa12e67257e4510837fc1f6cfc9484` with original commit ancestry and retains OpenSky branding, source-only installers, custom advertised Mac actions, exact window/document identity, native paste, viewport observations, input uncertainty and X11 Unicode delivery corrections.

The new native compact observation default requires **OpenSky SDK 0.2.2 or newer**. The driver advertises `windowStateViewCompatibility: "compact-v1"`; SDK transports request the complete structured response while preserving explicit output choices. Older SDK versions should be upgraded before this driver.

Native HTML options do not select through synthetic `.click()`. The DOM-event route now refuses them with `browser_action_unavailable` before dispatching a click. HTML tag and namespace are checked in the same live-node JavaScript turn; ordinary buttons and ARIA options keep their synthetic route. Trusted foreground input is not replaced or replayed. This addresses the false-success mechanism reported in [SDK issue54](https://github.com/tanishqkancharla/opensky/issues/54); actual new-driver Mac controls now verify the refusal and authorized foreground selection with SDK0.2.3. Issue54 is closed.

## Evidence at source reconciliation

- 67 contract, 1,034 core and 605 Mac unit tests pass; six desktop tests are ignored. These are unit/contract evidence.
- Canonical Python/TypeScript bindings regenerated from the Rust library.
- Native Mac CLI/tool registry regenerated. Linux/Windows snapshots retain explicit derivation provenance and require native dump verification.
- The whole Cargo lock owner checks16 lockfiles with zero failures/drift. Edited workflow expressions pass actionlint.
- Mac release driver/cursor build passes. Both the source installer and canonical binding generator keep compiler-plugin symbols intact; SDK/driver optimization remains unchanged.
- The original installed Driver0.34.1 reproduces native HTML option no-op in real isolated Chrome: selection stays Standard after a dispatched click; ARIA option succeeds. Exact native activation before a trusted parent-select click and native ArrowDown/Enter changes selection to Express. Earlier failed controls are retained privately.

## Installed Mac application evidence

Source `455a0a9a3223d25c0214ff63e19218f39cbc5a6e` is installed as Driver0.35.2, binary SHA256 `256e1a671c6b159ebd03a293651fbd45f0c1c44e7529775d752c68685c311955`. The certificate-backed `com.opensky.driver` requirement and both existing TCC grants remain unchanged. Actual daemon launch-observation settings remain250ms/50ms.326 CLI owners also pass, for2,032 unit/contract cases in total.

The real isolated Chrome option control independently observes Standard with zero click/change events after synthetic refusal, successful ARIA choice, then Express with one change after explicitly authorized exact-window foreground input. Chrome launch flags contain no sandbox bypass. SDK0.2.3 preserves `browser_action_unavailable` and the actionable hint. Mac focus/sheets, window transitions, tab identity, Chrome find/keys, and rich-text observations pass with owned cleanup. One competing-copy control independently saves all69 expected characters and preserves the newer clipboard, while SDK reports uncertainty without replay. Earlier competing-copy fixture failures are retained; the full clipboard group remains under investigation. These controls do not certify Linux.

## Fork release and installer maintenance

Upstream-only release preparation, AWS website dispatch and cua.ai branded canaries are scoped to `trycua/cua`; this fork has different source installers and no upstream credentials. An OpenSky lane verifies declared source routes and stable signing contracts. The preserved Cua PowerShell downloader is synchronized byte-for-byte from immutable merged upstream, fixing a0.33.4/0.34.0 release-state mismatch.118 focused installer/release owners pass, one platform case is skipped, and edited workflows pass actionlint. This is installer-contract evidence, not a signed public driver binary release.

TigerVNC/XFWM4 with sandboxed Chrome remains to be verified. [Issue53](https://github.com/tanishqkancharla/opensky/issues/53), covered GTK captures, remains unresolved. No VM, sandbox bypass, TCC reset, unsigned driver release or production Linux acceptance is claimed.


## Expired-session admission contract (issue55)

The actual installed455a driver reproduces SDK0.2.4 CLI and MCP base-client failure after ending only that client's session; explicit start_session restores both. Upstream0.35.2 and the current upstream serve.rs do not fix this boundary. The daemon's early ended-session guard now returns a real ToolResult with isError:true and structuredContent status:refused/refusal.code:session_ended, before authorization, registry invocation or platform dispatch. Telemetry observes the refusal without completing an admitted session action. The SDK's existing strict own-base-label revival path consumes this envelope; no SDK string matching, generic retry or uncertainty waiver is added.

Runtime authorization revocation remains distinct: named and anonymous calls return authorization_suspended, and public start_session cannot restore revoked authority. The existing real-socket owner fails before repair, then326 whole CLI owners pass, including exact refusal/no-effect/no-delivery fields, unchanged invocation counters and terminal revocation restart refusal. This is hermetic transport evidence. Source01d4b9e47705289e3b56a848e35e3508de28017f is now signed/installed with unchanged TCC, binary SHA256 e60fc0f8303e44abf8a1f4efb3cfe9c57c3ae4b85ba7ff5f795a1fcbac1c95c2. All four real Mac CLI/MCP automatic/explicit base-label recovery controls pass with SDK0.2.4. Fresh focus/sheet/window/tab gates and exact owned cleanup pass. Linux session control38458644876 is pending; no production Chrome acceptance is claimed.


Current SDK0.2.4 fixes custom-action display when upstream Markdown omits intrinsic ShowMenu but the structured inventory retains it. Real Notes selection/reparenting/exact styled-note archive, Finder labels and primary-button controls pass on the prior455a binary, which carries the same Mac platform implementation. Current native X11 capture control38453584125 confirms a black covered GTK frame on Bookworm/Node22.23.3 TigerVNC/XFWM4 compositor off with bounded grants:99.9827% of pixels differ from the visible frame while the driver declares valid=true. Foreground and AT-SPI text remain intact; all owned process cleanup passes. Capture issue53 remains unresolved. [SDK evidence](https://github.com/tanishqkancharla/opensky/blob/main/docs/linux-covered-capture-53.json).
