# Semantic query recall correction

This is a narrow matching bug fix, not the proposed collection-context API.

## Problem

The current semantic query selection switches to phrase-only filtering when
any in-scope node contains the contiguous query. Consequently, a result with
all the same terms in a different order is excluded. An exact match that is
subsequently removed as CSS-hidden or page-occluded can even suppress the
visible fallback results.

## Scope

- With an eligible exact phrase, retain both phrase and all-term matches.
- Only eligible, in-scope nodes may trigger the phrase-preference branch.
- With no eligible exact phrase, retain the existing ranked partial-term
  fallback, including its existing substring and repeated-term semantics.
- Preserve request/response schemas, query ranking, visibility labels,
  snapshot/ref identity, continuation and the response node budget.

An eligible node here is neither CSS-hidden nor page-occluded. Offscreen,
unknown and no-layout nodes remain eligible under the existing contract.
The ranking is still not page order, and matching nodes still do not provide
complete sibling or collection context. This fix makes no such claim.

## Evidence required

Focused semantic tests cover reordered terms across unrelated label families,
hidden/occluded phrase matches, partial fallback, subtree scope, normalization,
ranking and bounded continuation. The existing core suite and formatting check
must remain green. Contract fixtures are regression evidence, not real-driver
acceptance evidence. A source-pinned real-browser check is required before
claiming a deployed OpenSky improvement; missing platform/GUI coverage must be
reported explicitly. No app-specific rules or permission changes are included.

## Status

Implementation and validation pending. The personal fork is the workstream;
this document does not claim upstream selection, acceptance or release.
