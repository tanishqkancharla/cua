# Foreground pointer source experiment

Intake: https://github.com/tanishqkancharla/opensky/issues/27
Stacked on https://github.com/tanishqkancharla/cua/pull/32. Small unselected focused contribution.

## Evidence and hypothesis
BUTTON-N01 fails on signed da5914813: one exact foreground click on Safari NewTabButton (no advertised AXPress) leaves one tab. Same-window native Codex click creates two. CGPreflightPostEventAccess, Accessibility and Screen Recording are granted. Event posting is acknowledged, not consumption.

Test only the event source for foreground desktop pointer clicks: CombinedSessionState instead of HIDSystemState. Preserve PID/window guards, single delivery, coordinates, timing, cursor restoration, foreground policy and all background transports. No permission changes or retries. This is a hypothesis, not a diagnosed cause.

## Acceptance
Whole affected Mac crate and E2E typecheck; relevant existing real input gates before BUTTON-N01. Keep exact binary/source receipts and cleanup. If the counterexample still fails, revert this runtime experiment and retain the failed evidence. Only a passing reduction warrants original serial Terra verification. Canonical platform matrix required before ready/merge; no VM.
