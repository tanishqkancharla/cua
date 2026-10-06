# Exact document Open outcome proof

Selected by the OpenSky parity campaign. Refs https://github.com/tanishqkancharla/opensky/issues/13

## Reproduction and evidence

In the current OpenSky Mac parity runtime, open a fresh TextEdit Untitled document, invoke File Open and one advertised Open on an exact owned RTF item. AXOpen can return -25205 while TextEdit successfully opens the requested file by reusing the original Untitled CGWindowID. OpenSky reports that requested resource is unverified, although the immediate following public observation shows the exact RTF document. Native Codex's same advertised Open exposed the requested document without that error.

Two bounded single-action real Mac diagnostics reproduce this. Read-only native AX evidence proves that the original window advertises AXDocument and returns kAXErrorNoValue (-25212) before Open; afterward that same focused visible owner/window returns the exact requested file URL. No input was replayed. Source identity/hash and owned application cleanup passed.

The campaign verifier collapses known NoValue into the same missing entry as unreadable metadata and deliberately refuses every reused existing window with that entry. This is an information-loss bug in the transition oracle. Raising its timeout does not address that guard. Upstream main does not yet contain the campaign resource verifier; a contribution should include complete bounded exact-resource verification and preserve no-replay behavior.

## Selected scope / acceptance

Implement a Mac AXDocument observation that distinguishes explicit NoValue from unknown/unsupported/malformed data. After one advertised semantic Open, accept only a new or proven-changed exact focused visible same-process window with the requested native resource identity. A known-absent value can transition to that resource; unreadable prior data, a preexisting matching file, wrong resource/owner/window or sibling cannot establish success. No title-derived paths or pointer/input replay.

Run relevant existing real Open/document/selection GUI gates, then unchanged affected CLI verification. Add focused regression for reused NoValue and refusal cases if practical. Canonical candidate matrix and Linux/Windows/release certification remain separate; no VM is authorized in the current campaign.

Implementation pending. No candidate GUI or resource improvement claim.
