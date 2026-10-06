# OpenSky Driver 0.34.0 reconciliation

Refs https://github.com/tanishqkancharla/opensky/issues/14

Maintainer requested the latest upstream component release while preserving
OpenSky identity and parity fixes. This branch merges `cua-driver-rs-v0.34.0`
(`b0968e1b12834e485dda68789541a3cc57664a9f`) with the fork's contributor history.
Original dirty trees and installed0.23.2 receipts were backed up. No VM used.

The obsolete app-inventory workaround is removed, but no-overlay rich paste
still requires AppKit main-queue servicing. Its actual RICH-N01 conversion
timed out before any clipboard/input mutation. Reuse upstream main-loop entry
in graphical sessions without creating PiP/overlay UI; headless daemon keeps
its prior join behavior. Actual post-fix RICH-N01 verifies exact selected replacement and saved bold style.

## Reconciliation

| Local behavior | Current owner | Current validation |
| --- | --- | --- |
| Process inventory after launch/quit (#10) | Upstream kernel inventory replaces local AppKit run-loop patch | Owned APP-N01/02 passed |
| Native modifier flags (#8) | Upstream f71044a94 replaces local duplicate | KEY-N04 passed actual full Unicode selection and unchanged saved text |
| Cursor and stable index cache | Upstream overlay and SnapshotStore; old implementations removed | Fresh tokens enforced; live ownership pending |
| Attached sheets (#7) | Upstream owning-window proof plus local explicit AXSheet relation | SHEET-N01 passed again at trusted250/50ms bounds |
| Page title/cold Chrome AX (#9) | Upstream document title refresh and AX readiness | TEXT-B01/PASTE-B01 passed; current title passes with issue11 branch |
| Native paste/selection (#6) | Local tools ported to retained snapshot guards | SELECTION-OBS-N01, PASTE-N01 and RICH-N01 passed; saved bold style verified |
| Focused indexed metadata (#4) | Existing PR22 port; selection, formatting, writable/placeholder/resource metadata retained | Hermetic pass; actual focus pending |
| Semantic menu/document outcomes (#12,#13) | Existing PR23/24 code adapted; exact resource readback retained | Hermetic pass; live exact outcome pending |
| Browser SelectAll (#11) | Not proven fixed by upstream native keyboard change | Still fails on upgrade alone; separate PR26 actual selection/replacement passes |
| SDK Chrome launch (#5) | Existing SDK PR3/090e39b restored to campaign SDK; same PID/session poll | 111 focused SDK contracts pass; actual TEXT-B01/PASTE-B01 pass |

The source-managed bundle keeps com.opensky.driver, stable certificate identity,
offline OpenSky handshake, socket/state namespace and disabled upstream updater
and telemetry. Contract0.8.0-opensky.1 is separate from upstream0.8.0. Embedded
skills include WORKFLOW/RUNTIME. Initialization guidance is177words on Mac,
within the unchanged200-word gate.

Upstream closed schemas no longer accept numeric indices/snapshot IDs or
per-call window-observation timing. Offline identity explicitly declares
inputCompatibility=token-only-v1; both campaign CLI/MCP clients preserve the
exact token/PID/window, remove retired aliases, and refuse index-only input
before dispatch. Legacy drivers retain their argument contract. Observation
timing now follows upstream daemon launch settings. No per-call budget claimed.

## Evidence and gates

- cargo check passed. Core904, contract62, Mac511 and Driver302 hermetic unit
  tests passed; six desktop-only Mac tests remain ignored in this gate.
- Campaign SDK111 typed-browser/CUA and36 transport/install/parser tests passed; clean companion SDK PR15 has141 focused tests and a passing build.
- Earlier compile and assertion failures retained as reconciliation evidence.
- Signed installation retained permissions; owned app/browser/TextEdit selection/sheet/format/Calculator/Clock/Safari URL gates passed.
- No new paid-agent score, Linux/Windows acceptance, or canonical desktop matrix
  claimed. Draft PR25 stays draft; canonical platform matrix is still pending.

Local receipts: work/runs/iteration-101/driver-upgrade-034 in the parity workspace.
Existing issue drafts retain their own scope; this upgrade does not close them
from source inspection or unit results.

## Current local acceptance checkpoint

Signed combined upgrade+browser candidate d0b065bf12bf81cab99c7566a43a466277a3c722
is installed as0.34.0. Offline identity, SHA256 and designated signing requirement
are recorded; own daemon retains Accessibility/Screen Recording without regrant.
Its actual argv is serve --no-overlay and trusted environment250ms/50ms.

Current public schema/protocol binaries pass6 tests (one desktop cursor test
ignored), including generated contracts, authorization, portable targets and
health-report surface. Gate caught missing retained parameter descriptions and
noncanonical native-tool session guidance; both now use reviewed descriptions
and the canonical shared session schema. Mac delivery roster includes retained
set_value/native_paste explicitly. No runtime input behavior relaxed.

The clean SDK PR15 passed actual PASTE-B01, TEXT-B01 and new KEY-B01 on this
installed candidate: trusted selection0..11 before typing, exact Unicode
replacement and current owned title. Native PASTE-N01/RICH-N01/SHEET-N01/KEY-N04
passed on d198becb5490389609c65c487de8bb6d1ce06d25; subsequent changes only add
parameter/session schema descriptions and a schema-test roster. ACT-N02 and
KEY-N03 pass on final d0b candidate; loaded URL is observed passively, without
input replay. Calculator/Clock/app inventory and selection/format tests passed
earlier upgrade revisions; those distinct receipts are preserved.

Post-checkpoint Git diffs here are documentation only. Full catalog/native helper
final inventory and every owned cleanup receipt are clear. Focused Reminders,
semantic menu/document-resource gates and fresh paired agent comparisons remain
next steps. No fresh score or cross-platform matrix certification inferred.


## Iteration102 actual Mac checkpoint

Installed runtime remains97aa26e39360628aace9f6a86dd934a33f0aae65,
Driver0.34.0, SHA25679de753294e7b3a7d8dfa243b282b8bec0c5f062e647ccbbb62e8af18c140f3b.
Both Accessibility/Screen Recording grants persist with stable signing.
Canonical Unix/Windows installers currently bake0.34.0; the matching component
release and platform artifacts exist. No newer driver component release observed.

Controlled actual native/OpenSky Find setValue comparison matches: field changes
but match counter/Next stay unavailable; Cmd+A/typeText computes Result1of2.
This shared Chromium behavior receives no OpenSky-specific fix. Previous paid
Find1117/1118 are explicitly reused as1123/1124, preserving their worse1.54179
index; no new performance sample or favorable-score retry.

Existing FOCUS-N02 passes real two-window TextEdit focus isolation. MENU-N05
fails filename setup before any semantic menu/Open assertion; one-second walks
are partial after378–449nodes with ancestor columns. DOC-N01 cold open_target
fails NSWorkspace callback timeout before process registration, also after an
unchanged daemon restart and in a separate temporary-directory control. Cause
is unproven; a Documents-only explanation is unsupported. Those failures do not
establish current menu/resource acceptance. Original failed receipts remain;
independent later cold absence plus exact outer baseline audits recover cleanup
without pretending a launched identity was correlated.

Four fresh serial independent Terra pairs complete with inspected actual input
and state: TextEdit1125/1126 original file identity/bytes unchanged and
MoveTo/Other/Cancel restored; Calculator1127/1128 expression68.5;
Safari1129/1130 three task tabs plus preserved starter, third closed, first
visible; Maps1131/1132 observed place cards and Satellite→Explore. Min-resource
indices0.956705/0.407101/0.536800/0.682260 respectively. Strategy and persisted
starting-state differences prevent causal upgrade claims. SDK TextEdit primary
file-label click refuses editable-focus confirmation then advertised secondary
Open succeeds; matched native primary-click diagnostic remains pending.
Safari network reload count and Maps numeric zoom are not independently exposed.

All owned run/app/helper/artifact cleanup and final catalog app/helper inventory
are clear; all141 active latest task pairs scored, aggregate65.1172/native100,
lower is better. These local results do not certify the full exact-candidate
Mac/Linux/Windows desktop matrix. Related PRs remain drafts.
