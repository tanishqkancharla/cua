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

## Exact installed app paths (2026-09-08)

Observed through the real OpenSky public REPL against a disposable VS Code
installation: getApp with its absolute .app path failed with "Provide either
bundle_id or name" although the process was already running. The native
reference accepted the same path. The driver set every running launch_path to
None, then borrowed installed metadata by bundle ID; multiple copies can share
that ID. The fix reads NSRunningApplication.bundleURL, merges installed entries
by exact path, leaves unknown paths unknown, and accepts an absolute launch_path
through the existing macOS launch route. The SDK preserves an explicit path
instead of replacing it with a bundle ID during launch recovery.

This corrects the macOS adapter; it does not claim new Windows/Linux behavior.
Three inventory checks and eleven existing launch checks pass; the local driver
build succeeds. Swift dependency duplicate-symbol linker warnings remain in the
existing build. Real path binding, cold launch, and stable-signing verification
are pending. The old permissioned driver has not been replaced. The fork has
issues disabled and no matching active PR was found; this document records the
narrow problem/decision for the dedicated codex/exact-app-paths branch.

## Fork build and contract checks (2026-09-08)

PR #2 ordinary CI exposed upstream identity assumptions left behind by the
OpenSky rename. The compatibility baseline now explicitly adapts only the CLI
command and MCP server name. The exact discovery manifest includes the fork's
`browser_key` and `close_window` tools and their two output schemas. The local
macOS metadata probe verified 58 tools / 34 output schemas and exited cleanly.
All four CLI/MCP compatibility checks passed (0.86 s) with the signed 05624b9bc
candidate selected for daemon fixtures. Bare unbundled daemon fixtures stalled
before creating a socket; cleanup was independently checked. This is protocol
evidence, not GUI acceptance or proof that unbundled startup works locally.

