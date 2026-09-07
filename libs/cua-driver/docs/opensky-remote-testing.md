# Remote OpenSky Driver testing

Use the fork's `.github/workflows/opensky-driver.yml` on branch `opensky-driver`.
Every job runs on a disposable GitHub-hosted runner. No job uses the developer's
Mac, installs a local daemon, changes local permissions, or drives local apps.

## Build and unit checks

Relevant pushes build the current commit on Ubuntu 24.04, Windows 2025, and
macOS 26, run the driver binary and shared contract/core unit suites, then verify
OpenSky's offline version and identity. Each `opensky-build-*` artifact contains
logs, a development executable, and source/OS/architecture provenance. These are
unsigned debug builds, not installers or production releases. macOS build success
is not Accessibility, Screen Recording, Safari, clipboard, or native GUI evidence.

## Desktop checks

Desktop tests are opt-in. Dispatch `OpenSky Driver` on the fork branch with
`desktop=native`, `shared`, `capture`, or `all`:

```sh
gh workflow run opensky-driver.yml --repo tanishqkancharla/cua \
  --ref opensky-driver -f desktop=native
gh run list --repo tanishqkancharla/cua --workflow opensky-driver.yml
gh run view RUN_ID --repo tanishqkancharla/cua --log-failed
gh run download RUN_ID --repo tanishqkancharla/cua --dir work/driver-ci-RUN_ID
```

GitHub may require a workflow to exist on the default branch before accepting
manual dispatch. Until then, include `[desktop-e2e:native]` (or `shared`, `capture`,
`all`) in a relevant commit message and push `opensky-driver`. The push and all its
jobs use that exact commit. Routine pushes omit this marker and only build/test.

The jobs call the existing canonical Linux X11 and Windows harnesses. Linux uses
Xvfb, Openbox, D-Bus accessibility, and real fixture applications; Windows must
pass the interactive-desktop preflight with `-RequireGui`. The harnesses forbid
silent skips and retain assertions, preflight results, and trajectory recordings
in `opensky-desktop-*` artifacts for 14 days. Failed jobs remain failures; a
successful build never substitutes for a failed or unavailable desktop lane.

`native` is a diagnostic subset. Stable candidate certification uses `all`
(shared, native, and capture) plus the applicable standalone-browser and platform
lanes described in `test-harnesses-guide.md`. The SDK's consumer E2E drafts live
in the OpenSky repository and are separate from these Rust harnesses. There is
no Codex in-app-browser target.

## macOS GUI work

The existing macOS acceptance runner is
`libs/cua-driver/tests/runners/macos-lume/run-all.sh`. It requires a separate
Apple Silicon Mac running a disposable Lume guest with a logged-in Aqua session,
stable signing, and Accessibility/Screen Recording grants. Its upstream bundle
and install-path assumptions also need adapting to OpenSky before use.

No such remote Mac is configured here. GitHub's macOS build lane does not provision
that GUI environment. Hosted macOS arm64 runners do not support nested
virtualization, so running Lume inside that lane is not a supported solution.
exe.dev's Linux VMs can host persistent Linux testing, but cannot validate macOS
APIs or TCC behavior. Remote Mac GUI acceptance remains an explicit infrastructure
gap, including the macOS-specific items in `opensky-followups.md`.

