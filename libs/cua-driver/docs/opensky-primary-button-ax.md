# Primary AX button action despite incomplete enumeration

Intake: https://github.com/tanishqkancharla/opensky/issues/27. Stacked on driver32. Small focused unselected contribution, no VM.

## Evidence
Corrected exact single-window diagnostic on signed2fa0a1bbf: SDK primary NewTabButton click is acknowledged global input but leaves one tab. Separate trusted helper then performs exactly one AXPress on that same live enabled AXButton, without activation or input replay, returns0 and SDK read-only state confirms two tabs in unchanged window56192/UUID15994B5F-E425-45BF-98B3-1F6152D0B2DB. Its advertised actions still omit AXPress. The initial probe stopped before input because AXFocusedApplication was unavailable; corrected guard uses NSWorkspace frontmost Safari PID plus exact focused AX window. Both owned cleanup pass; this is a controlled diagnostic, not a benchmark arm. Native mapped click on an identical retained window earlier also creates two tabs; native internal transport is not observed.

## Selected change
Use one primary AXPress for a plain single left indexed click on a live enabled AXButton when enumeration omits AXPress. Treat the standard button role and explicit primary click as authority for this primary action; do not guess arbitrary secondary actions. Preserve exact PID/window/token ownership, delivery policy and focus suppression/activation guards. No automatic physical/shortcut fallback after dispatch. Report acknowledgment as unverified and require next UI observation for effect. Other roles, modifiers, double/right/middle gestures and advertised actions keep existing paths.

## Acceptance
Whole Mac tests and SDK E2E typecheck; existing real window/tab/renderer/sheet/focus/file-label cases before existing BUTTON-N01 old-fail/new-pass. Stable certificate/TCC unchanged. Only passing focused checks warrant fresh original serial Terra Safari verification with current source/executable certificate. Canonical desktop platform matrix remains required before ready/merge; no claim about unresolved global hotkey delivery from a button repair.


## Revision: primary pointer delivery for an unadvertised action

The earlier immediate two-tab observation is insufficient. On installed190060e03, a history/two-tab workflow later Reloads once but reports AXPress uncertainty and opens a visible unrelated toolbar menu. Direct AXPress on Reload reproduces this outside the click wrapper; a one-shot foreground screenshot click on Reload after the same preparation also opens the menu without reloading. Omitted foreground mode is not sufficient; generic SDK foreground candidate28 was withdrawn.

Two one-factor controls change only preparatory New Tab delivery: Cmd+T, or a single public SDK screenshot click on NewTabButton. Both open two tabs and make the original subsequent SDK Reload apply once, return normally and expose no menu. HID/combined modifier/button read-backs are clear before and after those controls. This supports changing the unadvertised New Tab primary route; it does not establish Safari's internal event/tracking mechanism.

Replace the speculative unadvertised AXPress branch with one foreground HID pointer click selected before dispatch for the existing generic plain indexed AXButton predicate. Use the retained exact PID/window/token, live role/enabled/ancestry, fresh element center and current window bounds inside the existing exact-focus HID guard. Refuse missing/nonfinite/out-of-window geometry before input. Explicit background delivery of such an unadvertised primary button refuses with background_unavailable and recommends foreground; it must not guess AXPress. Current SDK already requests foreground for that case. Advertised AXPress, secondary operations, other roles/modifiers/right/middle/double clicks retain their paths. No AX attempt, shortcut or pointer replay on this selected route, no signing/permission changes.

Acceptance: reuse unchanged BUTTON-N01 and extended existing AX-N01 (one actual request/displayed count plus popup absence); whole Mac crate units and relevant existing window/tab/focus/renderer/file-label/Calculator gates. Then fresh affected independent serial Terra native/SDK comparison under a new source/executable certificate. Keep this same draft and contributor history; canonical platform matrix and inactive/background limitation coverage remain required before ready/merge. No repair or resource claim before these checks.

Fixture repair encountered during validation: retryability and rich-paste tests share the unavailable-initialization counter, so concurrent rich-paste calls can make the retry assertion observe3 rather than2. Give the rich-paste failure test its own noncapturing error initializer at the existing test seam; no production clipboard behavior, test serialization or host clipboard access changes. Rerun the whole Mac crate.


## Current 0.34.0 regression: guarded PID-addressed primary click

Issue: https://github.com/tanishqkancharla/opensky/issues/27. This small unselected follow-up preserves the contribution in driver PR34 and is stacked on PR41 to keep the installed Mac fixes. It does not restore the earlier unadvertised AXPress approach, whose later Reload collateral failure remains authoritative.

On installed013c5f68/binaryf6aa7f23, native mapped NewTab opens two tabs in a fresh window and after actual com/org/back/forward navigation. Current SDK primary click chooses global HID and leaves one tab. Existing BUTTON-N01 also fails its two-tab assertion after the single guarded foreground click; background refusal passes. Optional overlay disabling leaves the failure unchanged, and the original configuration was restored. A separate diagnostic AXPress opens a second tab but is not a repair due to the retained later-action counterexample. The SDK history reducer stopped before actual navigation; that incomplete subcase remains recorded.

Candidate: for the existing plain single unmodified primary AXButton without advertised AXPress, retain exact PID/window/token, live role/enabled/ancestry, fresh finite in-window geometry, foreground focus guard and background refusal. Select the existing public PID-addressed window-local pointer primitive once, with foreground delivery; report its cgevent transport honestly and leave effect unverified until observation. No unadvertised AX action or fallback/replay. Other indexed actions, desktop pixel input, hotkeys and platform adapters are unchanged. This uses the existing foreground PID pointer semantics, including its mouseMoved primer; it does not assert global HID consumption or resolve hotkey no-ops.

Acceptance pending: whole Mac crate, existing BUTTON-N01 and extended AX-N01, affected window/tab/focus/sheet/file/Calculator/Finder owners, then fresh independent serial Terra Safari verification. Keep draft until canonical desktop matrix and review. No score or runtime improvement claimed from the candidate source alone.