ScreenCaptureKit 6.0.1 and apple-cf both compiled Swift types into the same
CoreMediaBridge module. Updated to the published 8.0.1 dependency containing
[upstream PR #159](https://github.com/doom-fish/screencapturekit-rs/pull/159),
which gives the ScreenCaptureKit bridge its own Swift module namespace. The
local driver build succeeds with no duplicate Swift symbol messages (20.84 s).
Release linking and GUI capture/recording remain to be verified; neither the
installed permissioned driver nor the staged signed candidate was replaced.
Only 4.7 GiB was free locally, so the larger release build is assigned to CI.

Generated Python UniFFI bindings and release validation's assumption of baked
upstream download installers are still pending. OpenSky's public installers
build this fork from source. These failures must be repaired without restoring
an upstream binary fallback or silently disabling meaningful release checks.

CI at 33283a34c confirmed the macOS release build links successfully (6m 10s),
then failed on generated CLI/MCP reference drift. Regenerated both references
from the built driver and verified the drift check; the five existing generator
tests pass. Supplying CUA_DRIVER_BINARY now avoids an unrelated release rebuild
before extracting that explicitly selected binary's documentation.

Release metadata CI explicitly selects source installation mode. All existing
source/package/documentation version comparisons still run; baked download
version checks remain available in the default release mode for historical
release tooling. Fourteen version-validation and stamping tests pass, including
source package drift rejection. Historical stamping tests use installer-text
inputs containing a baked version, rather than treating OpenSky's current
source wrapper as a release downloader. Remaining upstream installer-wiring
assumptions are still pending; this does not certify an installer release.

UniFFI CI now generates bindings, fails on tracked or untracked generated-file
drift, and retains the candidate files plus the actual checkout commit as an
artifact even when that drift check fails. This preserves the freshness gate
while making repairs reviewable without a large local build. No generated
bindings have been accepted or silently updated by CI; their repair is pending.

CI on e288372f7 passed generated documentation, source release metadata, all
three portable protocol jobs, and pinned-client discovery. Generated bindings
were still stale. Reviewed artifact 10046235837 from run 34201382166, verified
archive SHA-256 b1914915ecb4ed08dd9c25f72bfc003e878506580a2fc6a58f19d5819c5a3bc5,
and confirmed checkout 6fb5f2625505a9c8ec74ab5d9013f32eccd650be has the same
source tree as e288372f7 (80f1188cc865395b9d496efc77101a998bb6e711). Applied only
owned generated files: Python and TypeScript close_window bindings/checksums.
Handwritten node-runtime.ts was not copied. Python syntax passed; regeneration,
TypeScript/native SDK execution and freshness acceptance await the next CI run.

Four installer guidance checks now assert OpenSky's actual identity, home, SDK
link and doctor command; all four pass. The Linux missing-daemon test now expects
the OpenSky name while retaining the failure requirement. A real compiled CLI
probe against an absent private socket returned the required failure and exact
message; its process exited and temporary directory was removed. No GUI apps
opened and no model calls occurred. Remaining installer/channel/telemetry wiring
failures and Windows history-uninstall validation are unresolved; do not remove
meaningful lifecycle or privacy checks simply because a wrapper changed.

## Windows uninstall preserves history unless explicitly purged (2026-09-08)

The source-product uninstaller recursively removed LOCALAPPDATA/opensky-driver,
which contains encrypted Computer History, without invoking native key purge.
Normal uninstall now preserves its computer-history child. Explicit -Purge is
forwarded by the public wrapper and invokes the exact installed OpenSky helper
before executable removal. Missing, unlaunchable or unsuccessful helpers abort
removal; only native purge may destroy the key and remove history. The prompt
and ValidateOnly output state the selected behavior. Windows CI checks parsing,
ordering/refusal requirements and both public validation modes. PowerShell is
not installed on the development Mac: execution and complete uninstall/reinstall
acceptance remain pending, and no local uninstaller was executed.

Seven legacy download/channel/telemetry/UAC checks now explicitly target the
retained upstream Windows downloader, alongside its retained Unix counterpart.
They retain their assertions and names distinguish historical download coverage
from OpenSky's public source installer. Both affected Python modules pass 55
checks (0.16 s); this does not certify source installation or lifecycle behavior.
The public Unix install.sh --help command succeeds without building/installing.

After installing the CI-declared jsonschema/toml dependencies in the isolated
driver test environment, the full script suite passed 248 tests and 21 subtests
in 3.96 s. Workflow YAML parsed. Windows PowerShell execution remains assigned
to CI; these local results do not prove Windows uninstall behavior.

## Python loader uses the real driver (2026-09-08)
CI cc1140127 passed generated binding freshness, Rust SDK/contract tests,
packaging helpers and external C ABI validation. The Python loader ran four
tests in 0.440 s with two errors: fake daemons advertised contract 0.7.0, which
the actual SDK correctly rejected against 0.7.0-opensky.1. Their unfinished
fixture thread/child prevented process exit until the 15-minute job limit
canceled the run; the runner reaped an orphan Python process. It is not an
accepted SDK run, and increasing time alone would not repair those failures.

Replaced the two fabricated socket/process backends with the real built driver.
Fixture-owned EmbeddedCuaDriverHost startup/cleanup now supports actual metadata,
tool discovery, typed session start/state/end and host/client shutdown scenarios.
The existing in-process runtime scenario also registers cleanup before assertions.
No captured internal request assertions or fabricated action results remain in
this loader file. Python exports now include all three close-window contract
types. CI explicitly builds the real executable and allows 30 minutes for the
combined native build/test job; this is independent of uncapped agent runs.
Python syntax and workflow YAML passed. Real loader execution on this candidate
is pending CI, as are TypeScript native SDK tests and Windows uninstall execution.


## No-overlay macOS app discovery (2026-09-08)

A live SDK retry opened TextEdit PID 48904 but the existing no-overlay daemon
reported dead PID 58643. The app was visible and both TCC checks were granted.
NSWorkspace runningApplications and NSRunningApplication properties refresh
through the main run loop; the no-overlay serve branch instead joined its
background server thread. Share the existing accessory NSApplication event loop
with that branch, preserving the PiP path and creating no overlay/Dock icon.
A simpler CFRunLoop-only attempt failed and is not included in the final change.

A temporary signed build of fa11d162c plus this repair passed a launch/quit
public-SDK probe and two permanent Vitest regressions in the OpenSky SDK repo:
`e2e/specs/native-app-discovery.test.ts` (2 passed, 16.29 s). The same daemon was
used across each lifecycle. All owned apps/documents and the private daemon,
socket and temporary bundle were removed. Evidence is in the SDK repo at
`evals/runs/native-app-discovery-e2e-1`. No new TCC grants or agent spending.
This verifies inventory behavior, not Unicode selection, AX, capture or the
complete canonical macOS desktop matrix. The installed driver is unchanged.

CI fa11d162c also passed real Python/TypeScript SDK, binding freshness, portable
contracts and Windows unit checks. Linux/Nix were blocked by a Hermes fixture
using the old skill name; its two paths now use opensky-driver and the real CLI
regression passed locally. Full next-candidate CI and desktop gates remain open.


### DRV-L06: Linux Calc virtual table traversal

The real SDK income-statement setup check fails because the Calc sheet reports
2,147,483,647 children and the native walker requests GetChildren for all of
them. Bounded D-Bus diagnostics confirm a visible range of rows 0–36 and
columns 0–11 through Component and Table queries. The candidate checks child
counts first, queries bounded visible cells for large tables, clips them to the
app frame, and reports partial child lists explicitly. Large non-table
containers use a bounded prefix. Existing per-call/operation limits remain.

The same collector supplies snapshot and action indices. Merged cell object
references are deduplicated within the table. Visible-range discovery refuses
unusable corner coordinates instead of issuing a full child-list request.
This is a candidate, not accepted behavior: Linux compilation, unchanged
SETUP-L02, current keyboard/dialog regressions, scrolled and merged-cell cases
and Wayland window geometry still need validation. No macOS or Windows GUI
behavior has been changed or certified by this Linux-only implementation.

The first virtual-table candidate compiled and passed focused unit checks
(CI 34294757380). SETUP-L02 completed in 35.929 s instead of about 130 s,
with responsive 2,316-node walks and verified cleanup, but still failed: cells
were named by coordinates and string cells displayed numeric Value=0. The
existing renderer discarded Text whenever Name was nonempty. The next candidate
keeps the cell coordinate as Name and its observed Text as the displayed value,
including blank cells, so numeric Value=0 cannot hide a text header. Real saved
cell text and snapshot acceptance still need verification.

The cell-text candidate cc2cf0d74 compiled and passed SETUP-L02 in 34.755 s.
The new CALC-L01 outcome test then found the formula saved in previously active
G17 instead of observed/clicked E2; both cleanups passed. Calc ignored the
background XSendEvent click. The next candidate refuses table-cell background
clicks before any input, allowing the existing SDK typed-refusal retry to send
a real foreground pointer click to the same target. Coordinate cell hits follow
the same rule. Modified index clicks are classified without invoking an
unmodified accessibility action. Acceptance remains pending.

DRV-L06 exact-window geometry follow-up: candidate
978fe0adf32f424599486d4dc7896967e56f4c57 built successfully in hosted run
34307064623 (binary SHA256
a2f8c4150bf707a57bb01204f4c714700e3d5193ac3169899c72cc718f856e30).
Remote calc-candidate-03 passed SETUP-L02 in 35.000 s but CALC-L01 still
failed: the saved workbook contains formula B2-C2 with cached value 75000 in
E1 instead of requested E2. Both owned process groups verified exited; the
remote Docker inventory was empty when artifacts were collected. Thus real
pointer input improves on the prior G17 no-op but has not passed cell targeting.
Snapshot diagnostics apply a SCREEN-to-X11 frame rebase of (0,17), while
get_element_bounds does not share that conversion or an exact-window hint.
This is the next geometry hypothesis to verify, not accepted driver behavior.
Keep the unchanged saved-E2 assertion; share the coordinate conversion before
rerunning the focused tests and existing input regressions. Local evidence:
evals/runs/linux-office-setup/calc-candidate-03 in the OpenSky SDK checkout.

DRV-L06 geometry candidate now shares snapshot bounds conversion with indexed
input, keeps the exact target window, filters frame geometry without renumbering
application-wide indices, and only queries bounds for the requested element.
Foreground delivery translates the freshly resolved local point after focus,
rather than reusing a best-effort overlay position. The saved-E2 regression is
unchanged; Linux build and real Calc validation are pending. Other platform
adapters are unchanged; Wayland behavior still needs its separate native lane.

DRV-L06 verified Linux candidate df79e0cf1 (binary SHA256
3d21cdb9d26f67210c6ba80aef5066bcd895128203c8545294dc928f323037c8):
Calc read and saved E2 pass; newly observed E50 after Page Down saves correctly.
All four existing TYPE/KEY/CLICK/COORD regressions also pass on that exact binary.
Seven app cleanup receipts passed; all four input keymaps restored. An initial
Writer setup failure was traced to the older image lacking a UTF-8 locale;
unchanged tests passed with the locale already used by the full desktop image.
Merged-cell and separate compositor/canonical desktop lanes remain unverified.

### DRV-L07: foreground typing repeats full accessibility scans

The exact df79e0cf1 binary passed TYPE-L01 and COORD-L01 in the remote
`type-profile-01` diagnostic, with app exit and keyboard restoration verified.
Typing 30 characters took 4.309 seconds to refuse background delivery, then
5.913 seconds in foreground; typing 16 characters into a selected font field
took 10.014 seconds to refuse, then 10.155 seconds in foreground. Timestamped
AT-SPI markers show the same 1,964-node walk repeated in the first case and
two 2,365-node walks repeated in the second. These are diagnostic timings with
a transparent CLI wrapper, not agent task completion measurements.

The candidate sends unindexed foreground text directly through the existing
XTest route, preserving the current focused field. Native indexed writes,
background capability checks, terminal handling, and Wayland routes retain
their existing behavior. Linux compilation and the same real SDK saved-file,
Unicode/selection, and keyboard-restoration checks must pass before this is
accepted. No fresh paired-agent speedup or full desktop matrix is claimed.

DRV-L07 candidate c2f9a2a60 compiled and passed focused Rust checks in OpenSky
CI 34314822844. Exact binary f54210824189d942a0118a6285eabf7570082943f5b7bd92e47d776b815fc720
passed the same TYPE-L01 and COORD-L01 tests in `type-profile-after-01`, with
saved/visible outcomes, both app exits and both keyboard maps verified. Matching
image, wrapper, SDK and test-source hashes isolate the changed driver. Foreground
typing fell from 5.913s to 1.609s (30 characters) and 10.155s to 0.197s (selected
16-character font name). Including the unchanged background attempt, the total
typing durations fell from 10.222s to 5.911s and 20.169s to 10.310s respectively.
Combined test runtime was 96.07s before and 81.36s after. One sequential diagnostic
pair is not an agent benchmark. KEY-L01 and CLICK-L01 also passed on this exact
binary in typing-regression-c2f9-01, with both app exits and keymaps verified.
The full native desktop matrix and paired agent rerun remain pending.

### DRV-L08: Calc screenshot clicks miss the name box and new-sheet control

The unchanged public SDK with df79 reproduced both failures on the remote
desktop in `calc-coordinate-before-03`: CALC-L03 saved an empty E2 after a
name-box click and formula entry; CALC-L04 did not expose Sheet2 after clicking
the visible plus button. Both app exits passed. This reproduces the agent arm
22 symptoms without a model. Earlier missing-input and screenshot-size guard
failures are setup-only evidence, not driver failures.

X11 coordinate hit-testing used raw toolkit Window extents rather than the
snapshot's shared frame conversion. An accessibility miss could then fall back
to XSendEvent on GTK even without a real background pointer. The candidate uses
the same exact-window bounds conversion as screenshots/indexed input, translates
the requested pixel to screen coordinates, and retains application-wide indices.
After an accessibility miss, GTK without a real background pointer refuses
before input so callers can use the existing foreground XTest route.

The saved-E2 and Sheet2 assertions are unchanged. Candidate db81cc461 compiled
in build 34316016256, but both Calc reproductions still failed on exact binary
ec5761dd43734a6636fc534849960545787032d5a67bb62958baa94e59b969c2
(`calc-coordinate-after-01`). A subsequent observation diagnostic confirmed
the name-box action chose accessibility delivery; the new-sheet foreground
retry moved the real pointer to the correct translated screen coordinate
(116,861) without producing Sheet2. Both app exits and container removal
passed. The candidate is not accepted. Input regressions and a fresh agent
comparison await a correction that passes these unchanged outcomes.
The change is in X11 coordinate delivery; no new Wayland/macOS/Windows acceptance
or full canonical desktop matrix is claimed.

Further controlled diagnostics on the same image isolated two delivery issues.
`calc-external-paced-01` bypassed SDK clicks with real paced mouse input and
passed both saved outcomes. `calc-external-unpaced-01` used a single XTest
connection with no press/release pause and left focus unchanged: the name-box
workflow passed, but new-sheet creation failed. `calc-external-held-01` added
only a flush and 50ms button hold to that XTest gesture and passed both outcomes.
These are diagnostic controls, not SDK acceptance. All app groups exited.
The read-only accessibility probe found the name box represented by a `panel`
with a `press` action and no EditableText node at the clicked point.

The next candidate requests foreground pixel delivery for such compound panels
instead of actuating their container action. XTest clicks flush the press and
hold it for 50ms before release, retaining modifier/button cleanup on errors.
Normal focus restoration remains unchanged. The saved-E2/Sheet2 public-SDK
tests, prior input regressions and paired agent timing must validate this exact
candidate before acceptance; the controls above do not substitute for them.

Candidate a9501e0ca47b702b9fd44b8e9941164d9cbf2652 passed build/focused Rust
checks in OpenSky CI 34318630951. Its exact binary SHA256
27329643542ab30c5a9058ee140ee3bb69006e8f6af7e3f946a789d1861160d1
then passed eight unchanged real SDK outcomes: both coordinate Calc workflows
(98.06s), four existing input regressions (132.26s), and both existing indexed
and scrolled-cell Calc workflows (124.13s). All eight app exits, all four
keyboard-map restorations and container removal were verified. The evidence is
`calc-coordinate-held-01`, `input-regression-a950-01`, `calc-indexed-a950-01`.
The fresh Terra agent attempt is OpenSky CI 34319911691 at SDK2c5f01a. It has
not yet produced a new score. Real double-click coverage and the full canonical
platform matrix remain pending; the PR remains draft. Subsequent documentation
updates do not change the tested a950 executable.


### Already-focused X11 typing fast path — candidate, not accepted

The unindexed background typing ladder scans the accessibility tree to classify
focus, then may scan it again to find an editable before refusing and allowing
foreground input. A candidate now first checks the exact X11 active toplevel
and core input-focus ancestry on the injection connection. If the target
already owns both, it sends real keys without raising, activating or restoring
any window. Indexed typing, terminals, native Wayland and the pre-input miss
fallback retain their existing paths.

Focus is checked before every character; no focus state is cached across
calls. A miss before the first key permits the original accessibility fallback.
A later miss returns an error describing possibly partial delivery; the SDK
must not retry it. These checks are not an atomic focus lease: another X11
client can change focus between a check and event delivery. The candidate does
not hold the X server while typing, which would freeze other clients during
long input. No no-leak guarantee is claimed under concurrent focus changes.

Acceptance is pending on the exact candidate: saved Calc range/formula outcomes,
foreground Unicode typing and keyboard-map restoration, a sibling document
holding focus, an active modal distinct from the addressed document, and indexed
input to the intended field. Record actual method timing alongside saved outcomes;
format checks or a successful build are not new real-driver acceptance. This
optimization is X11-specific; it makes no new Wayland/macOS/Windows claim.

### Exact-window background typing — candidate, not accepted

The real SDK `FOCUS-L03` regression found that typing through an exact document
handle while its Find dialog was active appended the requested document text to
the dialog's search field. Both the a950 control and the already-focused typing
candidate reproduced `Keep this searchFORBIDDEN DOCUMENT INPUT`. This is an
existing wrong-window mutation, not a harmless success receipt.

The pending correction carries the requested XID through focused-widget
classification, both EditableText attempts and the final AT-SPI fallback. Each
walk correlates the requested native window with the same walk's frame seeds,
validates the selected frame's native geometry even when AT-SPI publishes only
one frame, and restricts candidate widgets to that frame. If the application's
focused widget belongs to another frame or the window identity is ambiguous,
a typed pre-input refusal terminates the ladder; it cannot fall through to the
Qt focus workaround, another editable, or automatic foreground escalation.
The synthetic GTK fallback also retains the caller's XID rather than selecting
the application's first window. Indexed typing is unchanged.

`FOCUS-L03` must pass unchanged on the exact correction, with its saved-document
and modal-content assertions, alongside the foreground/sibling, range/formula,
Unicode, indexed and existing input checks. Formatting or build success alone
is not acceptance. The change affects the X11 unindexed background ladder;
no new macOS/Windows/Wayland acceptance is claimed.


Exact source435ef5940 passed build/focused Rust checks in OpenSky CI34394343940.
BinarySHA2564031b914b757bde656f5ea6d0b4ca4cb612fbaa8d3d91e3e7b3e4671fd796d99
passed ten unchanged real SDK checks: FOCUS-L01/02/03, CALC-L01/02/05,
and TYPE/KEY/CLICK/COORD-L01. The previous modal mutation is refused and both
documents remain unchanged. All13 owned app groups exited, four keyboard maps
were restored, focus temporary directories and all containers were removed.
The saved range contains all nine expected E2:E10 formulas; its workflow took
64.476s and range typing0.245s. Evidence names: focus-modal-435-01,
focus-siblings-435-01, calc-regression-435-01, input-regression-435-01.
The evaluation workflow now pins this binary. Fresh agent performance is pending;
these supporting SDK checks do not replace the canonical desktop matrix.

Two limitations remain explicit. The frame-focus refusal also rejects ordinary
nonmodal sibling windows in the same process; FOCUS-L02 tests independent
processes. The existing Tk fallback uses a global application name and does not
honor caller PID/XID. Neither limitation is resolved by the ten passing checks.
This documentation-only follow-up does not change the tested435 executable.


### Already-focused X11 coordinate clicks — candidate, not accepted

The public SDK agent trace spends most input time on coordinate clicks. The
background ladder scans accessibility before falling back to real pointer input,
even when the requested window already owns focus. The candidate first checks
exact active/core focus, live client geometry, stacking and input shapes on the
injection connection. Only an unobstructed point belonging to that target uses
real XTest directly, with existing modifier cleanup, press hold and multi-click
timing. It never activates/restores a window. A miss falls back only before input;
a later failure reports possible input delivery and must not be replayed.

Input-transparent overlays are skipped when checking the actual topmost input
surface. Another client can still change focus/stacking between validation and
input; this is not an atomic lease. Indexed clicks, native Wayland and the earlier
Chromium background refusal keep their existing paths. Exact candidate compile,
saved Calc/name-box/new-sheet/double-click workflows, dialog alignment and
foreground/sibling/modal regression checks are pending. No speed or broad
isolation acceptance is claimed from the source review.


Candidate4e3028d6f compiled, but its first four real SDK checks all stopped at
`action_outcome_mismatch`: the new raw path name was not registered with the
common execution-record publisher. The correction uses the existing `xtest`
transport label, already used by focused typing; it does not bypass outcome
validation or claim a verified click effect. Failed evidence is retained as
focused-click-after-01. The unchanged four saved-file tests must be rerun on
the corrected exact binary before any performance/acceptance claim.

The exact4fb1e892e executable has now passed17 real SDK cases: four Calc
coordinate/range/double-click cases, eight input/dialog/screenshot cases, three
focus/isolation cases, and two fresh/scrolled indexed-cell cases. Artifacts:
focused-click-after-02, click-dialog-4fb-01, click-focus-4fb-01,
click-indexed-4fb-01. All20 owned app groups exited, eight keyboard maps match,
focus temporary directories were removed, and all four containers are gone.
Retrieved XLSX, DOCX and ODT saved outcomes were independently checked.
BuildCI34400764844 produced SHA256
3550951c28dd413893f39b2fb4e7c5c5cadb1256517800b8937bad8daab420ce.
This is supporting Linux SDK acceptance; the canonical cross-platform matrix
and a fresh agent benchmark remain pending. The SDK evaluation workflow selects this verified build.

### One live indexed-click target per X11 request — candidate

The range replay spends24.640s of a24.835s indexed click in five complete
AT-SPI walks. The candidate resolves the global index once per request, requires
its exact native window/frame correlation, and retains the live accessible
proxies for activation and refreshed geometry. Plain single-left AX activation
remains preferred; other pointer gestures use the shared checked focused-XTest
route when possible. A foreground retry resolves afresh rather than caching
across calls. Wayland and legacy callers retain their routes.

Only typed pre-input unavailability allows pointer fallback. Uncertain AX
errors, timeouts and negative doAction acknowledgments return errors without
replay. Live proxies do not provide an atomic lease against concurrent app
changes. Compilation, indexed/scrolled/range saved outcomes, real AX buttons,
and cross-window rejection need validation before any evaluation pin change.
