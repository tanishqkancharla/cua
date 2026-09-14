# Linux observation visibility

## Reproduced behavior

OpenSky agent run34530155297 saved empty dropdown lists in Calc. Its observation
offered a hidden numeric field as an actionable text control beside the visible
Entries editor. Writing to the hidden field changed the reported text while the
actual dialog screenshot remained byte-identical.

The read-only real-app probe visibility-probe-before-01 confirms that Calc marks
the numeric field Showing=false/Visible=false. Contents of inactive tabs are
Showing=false/Visible=true. The Entries editor is Showing=true/Visible=true,
under a panel whose labelled-by relationship names Entries. Current snapshots
discard those states and label relationships. Raw text reads in that first probe
hit a GI method-name collision; state, bounds and relation reads succeeded.

## Correction scope

Preserve the native traversal and application-wide ordinal space. Omit controls
the toolkit positively reports as not showing from both rendered and structured
observations, while consuming their original indices. Keep unknown visibility
explicitly distinguishable from false. Expose actual label relationships where
available; do not infer labels from screen proximity or modify task prompts.

Validate through public SDK observations and a saved List validation outcome,
with real tab switching, menu navigation, scrolled cells and retained-index
regressions. Keep the prior identity correction intact. This is a Linux AT-SPI
change; it does not claim macOS/Windows behavior changes or complete parity.

## Validation status

Implementation retains Showing/Visible separately from ordinal eligibility, consumes
hidden indices without emitting them, and reads both labelled-by and label-for
relations. Related labels preserve captured Text content as a value, including read-only
widgets. Snapshot bounds include only emitted controls, also when unscoped. Passive
and hidden siblings retire stale parent ancestry. Unknown state is retained.

VISIBLE-L01 fails on driver8ed at the hidden Maximum field assertion; app exit
and empty remote container inventory are verified. The unchanged test then
locates Entries by its public label, changes tabs, and checks all28saved choices
plus preserved cell values. Candidate build and after acceptance are pending.
The prior seven identity checks do not certify this change. Canonical desktop certification remains a
separate gate before readiness or merge. Historical agent scores are preserved.

The adapter also serves Linux Wayland observations; X11 desktop evidence does
not certify compositor-specific Wayland behavior. Closed menu descendants will
not appear until the toolkit reports them showing. Keyboard/menu-path operations
retain their existing traversal; public menu navigation must be verified.
