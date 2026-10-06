Refs https://github.com/tanishqkancharla/opensky/issues/17

## Change

Add Mac list_windows.focused_element_window_id, separately from AXFocusedWindow.
Read the actual focused element, verify its PID, resolve its own AX window, then
require that exact visible/current WindowServer record under the requested PID.
Missing/foreign/hidden identities remain null. No driver input guard changes.

Chrome Find actually owns a child window: raw PID64024 main51917, focused
control/buttons51930, while AXFocusedWindow still names51917. Native1113 Next /
Previous succeeds; pre-fix SDK1114 refuses Next before input and uses shortcuts.
Current cold controlled parent repeats the refusal with domain/Result1of2 and
captures the complete AX ownership chain. A prior fixture setup that set only
AXValue without triggering matches is retained as failed diagnostic setup.

## Validation

Installed runtime dfbf383002e9f2d07a263e1fe0f4919f3869ec5e, Driver0.34, same signing
and TCC identity. Whole Mac crate511 pass +2 integration units;6 desktop ignored.
Existing actual ACT-N01/GROUP-N01/KEY-N04/SHEET-N01, PASTE-B01/TEXT-B01 and
KEY-S01/02/03 pass. Original no-test-file invocation is retained separately.
New FIND-N01 passes in campaign and clean SDK checkout: app observation binds
panel; Next2of2, Previous1of2, Close restores document; exact main-document click
still refuses before input and leaves counter unchanged. Exact app/session/
helper cleanup clear. Canonical platform matrix remains pending.

Companion SDK prefers/probes the control's exact surface for observations and
indexed actions. Named-app ambient keyboard context follows AXFocusedWindow;
opaque CUA/document handles remain exact. Clean SDK focus5/CUA23 contracts,
build and E2E typecheck pass. The local fixture can initially publish0of2;
it now explicitly activates first match once before button assertions. Earlier
attribution of0of2 to panel keyboard routing was not established; both original
failed fixtures remain retained, and no input is blindly replayed.

Draft stacked on DriverPR27 (including PR25/26); companion SDK https://github.com/tanishqkancharla/opensky/pull/18.
Fresh affected serial Terra comparison next; no new causal resource claim.

Fresh serial Terra1115/1116 exposed a second panel issue: indexed clicks now pass, but select_text requests an impossible child AXFocusedWindow transition. The extended FIND-N01 reproduces this exact refusal on unchanged dfbf383 runtime with cleanup clear. Selection now preserves the exact existing first responder only in the frontmost app after the existing ownership gate. Background/non-focused editors still use exact-window activation. Exact focus and foreground are rechecked before the existing UTF-16 range write/read-back. Companion SDK draft18; local acceptance and fresh resource comparison pending.

Selection follow-up runtime97aa26e39360628aace9f6a86dd934a33f0aae65 installedSHA25679de753294e7b3a7d8dfa243b282b8bec0c5f062e647ccbbb62e8af18c140f3b. Same stable designated requirement; both permissions remained true. Mac511+2 units pass; six existing actual selection/format/two-window E2Es pass. Extended FIND-N01 fails on unchanged previous binary and passes on this binary in campaign and clean SDK, including exact token/window UTF-16 selection receipt, actual replacement/clear, and retained main-window ownership refusal. Fresh paid comparison and canonical platform matrix pending. Final diff after installed runtime is this non-executable evidence document only.


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
