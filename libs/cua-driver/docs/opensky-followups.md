# Deferred driver and backend work

The 2026-09-06 rename makes OpenSky Driver the exclusive native backend.
Native implementation now belongs in the `tanishqkancharla/cua` fork under
`libs/cua-driver`; SDK integration and consumer tests remain in OpenSky.
These capability gaps remain deferred: renaming the driver does not fix them.
Historical evidence IDs refer to OpenSky’s `docs/harness-friction.md`.
Historical failures were not rerun or reclassified as passes here.

Scope correction (2026-09-06): OpenSky targets standalone SDK consumers. Codex
in-app browsers and Codex deliverable/handoff lifecycle are not parity gaps.
Hidden standalone browser support is not inferred from an in-app browser.
Consumer-style SDK E2E drafts are in OpenSky’s `e2e/README.md`, with
real-fixture prerequisites and pending cases stated separately.

## Driver/backend requirements

| Gap | Required capability | Acceptance before claiming parity |
| --- | --- | --- |
| Native paste, including formatted and multiline text (LIB-009) | One exact-target compound paste operation that saves all clipboard formats, writes the requested text/HTML, dispatches paste, and conditionally restores only if the clipboard still belongs to that operation. Report ambiguous delivery without replay. | Actual editable app: text, Markdown and HTML; preserved unrelated clipboard formats; concurrent user clipboard write; wrong-focus/window tests; no duplicate paste after a failed receipt. |
| Browser paste (LIB-009/014) | Exact-tab paste with actual paste semantics, format negotiation, and page-scoped input. Computer documents Markdown as source text and no browser clipboard restoration. Do not silently implement paste with typing. | Textarea/contenteditable and a paste-event listener: multiline values, HTML, Markdown source, event delivery and exact-tab isolation. |
| Existing browser tabs and provider identity (LIB-014) | Enumerate provider/profile/tab IDs and selected tab; bind to an existing tab using verified provider identity. Distinguish user-owned from agent-created tabs. Current isolated owned sessions cannot establish the user's inventory. | Two profiles, duplicate titles/URLs, close/reopen with reused numeric IDs, explicit tab mentions, selected-tab changes, and cleanup that leaves user tabs open. |
| macOS trusted coordinate scroll (DRV-002) | Exact-tab trusted scrolling without silently activating a different window; preserve the underlying rejection when the platform refuses it. | A real canvas/custom scroller after a same-tab screenshot; movement visible in fresh state; background/Space changes, stale mapping and sibling isolation. |
| Semantic collection/context and text fidelity (LIB-015/021/022/023/024) | Retain required AX properties, text, source identity, order and qualifiers before projection. Supply bounded context/continuation with correct omissions and lifetime. Report readiness/change epochs if the source can prove them. | Repeat the declared generic article/list/table and separated-match probes against the exact driver build; test delayed content and virtualized lists without claiming completeness from an unchanged subset. The earlier e40 driver candidate remains separately awaiting live acceptance. |
| Exact action refusal details (DRV-003/005) | Preserve inner error code, reason and structured payload through the driver, CLI and MCP projection. A TypeScript parser cannot reconstruct discarded details. | Actual refused input with matching inner and public diagnostics; unknown delivery must never be automatically replayed. Existing TypeScript MCP support stays opt-in until its separate acceptance is complete. |
| Finder/Desktop and Safari/WebKit (DRV-001/SET-007) | Address Finder's nonordinary Desktop/file surface and prove exact Safari/WebKit page/window identity and input authority. | Open the requested resource, verify identity from fresh state, act on it, and clean up only proven-owned targets. |
| Native text selection/range actions (LIB-007/008) | Atomic exact-element selection/range updates when the current targeted-key route cannot deliver correct semantics or bounded latency. | Unicode, multiline and repeated-text disambiguation; cursor-before/after; background focus; visible control/selection change rather than a nominal acknowledgment. |
| Auxiliary windows and lifecycle (CLN-001/006) | Prove request-correlated sheets/panels/windows and expose exact cooperative close plus durable ownership receipts where missing. Cross-process TypeScript state transactions are a separate host concern. | Multiple windows, save sheets, interrupted close, crashes and concurrent runtimes; no broad close hotkeys or guessed sibling adoption. |
| Windows/Linux parity (SET-007) | Equivalent per-platform identity, AX/input, screenshot and cleanup contracts. | Real provisioned Windows/Linux runs; macOS fixtures do not certify these platforms. |

## Host integration, not automatically a driver rewrite

These also prevent complete Computer parity, but assigning them all to the native
driver would be misleading:

- **File/remote image URLs**: `nodeRepl.emitImage` needs an authorized host asset
  resolver. The strict evaluator currently accepts in-memory bytes/data URLs and
  keeps filesystem/network access unavailable.
