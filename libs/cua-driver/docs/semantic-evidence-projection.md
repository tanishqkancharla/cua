# Semantic context evidence projection

This private-fork contract describes the optional additive metadata emitted on
`semantic_v2` context blocks and context pages when
`member_projection` is `semantic_evidence_v1`. It does not change query match
selection, query coalescing, action refs, snapshot collection, or the ordinary
non-context snapshot response.

## Projected member units

The context member sequence omits a stored semantic node only when all of the
following are true:

- its normalized role is `generic`;
- it has no non-empty name, value, or destination URL;
- it has no retained semantic states; and
- it has no actions.

The structural group is retained. Projection membership is a property of that
exact group and snapshot, independent of which exact node was used as the
anchor. An omitted transparent anchor is still returned as the exact
`anchor_ref` in the response ref union, but it is metadata-only: it does not
consume a member slot or shift member offsets. An around-anchor window uses the
anchor's proven insertion boundary in the projected AX source order.

`member_refs`, `selected_nodes`, `total_nodes`, `before_omitted`,
`after_omitted`, and context continuation offsets are all measured in projected
member units. Every page also reports:

- `source_member_nodes`: visible, ancestry-valid eligible nodes in the stored
  `SemanticDocument` group before this projection;
- `projected_out_nodes`: eligible transparent generic nodes omitted by this
  projection; and
- `member_projection`: the literal projection identifier.

For every context page:

```text
before_omitted + selected_nodes + after_omitted + projected_out_nodes
  = source_member_nodes
```

The same equation holds with `member_refs.length` in place of
`selected_nodes` on the wire. Projection never mints action authority: newly
issued context refs remain read-only, while an already-issued exact action ref
retains only its previously stored capabilities.

## Completeness boundaries

`group_complete` means the full projected semantic-evidence sequence for the
proved group is present and the existing source-structure and document
collection gates passed. It does not mean every source semantic node was
rendered. `document_collection_complete`, `source_member_nodes`, and
`projected_out_nodes` disclose those distinct boundaries. Virtualized content
outside the collected document remains unknown.

Projection happens after existing ancestry and visibility validation.
CSS-hidden and page-occluded nodes are therefore not `source_member_nodes`, and
malformed, cyclic, ambiguous, or unproven ancestry still prevents a truthful
complete result. Transparent nodes can still appear in the outline when needed
as ancestors of selected evidence nodes; wrapper-only leaves can disappear and
are represented only in `projected_out_nodes`.

These counts describe the normalized, collected `SemanticDocument`, not the raw
CDP AX node array. “No states” means no state retained by the current semantic
state allowlist; it is not a claim that the raw AX node had no other property.
Names, values, and destinations also retain the existing per-field length
limits, so this projection is not a full-text or lossless DOM representation.

## Automatic query context coverage

Query matching, ranking, and coalescing are unchanged. Automatic context
blocks are considered in the query result's existing source order. A later
selected match in the same exact group receives another bounded context block
unless that exact semantic node identity was actually returned as a projected
member of an earlier block for that group. Coverage is recorded only after
node and outline-byte clipping, so a match outside the emitted window is not
silently treated as observed.

Multiple blocks for one group may overlap. Every repeated member and outline
byte is charged normally against the shared six-block, 96-member, and
24,000-byte automatic-query limits. This deliberately favors truthful local
evidence over pretending that one prefix represents an entire group; the
standalone context-page limit remains 25 projected members. An exact duplicate
projected range for the same group is emitted only once; this matters when two
metadata-only transparent anchors share one projected insertion boundary.

Exact anchor membership proves only that the selected semantic node appeared
in that block. It does not prove that the block contains an entire conceptual
item or every nearby qualifier. Callers must continue to honor omission counts,
`group_complete`, and context cursors. Malformed or unproven ancestry remains
ineligible for automatic context, and transparent generic anchors remain
metadata-only rather than being counted as covered projected members.
