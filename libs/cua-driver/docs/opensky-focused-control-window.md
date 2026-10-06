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

Draft stacked on DriverPR27 (including PR25/26); companion SDK PR pending.
Fresh affected serial Terra comparison next; no new causal resource claim.

Fresh serial Terra1115/1116 exposed a second panel issue: indexed clicks now pass, but select_text requests an impossible child AXFocusedWindow transition. The extended FIND-N01 reproduces this exact refusal on unchanged dfbf383 runtime with cleanup clear. Selection now preserves the exact existing first responder only in the frontmost app after the existing ownership gate. Background/non-focused editors still use exact-window activation. Exact focus and foreground are rechecked before the existing UTF-16 range write/read-back. Companion SDK draft18; local acceptance and fresh resource comparison pending.
