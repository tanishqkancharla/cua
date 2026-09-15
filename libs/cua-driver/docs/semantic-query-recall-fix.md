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

Implemented in the shared semantic selector with no protocol changes. Seven
focused tests pass; five reproduced the defect before the implementation. The
full core suite passes (608 unit tests, two contract tests, three lifecycle
tests; one existing doc test is ignored), as does the workspace formatting
check. These are deterministic regression results, not real-driver evidence.

The draft is stacked on the existing OpenSky driver baseline
`8f5353b30a7be22094d1f3ab20b452078a560129` so prior driver work does not appear
as part of this fix. The personal fork is the workstream; this document does
not claim upstream selection, acceptance or release. Real-browser verification
and cross-platform GUI certification are still pending.