- **Optional browser APIs**: capabilities, clipboard inventory, read-only page
  evaluation, locators, dialogs, exports, user-tab claiming and browser history
  need actual provider implementations. Inventory/capability APIs should advertise
  only supported operations, not fabricated objects.
- **Approval mediation and real-driver CI** (SET-004/005): a host/runner must supply
  consent handling and a provisioned, permissioned desktop. No tests or API stubs
  substitute for that deployment.
- **Concurrent state persistence** (CLN-006): locking/versioned transactions belong
  in the TypeScript session store; native ownership receipts and crash recovery
  are separate requirements. The current pass preserves the existing limitation.

For each later capability, record the exact driver/host revision, a failing
baseline, the corrected fresh state, and exact cleanup evidence. Keep contract
fixtures, live controlled probes and model evaluations separately labeled.


## Driver upgrade CI: Nix unit source metadata

Exact candidate03b213fb Nix unit job113449151606/37817169142 fails compilation because the filtered rustTestSrc omits installer-distribution.json, which four existing Rust owner tests include. Add the metadata file only to the test source fileset; runtime/shipped source and test assertions stay unchanged. The accepted comparison ELF43bcbdd4/source03b213fb is still pinned and no daemon rebuild or restart is performed. Existing workflow path-filter owners and local compile-input resolution checks are required; actual Linux Nix compilation remains pending. Keep the upgrade PR draft until all affected canonical gates pass on its final candidate.


## Driver upgrade CI: existing tools missing reference categories

Source03b213fb Linux reference job113454629383/37817169067 fails both checking and regeneration because mergeTools rejects browser_key and close_window. Categorize those existing exported tools as browser input and app/window operations respectively. Preserve the rejection for any genuinely uncategorized tool. This changes reference metadata only; tool schemas, authority, runtime and comparison driver stay unchanged. Existing generator/platform owners must pass; native reference regeneration and canonical documentation sync remain pending on the final candidate. Failed regeneration artifacts must not be treated as fresh native snapshots.


## SET340: closing X11 popup can terminate the daemon

OpenSky issue https://github.com/tanishqkancharla/opensky/issues/50 retains the original SDK1344/37874595367 failure on accepted driver03b213fb/ELF43bcbdd4. The named Green color is visible on slide1, but a subsequent pointer/observation cell loses the daemon and the saved file remains original. The driver log confirms fatal BadWindow on X_QueryTree. popup_info releases its attribute-query error handler before the PID fallback calls unguarded window_children; a closing transient can reach Xlib's default process-terminating handler. No failing native stack was captured.

The new closing-popup keeper uses the existing public helper and runs in the whole canonical click_input_x11 binary alongside its four original owners. Its pre-repair canonical run37875782929 is terminal red: the keeper exits on fatal BadWindow/opcode15 X_QueryTree. The PR merge checkout95aff204 has identical input/helper and keeper blobs to head34a770812; broad units pass, while later input owners are skipped after the keeper fails. The repair uses a checked x11rb child query on the exact Xlib display, retaining empty-on-missing-window semantics without adding a process-global handler swap, input replay, test hooks or public protocol changes. At the initial repair checkpoint those validations were pending. Corrected exact-source canonical Linux37877193111 subsequently passes the whole5click/5key binaries and all remaining Linux/Arch gates. Release/public Writer37877302884 passes core908/Linux641, both whole input binaries and all11existing Writer owners on one isolated desktop; saved text, keyboard maps, source/digest and exact owned cleanup are independently verified. SDK1345/37879430874 then completes the identical affected Impress/save prompt: same palette coordinate succeeds, saved slide1 RGB00A933, unchanged slide2/text and original grader1. Its exact app/helper/profile cleanup and usage are retained. Native1343 stays incomplete, so this pair remains unscored; no causal resource improvement is claimed. The comparison now pins source612355b6/ELF11ad0b43. Installed signed Mac remains unchanged and full stable-candidate cross-platform certification remains pending.

Existing whole click/key binaries now run before the broad all-target and embedding steps, without removing or filtering any owner. Actionlint and all15 existing workflow/inventory CPU owners pass; the original corrected hosted Linux run confirms those input binaries execute earlier. A causal latency gain is not claimed across different workers/caches.


## Driver upgrade CI: Mac text tools missing reference categories

Original Docs run37877194578 on source612355b6 fails Mac native regeneration because existing native_paste and select_text have no MCP category. Linux and Windows native regeneration succeeds but their generated pages/snapshots still show drift. Categorize select_text with text input/selection and native_paste with value/clipboard operations; preserve stable slugs and refusal for genuinely uncategorized tools. This changes generation metadata only, without runtime, binary, API, permissions or comparison-pin changes. All14existing generator/platform owners pass locally. Fresh ordinary PR Docs CI must regenerate valid native snapshots on all three platforms before canonical combined pages are regenerated. A failed Mac regeneration artifact is not a fresh native snapshot. Keep the upgrade draft until final gates and candidate certification are complete.


