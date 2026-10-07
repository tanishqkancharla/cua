# Exact Mac window probes without descendant collection

## Problem

Public get_window_state(probe_only:true) verifies exact WindowServer and AX window ownership, but currently performs a budgeted full descendant AX walk before discarding controls and returning metadata. A retained real Notes control on source0ae94958, driver0.34.0, shows ~1.03s probe plus ~1.06s default capture timeout and ~1.42s bounded capture recovery per public app observation. The probe overhead is separable from capture recovery.

## Scope

Reuse the existing exact ownership, Chromium enablement, application launch wait and top-level AX window matching path. A metadata-only probe stops before descendant AX reads, rendering, snapshot/capture publication or action cache replacement. Normal snapshots, unknown ownership refusals, other-Space resolution and public payload shape remain unchanged. This is a Mac implementation optimization of the existing probe contract; no input or SDK delivery policy change. Dependent on the installed source carried by draft34; no primary-button changes added here.

## Acceptance

Whole platform-macos hermetic tests, existing owner/refusal assertions, actual Notes old/new reduced control preserving probe identity and full capture recovery, then affected existing exact window/focus/tab contracts. Verify tokens from a prior full snapshot remain valid after public probes. Retain binary/signing grants and rollback archive. Paid Notes verification only after relevant acceptance; reuse matching native baseline1185 if prompt/state unchanged. Canonical cross-platform desktop matrix remains required before ready/merge.
