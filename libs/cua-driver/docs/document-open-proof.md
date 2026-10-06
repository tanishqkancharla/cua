# Exact document Open outcome proof

Selected by the OpenSky parity campaign. Refs https://github.com/tanishqkancharla/opensky/issues/13

## Reproduction and evidence

In the current OpenSky Mac parity runtime, open a fresh TextEdit Untitled document, invoke File Open and one advertised Open on an exact owned RTF item. AXOpen can return -25205 while TextEdit successfully opens the requested file by reusing the original Untitled CGWindowID. OpenSky reports that requested resource is unverified, although the immediate following public observation shows the exact RTF document. Native Codex's same advertised Open exposed the requested document without that error.

Two bounded single-action real Mac diagnostics reproduce this. Read-only native AX evidence proves that the original window advertises AXDocument and returns kAXErrorNoValue (-25212) before Open; afterward that same focused visible owner/window returns the exact requested file URL. No input was replayed. Source identity/hash and owned application cleanup passed.

The campaign verifier collapses known NoValue into the same missing entry as unreadable metadata and deliberately refuses every reused existing window with that entry. This is an information-loss bug in the transition oracle. Raising its timeout does not address that guard. Upstream main does not yet contain the campaign resource verifier; a contribution should include complete bounded exact-resource verification and preserve no-replay behavior.

## Selected scope / acceptance

Implement a Mac AXDocument observation that distinguishes explicit NoValue from unknown/unsupported/malformed data. After one advertised semantic Open, accept only a new or proven-changed exact focused visible same-process window with the requested native resource identity. A known-absent value can transition to that resource; unreadable prior data, a preexisting matching file, wrong resource/owner/window or sibling cannot establish success. No title-derived paths or pointer/input replay.

Run relevant existing real Open/document/selection GUI gates, then unchanged affected CLI verification. Add focused regression for reused NoValue and refusal cases if practical. Canonical candidate matrix and Linux/Windows/release certification remain separate; no VM is authorized in the current campaign.

Implementation now distinguishes explicit AXNoValue from unreadable metadata and preserves unknown existing windows in the baseline. Only a new/changed exact focused visible resource identity acknowledges an AXOpen error; the semantic action is sent once.

Validation: isolated cargo check passes; five focused unit checks pass (NoValue classification, new/changed/unknown/preexisting resource outcomes, wrong owner/focus/visibility/space and file-reference identity). Integrated local runtime SHA451ad0848491864384fa49c0dde1d5a827e8b59c69178cfc62c71130733ac5d2 passes existing real OPEN-N02 Preview and OPEN-F01 Finder GUI cases, with exact cleanup. The unchanged one-Open reproduction now returns no error; native AX independently proves the same window transitioned from explicit NoValue to the exact RTF URL, with unchanged source hash and exact cleanup.

The integrated campaign runtime contains prior modifications; this is supporting Mac evidence, not standalone branch/full canonical desktop certification. Fresh unchanged affected gpt-5.6-terra verification is complete: OpenSky1083 has14 actual calls with no driver errors (15 cells including one preinput handle refusal), immediate exact RTF after filename Open, actual Other picker and final Cancel, title selection/caret105, unchanged original file identity/hash and exact app cleanup. OpenSky102.286s/376365 aggregate input/$0.337026 estimated versus retained unchanged native1081 224.670s/649663/$0.5004568. Previous OpenSky1082 took282.865s/823083/$0.6192564 with opening errors and extra recovery. The unchanged single-action diagnostic verifies the functional fix separately; the whole-task resource differences include agent batching/recovery/cache variation.

A durable new GUI regression and standalone canonical matrix remain pending. No Linux/Windows/release acceptance claim.
