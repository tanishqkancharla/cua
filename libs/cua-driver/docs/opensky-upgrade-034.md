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
its prior join behavior. Actual post-fix saved-rich-file gate pending.

## Reconciliation

| Local behavior | Current owner | Current validation |
| --- | --- | --- |
| Process inventory after launch/quit (#10) | Upstream kernel inventory replaces local AppKit run-loop patch | Owned APP-N01/02 passed |
| Native modifier flags (#8) | Upstream f71044a94 replaces local duplicate | Live native Cmd+A pending |
| Cursor and stable index cache | Upstream overlay and SnapshotStore; old implementations removed | Fresh tokens enforced; live ownership pending |
| Attached sheets (#7) | Upstream owning-window proof plus local explicit AXSheet relation | SHEET-N01 passed; current250ms recheck pending |
| Page title/cold Chrome AX (#9) | Upstream document title refresh and AX readiness | TEXT-B01/PASTE-B01 passed; current title passes with issue11 branch |
| Native paste/selection (#6) | Local tools ported to retained snapshot guards | Actual Unicode selection passed; rich converter run-loop regression found and repaired, recheck pending |
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
- SDK111 typed-browser/CUA and36 transport/install/parser tests passed; builds passed.
- Earlier compile and assertion failures retained as reconciliation evidence.
- Signed installation retained permissions; owned app/browser/TextEdit selection/sheet/format/Calculator/Clock/Safari URL gates passed.
- No new paid-agent score, Linux/Windows acceptance, or canonical desktop matrix
  claimed. Draft PR25 stays draft until exact-candidate acceptance is available.

Local receipts: work/runs/iteration-101/driver-upgrade-034 in the parity workspace.
Existing issue drafts retain their own scope; this upgrade does not close them
from source inspection or unit results.
