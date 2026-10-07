## Reproduction / observed discrepancy
macOS27.0, OpenSky Driver0.34 signed candidate ea579178149dd0e696b6b8e723a04a742d3b1f42, source hash bfeb7f440407ee452e7135df8ed8f686876613c0a849e74568abee95312ecc01; Accessibility and Screen Recording report granted. Own a fresh Safari process/window, hold one public app handle, Cmd+N, observe NewTabButton, click once, then observe. Two independent SDK reductions leave one tab; later Cmd+N/Return also have no visible effects. Safari remains frontmost, onConsole/loginDone true, ready true, no screensaver.

A same-window comparison narrows the issue: SDK NewTab click leaves the exact native window UUID71FDF1B1-1809-420B-AB3B-7F46ECEDBCC2 unchanged; one mapped native Codex click78 on that identical window creates two tabs. SDK's read-only observation confirms the two native-created tabs. One subsequent SDK Cmd+T does not create a third. No input replay/fallback was added. Every owned app/helper cleanup passes. Native also opens a tab after the full example.com/org back/forward sequence. The SDK historical-sequence reduction cannot reach navigation after its no-op, so that subcase is incomplete.

## Current inference and selected next scope
The matched evidence proves SDK-only no-effect behavior, but not its cause. Global input posting, event-source state, activation or filtering remain hypotheses; neither TCC grants nor a successful void CGEventPost establish consumption. Add an additive read-only health_report check for the daemon's actual CGPreflightPostEventAccess value with explicit macOS applicability/attribution. Do not request permission, mutate TCC, change signing identity, replay inputs, or blindly change event delivery. Probe the installed signed candidate, then reduce/repair the demonstrated layer.

Apple API: https://developer.apple.com/documentation/coregraphics/cgpreflightposteventaccess%28%29?language=objc
Source-state guidance: https://developer.apple.com/documentation/coregraphics/cgeventsourcestateid

## Acceptance / limits
Whole affected core/Mac crates, focused truthful denied/granted readiness coverage, actual signed daemon readout, then relevant live input gates before original serial Terra verification. Full canonical platform matrix remains separate and required before ready/merge. Related historical input issues8/11 have different focused reproductions; this work initially adds the missing diagnostic and does not claim their fixes. Driver issues are disabled; this SDK issue tracks intake. Small unselected focused draft, no VM.

Intake: https://github.com/tanishqkancharla/opensky/issues/27