References: [GitHub runner limitations](https://docs.github.com/en/actions/reference/runners/github-hosted-runners),
[exe.dev documentation](https://exe.dev/docs/all), and the repository's
[test harness guide](test-harnesses-guide.md).

## Initial hosted evidence — 2026-09-07

The initial diagnostic run is
[34149118620](https://github.com/tanishqkancharla/cua/actions/runs/34149118620),
source `407c83ae8276e5e7df134858e9ad3b9a1506a856`. Linux native E2E passed:
32 delivered actions, seven expected refusals, zero failures, and zero skips.
The environment preflight and video validation passed. The
[Linux native artifact](https://github.com/tanishqkancharla/cua/actions/runs/34149118620/artifacts/10028976101)
contains the generated report, structured results, and recordings. Expected
refusals verify the declared limitations; they do not prove those operations
are supported.

The first build run successfully compiled all three platforms and passed their
driver binary unit suites, then exposed a stale upstream contract-version
assertion. The next run exposed four Windows-only core failures: two Unix-path
fixtures and two startup-instruction word-budget checks. The fixes preserve the
fork contract assertion, use platform-appropriate synthetic paths in both
manifests and attestations, and shorten the Windows documentation pointer.
CI now retains build artifacts even if tests fail and checks the executable's
embedded source SHA. It does not suppress these tests or convert failures into
allowed failures.

The final build/test candidate is `637723da86b3ea42aadf9e12047258a4499d361c`:
[34150504250](https://github.com/tanishqkancharla/cua/actions/runs/34150504250).
The diagnostic GUI evidence above is from the initial SHA, not this later SHA.
Changes afterward are CI/evidence handling, a contract test assertion,
platform-aware test fixtures (including Windows canonical verbatim paths), and
a shorter Windows instruction pointer. No
native input implementation changed, and no full parity certification is claimed.

Windows native E2E also passed at the initial source SHA: 39 delivered actions,
four expected refusals, zero failures, and zero skips. The interactive-session,
UIA/capture preflight and video checks passed. WPF, WinUI3, WebView2, Electron,
launch, and cursor evidence is in the
[Windows native artifact](https://github.com/tanishqkancharla/cua/actions/runs/34149118620/artifacts/10029180653).
The initial workflow run is red because of its stale unit assertion; both
independent desktop jobs are green. Do not summarize the entire initial run as
passing.

Final build/unit result: **all three jobs passed**, with zero test failures or
ignored tests in the selected suites:

| Hosted platform | Driver binary unit | Contract unit | Core unit | Offline source identity |
| --- | ---: | ---: | ---: | --- |
| Ubuntu 24.04 | 188 | 33 | 609 | Exact candidate verified |
| Windows 2025 | 180 | 33 | 604 | Exact candidate verified |
| macOS 26 | 192 | 33 | 608 | Exact candidate verified |

The final run retains one development binary and logs per platform. Desktop
jobs in that build-only run are intentionally unselected; use the two independent
native jobs in the initial diagnostic run for GUI evidence. This record is a
documentation-only addition after the tested commit; it does not recertify other
source revisions or the deferred SDK/driver capability work.

## SDK consumer regression found on Linux (2026-09-07)

The first public SDK run [34159847059](https://github.com/tanishqkancharla/opensky/actions/runs/34159847059)
used driver `637723da86b3ea42aadf9e12047258a4499d361c` and SDK `a4330d5`.
STALE-B01 failed: a detached button retained by CDP still received a DOM click,
and the public final AX state showed `Discarded: 1`. Node resolution proved an
object still existed, not that a consumer could still interact with it. The
shared DOM click path now checks `isConnected` and dispatches in one JavaScript
turn, reports detached nodes as stale without input, and distinguishes a receipt
from unknown delivery. The existing SDK scenario is the real regression test;
the exact-build rerun is recorded below. It applies to the shared Chromium path;
Linux SDK evidence does not certify macOS/Windows GUI behavior.

SCROLL-B01 also failed before dispatch. Linux explicitly refuses standalone
trusted CDP input because it can activate the browser window, violating its
background-delivery contract. This is not fixed by switching to synthetic wheel
events or silently activating a window. A foreground-authorized route or a
proven background input implementation remains required. This first lane keeps
the positive scroll expectation failing until that capability is implemented.

The corrected driver SHA `59bdc18e03a276fa98c556f6110ba7791e97d101` passed the
[three-platform build/unit matrix](https://github.com/tanishqkancharla/cua/actions/runs/34160289413).
[SDK run 34160341420](https://github.com/tanishqkancharla/opensky/actions/runs/34160341420),
at SDK `a6f9fa042a0f03c5bbd9abee8672e77e2d027431`, passed STALE-B01 on hosted
Linux. Fresh public AX showed `Discarded: 0; replacement: 0; sibling: 0; retire: 1`;
exact owned-tab cleanup completed and a 5.4-second recording was retained.
SCROLL-B01 still failed with `route_unavailable`, leaving the SDK workflow red.
The other platforms' build/unit checks are not GUI acceptance. Later docs-only
commits do not modify this tested implementation.

## Context integration and browser paste (2026-09-07)

The `tanishq/query-context` history had not been integrated into OpenSky Driver.
Merge `d3100981785e8407a7589fbd1aff989d0e72594d` preserves those eleven commits
and the detached-node fix. [SDK run 34162512013](https://github.com/tanishqkancharla/opensky/actions/runs/34162512013)
passed all three list/article/table context cases, plus stale-reference isolation,
Unicode fidelity and delayed-result freshness. The same SHA passed the
[driver build/unit matrix](https://github.com/tanishqkancharla/cua/actions/runs/34162474847).

Browser paste is now implemented as `browser_type` with `mode=paste`, supporting
text, literal Markdown and HTML. It shares the exact tab/ref validation and focus
emulation route, writes native clipboard formats, and invokes Chromium's real
[editing command](https://chromedevtools.github.io/devtools-protocol/tot/Input/#method-dispatchKeyEvent).
Driver clipboard writers serialize with paste. Connected/editor-focus checks and
a readback detect changes before dispatch; they do not make external OS clipboard
changes atomic with Chromium input. Browser clipboard contents are intentionally
left in place; no restoration can overwrite a later copy. Bounded authorization
also requires clipboard access. Native paste still needs its separate restoration
contract. Status: implementation candidate, real SDK paste validation pending.

## Verified browser paste and follow-on input diagnostics (2026-09-07)

SDK run [34163189757](https://github.com/tanishqkancharla/opensky/actions/runs/34163189757)
at SDK `8f955ca` and driver `a92fac8a44d3574570b69f70dd8cbe432256acbe`
passed all three real paste cases and the six previous browser successes (9/10).
The native clipboard paste transaction remains unimplemented.
Driver build/unit run [34163145256](https://github.com/tanishqkancharla/cua/actions/runs/34163145256)
passed on Linux, Windows and macOS.

A macOS SDK probe found the CLI treating `bring_to_front` tool errors as success:
it flattened `structuredContent` and discarded `isError` and the text diagnostic.
The CLI now preserves the full error envelope and exits nonzero for tool errors.
This does not replay or reverse an action that may have partially completed.

The next scroll candidate uses `Input.synthesizeScrollGesture` with mouse input,
instead of the focus-activating `Input.dispatchMouseEvent` wheel route. Chromium
queues this gesture on its root widget, so subframe refs without a root transform
are refused; screenshot viewport coordinates and main-frame refs are supported.
See Chromium's [input handler](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/content/browser/devtools/protocol/input_handler.cc)
and [wheel gesture implementation](https://chromium.googlesource.com/chromium/src/+/HEAD/content/common/input/synthetic_smooth_move_gesture.cc).
The real SDK test now observes both visible canvas movement and an independent
foreground application's X11 focus history. The Linux SDK validation below passed; build success
alone does not establish desktop focus isolation or macOS GUI parity.

## Completed browser regression and macOS window candidate

[SDK run 34164047110](https://github.com/tanishqkancharla/opensky/actions/runs/34164047110)
passed all ten browser cases at SDK `65819a3` / driver
`f089f489a021c19ef84f5c75530ca589c82bc5ac`, including the X11 foreground-history
assertion during trusted wheel scrolling. The macOS CLI from that driver was
also exercised through the real SDK against the existing daemon and retained the
original partial-activation diagnostic, code and payload.

A local read-only AX diagnostic found TextEdit returning an empty AXWindows list
while AXFocusedWindow and AXMainWindow both mapped to the exact owned window
and exposed its children. `38c05be79d0bfc7ae03d57ccb82ca6319998c736` incorporates
those fresh, same-process window candidates, deduplicating them before existing
CGWindowID/WindowServer and element-ancestry checks. Candidates are not claimed
to be a complete inventory. Its [build/unit matrix](https://github.com/tanishqkancharla/cua/actions/runs/34164488643)
passed on all three operating systems. An earlier candidate failed compilation
because the AX PID FFI and CFEqual import were missing; those are corrected.

The development macOS app was updated with that exact artifact. Its ad-hoc
signature changed, so macOS requested permission again. Native SDK selection
and cooperative cleanup remain pending that reauthorization; neither the native
diagnostic nor the build matrix counts as native GUI acceptance. Stable local
code signing is a follow-up to avoid repeated permission grants across rebuilds.

### Restored native document identity candidate (2026-09-07)

After granting the previous development binary both permissions, the real SDK
open failed because TextEdit restored two same-named documents beside the
requested file. A read-only AX diagnostic confirmed distinct AXDocument URLs.
`list_windows` now accepts `include_document_urls: true` with an explicit positive
PID and reports each exact AXWindow's document URL, joined by PID and CGWindowID.
The field is opt-in and null when unavailable; ordinary inventory does not start
AX reads. The SDK uses canonical paths and rejects ambiguous document matches.
This metadata is macOS-only; it does not change Windows/Linux window contracts.

The local macOS driver build and focused window-record tests pass. Public SDK
OPEN-N01 was added to verify same-named document identity and exact close with a
surviving sibling. Its live run passed after the grants were refreshed: SDK `5f42f0c`, driver
runtime code `aa31c70ee`, macOS 15.7.9, 17.66 seconds. Exact close preserved a
readable same-named sibling and fixture teardown completed. The local runtime
was built before formatting/test/doc-only changes; executable behavior matches
the committed candidate. All three OS build/unit jobs passed in
[34167061105](https://github.com/tanishqkancharla/cua/actions/runs/34167061105).
All ten hosted browser SDK workflows passed in
[34167084511](https://github.com/tanishqkancharla/opensky/actions/runs/34167084511).
No native selection or clipboard acceptance is inferred.

The repeated TCC grants were caused by ad-hoc development signing: the observed
designated requirement was a binary-specific cdhash. A consistent certificate
and designated requirement should be established before routine local updates.
No certificate/Keychain entry was created; this distribution issue remains open.
