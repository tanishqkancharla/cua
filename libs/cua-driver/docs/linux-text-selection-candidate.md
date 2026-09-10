# Linux exact text selection candidate

Unpromoted implementation based on `fbf0d0c632763f0daae0e326c1d2fe3a44a89f2a`.
This changes no frozen evaluation evidence or accepted release.

`select_text` selects a unique exact match in one observed element through its
live AT-SPI Text interface. It does not replace document text, activate a
window, click, synthesize keyboard navigation, or replay a failed mutation.

## Request and result

Required `pid` and nonempty `text`; target by `element_token`, or by
`element_index` plus `snapshot_id`. An optional `window_id` must agree with the
snapshot. Optional adjacent `prefix` and `suffix` disambiguate occurrences.
`selection_type` is `text` (default), `cursor_before`, or `cursor_after`.
The optional `session` follows the existing runtime convention.

Success has `status: completed`, `verified: true`, `path: atspi_text`, and a
`range` with `start`, `end`, and `unit` (`unicode_scalar` or `utf16`). Offsets
are local to the live element in the verified toolkit unit, never UTF-8 bytes.

A pre-mutation failure is a refusal. Existing token validation errors retain
their typed refusal codes. A failure after the single native mutation is
`status: partial`, `verified: false`, and `retry_safe: false`; inspect the actual
selection before continuing. A timed-out operation is never replayed.

## Boundaries

- Linux X11 only. Native Wayland is refused because exact window/focus checks
  require X11. macOS and Windows do not register this candidate tool.
- Existing snapshot token membership, process/window ownership, app-wide index
  order and exact frame correlation are retained. A new snapshot invalidates
  its preceding tokens through the existing registry.
- The operation rechecks same-process native window topology, other modal
  windows, text, character count, selection count and desktop focus before
  mutation. It retains the selected proxy instead of resolving another target.
- These checks are not an atomic transaction with an external application.
  A UI change can race the last check; the token format also identifies a
  snapshot ordinal rather than a persistent AT-SPI object identity.
- The reported Text character count is bounded to 1,000,000. Its exact match
  against the live scalar count or UTF-16 count selects the candidate unit;
  BMP-only text is equivalent and reported as unicode_scalar. Any other count
  is refused with reported/scalar/UTF-16 counts in the diagnostic.
- Before mutation, GetText at the full matched offsets must return exactly the
  requested substring in the chosen unit. Caret modes collapse that span only
  after this check. A count match alone never authorizes selection.
- At most one existing selection is supported. Caret placement must read back
  a collapsed/absent selection; a toolkit that retains an unrelated selection
  produces an uncertain result without a second mutation to remove it.
- Range/caret, unchanged text and unchanged X11 desktop focus are read back.
  X11 focus verification cannot prove that an application did not change its
  internal accessibility focus. No automatic focus restoration is attempted.

## Validation at implementation handoff

Three pure Rust matching tests pass: Unicode/combining offsets, ambiguous and
overlapping matches with context, and before/after caret offsets. macOS-host
`cargo check --offline -p platform-linux` passes, but excludes Linux-only native
and tool modules. Rustfmt parses the changed Rust files and `git diff --check`
passes. A Linux compile was not completed locally: only the macOS target is
installed, and offline dependency unpacking was denied by the local sandbox.

Required before promotion: Linux compilation; public SDK saved-document tests
for a non-first paragraph, Unicode before the target, context-disambiguated
matches, both caret modes, stale tokens, sibling windows and modal isolation.
Compare resulting document text and formatting, and verify no unrelated input
or window activation. A success in pure matching tests does not establish
native selection support in LibreOffice.

## Risk inventory integration

`select_text` is reviewed as R1 reversible local control through the existing
active `desktop_input` adapter. It retains the canonical own-process refusal,
window capture-scope restrictions and browser-origin manifest bypass exclusion.
It advertises `accessibility.element_tokens`. Unknown tools remain denied.
The tool returns its explicit verified-range result, not the separate shared
`ActionResult` schema; no generated cross-platform typed contract is claimed.

Offline core checks pass: 41 authorization/session-authorization tests, the new
window-scope test, the existing origin-bypass manifest test (now including
selection), and the existing token-capability inventory test.

## Toolkit offset candidate

The real SELECT-L01 run on 00fb reached native Text but refused because its
reported count differed from the live scalar count. UTF-16 is a hypothesis for
that toolkit, not yet a verified cause. This candidate admits only an exact
scalar/UTF-16 count match plus the native substring probe described above.
Five pure tests pass, including emoji, combining marks, explicit unit selection,
unknown-count rejection, context disambiguation and unit-correct caret edges.
Linux compilation and the repeated saved-document test remain required before
claiming that this resolves the observed refusal. No guard or replay rule was
removed.
