## Problem
Mac Safari same-title tabs swap public indices after selection with the OpenSky 0.34 driver candidate39a8cd5. The default diff omits both tab rows; clicking the previously observed second-tab index selects the first tab. SDK paid run1160 replaces its task window and remains incomplete.

## Reproduction and evidence
Use native app CUA to create a new Safari window; load example.org; create another tab and load example.net; observe two same-title tabs; click the first; observe again. Existing real-driver TAB-N01 fails: before[83,84], after[84,83]. WINDOW-N01/N02 pass in the same batch; all owned cleanup passes. A focused native comparison with fresh mapped indices correctly switches org→net and closes only the second tab.

## Cause and intended scope
The old local driver exported optional CFEqual-backed native_object_id from its retained element cache. Upstream0.34 replaced the cache with SnapshotStore; this local metadata behavior was not reconciled. Adapt that established Mac observation identity to snapshot publication under the existing lock. Retain current tokens, PID/window/session checks and stale-token refusals. No input replay or inferred identity from frame/position/title. Other platforms unchanged.

## Acceptance
Focused shared-store/Mac ownership units, existing TAB-N01/WINDOW-N01/N02 and related observation regressions, stable signed rebuild without permission reset, then affected paid Safari verification. Preserve original failure. Full canonical platform matrix required before any driver draft becomes ready.

Driver issues are disabled; the linked SDK issue carries the intake record. This draft adapts existing local contributor work rather than claiming a new native identity design.
