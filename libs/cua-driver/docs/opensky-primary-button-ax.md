# Primary AX button action despite incomplete enumeration

Intake: https://github.com/tanishqkancharla/opensky/issues/27. Stacked on driver32. Small focused unselected contribution, no VM.

## Evidence
Corrected exact single-window diagnostic on signed2fa0a1bbf: SDK primary NewTabButton click is acknowledged global input but leaves one tab. Separate trusted helper then performs exactly one AXPress on that same live enabled AXButton, without activation or input replay, returns0 and SDK read-only state confirms two tabs in unchanged window56192/UUID15994B5F-E425-45BF-98B3-1F6152D0B2DB. Its advertised actions still omit AXPress. The initial probe stopped before input because AXFocusedApplication was unavailable; corrected guard uses NSWorkspace frontmost Safari PID plus exact focused AX window. Both owned cleanup pass; this is a controlled diagnostic, not a benchmark arm. Native mapped click on an identical retained window earlier also creates two tabs; native internal transport is not observed.

## Selected change
Use one primary AXPress for a plain single left indexed click on a live enabled AXButton when enumeration omits AXPress. Treat the standard button role and explicit primary click as authority for this primary action; do not guess arbitrary secondary actions. Preserve exact PID/window/token ownership, delivery policy and focus suppression/activation guards. No automatic physical/shortcut fallback after dispatch. Report acknowledgment as unverified and require next UI observation for effect. Other roles, modifiers, double/right/middle gestures and advertised actions keep existing paths.

## Acceptance
Whole Mac tests and SDK E2E typecheck; existing real window/tab/renderer/sheet/focus/file-label cases before existing BUTTON-N01 old-fail/new-pass. Stable certificate/TCC unchanged. Only passing focused checks warrant fresh original serial Terra Safari verification with current source/executable certificate. Canonical desktop platform matrix remains required before ready/merge; no claim about unresolved global hotkey delivery from a button repair.