## Driver upgrade CI: Memcheck identity assertion

Original hosted Memcheck37817169197 reaches MCP initialize and rejects the actual opensky-driver server name because its runner hard-codes cua-driver. Read the expected name from the canonical installer-distribution.json product, as the existing Rust compatibility owner does, and keep exact equality plus invalid-metadata refusal. No product runtime, leak policy, shutdown checks or cleanup assertions change. One boundary owner proves the original runner red for the declared product and the corrected runner green; it also refuses two unrelated names with the exact validation error. All12runner owners pass locally. Hosted Memcheck acceptance remains pending; a local protocol fixture does not certify leak freedom.


## Upgrade reference and MCP client synchronization

Original Docs37881668926 on head3a0601ab8 successfully regenerates native Mac/Windows/Linux snapshots; its stale committed-reference checks remain red. All3actual checkouts29830db87 have exactly the same Git tree ae26940bb as the head. Native CLI JSON is byte-identical across all3. Import only their unchanged successful native snapshots and native CLI, then regenerate pages through canonical renderReference/loadSnapshots/syncFiles. All14generator/platform owners pass on these fresh snapshots; no hand-edited schema or failed artifact is used. Final ordinary Docs sync remains pending.

Original contract-client37881668850 fails exact raw rosters on all3platforms: existing fork additions browser_key/close_window, plus native_paste/select_text on Mac, are missing from the historical manifest. Extend the closed roster to60base tools plus2Mac/4Linux/1Windows tools, matching all3fresh native snapshot rosters exactly. Account for the2shared advertised output contracts in the exact schema counts (37Mac/41Linux/37Windows); actual wire count acceptance remains pending. Preserve all old expected tools and pinned clients. Like the existing Rust compatibility owner, validate actual legacy and modern MCP identity against canonical installer product while keeping historical fixture branding and stable skill URIs unchanged. Both client files parse and all52existing release-wiring owners pass. The first CPU command had an import-path setup error, retained separately; no native retry was used for that correction. Hosted raw/pinned/modern-client validation remains pending.

## Upgrade feedback improvements

Extend the already adopted main-or-same-repository-PR rust-cache save policy to3contract-client producers and Memcheck. Docs remains restore-only on the existing shared producer key. Compiler/lock/environment cache keying, exact source checks, permissions and test selections remain unchanged; no external-fork cache writes or measured speed-gain claim. Reuse OpenSky issue38 for this follow-up.

Original generated-bindings job113662512758 fails canonical UniFFI --check for _native.py/_native_contract.py. Add a failure-only canonical regeneration/artifact step, leaving the drift check red and all ordinary gates intact. Only successful regeneration publishes owned Python/TypeScript sources/manifests; parent must verify artifact source and import through exact generator ownership before rechecking. This avoids requiring a separate local Rust build for repair. Actionlint passes for all3affected workflows. Actual cache reuse, binding-artifact acceptance and final native checks remain pending. No product runtime, comparison pin, model prompt, grading or limit changes.


## Canonical binding drift and modern MCP namespace correction

Exact b4ba27628 ordinary contract-client run37884171640 fails the unchanged canonical UniFFI check. Its failure-only canonical regeneration and artifact upload both succeed on merge92e19c92e0d34205cca6799a192bd1a85c96137c, whose complete tree equals b4ba27628. Import only the seven manifest-owned generated outputs; five differ (two Python and three TypeScript), adding the existing close_window contract/SDK method. The wildcard-uploaded node-runtime.ts is byte-identical and is not imported. No bindings are hand-edited and the original failed gate remains retained. All52existing release-wiring owners, generated Python syntax, modern verifier syntax and diff checks pass locally; exact-candidate canonical --check and native SDK/client gates remain pending in ordinary CI.

The same original run passes pinned legacy discovery but all3modern clients reject the parent-introduced assertion expecting the distribution product in modern response metadata. Existing mcp_wire.rs and its native owner deliberately emit the stable cua-driver namespace; legacy initialize emits opensky-driver. Correct only the modern verifier to use the existing expected-tools.json namespace, preserving exact equality, every wire/schema/resource/skill assertion, stable skill URIs and the legacy product check. This is an assertion correction, not a runtime protocol rename or new desktop acceptance.

Hosted Memcheck37884171672 at b4ba27628 succeeds with0errors and the unchanged leak policy, and TestScripts/Release metadata pass. Docs checks, generated binding recheck, all3actual modern clients, attribution and final stable-candidate certification remain open. The accepted paid comparison ELF stays source612355b6/11ad0b43; installed Mac is unchanged.
