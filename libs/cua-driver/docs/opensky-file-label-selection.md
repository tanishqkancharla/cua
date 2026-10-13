## Reproduction / expected behavior
On macOS27.0 with the current signed OpenSky Driver0.34 candidatef6d81a7, open a fresh TextEdit Untitled document, File Open, Go To the controlled RTF, observe its exact file-URL-bearing AXTextField, and click it once. OpenSky returns `AX action failed: editable click could not confirm the exact field has focus`. Native Codex acknowledges the same mapped file-label click. A subsequent single advertised Open succeeds on both interfaces; the original Untitled window is reused with the exact RTF resource. This issue is filename selection routing, distinct from issue13's historical document outcome oracle.

## Cause / selected scope
The label exposes AXSelected, AXOpen and a native file URL while AXValue remains settable for rename. The selection helpers only admit row/cell/list-item/image roles and skip this file label. It reaches editable-focus verification even though it is a collection item. Add only native file-URL/AXOpen-backed text labels to the existing exact selection path; ordinary editable text controls retain focus verification. Keep exact observed token/window/PID and existing pointer gates/read-back. Do not infer resource identity from title or open the file as a click fallback.

## Acceptance / limits
Matched existing native/SDK parent control proves the discrepancy and successful one-shot Open; owned app/helper cleanup passes. Run whole Mac unit crate, relevant existing input/focus/selection/open E2Es, a file-label keeper and the original affected Terra task. Keep the driver contribution draft until canonical platform matrix on exact final source; Mac-only scope and noVM. Driver repo issues are disabled, so this SDK issue tracks intake.

Refs https://github.com/tanishqkancharla/opensky/issues/26
Stacked on native observation identity draft30. Implementation pending; unselected focused contribution.
