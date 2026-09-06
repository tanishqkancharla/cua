//! Deterministic semantic browser snapshots.
//!
//! The collector joins Chrome's accessibility tree with pierced DOM metadata
//! and layout snapshot evidence. Accessibility supplies readable semantics,
//! DOM backend ids preserve exact mutation capabilities, and layout evidence
//! keeps hidden retained application state from displacing the active view.

use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::Value;
use url::Url;

use super::store::{
    BrowserActionKind, BrowserVisibility, FrameKind, FrameRef, RefEntry, SemanticNodeIdentity,
};

pub(crate) const SEMANTIC_COMPUTED_STYLES: &[&str] = &[
    "display",
    "visibility",
    "opacity",
    "pointer-events",
    "cursor",
    "position",
    "z-index",
    "overflow-x",
    "overflow-y",
];
pub(crate) const DEFAULT_SEMANTIC_NODE_BUDGET: usize = 300;
pub(crate) const CONTEXT_BEFORE_NODES: usize = 8;
pub(crate) const CONTEXT_AFTER_NODES: usize = 16;
pub(crate) const CONTEXT_OUTLINE_MAX_BYTES: usize = 12_000;
pub(crate) const QUERY_CONTEXT_MAX_BLOCKS: usize = 6;
pub(crate) const QUERY_CONTEXT_MAX_NODES: usize = 96;
pub(crate) const QUERY_CONTEXT_OUTLINE_MAX_BYTES: usize = 24_000;
pub(crate) const SEMANTIC_EVIDENCE_MEMBER_PROJECTION: &str = "semantic_evidence_v1";
const NEAR_VIEWPORT_MARGIN: f64 = 1_000.0;
const MAX_SEMANTIC_TEXT_CHARS: usize = 1_000;
const MAX_LINK_DESTINATION_CHARS: usize = 2_048;
// Raw AX node ids with no text cannot become semantic nodes, so the empty id
// is a collision-free internal marker for an unprovable collapsed edge.
const UNRESOLVED_AX_ID: &str = "";

#[derive(Debug, Clone, Copy, PartialEq)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Rect {
    fn from_value(value: &Value) -> Option<Self> {
        let values = value.as_array()?;
        Some(Self {
            x: values.first()?.as_f64()?,
            y: values.get(1)?.as_f64()?,
            width: values.get(2)?.as_f64()?,
            height: values.get(3)?.as_f64()?,
        })
    }

    fn has_area(self) -> bool {
        self.width > 0.0 && self.height > 0.0
    }

    fn intersects(self, other: Self) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    fn expanded(self, margin: f64) -> Self {
        Self {
            x: self.x - margin,
            y: self.y - margin,
            width: self.width + margin * 2.0,
            height: self.height + margin * 2.0,
        }
    }

    fn covers(self, other: Self) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && self.x + self.width >= other.x + other.width
            && self.y + self.height >= other.y + other.height
    }
}

#[derive(Debug, Clone, Default)]
struct DomMeta {
    tag: String,
    attrs: HashMap<String, String>,
    destination_url: Option<String>,
    order: usize,
    css_hidden: bool,
    parent_backend_node_id: Option<i64>,
    frame_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DomIndex {
    nodes: HashMap<i64, DomMeta>,
    pub(crate) css_hidden_count: usize,
}

#[derive(Debug, Clone, Default)]
struct LayoutMeta {
    bounds: Option<Rect>,
    client_rect: Option<Rect>,
    scroll_rect: Option<Rect>,
    styles: HashMap<String, String>,
    paint_order: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LayoutIndex {
    nodes: HashMap<i64, LayoutMeta>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Viewport {
    rect: Option<Rect>,
}

#[derive(Debug, Clone)]
pub(crate) struct SemanticNode {
    pub(crate) ax_id: String,
    pub(crate) parent_ax_id: Option<String>,
    pub(crate) child_ax_ids: Vec<String>,
    pub(crate) backend_node_id: Option<i64>,
    pub(crate) role: String,
    pub(crate) name: Option<String>,
    pub(crate) value: Option<String>,
    /// Display-only page-authored metadata. This is never consulted when a
    /// ref is resolved or when its mutation capabilities are checked.
    pub(crate) destination_url: Option<String>,
    pub(crate) states: BTreeMap<String, Value>,
    pub(crate) frame: FrameRef,
    pub(crate) visibility: BrowserVisibility,
    pub(crate) actions: Vec<BrowserActionKind>,
    pub(crate) document_order: usize,
}

impl SemanticNode {
    pub(crate) fn identity(&self) -> SemanticNodeIdentity {
        SemanticNodeIdentity {
            ax_id: self.ax_id.clone(),
            document_order: self.document_order,
            frame: self.frame.clone(),
        }
    }

    pub(crate) fn to_ref_entry(&self) -> Option<RefEntry> {
        let backend_node_id = self.backend_node_id?;
        Some(RefEntry {
            backend_node_id,
            node_name: self.role.clone(),
            label: self.name.clone(),
            actions: self.actions.clone(),
            visibility: Some(self.visibility),
            semantic: true,
            context_only: false,
            semantic_node: Some(self.identity()),
            frame: self.frame.clone(),
        })
    }

    pub(crate) fn to_context_ref_entry(&self) -> RefEntry {
        RefEntry {
            backend_node_id: self.backend_node_id.unwrap_or(0),
            node_name: self.role.clone(),
            label: self.name.clone(),
            actions: Vec::new(),
            visibility: Some(self.visibility),
            semantic: true,
            context_only: true,
            semantic_node: Some(self.identity()),
            frame: self.frame.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct OmissionCounts {
    pub(crate) css_hidden: usize,
    pub(crate) offscreen: usize,
    pub(crate) page_occluded: usize,
    pub(crate) no_layout: usize,
    pub(crate) unknown: usize,
    pub(crate) budget: usize,
    pub(crate) unprovable_frame: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct SemanticPage {
    pub(crate) outline: String,
    pub(crate) selected: Vec<SemanticNode>,
    pub(crate) selected_nodes: usize,
    pub(crate) total_nodes: usize,
    pub(crate) next_offset: Option<usize>,
    pub(crate) omissions: OmissionCounts,
    pub(crate) hierarchy_complete: bool,
    selected_indices: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SemanticContextError {
    AnchorUnproven,
    AnchorAmbiguous,
    GroupUnavailable,
}

#[derive(Debug, Clone)]
pub(crate) struct SemanticContextPage {
    pub(crate) outline: String,
    pub(crate) nodes: Vec<SemanticNode>,
    pub(crate) group: SemanticNode,
    pub(crate) anchor: SemanticNode,
    pub(crate) parent_group: Option<SemanticNode>,
    pub(crate) selected_nodes: usize,
    pub(crate) total_nodes: usize,
    pub(crate) before_omitted: usize,
    pub(crate) after_omitted: usize,
    pub(crate) group_complete: bool,
    pub(crate) document_collection_complete: bool,
    pub(crate) member_projection: &'static str,
    pub(crate) source_member_nodes: usize,
    pub(crate) projected_out_nodes: usize,
    pub(crate) omissions: OmissionCounts,
    pub(crate) range_start: usize,
    pub(crate) range_end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SemanticContextWindow {
    Around,
    Forward { start: usize },
    Backward { end: usize },
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SemanticDocument {
    pub(crate) nodes: Vec<SemanticNode>,
    pub(crate) css_hidden_dom_count: usize,
    pub(crate) unprovable_frame_count: usize,
    pub(crate) complete: bool,
}

impl SemanticDocument {
    pub(crate) fn extend(&mut self, mut other: Self) {
        let was_empty = self.nodes.is_empty();
        let offset = self.nodes.len();
        for node in &mut other.nodes {
            node.document_order += offset;
        }
        self.nodes.extend(other.nodes);
        self.css_hidden_dom_count += other.css_hidden_dom_count;
        self.unprovable_frame_count += other.unprovable_frame_count;
        self.complete = if was_empty {
            other.complete
        } else {
            self.complete && other.complete
        };
    }

    pub(crate) fn page(
        &self,
        offset: usize,
        budget: usize,
        query: Option<&str>,
        scope_backend_node_id: Option<i64>,
    ) -> SemanticPage {
        let scoped = scoped_indices(&self.nodes, query, scope_backend_node_id);
        let mut candidates = scoped.indices;
        candidates.retain(|idx| {
            !matches!(
                self.nodes[*idx].visibility,
                BrowserVisibility::CssHidden | BrowserVisibility::PageOccluded
            )
        });
        candidates.sort_by_key(|idx| {
            (
                Reverse(query.map_or(0, |query| query_score(&self.nodes[*idx], query))),
                rank(&self.nodes[*idx]),
                self.nodes[*idx].document_order,
            )
        });

        let start = offset.min(candidates.len());
        let end = (start + budget.max(1)).min(candidates.len());
        let ancestry = unique_ax_indices(&self.nodes);
        let raw_page_slice = &candidates[start..end];
        let mut page_slice = Vec::with_capacity(raw_page_slice.len());
        let mut malformed_hierarchy = 0;
        let mut hierarchy_complete = scoped.hierarchy_complete;
        for idx in raw_page_slice {
            if ancestor_indices(&self.nodes, &ancestry, *idx).is_some() {
                page_slice.push(*idx);
            } else {
                hierarchy_complete = false;
                if self.nodes[*idx].visibility != BrowserVisibility::Unknown {
                    malformed_hierarchy += 1;
                }
            }
        }
        let selected = with_ancestors(&self.nodes, &page_slice);
        let outline = render_outline(&self.nodes, &selected);
        let selected_nodes = page_slice
            .iter()
            .map(|idx| self.nodes[*idx].clone())
            .collect::<Vec<_>>();
        let mut omissions = self.omission_counts();
        // Cyclic parent graphs are not truthful hierarchy evidence. Omit their
        // selected nodes and account for them through the existing unknown
        // omission bucket without extending the public response schema.
        omissions.unknown += malformed_hierarchy;
        omissions.budget = candidates.len().saturating_sub(end);

        SemanticPage {
            outline,
            selected: selected_nodes,
            selected_nodes: page_slice.len(),
            total_nodes: candidates.len(),
            next_offset: (end < candidates.len()).then_some(end),
            omissions,
            hierarchy_complete,
            selected_indices: page_slice,
        }
    }

    pub(crate) fn context(
        &self,
        anchor: &RefEntry,
    ) -> Result<SemanticContextPage, SemanticContextError> {
        self.context_window(
            anchor,
            None,
            SemanticContextWindow::Around,
            CONTEXT_BEFORE_NODES + 1 + CONTEXT_AFTER_NODES,
            CONTEXT_OUTLINE_MAX_BYTES,
        )
    }

    pub(crate) fn query_contexts(&self, page: &SemanticPage) -> Vec<SemanticContextPage> {
        let by_ax_id = unique_ax_indices(&self.nodes);
        let mut selected = page.selected_indices.clone();
        selected.sort_by_key(|idx| self.nodes[*idx].document_order);
        let mut group_cache = HashMap::new();
        let mut covered_members_by_group: Vec<(SemanticNodeIdentity, Vec<SemanticNodeIdentity>)> =
            Vec::new();
        let mut contexts: Vec<SemanticContextPage> = Vec::new();
        let mut remaining_nodes = QUERY_CONTEXT_MAX_NODES;
        let mut remaining_bytes = QUERY_CONTEXT_OUTLINE_MAX_BYTES;
        for anchor_idx in selected {
            if contexts.len() >= QUERY_CONTEXT_MAX_BLOCKS
                || remaining_nodes == 0
                || remaining_bytes == 0
            {
                break;
            }
            let Some(groups) =
                cached_context_groups(&self.nodes, &by_ax_id, anchor_idx, &mut group_cache)
            else {
                continue;
            };
            let Some(nearest) = groups.first().copied() else {
                continue;
            };
            let enclosing = groups.get(1).copied().unwrap_or(nearest);
            let group_identity = self.nodes[enclosing].identity();
            let anchor_identity = self.nodes[anchor_idx].identity();
            if covered_members_by_group
                .iter()
                .find(|(group, _)| group == &group_identity)
                .is_some_and(|(_, covered)| covered.contains(&anchor_identity))
            {
                continue;
            }
            let anchor = self.nodes[anchor_idx].to_context_ref_entry();
            let Ok(context) = self.context_window(
                &anchor,
                Some(&group_identity),
                SemanticContextWindow::Around,
                remaining_nodes.min(CONTEXT_BEFORE_NODES + 1 + CONTEXT_AFTER_NODES),
                remaining_bytes,
            ) else {
                continue;
            };
            if context.outline.len() > remaining_bytes {
                continue;
            }
            let returned_identities = context
                .nodes
                .iter()
                .map(SemanticNode::identity)
                .collect::<Vec<_>>();
            // A retained query match must really be present after node and
            // byte clipping before it can suppress a later automatic block.
            // Transparent generic anchors are metadata-only under the member
            // projection and therefore do not enter coverage here.
            if !is_transparent_generic(&self.nodes[anchor_idx])
                && !returned_identities.contains(&anchor_identity)
            {
                continue;
            }
            // Metadata-only transparent anchors can share the same projected
            // insertion boundary. Do not spend the bounded response on an
            // identical window that adds no member evidence.
            if contexts.iter().any(|emitted| {
                emitted.group.identity() == group_identity
                    && emitted.range_start == context.range_start
                    && emitted.range_end == context.range_end
            }) {
                continue;
            }
            remaining_nodes -= context.nodes.len();
            remaining_bytes -= context.outline.len();
            let group_position = covered_members_by_group
                .iter()
                .position(|(group, _)| group == &group_identity)
                .unwrap_or_else(|| {
                    covered_members_by_group.push((group_identity, Vec::new()));
                    covered_members_by_group.len() - 1
                });
            let covered = &mut covered_members_by_group[group_position].1;
            for identity in returned_identities {
                if !covered.contains(&identity) {
                    covered.push(identity);
                }
            }
            contexts.push(context);
        }
        contexts
    }

    pub(crate) fn context_window(
        &self,
        anchor: &RefEntry,
        requested_group: Option<&SemanticNodeIdentity>,
        window: SemanticContextWindow,
        node_budget: usize,
        outline_max_bytes: usize,
    ) -> Result<SemanticContextPage, SemanticContextError> {
        let Some(identity) = &anchor.semantic_node else {
            return Err(SemanticContextError::AnchorUnproven);
        };
        if !anchor.semantic || identity.frame.identity.is_none() {
            return Err(SemanticContextError::AnchorUnproven);
        }
        let matching = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                node.ax_id == identity.ax_id
                    && node.document_order == identity.document_order
                    && node.frame == identity.frame
            })
            .map(|(idx, _)| idx)
            .collect::<Vec<_>>();
        let [anchor_idx] = matching.as_slice() else {
            return Err(if matching.is_empty() {
                SemanticContextError::AnchorUnproven
            } else {
                SemanticContextError::AnchorAmbiguous
            });
        };

        let by_ax_id = unique_ax_indices(&self.nodes);
        let ancestors = ancestor_indices(&self.nodes, &by_ax_id, *anchor_idx)
            .ok_or(SemanticContextError::AnchorAmbiguous)?;
        let groups = std::iter::once(*anchor_idx)
            .chain(ancestors.iter().copied())
            .filter(|idx| is_context_group_role(&self.nodes[*idx].role))
            .collect::<Vec<_>>();
        let group_idx = match requested_group {
            Some(requested) => groups
                .iter()
                .copied()
                .find(|idx| self.nodes[*idx].identity() == *requested)
                .ok_or(SemanticContextError::GroupUnavailable)?,
            None => *groups
                .first()
                .ok_or(SemanticContextError::GroupUnavailable)?,
        };
        let group_position = groups.iter().position(|idx| *idx == group_idx).unwrap();
        let parent_group = groups
            .get(group_position + 1)
            .map(|idx| self.nodes[*idx].clone());

        let mut members = HashSet::new();
        let mut member_order = Vec::new();
        let mut stack: Vec<(usize, Option<usize>)> = vec![(group_idx, None)];
        let mut structure_complete = true;
        while let Some((idx, expected_parent)) = stack.pop() {
            if let Some(parent_idx) = expected_parent {
                if self.nodes[idx].parent_ax_id.as_deref()
                    != Some(self.nodes[parent_idx].ax_id.as_str())
                {
                    structure_complete = false;
                    continue;
                }
            }
            if !members.insert(idx) {
                structure_complete = false;
                continue;
            }
            member_order.push(idx);
            // Stack traversal reverses insertion, so push children in reverse
            // to preserve the validated AX childIds preorder.
            for child_id in self.nodes[idx].child_ax_ids.iter().rev() {
                match ax_lookup(&by_ax_id, &self.nodes[idx].frame, child_id) {
                    AxLookup::Unique(child_idx) => stack.push((child_idx, Some(idx))),
                    AxLookup::Missing | AxLookup::Ambiguous | AxLookup::Unproven => {
                        structure_complete = false;
                    }
                }
            }
        }

        let mut eligible = Vec::new();
        for idx in member_order.iter().copied() {
            if ancestor_indices(&self.nodes, &by_ax_id, idx).is_none() {
                structure_complete = false;
                continue;
            }
            if !matches!(
                self.nodes[idx].visibility,
                BrowserVisibility::CssHidden | BrowserVisibility::PageOccluded
            ) {
                eligible.push(idx);
            }
        }
        let source_member_nodes = eligible.len();
        // Projection is a property of the proven group, not of the caller's
        // anchor. Keep the exact anchor as response metadata/authority, but do
        // not let an otherwise transparent anchor shift this group's cursor
        // offsets or totals.
        let is_evidence_member =
            |idx: usize| idx == group_idx || !is_transparent_generic(&self.nodes[idx]);
        let evidence = eligible
            .iter()
            .copied()
            .filter(|idx| is_evidence_member(*idx))
            .collect::<Vec<_>>();
        let projected_out_nodes = source_member_nodes.saturating_sub(evidence.len());
        let anchor_source_position = eligible
            .iter()
            .position(|idx| *idx == *anchor_idx)
            .ok_or(SemanticContextError::AnchorUnproven)?;
        let projected_before_anchor = eligible[..anchor_source_position]
            .iter()
            .filter(|idx| is_evidence_member(**idx))
            .count();
        let anchor_is_member = is_evidence_member(*anchor_idx);
        // This is the boundary immediately after a retained anchor, or the
        // insertion boundary at an omitted transparent anchor.
        let anchor_boundary = projected_before_anchor + usize::from(anchor_is_member);
        let (mut start, mut end) = match window {
            SemanticContextWindow::Around => {
                let start = if group_idx == *anchor_idx {
                    0
                } else {
                    projected_before_anchor.saturating_sub(CONTEXT_BEFORE_NODES)
                };
                let end = (if group_idx == *anchor_idx {
                    CONTEXT_BEFORE_NODES + 1 + CONTEXT_AFTER_NODES
                } else {
                    anchor_boundary + CONTEXT_AFTER_NODES
                })
                .min(evidence.len());
                let mut start = start;
                let mut end = end;
                while end.saturating_sub(start) > node_budget.max(1) {
                    if end > anchor_boundary {
                        end -= 1
                    } else {
                        start += 1
                    }
                }
                (start, end)
            }
            SemanticContextWindow::Forward { start } => {
                let start = start.min(evidence.len());
                (start, (start + node_budget.max(1)).min(evidence.len()))
            }
            SemanticContextWindow::Backward { end } => {
                let end = end.min(evidence.len());
                (end.saturating_sub(node_budget.max(1)), end)
            }
        };
        let render_range = |start: usize, end: usize| {
            let slice = &evidence[start..end];
            let selected = with_ancestors(&self.nodes, slice);
            let mut context_order =
                ancestor_indices(&self.nodes, &by_ax_id, group_idx).unwrap_or_default();
            context_order.reverse();
            context_order.extend(member_order.iter().copied());
            render_outline_ordered(&self.nodes, &selected, context_order)
        };
        let mut outline = render_range(start, end);
        while outline.len() > outline_max_bytes && end.saturating_sub(start) > 1 {
            match window {
                SemanticContextWindow::Backward { .. } => start += 1,
                SemanticContextWindow::Around if end > anchor_boundary => end -= 1,
                SemanticContextWindow::Around => start += 1,
                SemanticContextWindow::Forward { .. } => end -= 1,
            }
            outline = render_range(start, end);
        }
        if outline.len() > outline_max_bytes {
            return Err(SemanticContextError::GroupUnavailable);
        }
        let selected_indices = &evidence[start..end];
        let before_omitted = start;
        let after_omitted = evidence.len().saturating_sub(end);
        let group_complete =
            self.complete && structure_complete && before_omitted == 0 && after_omitted == 0;
        let mut omissions = self.omission_counts();
        omissions.budget = before_omitted + after_omitted;
        Ok(SemanticContextPage {
            outline,
            nodes: selected_indices
                .iter()
                .map(|idx| self.nodes[*idx].clone())
                .collect(),
            group: self.nodes[group_idx].clone(),
            anchor: self.nodes[*anchor_idx].clone(),
            parent_group,
            selected_nodes: selected_indices.len(),
            total_nodes: evidence.len(),
            before_omitted,
            after_omitted,
            group_complete,
            document_collection_complete: self.complete,
            member_projection: SEMANTIC_EVIDENCE_MEMBER_PROJECTION,
            source_member_nodes,
            projected_out_nodes,
            omissions,
            range_start: start,
            range_end: end,
        })
    }

    fn omission_counts(&self) -> OmissionCounts {
        let semantic_css_hidden = self
            .nodes
            .iter()
            .filter(|node| node.visibility == BrowserVisibility::CssHidden)
            .count();
        let mut omissions = OmissionCounts {
            css_hidden: self.css_hidden_dom_count.max(semantic_css_hidden),
            unprovable_frame: self.unprovable_frame_count,
            ..Default::default()
        };
        for node in &self.nodes {
            match node.visibility {
                BrowserVisibility::CssHidden => {}
                BrowserVisibility::Offscreen => omissions.offscreen += 1,
                BrowserVisibility::PageOccluded => omissions.page_occluded += 1,
                BrowserVisibility::NoLayout => omissions.no_layout += 1,
                BrowserVisibility::Unknown => omissions.unknown += 1,
                BrowserVisibility::InViewport | BrowserVisibility::NearViewport => {}
            }
        }
        omissions
    }
}

impl DomIndex {
    fn is_ancestor_of(&self, ancestor: i64, mut descendant: i64) -> bool {
        let mut visited = HashSet::new();
        while visited.insert(descendant) {
            let Some(parent) = self
                .nodes
                .get(&descendant)
                .and_then(|node| node.parent_backend_node_id)
            else {
                return false;
            };
            if parent == ancestor {
                return true;
            }
            descendant = parent;
        }
        false
    }

    fn shares_dom_branch(&self, left: i64, right: i64) -> bool {
        self.is_ancestor_of(left, right) || self.is_ancestor_of(right, left)
    }
}

pub(crate) fn build_dom_index(root: &Value) -> DomIndex {
    fn walk(
        node: &Value,
        inherited_hidden: bool,
        parent_backend_node_id: Option<i64>,
        inherited_frame_id: Option<&str>,
        inherited_base_url: Option<&Url>,
        order: &mut usize,
        index: &mut DomIndex,
    ) {
        let node_type = node.get("nodeType").and_then(Value::as_i64).unwrap_or(0);
        let attrs = attributes(node);
        let hidden = inherited_hidden || statically_hidden(&attrs);
        let backend_node_id = node.get("backendNodeId").and_then(Value::as_i64);
        let frame_id = if node_type == 9 {
            node.get("frameId").and_then(Value::as_str)
        } else {
            inherited_frame_id
        };
        let document_base_url = (node_type == 9)
            .then(|| {
                node.get("baseURL")
                    .or_else(|| node.get("documentURL"))
                    .and_then(Value::as_str)
                    .and_then(|value| Url::parse(value).ok())
            })
            .flatten();
        let base_url = if node_type == 9 {
            document_base_url.as_ref()
        } else {
            inherited_base_url
        };
        if let Some(backend) = backend_node_id {
            let tag = node
                .get("nodeName")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_ascii_lowercase();
            if node_type == 1 && hidden && !inherited_hidden {
                index.css_hidden_count += 1;
            }
            index.nodes.insert(
                backend,
                DomMeta {
                    tag,
                    destination_url: attrs
                        .get("href")
                        .and_then(|href| resolve_link_destination(href, base_url)),
                    attrs,
                    order: *order,
                    css_hidden: hidden,
                    parent_backend_node_id,
                    frame_id: frame_id.map(str::to_owned),
                },
            );
            *order += 1;
        }
        if let Some(children) = node.get("children").and_then(Value::as_array) {
            for child in children {
                walk(
                    child,
                    hidden,
                    backend_node_id.or(parent_backend_node_id),
                    frame_id,
                    base_url,
                    order,
                    index,
                );
            }
        }
        if let Some(shadow_roots) = node.get("shadowRoots").and_then(Value::as_array) {
            for shadow_root in shadow_roots {
                if shadow_root.get("shadowRootType").and_then(Value::as_str) == Some("user-agent") {
                    continue;
                }
                walk(
                    shadow_root,
                    hidden,
                    backend_node_id.or(parent_backend_node_id),
                    frame_id,
                    base_url,
                    order,
                    index,
                );
            }
        }
        if let Some(content_document) = node.get("contentDocument") {
            walk(
                content_document,
                hidden,
                backend_node_id.or(parent_backend_node_id),
                content_document.get("frameId").and_then(Value::as_str),
                None,
                order,
                index,
            );
        }
    }

    let mut index = DomIndex::default();
    let mut order = 0;
    walk(
        root,
        false,
        None,
        root.get("frameId").and_then(Value::as_str),
        None,
        &mut order,
        &mut index,
    );
    index
}

fn resolve_link_destination(raw: &str, base_url: Option<&Url>) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > MAX_LINK_DESTINATION_CHARS {
        return None;
    }
    let resolved = Url::parse(raw)
        .or_else(|_| {
            base_url
                .ok_or(url::ParseError::RelativeUrlWithoutBase)?
                .join(raw)
        })
        .ok()?;
    if !matches!(resolved.scheme(), "http" | "https" | "about" | "mailto") {
        return None;
    }
    if matches!(resolved.scheme(), "http" | "https") && resolved.host_str().is_none() {
        return None;
    }
    if !resolved.username().is_empty() || resolved.password().is_some() {
        return None;
    }
    let destination = resolved.to_string();
    (destination.len() <= MAX_LINK_DESTINATION_CHARS).then_some(destination)
}

pub(crate) fn build_layout_index(snapshot: &Value) -> LayoutIndex {
    let Some(strings) = snapshot.get("strings").and_then(Value::as_array) else {
        return LayoutIndex::default();
    };
    let string_at = |idx: i64| -> Option<String> {
        usize::try_from(idx)
            .ok()
            .and_then(|idx| strings.get(idx))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };

    let mut out = LayoutIndex::default();
    let Some(documents) = snapshot.get("documents").and_then(Value::as_array) else {
        return out;
    };
    for document in documents {
        let Some(backend_ids) = document
            .pointer("/nodes/backendNodeId")
            .and_then(Value::as_array)
        else {
            continue;
        };
        let layout = document.get("layout").unwrap_or(&Value::Null);
        let node_indices = layout
            .get("nodeIndex")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let bounds = layout
            .get("bounds")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let styles = layout
            .get("styles")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let paint_orders = layout
            .get("paintOrders")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let client_rects = layout
            .get("clientRects")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let scroll_rects = layout
            .get("scrollRects")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        for (layout_idx, node_index) in node_indices.iter().enumerate() {
            let Some(node_index) = node_index.as_u64().and_then(|v| usize::try_from(v).ok()) else {
                continue;
            };
            let Some(backend) = backend_ids.get(node_index).and_then(Value::as_i64) else {
                continue;
            };
            let mut computed = HashMap::new();
            if let Some(style_indices) = styles.get(layout_idx).and_then(Value::as_array) {
                for (name, value_idx) in SEMANTIC_COMPUTED_STYLES
                    .iter()
                    .zip(style_indices.iter().filter_map(Value::as_i64))
                {
                    if let Some(value) = string_at(value_idx) {
                        computed.insert((*name).to_owned(), value);
                    }
                }
            }
            out.nodes.insert(
                backend,
                LayoutMeta {
                    bounds: bounds.get(layout_idx).and_then(Rect::from_value),
                    client_rect: client_rects.get(layout_idx).and_then(Rect::from_value),
                    scroll_rect: scroll_rects.get(layout_idx).and_then(Rect::from_value),
                    styles: computed,
                    paint_order: paint_orders.get(layout_idx).and_then(Value::as_i64),
                },
            );
        }
    }
    out
}

pub(crate) fn parse_viewport(metrics: &Value) -> Viewport {
    let viewport = metrics
        .get("cssVisualViewport")
        .or_else(|| metrics.get("visualViewport"));
    let Some(viewport) = viewport else {
        return Viewport::default();
    };
    let x = viewport
        .get("pageX")
        .or_else(|| viewport.get("offsetX"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let y = viewport
        .get("pageY")
        .or_else(|| viewport.get("offsetY"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let width = viewport.get("clientWidth").and_then(Value::as_f64);
    let height = viewport.get("clientHeight").and_then(Value::as_f64);
    Viewport {
        rect: width.zip(height).map(|(width, height)| Rect {
            x,
            y,
            width,
            height,
        }),
    }
}

pub(crate) fn compose_accessibility_tree(
    ax_tree: &Value,
    dom: &DomIndex,
    layout: &LayoutIndex,
    viewport: &Viewport,
    frame: FrameRef,
) -> SemanticDocument {
    let Some(ax_nodes) = ax_tree.get("nodes").and_then(Value::as_array) else {
        return SemanticDocument {
            complete: false,
            css_hidden_dom_count: dom.css_hidden_count,
            ..Default::default()
        };
    };

    // Chrome may retain ignored structural intermediates (notably rowgroup)
    // between exposed semantic nodes. Collapse only those exact raw AX edges;
    // unknown, duplicate, or cyclic ids remain unresolved so later hierarchy
    // checks fail closed rather than inventing ancestry.
    let mut raw_by_id: HashMap<String, Option<&Value>> = HashMap::new();
    for ax in ax_nodes {
        let Some(id) = ax
            .get("nodeId")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
        else {
            continue;
        };
        raw_by_id
            .entry(id.to_owned())
            .and_modify(|entry| *entry = None)
            .or_insert(Some(ax));
    }
    let retained_ids = raw_by_id
        .iter()
        .filter_map(|(id, raw)| raw.filter(|raw| retained_ax_node(raw)).map(|_| id.clone()))
        .collect::<HashSet<_>>();
    let raw_children = raw_by_id
        .iter()
        .filter_map(|(id, raw)| raw.map(|raw| (id.clone(), RawAxChildren::from_node(raw))))
        .collect::<HashMap<_, _>>();
    let traversal_budget = raw_by_id
        .len()
        .saturating_add(
            raw_children
                .values()
                .map(|children| children.valid.len())
                .sum::<usize>(),
        )
        .saturating_add(1);
    let unresolved_ax_id = UNRESOLVED_AX_ID;

    let mut nodes = Vec::new();
    let mut parent_cache = HashMap::new();
    for (fallback_order, ax) in ax_nodes.iter().enumerate() {
        let ax_id = ax
            .get("nodeId")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        if ax_id.is_empty() {
            continue;
        }
        let role = ax_value_string(ax.get("role"))
            .unwrap_or_else(|| "unknown".to_owned())
            .to_ascii_lowercase();
        if !retained_ids.contains(&ax_id) {
            continue;
        }
        let backend_node_id = ax.get("backendDOMNodeId").and_then(Value::as_i64);
        let dom_meta = backend_node_id.and_then(|backend| dom.nodes.get(&backend));
        let layout_meta = backend_node_id.and_then(|backend| layout.nodes.get(&backend));
        let states = ax_states(ax);
        let visibility = classify_visibility(dom_meta, layout_meta, viewport);
        let actions = action_kinds(&role, dom_meta, &states, layout_meta);
        let name = ax_value_string(ax.get("name")).and_then(clean_semantic_text);
        let value = ax_value_string(ax.get("value")).and_then(clean_semantic_text);
        let destination_url = (role == "link")
            .then(|| dom_meta.and_then(|meta| meta.destination_url.clone()))
            .flatten();
        let document_order = dom_meta.map_or(fallback_order, |meta| meta.order);
        let mut child_ax_ids = normalized_ax_children(
            ax,
            &raw_by_id,
            &raw_children,
            &retained_ids,
            traversal_budget,
        );
        if !child_ax_ids.complete {
            child_ax_ids.ids.push(unresolved_ax_id.to_owned());
        }
        let parent_ax_id = normalized_ax_parent(
            &ax_id,
            &raw_by_id,
            &raw_children,
            &retained_ids,
            &mut parent_cache,
        )
        .to_ax_id(unresolved_ax_id);
        nodes.push(SemanticNode {
            ax_id,
            parent_ax_id,
            child_ax_ids: child_ax_ids.ids,
            backend_node_id,
            role,
            name,
            value,
            destination_url,
            states,
            frame: frame.clone(),
            visibility,
            actions,
            document_order,
        });
    }

    supplement_dom_actions(&mut nodes, dom, layout, viewport, &frame);
    label_document_scroll_roots(&mut nodes, dom);
    apply_page_occlusion(&mut nodes, dom, layout);
    remove_redundant_static_text(&mut nodes);
    SemanticDocument {
        nodes,
        css_hidden_dom_count: dom.css_hidden_count,
        unprovable_frame_count: 0,
        complete: true,
    }
}

fn retained_ax_node(ax: &Value) -> bool {
    if ax.get("ignored").and_then(Value::as_bool) == Some(true) {
        return false;
    }
    ax_value_string(ax.get("role")).is_none_or(|role| !role.eq_ignore_ascii_case("inlinetextbox"))
}

#[derive(Debug)]
struct RawAxChildren {
    valid: Vec<String>,
    valid_set: HashSet<String>,
    complete: bool,
}

impl RawAxChildren {
    fn from_node(ax: &Value) -> Self {
        let Some(value) = ax.get("childIds") else {
            return Self {
                valid: Vec::new(),
                valid_set: HashSet::new(),
                complete: true,
            };
        };
        let Some(children) = value.as_array() else {
            return Self {
                valid: Vec::new(),
                valid_set: HashSet::new(),
                complete: false,
            };
        };
        let mut counts = HashMap::new();
        let mut complete = true;
        for child in children {
            if let Some(id) = child.as_str().filter(|id| !id.is_empty()) {
                *counts.entry(id).or_insert(0_usize) += 1;
            } else {
                complete = false;
            }
        }
        let valid = children
            .iter()
            .filter_map(Value::as_str)
            .filter(|id| counts.get(id) == Some(&1))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if valid.len() != children.len() {
            complete = false;
        }
        let valid_set = valid.iter().cloned().collect();
        Self {
            valid,
            valid_set,
            complete,
        }
    }
}

fn is_unresolved_ax_id(ax_id: &str) -> bool {
    ax_id == UNRESOLVED_AX_ID
}

#[derive(Debug, Clone)]
enum NormalizedAxParent {
    Root,
    Retained(String),
    Unresolved,
}

impl NormalizedAxParent {
    fn to_ax_id(&self, unresolved_ax_id: &str) -> Option<String> {
        match self {
            Self::Root => None,
            Self::Retained(id) => Some(id.clone()),
            Self::Unresolved => Some(unresolved_ax_id.to_owned()),
        }
    }
}

fn normalized_ax_parent(
    ax_id: &str,
    raw_by_id: &HashMap<String, Option<&Value>>,
    raw_children: &HashMap<String, RawAxChildren>,
    retained_ids: &HashSet<String>,
    cache: &mut HashMap<String, NormalizedAxParent>,
) -> NormalizedAxParent {
    if let Some(resolution) = cache.get(ax_id) {
        return resolution.clone();
    }
    let mut child = ax_id.to_owned();
    let mut visited = HashSet::new();
    let mut path = Vec::new();
    let resolution = loop {
        if let Some(resolution) = cache.get(&child) {
            break resolution.clone();
        }
        if !visited.insert(child.clone()) {
            break NormalizedAxParent::Unresolved;
        }
        path.push(child.clone());
        let Some(Some(raw)) = raw_by_id.get(&child) else {
            break NormalizedAxParent::Unresolved;
        };
        let Some(parent_value) = raw.get("parentId") else {
            break NormalizedAxParent::Root;
        };
        let Some(parent) = parent_value.as_str().filter(|parent| !parent.is_empty()) else {
            break NormalizedAxParent::Unresolved;
        };
        let Some(Some(parent_raw)) = raw_by_id.get(parent) else {
            break NormalizedAxParent::Unresolved;
        };
        if raw_children
            .get(parent)
            .is_none_or(|children| !children.valid_set.contains(&child))
            || parent_raw.get("nodeId").and_then(Value::as_str) != Some(parent)
        {
            break NormalizedAxParent::Unresolved;
        }
        if retained_ids.contains(parent) {
            break NormalizedAxParent::Retained(parent.to_owned());
        }
        child = parent.to_owned();
    };
    for id in path {
        cache.insert(id, resolution.clone());
    }
    resolution
}

fn normalized_ax_children(
    ax: &Value,
    raw_by_id: &HashMap<String, Option<&Value>>,
    raw_children: &HashMap<String, RawAxChildren>,
    retained_ids: &HashSet<String>,
    traversal_budget: usize,
) -> NormalizedAxChildren {
    let Some(root_id) = ax.get("nodeId").and_then(Value::as_str) else {
        return NormalizedAxChildren {
            ids: Vec::new(),
            complete: false,
        };
    };
    let Some(root_children) = raw_children.get(root_id) else {
        return NormalizedAxChildren {
            ids: Vec::new(),
            complete: false,
        };
    };
    let mut complete = root_children.complete;
    let mut out = Vec::new();
    let mut stack = root_children
        .valid
        .iter()
        .rev()
        .map(|child| (root_id.to_owned(), child.clone()))
        .collect::<Vec<_>>();
    let mut visited = HashSet::from([root_id.to_owned()]);
    let mut steps = 0_usize;
    while let Some((parent_id, child_id)) = stack.pop() {
        steps += 1;
        if steps > traversal_budget {
            complete = false;
            break;
        }
        if !visited.insert(child_id.clone()) {
            complete = false;
            continue;
        }
        let Some(Some(child)) = raw_by_id.get(&child_id) else {
            complete = false;
            continue;
        };
        if child.get("parentId").and_then(Value::as_str) != Some(parent_id.as_str()) {
            complete = false;
            continue;
        }
        if retained_ids.contains(&child_id) {
            out.push(child_id);
            continue;
        }
        let Some(children) = raw_children.get(&child_id) else {
            complete = false;
            continue;
        };
        complete &= children.complete;
        for grandchild in children.valid.iter().rev() {
            stack.push((child_id.clone(), grandchild.clone()));
        }
    }
    NormalizedAxChildren { ids: out, complete }
}

struct NormalizedAxChildren {
    ids: Vec<String>,
    complete: bool,
}

fn label_document_scroll_roots(nodes: &mut [SemanticNode], dom: &DomIndex) {
    for node in nodes {
        let is_document_body = node
            .backend_node_id
            .and_then(|backend| dom.nodes.get(&backend))
            .is_some_and(|meta| meta.tag == "body");
        if is_document_body
            && node.name.is_none()
            && node.actions.contains(&BrowserActionKind::Scroll)
        {
            node.name = Some("Document".to_owned());
        }
    }
}

fn apply_page_occlusion(nodes: &mut [SemanticNode], dom: &DomIndex, layout: &LayoutIndex) {
    // Most layout nodes are ordinary in-flow content and can never cover another
    // control under this conservative occlusion model. Filter them once instead
    // of running a pair of DOM ancestry walks for every semantic/layout pair.
    let overlay_candidates = page_occlusion_candidates(layout);
    for node in nodes {
        if node.visibility != BrowserVisibility::InViewport {
            continue;
        }
        let Some(target_backend) = node.backend_node_id else {
            continue;
        };
        let Some(target) = layout.nodes.get(&target_backend) else {
            continue;
        };
        let (Some(target_bounds), Some(target_paint)) = (target.bounds, target.paint_order) else {
            continue;
        };
        let covered = overlay_candidates.iter().any(|(backend, overlay)| {
            let backend = *backend;
            if backend == target_backend
                || overlay
                    .paint_order
                    .is_none_or(|paint| paint <= target_paint)
            {
                return false;
            }
            let covers = overlay
                .bounds
                .is_some_and(|bounds| bounds.covers(target_bounds));
            covers && !dom.shares_dom_branch(backend, target_backend)
        });
        if covered {
            node.visibility = BrowserVisibility::PageOccluded;
        }
    }
}

fn page_occlusion_candidates(layout: &LayoutIndex) -> Vec<(i64, &LayoutMeta)> {
    layout
        .nodes
        .iter()
        .filter_map(|(&backend, overlay)| {
            let positioned = overlay
                .styles
                .get("position")
                .is_some_and(|position| matches!(position.as_str(), "fixed" | "absolute"));
            let accepts_pointer_events = !overlay
                .styles
                .get("pointer-events")
                .is_some_and(|value| value == "none");
            let has_bounds = overlay.bounds.is_some_and(Rect::has_area);
            (positioned
                && accepts_pointer_events
                && !layout_hidden(overlay)
                && overlay.paint_order.is_some()
                && has_bounds)
                .then_some((backend, overlay))
        })
        .collect()
}

fn supplement_dom_actions(
    nodes: &mut Vec<SemanticNode>,
    dom: &DomIndex,
    layout: &LayoutIndex,
    viewport: &Viewport,
    frame: &FrameRef,
) {
    let existing = nodes
        .iter()
        .filter_map(|node| node.backend_node_id)
        .collect::<HashSet<_>>();
    let expected_frame = frame
        .identity
        .as_ref()
        .map(|identity| identity.frame_id.as_str());
    let by_backend = nodes
        .iter()
        .filter_map(|node| Some((node.backend_node_id?, node.ax_id.clone())))
        .collect::<HashMap<_, _>>();
    let mut candidates = dom.nodes.iter().collect::<Vec<_>>();
    candidates.sort_by_key(|(_, meta)| meta.order);
    for (&backend_node_id, meta) in candidates {
        if existing.contains(&backend_node_id)
            || expected_frame.is_some_and(|expected| meta.frame_id.as_deref() != Some(expected))
        {
            continue;
        }
        let layout_meta = layout.nodes.get(&backend_node_id);
        let visibility = classify_visibility(Some(meta), layout_meta, viewport);
        if matches!(
            visibility,
            BrowserVisibility::CssHidden | BrowserVisibility::PageOccluded
        ) {
            continue;
        }
        let role = meta
            .attrs
            .get("role")
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_else(|| match meta.tag.as_str() {
                "a" => "link".to_owned(),
                "button" => "button".to_owned(),
                "input" => match meta.attrs.get("type").map(String::as_str) {
                    Some("checkbox") => "checkbox".to_owned(),
                    Some("radio") => "radio".to_owned(),
                    Some("button" | "submit" | "reset" | "image") => "button".to_owned(),
                    Some("range") => "slider".to_owned(),
                    _ => "textbox".to_owned(),
                },
                "textarea" => "textbox".to_owned(),
                "select" => "combobox".to_owned(),
                "option" => "option".to_owned(),
                "summary" => "summary".to_owned(),
                _ => "generic".to_owned(),
            });
        let mut states = BTreeMap::new();
        if meta.attrs.contains_key("disabled") {
            states.insert("disabled".to_owned(), Value::Bool(true));
        }
        let actions = action_kinds(&role, Some(meta), &states, layout_meta);
        if actions.is_empty() {
            continue;
        }
        let name = ["aria-label", "placeholder", "title", "name", "id"]
            .iter()
            .find_map(|key| meta.attrs.get(*key).cloned())
            .and_then(clean_semantic_text);
        let destination_url = (role == "link")
            .then(|| meta.destination_url.clone())
            .flatten();
        nodes.push(SemanticNode {
            ax_id: format!("dom-{backend_node_id}"),
            parent_ax_id: meta
                .parent_backend_node_id
                .and_then(|parent| by_backend.get(&parent).cloned()),
            child_ax_ids: Vec::new(),
            backend_node_id: Some(backend_node_id),
            role,
            name,
            value: meta
                .attrs
                .get("value")
                .cloned()
                .and_then(clean_semantic_text),
            destination_url,
            states,
            frame: frame.clone(),
            visibility,
            actions,
            document_order: meta.order,
        });
    }
}

fn attributes(node: &Value) -> HashMap<String, String> {
    node.get("attributes")
        .and_then(Value::as_array)
        .map(|attrs| {
            attrs
                .chunks_exact(2)
                .filter_map(|pair| {
                    Some((
                        pair[0].as_str()?.to_ascii_lowercase(),
                        pair[1].as_str()?.to_owned(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn statically_hidden(attrs: &HashMap<String, String>) -> bool {
    if attrs.contains_key("hidden") || attrs.get("aria-hidden").is_some_and(|v| v == "true") {
        return true;
    }
    let style = attrs
        .get("style")
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    style.contains("display:none")
        || style.contains("display: none")
        || style.contains("visibility:hidden")
        || style.contains("visibility: hidden")
        || style.contains("opacity:0")
        || style.contains("opacity: 0")
}

fn layout_hidden(layout: &LayoutMeta) -> bool {
    layout
        .styles
        .get("display")
        .is_some_and(|value| value.eq_ignore_ascii_case("none"))
        || layout
            .styles
            .get("visibility")
            .is_some_and(|value| value.eq_ignore_ascii_case("hidden"))
        || layout
            .styles
            .get("opacity")
            .and_then(|value| value.parse::<f64>().ok())
            .is_some_and(|opacity| opacity <= 0.0)
}

fn classify_visibility(
    dom: Option<&DomMeta>,
    layout: Option<&LayoutMeta>,
    viewport: &Viewport,
) -> BrowserVisibility {
    if dom.is_some_and(|meta| meta.css_hidden) || layout.is_some_and(layout_hidden) {
        return BrowserVisibility::CssHidden;
    }
    let Some(layout) = layout else {
        return BrowserVisibility::Unknown;
    };
    let Some(bounds) = layout.bounds else {
        return BrowserVisibility::NoLayout;
    };
    if !bounds.has_area() {
        return BrowserVisibility::NoLayout;
    }
    let Some(viewport) = viewport.rect else {
        return BrowserVisibility::Unknown;
    };
    if bounds.intersects(viewport) {
        BrowserVisibility::InViewport
    } else if bounds.intersects(viewport.expanded(NEAR_VIEWPORT_MARGIN)) {
        BrowserVisibility::NearViewport
    } else {
        BrowserVisibility::Offscreen
    }
}

fn action_kinds(
    role: &str,
    dom: Option<&DomMeta>,
    states: &BTreeMap<String, Value>,
    layout: Option<&LayoutMeta>,
) -> Vec<BrowserActionKind> {
    if states.get("disabled").and_then(Value::as_bool) == Some(true) {
        return Vec::new();
    }
    let mut actions = Vec::new();
    if matches!(
        role,
        "button"
            | "link"
            | "checkbox"
            | "radio"
            | "switch"
            | "tab"
            | "menuitem"
            | "menuitemcheckbox"
            | "menuitemradio"
            | "option"
            | "treeitem"
            | "slider"
            | "spinbutton"
            | "combobox"
            | "listbox"
            | "summary"
    ) {
        actions.push(BrowserActionKind::Click);
    }
    let tag = dom.map(|meta| meta.tag.as_str()).unwrap_or("");
    let editable = states.get("editable").is_some_and(|value| {
        value.as_bool() == Some(true) || value.as_str().is_some_and(|value| value != "false")
    }) || dom.is_some_and(|meta| {
        meta.attrs
            .get("contenteditable")
            .is_some_and(|value| value.is_empty() || value.eq_ignore_ascii_case("true"))
    });
    let file_input = tag == "input"
        && dom.is_some_and(|meta| {
            meta.attrs
                .get("type")
                .is_some_and(|value| value.eq_ignore_ascii_case("file"))
        });
    if file_input {
        actions.push(BrowserActionKind::Upload);
    } else if matches!(role, "textbox" | "searchbox")
        || matches!(tag, "input" | "textarea")
        || editable
    {
        actions.push(BrowserActionKind::Type);
    }
    if actions.is_empty()
        && dom.is_some_and(|meta| {
            meta.attrs.contains_key("onclick")
                || meta
                    .attrs
                    .get("tabindex")
                    .is_some_and(|value| value != "-1")
                || (!matches!(meta.tag.as_str(), "html" | "body")
                    && layout.is_some_and(|layout| {
                        layout
                            .styles
                            .get("cursor")
                            .is_some_and(|cursor| cursor.eq_ignore_ascii_case("pointer"))
                            && !layout
                                .styles
                                .get("pointer-events")
                                .is_some_and(|value| value.eq_ignore_ascii_case("none"))
                    }))
        })
        && layout.is_some_and(|meta| meta.bounds.is_some_and(Rect::has_area))
    {
        actions.push(BrowserActionKind::Click);
    }
    if let Some(layout) = layout.filter(|meta| {
        meta.bounds.is_some_and(Rect::has_area)
            && !meta
                .styles
                .get("pointer-events")
                .is_some_and(|value| value.eq_ignore_ascii_case("none"))
    }) {
        if !actions.is_empty() {
            actions.push(BrowserActionKind::Pointer);
        }
        if layout_is_scrollable(layout) || (tag == "body" && layout_has_scroll_extent(layout)) {
            actions.push(BrowserActionKind::Scroll);
        }
    }
    actions
}

fn layout_is_scrollable(layout: &LayoutMeta) -> bool {
    let (Some(client), Some(scroll)) = (layout.client_rect, layout.scroll_rect) else {
        return false;
    };
    let scrollable_axis = |overflow: Option<&String>, scroll_extent: f64, client_extent: f64| {
        overflow.is_some_and(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "auto" | "scroll" | "overlay"
            )
        }) && scroll_extent > client_extent + 0.5
    };
    scrollable_axis(layout.styles.get("overflow-x"), scroll.width, client.width)
        || scrollable_axis(
            layout.styles.get("overflow-y"),
            scroll.height,
            client.height,
        )
}

fn layout_has_scroll_extent(layout: &LayoutMeta) -> bool {
    let (Some(client), Some(scroll)) = (layout.client_rect, layout.scroll_rect) else {
        return false;
    };
    scroll.width > client.width + 0.5 || scroll.height > client.height + 0.5
}

fn ax_states(node: &Value) -> BTreeMap<String, Value> {
    const KEPT: &[&str] = &[
        "checked",
        "disabled",
        "editable",
        "expanded",
        "focused",
        "focusable",
        "pressed",
        "required",
        "selected",
    ];
    node.get("properties")
        .and_then(Value::as_array)
        .map(|properties| {
            properties
                .iter()
                .filter_map(|property| {
                    let name = property.get("name").and_then(Value::as_str)?;
                    KEPT.contains(&name).then(|| {
                        (
                            name.to_owned(),
                            property
                                .pointer("/value/value")
                                .cloned()
                                .unwrap_or(Value::Null),
                        )
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn ax_value_string(value: Option<&Value>) -> Option<String> {
    let value = value?.get("value")?;
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn clean_semantic_text(value: String) -> Option<String> {
    let normalized = value
        .chars()
        .map(|ch| match ch {
            '\u{feff}' | '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2060}' | '\u{00a0}'
            | '\u{2007}' | '\u{202f}' => ' ',
            '\u{e000}'..='\u{f8ff}' => ' ',
            _ => ch,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        return None;
    }
    Some(normalized.chars().take(MAX_SEMANTIC_TEXT_CHARS).collect())
}

fn remove_redundant_static_text(nodes: &mut Vec<SemanticNode>) {
    let mut by_ax_id: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut inbound: HashMap<&str, Vec<usize>> = HashMap::new();
    for (idx, node) in nodes.iter().enumerate() {
        by_ax_id.entry(&node.ax_id).or_default().push(idx);
        for child in &node.child_ax_ids {
            inbound.entry(child).or_default().push(idx);
        }
    }

    let removable = nodes
        .iter()
        .enumerate()
        .filter_map(|(idx, node)| {
            if !matches!(node.role.as_str(), "statictext" | "text")
                || !node.child_ax_ids.is_empty()
                || by_ax_id.get(node.ax_id.as_str()).map(Vec::as_slice) != Some(&[idx])
            {
                return None;
            }

            let parent_idx = match node.parent_ax_id.as_deref() {
                None if inbound.get(node.ax_id.as_str()).is_none() => None,
                Some(parent_id) if !is_unresolved_ax_id(parent_id) => {
                    let [parent_idx] = by_ax_id.get(parent_id)?.as_slice() else {
                        return None;
                    };
                    let [inbound_idx] = inbound.get(node.ax_id.as_str())?.as_slice() else {
                        return None;
                    };
                    if parent_idx != inbound_idx {
                        return None;
                    }
                    Some(*parent_idx)
                }
                None | Some(_) => return None,
            };

            let redundant = node.name.is_none()
                || parent_idx.is_some_and(|parent_idx| {
                    nodes[parent_idx].name.as_deref() == node.name.as_deref()
                });
            redundant.then_some(node.ax_id.clone())
        })
        .collect::<HashSet<_>>();

    if removable.is_empty() {
        return;
    }
    for node in nodes.iter_mut() {
        node.child_ax_ids.retain(|child| !removable.contains(child));
    }
    nodes.retain(|node| !removable.contains(&node.ax_id));
}

fn rank(node: &SemanticNode) -> u8 {
    let priority_context = node.states.get("focused").and_then(Value::as_bool) == Some(true)
        || matches!(node.role.as_str(), "dialog" | "alertdialog");
    if priority_context {
        return 0;
    }
    match (node.visibility, node.actions.is_empty()) {
        (BrowserVisibility::InViewport, false) => 1,
        (BrowserVisibility::InViewport, true) => 2,
        (BrowserVisibility::NearViewport, false) => 3,
        (BrowserVisibility::NearViewport, true) => 4,
        (BrowserVisibility::Unknown | BrowserVisibility::NoLayout, false) => 5,
        (BrowserVisibility::Unknown | BrowserVisibility::NoLayout, true) => 6,
        (BrowserVisibility::Offscreen, false) => 7,
        (BrowserVisibility::Offscreen, true) => 8,
        (BrowserVisibility::CssHidden | BrowserVisibility::PageOccluded, _) => 9,
    }
}

struct ScopedIndices {
    indices: Vec<usize>,
    hierarchy_complete: bool,
}

fn scoped_indices(
    nodes: &[SemanticNode],
    query: Option<&str>,
    scope_backend_node_id: Option<i64>,
) -> ScopedIndices {
    let by_ax_id = unique_ax_indices(nodes);
    let unproven_ax_ids = nodes
        .iter()
        .filter(|node| semantic_ax_key(&node.frame, &node.ax_id).is_none())
        .map(|node| node.ax_id.as_str())
        .collect::<HashSet<_>>();
    let mut allowed = HashSet::new();
    let mut hierarchy_complete = true;
    if let Some(scope_backend) = scope_backend_node_id {
        let mut matches = nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.backend_node_id == Some(scope_backend));
        let first = matches.next().map(|(idx, _)| idx);
        let duplicate = matches.next().is_some();
        if duplicate {
            // Backend ids are not a cross-frame authority. Refuse to choose an
            // arbitrary subtree when merged documents contain a collision.
            hierarchy_complete = false;
        } else if let Some(scope_idx) = first {
            let mut stack = vec![scope_idx];
            while let Some(idx) = stack.pop() {
                if !allowed.insert(idx) {
                    continue;
                }
                for child_id in &nodes[idx].child_ax_ids {
                    match scoped_child_index(
                        &by_ax_id,
                        &unproven_ax_ids,
                        &nodes[idx].frame,
                        child_id,
                    ) {
                        AxLookup::Unique(child_idx) => stack.push(child_idx),
                        // Missing children can be nodes deliberately ignored by
                        // collection and do not make the selected outline false.
                        AxLookup::Missing => {}
                        AxLookup::Ambiguous | AxLookup::Unproven => {
                            hierarchy_complete = false;
                        }
                    }
                }
            }
        }
    }
    let query = query.map(|value| value.trim().to_ascii_lowercase());
    let in_scope = |idx: usize| scope_backend_node_id.is_none() || allowed.contains(&idx);
    let term_count = query
        .as_deref()
        .map_or(0, |query| query_terms(query).count());
    let exact_match_exists = query.as_ref().is_some_and(|query| {
        !query.is_empty()
            && nodes.iter().enumerate().any(|(idx, node)| {
                in_scope(idx)
                    && !matches!(
                        node.visibility,
                        BrowserVisibility::CssHidden | BrowserVisibility::PageOccluded
                    )
                    && node_contains_query(node, query)
            })
    });
    let indices = nodes
        .iter()
        .enumerate()
        .filter(|(idx, node)| {
            in_scope(*idx)
                && query.as_ref().is_none_or(|query| {
                    query.is_empty()
                        || if exact_match_exists {
                            node_contains_query(node, query)
                                // A phrase is preferred by ranking, but must not
                                // erase a result containing all reordered terms.
                                || (term_count > 0 && query_score(node, query) == term_count)
                        } else {
                            query_score(node, query) > 0
                        }
                })
        })
        .map(|(idx, _)| idx)
        .collect();
    ScopedIndices {
        indices,
        hierarchy_complete,
    }
}

fn node_contains_query(node: &SemanticNode, query: &str) -> bool {
    node.role.to_ascii_lowercase().contains(query)
        || node
            .name
            .as_ref()
            .is_some_and(|name| name.to_ascii_lowercase().contains(query))
        || node
            .value
            .as_ref()
            .is_some_and(|value| value.to_ascii_lowercase().contains(query))
}

fn query_terms(query: &str) -> impl Iterator<Item = &str> {
    query
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.is_empty())
}

fn query_score(node: &SemanticNode, query: &str) -> usize {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return 0;
    }
    if node_contains_query(node, &query) {
        return usize::MAX;
    }
    let fields = [
        Some(node.role.as_str()),
        node.name.as_deref(),
        node.value.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::to_ascii_lowercase)
    .collect::<Vec<_>>();
    query_terms(&query)
        .filter(|term| fields.iter().any(|field| field.contains(term)))
        .count()
}

fn with_ancestors(nodes: &[SemanticNode], selected: &[usize]) -> HashSet<usize> {
    let by_ax_id = unique_ax_indices(nodes);
    let mut keep: HashSet<usize> = selected.iter().copied().collect();
    for idx in selected {
        // A cyclic or ambiguous ancestry chain cannot truthfully supply
        // context. The page path filters it; callers of this helper get no
        // invented partial ancestry.
        if let Some(ancestors) = ancestor_indices(nodes, &by_ax_id, *idx) {
            keep.extend(ancestors);
        }
    }
    keep
}

fn render_outline(nodes: &[SemanticNode], selected: &HashSet<usize>) -> String {
    let mut ordered: Vec<usize> = selected.iter().copied().collect();
    ordered.sort_by(|left, right| {
        nodes[*left]
            .document_order
            .cmp(&nodes[*right].document_order)
            .then_with(|| nodes[*left].ax_id.cmp(&nodes[*right].ax_id))
    });
    render_outline_ordered(nodes, selected, ordered)
}

fn render_outline_ordered(
    nodes: &[SemanticNode],
    selected: &HashSet<usize>,
    ordered: impl IntoIterator<Item = usize>,
) -> String {
    let by_ax_id = unique_ax_indices(nodes);
    let mut lines = Vec::new();
    for idx in ordered {
        if !selected.contains(&idx) {
            continue;
        }
        let node = &nodes[idx];
        if matches!(node.role.as_str(), "rootwebarea" | "webarea") {
            continue;
        }
        // Page selection already omits cyclic nodes. Retain that fail-closed
        // behavior if this renderer is ever called with an unchecked set.
        let Some(ancestors) = ancestor_indices(nodes, &by_ax_id, idx) else {
            continue;
        };
        let depth = ancestors
            .into_iter()
            .filter(|parent_idx| {
                selected.contains(parent_idx)
                    && !matches!(nodes[*parent_idx].role.as_str(), "rootwebarea" | "webarea")
            })
            .count();
        let mut line = format!("{}- {}", "  ".repeat(depth), node.role);
        if let Some(name) = &node.name {
            line.push(' ');
            line.push_str(&serde_json::to_string(name).unwrap_or_else(|_| "\"\"".to_owned()));
        }
        if let Some(value) = &node.value {
            if node.name.as_deref() != Some(value) {
                line.push_str(": ");
                line.push_str(value);
            }
        }
        if !node.states.is_empty() {
            let states = node
                .states
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join(",");
            line.push_str(&format!(" [{states}]"));
        }
        lines.push(line);
    }
    lines.join("\n")
}

type SemanticAxKey<'a> = (
    &'static str,
    Option<&'a str>,
    Option<&'a str>,
    Option<&'a str>,
    &'a str,
);

fn semantic_ax_key<'a>(frame: &'a FrameRef, ax_id: &'a str) -> Option<SemanticAxKey<'a>> {
    let identity = frame.identity.as_ref();
    let authority_is_proven = match frame.kind {
        FrameKind::Main => frame.oopif_target_id.is_none(),
        FrameKind::Iframe => identity.is_some() && frame.oopif_target_id.is_none(),
        FrameKind::Oopif => identity.is_some() && frame.oopif_target_id.is_some(),
    };
    if !authority_is_proven {
        return None;
    }
    Some((
        frame.kind.as_str(),
        frame.oopif_target_id.as_deref(),
        identity.map(|identity| identity.frame_id.as_str()),
        identity.map(|identity| identity.loader_id.as_str()),
        ax_id,
    ))
}

fn is_context_group_role(role: &str) -> bool {
    matches!(
        role,
        "document"
            | "rootwebarea"
            | "webarea"
            | "main"
            | "article"
            | "region"
            | "feed"
            | "list"
            | "listitem"
            | "table"
            | "rowgroup"
            | "row"
    )
}

fn is_transparent_generic(node: &SemanticNode) -> bool {
    node.role == "generic"
        && node.name.as_deref().is_none_or(str::is_empty)
        && node.value.as_deref().is_none_or(str::is_empty)
        && node.destination_url.as_deref().is_none_or(str::is_empty)
        && node.states.is_empty()
        && node.actions.is_empty()
}

fn cached_context_groups(
    nodes: &[SemanticNode],
    by_ax_id: &HashMap<SemanticAxKey<'_>, Option<usize>>,
    start: usize,
    cache: &mut HashMap<usize, Option<Vec<usize>>>,
) -> Option<Vec<usize>> {
    if let Some(cached) = cache.get(&start) {
        return cached.clone();
    }
    let mut path = Vec::new();
    let mut seen = HashSet::new();
    let mut cursor = start;
    let mut suffix = loop {
        if let Some(cached) = cache.get(&cursor) {
            break cached.clone();
        }
        if !seen.insert(cursor) {
            break None;
        }
        path.push(cursor);
        let Some(parent_id) = nodes[cursor].parent_ax_id.as_deref() else {
            break Some(Vec::new());
        };
        match ax_lookup(by_ax_id, &nodes[cursor].frame, parent_id) {
            AxLookup::Unique(parent) => cursor = parent,
            AxLookup::Missing => break Some(Vec::new()),
            AxLookup::Ambiguous | AxLookup::Unproven => break None,
        }
    };
    while let Some(idx) = path.pop() {
        suffix = suffix.map(|mut groups| {
            if is_context_group_role(&nodes[idx].role) {
                groups.insert(0, idx);
                // Automatic query context needs only the nearest group and
                // its enclosing group. Bounding every cached suffix avoids
                // quadratic cloning on deeply nested structural documents.
                groups.truncate(2);
            }
            groups
        });
        cache.insert(idx, suffix.clone());
    }
    cache.get(&start).cloned().flatten()
}

fn unique_ax_indices(nodes: &[SemanticNode]) -> HashMap<SemanticAxKey<'_>, Option<usize>> {
    let mut indices = HashMap::new();
    for (idx, node) in nodes.iter().enumerate() {
        let Some(key) = semantic_ax_key(&node.frame, &node.ax_id) else {
            continue;
        };
        indices
            .entry(key)
            .and_modify(|existing| *existing = None)
            .or_insert(Some(idx));
    }
    indices
}

enum AxLookup {
    Unique(usize),
    Missing,
    Ambiguous,
    Unproven,
}

fn ax_lookup(
    indices: &HashMap<SemanticAxKey<'_>, Option<usize>>,
    frame: &FrameRef,
    ax_id: &str,
) -> AxLookup {
    let Some(key) = semantic_ax_key(frame, ax_id) else {
        return AxLookup::Unproven;
    };
    match indices.get(&key) {
        Some(Some(idx)) => AxLookup::Unique(*idx),
        Some(None) => AxLookup::Ambiguous,
        None => AxLookup::Missing,
    }
}

fn scoped_child_index(
    indices: &HashMap<SemanticAxKey<'_>, Option<usize>>,
    unproven_ax_ids: &HashSet<&str>,
    frame: &FrameRef,
    ax_id: &str,
) -> AxLookup {
    if is_unresolved_ax_id(ax_id) {
        return AxLookup::Unproven;
    }
    match ax_lookup(indices, frame, ax_id) {
        AxLookup::Missing if unproven_ax_ids.contains(ax_id) => AxLookup::Unproven,
        lookup => lookup,
    }
}

/// Return the unique, same-frame ancestry for one node. `None` means the
/// declared parent graph is cyclic or ambiguous. A dangling parent terminates
/// the chain without guessing. The visited set bounds traversal by node count.
fn ancestor_indices(
    nodes: &[SemanticNode],
    by_ax_id: &HashMap<SemanticAxKey<'_>, Option<usize>>,
    start_idx: usize,
) -> Option<Vec<usize>> {
    let start_key = semantic_ax_key(&nodes[start_idx].frame, &nodes[start_idx].ax_id)?;
    if !matches!(by_ax_id.get(&start_key), Some(Some(idx)) if *idx == start_idx) {
        return None;
    }
    let mut ancestors = Vec::new();
    let mut visited = HashSet::from([start_idx]);
    let mut current_idx = start_idx;
    while let Some(parent_id) = nodes[current_idx].parent_ax_id.as_deref() {
        if is_unresolved_ax_id(parent_id) {
            return None;
        }
        let parent_key = semantic_ax_key(&nodes[current_idx].frame, parent_id)?;
        let parent_idx = match by_ax_id.get(&parent_key) {
            None => break,
            Some(None) => return None,
            Some(Some(parent_idx)) => *parent_idx,
        };
        if !visited.insert(parent_idx) {
            return None;
        }
        ancestors.push(parent_idx);
        current_idx = parent_idx;
    }
    Some(ancestors)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn file_inputs_expose_upload_instead_of_text_typing() {
        let dom = DomMeta {
            tag: "input".into(),
            attrs: HashMap::from([("type".into(), "file".into())]),
            destination_url: None,
            order: 0,
            css_hidden: false,
            parent_backend_node_id: None,
            frame_id: None,
        };
        assert_eq!(
            action_kinds("textbox", Some(&dom), &BTreeMap::new(), None),
            vec![BrowserActionKind::Upload]
        );
    }

    #[test]
    fn link_destinations_are_resolved_and_limited_to_observational_schemes() {
        let base = Url::parse("https://fixture.test/inbox/item-2").unwrap();
        assert_eq!(
            resolve_link_destination("../archive?view=all#today", Some(&base)).as_deref(),
            Some("https://fixture.test/archive?view=all#today")
        );
        assert_eq!(
            resolve_link_destination("mailto:help@example.test", Some(&base)).as_deref(),
            Some("mailto:help@example.test")
        );
        assert_eq!(
            resolve_link_destination("about:blank", Some(&base)).as_deref(),
            Some("about:blank")
        );
        for unsafe_destination in [
            "javascript:alert(1)",
            "data:text/plain,secret",
            "file:///etc/passwd",
            "https://user:password@example.test/private",
        ] {
            assert_eq!(
                resolve_link_destination(unsafe_destination, Some(&base)),
                None,
                "unexpected destination metadata for {unsafe_destination}"
            );
        }
    }

    #[test]
    fn only_semantic_links_inherit_dom_destination_metadata() {
        let dom = build_dom_index(&json!({
            "nodeType": 9,
            "baseURL": "https://fixture.test/base/",
            "children": [
                {"nodeType": 1, "nodeName": "A", "backendNodeId": 1,
                 "attributes": ["href", "reports/latest"]},
                {"nodeType": 1, "nodeName": "BUTTON", "backendNodeId": 2,
                 "attributes": ["href", "should-not-surface"]}
            ]
        }));
        let document = compose_accessibility_tree(
            &json!({"nodes": [
                {"nodeId": "link", "ignored": false, "backendDOMNodeId": 1,
                 "role": {"value": "link"}, "name": {"value": "Latest report"}},
                {"nodeId": "button", "ignored": false, "backendDOMNodeId": 2,
                 "role": {"value": "button"}, "name": {"value": "Odd button"}}
            ]}),
            &dom,
            &LayoutIndex::default(),
            &Viewport::default(),
            frame(),
        );
        let link = document
            .nodes
            .iter()
            .find(|node| node.ax_id == "link")
            .unwrap();
        let button = document
            .nodes
            .iter()
            .find(|node| node.ax_id == "button")
            .unwrap();
        assert_eq!(
            link.destination_url.as_deref(),
            Some("https://fixture.test/base/reports/latest")
        );
        assert_eq!(button.destination_url, None);
    }
    use crate::browser::store::{FrameIdentity, FrameKind, FrameRef};

    fn frame() -> FrameRef {
        FrameRef::main_unproven()
    }

    fn identified_frame(frame_id: &str, loader_id: &str) -> FrameRef {
        FrameRef {
            kind: FrameKind::Iframe,
            oopif_target_id: None,
            identity: Some(FrameIdentity {
                frame_id: frame_id.to_owned(),
                loader_id: loader_id.to_owned(),
            }),
        }
    }

    fn identified_main_frame() -> FrameRef {
        FrameRef {
            kind: FrameKind::Main,
            oopif_target_id: None,
            identity: Some(FrameIdentity {
                frame_id: "main".into(),
                loader_id: "loader".into(),
            }),
        }
    }

    fn oopif_frame(target_id: &str, frame_id: &str, loader_id: &str) -> FrameRef {
        FrameRef {
            kind: FrameKind::Oopif,
            oopif_target_id: Some(target_id.to_owned()),
            identity: Some(FrameIdentity {
                frame_id: frame_id.to_owned(),
                loader_id: loader_id.to_owned(),
            }),
        }
    }

    fn query_fixture(labels: &[(&str, BrowserVisibility)]) -> SemanticDocument {
        SemanticDocument {
            nodes: labels
                .iter()
                .enumerate()
                .map(|(order, (label, visibility))| SemanticNode {
                    ax_id: format!("node-{order}"),
                    parent_ax_id: None,
                    child_ax_ids: Vec::new(),
                    backend_node_id: Some(order as i64 + 1),
                    role: "link".into(),
                    name: Some((*label).into()),
                    value: None,
                    destination_url: None,
                    states: BTreeMap::new(),
                    frame: frame(),
                    visibility: *visibility,
                    actions: vec![BrowserActionKind::Click],
                    document_order: order,
                })
                .collect(),
            complete: true,
            ..Default::default()
        }
    }

    fn query_names(page: &SemanticPage) -> Vec<&str> {
        page.selected
            .iter()
            .filter_map(|n| n.name.as_deref())
            .collect()
    }

    #[test]
    fn semantic_query_context_includes_unmatched_sibling_qualifiers_in_group_order() {
        let ax = json!({"nodes": [
            {"nodeId":"root","ignored":false,"role":{"value":"RootWebArea"},"childIds":["main"]},
            {"nodeId":"main","parentId":"root","ignored":false,"role":{"value":"main"},"childIds":["list"]},
            {"nodeId":"list","parentId":"main","ignored":false,"role":{"value":"list"},"childIds":["one","two","three"]},
            {"nodeId":"one","parentId":"list","ignored":false,"role":{"value":"listitem"},"childIds":["s1","p1"]},
            {"nodeId":"s1","parentId":"one","ignored":false,"role":{"value":"generic"},"name":{"value":"Sponsored"},"childIds":[]},
            {"nodeId":"p1","parentId":"one","ignored":false,"backendDOMNodeId":11,"role":{"value":"link"},"name":{"value":"Alpha vertical mouse"},"childIds":[]},
            {"nodeId":"two","parentId":"list","ignored":false,"role":{"value":"listitem"},"childIds":["s2","p2"]},
            {"nodeId":"s2","parentId":"two","ignored":false,"role":{"value":"generic"},"name":{"value":"Sponsored"},"childIds":[]},
            {"nodeId":"p2","parentId":"two","ignored":false,"backendDOMNodeId":12,"role":{"value":"link"},"name":{"value":"Beta vertical mouse"},"childIds":[]},
            {"nodeId":"three","parentId":"list","ignored":false,"role":{"value":"listitem"},"childIds":["p3"]},
            {"nodeId":"p3","parentId":"three","ignored":false,"backendDOMNodeId":13,"role":{"value":"link"},"name":{"value":"Gamma vertical mouse"},"childIds":[]}
        ]});
        let document = compose_accessibility_tree(
            &ax,
            &DomIndex::default(),
            &LayoutIndex::default(),
            &Viewport::default(),
            identified_main_frame(),
        );
        let page = document.page(
            0,
            DEFAULT_SEMANTIC_NODE_BUDGET,
            Some("vertical mouse"),
            None,
        );
        assert!(page
            .selected
            .iter()
            .all(|node| node.name.as_deref() != Some("Sponsored")));
        let contexts = document.query_contexts(&page);
        assert_eq!(contexts.len(), 1);
        let context = &contexts[0];
        assert_eq!(context.group.role, "list");
        assert!(context.group_complete);
        assert_eq!(context.before_omitted, 0);
        assert_eq!(context.after_omitted, 0);
        let outline = &context.outline;
        assert!(
            outline.find("Sponsored") < outline.find("Alpha vertical mouse"),
            "{outline}"
        );
        assert!(
            outline.find("Alpha vertical mouse") < outline.find("Beta vertical mouse"),
            "{outline}"
        );
        assert!(
            outline.find("Beta vertical mouse") < outline.find("Gamma vertical mouse"),
            "{outline}"
        );
        assert!(context
            .nodes
            .iter()
            .any(|node| node.name.as_deref() == Some("Sponsored")));
    }

    #[test]
    fn semantic_query_context_covers_late_repeated_matches_in_one_peer_list() {
        let mut document = context_fixture(0);
        document.nodes[1].child_ax_ids = (0..40).map(|index| format!("item-{index}")).collect();
        for index in 0..40 {
            let item_id = format!("item-{index}");
            let link_id = format!("link-{index}");
            let qualifier_id = format!("qualifier-{index}");
            let matched = matches!(index, 2 | 30);
            document.nodes.push(SemanticNode {
                ax_id: item_id.clone(),
                parent_ax_id: Some("node-1".into()),
                child_ax_ids: if matched {
                    vec![qualifier_id.clone(), link_id.clone()]
                } else {
                    vec![link_id.clone()]
                },
                backend_node_id: None,
                role: "listitem".into(),
                name: None,
                value: None,
                destination_url: None,
                states: BTreeMap::new(),
                frame: identified_main_frame(),
                visibility: BrowserVisibility::InViewport,
                actions: Vec::new(),
                document_order: index * 3 + 2,
            });
            if matched {
                document.nodes.push(SemanticNode {
                    ax_id: qualifier_id,
                    parent_ax_id: Some(item_id.clone()),
                    child_ax_ids: Vec::new(),
                    backend_node_id: None,
                    role: "generic".into(),
                    name: Some(format!("qualifier {index}")),
                    value: None,
                    destination_url: None,
                    states: BTreeMap::new(),
                    frame: identified_main_frame(),
                    visibility: BrowserVisibility::InViewport,
                    actions: Vec::new(),
                    document_order: index * 3 + 3,
                });
            }
            document.nodes.push(SemanticNode {
                ax_id: link_id,
                parent_ax_id: Some(item_id),
                child_ax_ids: Vec::new(),
                backend_node_id: Some(1_000 + index as i64),
                role: "link".into(),
                name: Some(if matched {
                    "Repeated match".into()
                } else {
                    format!("ordinary peer {index}")
                }),
                value: None,
                destination_url: None,
                states: BTreeMap::new(),
                frame: identified_main_frame(),
                visibility: BrowserVisibility::InViewport,
                actions: vec![BrowserActionKind::Click],
                document_order: index * 3 + 4,
            });
        }

        let page = document.page(
            0,
            DEFAULT_SEMANTIC_NODE_BUDGET,
            Some("Repeated match"),
            None,
        );
        let contexts = document.query_contexts(&page);

        assert_eq!(contexts.len(), 2);
        assert_eq!(contexts[0].group.ax_id, "node-1");
        assert_eq!(contexts[1].group.ax_id, "node-1");
        assert_eq!(contexts[0].anchor.ax_id, "link-2");
        assert_eq!(contexts[1].anchor.ax_id, "link-30");
        assert!(contexts[0].range_start < contexts[1].range_start);
        assert!(contexts[0].outline.contains("qualifier 2"));
        assert!(contexts[1].outline.contains("qualifier 30"));
        for context in &contexts {
            assert!(context
                .nodes
                .iter()
                .any(|node| node.identity() == context.anchor.identity()));
        }
        assert!(
            contexts
                .iter()
                .map(|context| context.nodes.len())
                .sum::<usize>()
                <= QUERY_CONTEXT_MAX_NODES
        );
        assert!(
            contexts
                .iter()
                .map(|context| context.outline.len())
                .sum::<usize>()
                <= QUERY_CONTEXT_OUTLINE_MAX_BYTES
        );
    }

    #[test]
    fn semantic_query_context_reaches_late_match_in_a_second_article() {
        let mut document = context_fixture(0);
        document.nodes[1].role = "main".into();
        document.nodes[1].child_ax_ids = std::iter::once("article-early".into())
            .chain((0..35).map(|index| format!("filler-{index}")))
            .chain(std::iter::once("article-late".into()))
            .collect();
        for (article, order) in [("early", 2), ("late", 100)] {
            document.nodes.push(SemanticNode {
                ax_id: format!("article-{article}"),
                parent_ax_id: Some("node-1".into()),
                child_ax_ids: vec![format!("heading-{article}")],
                backend_node_id: None,
                role: "article".into(),
                name: Some(format!("{article} article")),
                value: None,
                destination_url: None,
                states: BTreeMap::new(),
                frame: identified_main_frame(),
                visibility: BrowserVisibility::InViewport,
                actions: Vec::new(),
                document_order: order,
            });
            document.nodes.push(SemanticNode {
                ax_id: format!("heading-{article}"),
                parent_ax_id: Some(format!("article-{article}")),
                child_ax_ids: Vec::new(),
                backend_node_id: Some(2_000 + order as i64),
                role: "heading".into(),
                name: Some(format!("release marker {article}")),
                value: None,
                destination_url: None,
                states: BTreeMap::new(),
                frame: identified_main_frame(),
                visibility: BrowserVisibility::InViewport,
                actions: Vec::new(),
                document_order: order + 1,
            });
        }
        for index in 0..35 {
            document.nodes.push(SemanticNode {
                ax_id: format!("filler-{index}"),
                parent_ax_id: Some("node-1".into()),
                child_ax_ids: Vec::new(),
                backend_node_id: None,
                role: "paragraph".into(),
                name: Some(format!("unmatched filler {index}")),
                value: None,
                destination_url: None,
                states: BTreeMap::new(),
                frame: identified_main_frame(),
                visibility: BrowserVisibility::InViewport,
                actions: Vec::new(),
                document_order: index + 4,
            });
        }

        let page = document.page(
            0,
            DEFAULT_SEMANTIC_NODE_BUDGET,
            Some("release marker"),
            None,
        );
        let contexts = document.query_contexts(&page);

        assert_eq!(contexts.len(), 2);
        assert_eq!(contexts[0].anchor.ax_id, "heading-early");
        assert_eq!(contexts[1].anchor.ax_id, "heading-late");
        assert!(contexts[0].range_end <= contexts[1].range_start);
        assert!(contexts[0].outline.contains("early article"));
        assert!(contexts[1].outline.contains("late article"));
    }

    #[test]
    fn semantic_query_context_marks_coverage_only_after_tight_byte_clipping() {
        let mut document = context_fixture(0);
        document.nodes[1].child_ax_ids = (0..30).map(|index| format!("link-{index}")).collect();
        for index in 0..30 {
            document.nodes.push(SemanticNode {
                ax_id: format!("link-{index}"),
                parent_ax_id: Some("node-1".into()),
                child_ax_ids: Vec::new(),
                backend_node_id: Some(3_000 + index as i64),
                role: "link".into(),
                name: Some(if matches!(index, 2 | 27) {
                    format!("byte-clipped match {index}")
                } else {
                    format!("peer {index} {}", "界".repeat(1_000))
                }),
                value: None,
                destination_url: None,
                states: BTreeMap::new(),
                frame: identified_main_frame(),
                visibility: BrowserVisibility::InViewport,
                actions: vec![BrowserActionKind::Click],
                document_order: index + 2,
            });
        }

        let page = document.page(
            0,
            DEFAULT_SEMANTIC_NODE_BUDGET,
            Some("byte-clipped match"),
            None,
        );
        let contexts = document.query_contexts(&page);

        assert_eq!(contexts.len(), 2);
        assert_eq!(contexts[0].anchor.ax_id, "link-2");
        assert_eq!(contexts[1].anchor.ax_id, "link-27");
        assert!(contexts.iter().all(|context| context
            .nodes
            .iter()
            .any(|node| node.identity() == context.anchor.identity())));
        assert!(
            contexts
                .iter()
                .map(|context| context.outline.len())
                .sum::<usize>()
                <= QUERY_CONTEXT_OUTLINE_MAX_BYTES
        );
        assert!(contexts
            .iter()
            .any(|context| context.nodes.len() < CONTEXT_BEFORE_NODES + 1 + CONTEXT_AFTER_NODES));
    }

    #[test]
    fn semantic_query_context_does_not_cover_malformed_selected_ancestry() {
        let mut document = context_fixture(0);
        document.nodes[1].child_ax_ids = vec!["bad".into()];
        document.nodes.push(SemanticNode {
            ax_id: "bad".into(),
            parent_ax_id: Some("bad".into()),
            child_ax_ids: Vec::new(),
            backend_node_id: Some(9_999),
            role: "link".into(),
            name: Some("malformed match".into()),
            value: None,
            destination_url: None,
            states: BTreeMap::new(),
            frame: identified_main_frame(),
            visibility: BrowserVisibility::InViewport,
            actions: vec![BrowserActionKind::Click],
            document_order: 2,
        });

        let page = document.page(
            0,
            DEFAULT_SEMANTIC_NODE_BUDGET,
            Some("malformed match"),
            None,
        );

        assert!(page.selected.is_empty());
        assert!(!page.hierarchy_complete);
        assert!(document.query_contexts(&page).is_empty());
    }

    #[test]
    fn semantic_query_context_deduplicates_identical_transparent_anchor_windows() {
        let mut document = context_fixture(3);
        document.nodes[2].child_ax_ids = vec!["transparent-a".into(), "transparent-b".into()];
        for (offset, ax_id) in ["transparent-a", "transparent-b"].into_iter().enumerate() {
            document.nodes.push(SemanticNode {
                ax_id: ax_id.into(),
                parent_ax_id: Some("node-2".into()),
                child_ax_ids: Vec::new(),
                backend_node_id: None,
                role: "generic".into(),
                name: None,
                value: None,
                destination_url: None,
                states: BTreeMap::new(),
                frame: identified_main_frame(),
                visibility: BrowserVisibility::InViewport,
                actions: Vec::new(),
                document_order: 3 + offset,
            });
        }

        let page = document.page(0, DEFAULT_SEMANTIC_NODE_BUDGET, Some("generic"), None);
        assert_eq!(page.selected.len(), 2);

        let contexts = document.query_contexts(&page);

        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].anchor.ax_id, "transparent-a");
        assert!(!contexts[0]
            .nodes
            .iter()
            .any(|node| is_transparent_generic(node)));
    }

    #[test]
    fn query_group_cache_is_bounded_on_deep_structural_ancestry() {
        let depth = 12_000;
        let nodes = (0..depth)
            .map(|idx| SemanticNode {
                ax_id: format!("group-{idx}"),
                parent_ax_id: (idx > 0).then(|| format!("group-{}", idx - 1)),
                child_ax_ids: (idx + 1 < depth)
                    .then(|| vec![format!("group-{}", idx + 1)])
                    .unwrap_or_default(),
                backend_node_id: None,
                role: "region".into(),
                name: None,
                value: None,
                destination_url: None,
                states: BTreeMap::new(),
                frame: identified_main_frame(),
                visibility: BrowserVisibility::InViewport,
                actions: Vec::new(),
                document_order: idx,
            })
            .collect::<Vec<_>>();
        let by_ax_id = unique_ax_indices(&nodes);
        let mut cache = HashMap::new();
        let groups = cached_context_groups(&nodes, &by_ax_id, depth - 1, &mut cache).unwrap();
        assert_eq!(groups, vec![depth - 1, depth - 2]);
        assert_eq!(cache.len(), depth);
        assert!(cache
            .values()
            .all(|groups| groups.as_ref().is_none_or(|groups| groups.len() <= 2)));
    }

    #[test]
    fn semantic_context_pages_are_contiguous_and_character_bounded() {
        let mut nodes = Vec::new();
        let child_ids = (0..60)
            .flat_map(|i| {
                let mut ids = vec![format!("item-{i}")];
                if i % 6 == 5 {
                    ids.push(format!("wrapper-{i}"));
                }
                ids
            })
            .collect();
        nodes.push(SemanticNode {
            ax_id: "list".into(),
            parent_ax_id: None,
            child_ax_ids: child_ids,
            backend_node_id: None,
            role: "list".into(),
            name: Some("Results".into()),
            value: None,
            destination_url: None,
            states: BTreeMap::new(),
            frame: identified_main_frame(),
            visibility: BrowserVisibility::InViewport,
            actions: Vec::new(),
            document_order: 0,
        });
        for i in 0..60 {
            nodes.push(SemanticNode {
                ax_id: format!("item-{i}"),
                parent_ax_id: Some("list".into()),
                child_ax_ids: Vec::new(),
                backend_node_id: Some(i + 1),
                role: "listitem".into(),
                name: Some(if i == 30 {
                    "needle".into()
                } else {
                    format!("row-{i}-{}", "界".repeat(1_000))
                }),
                value: None,
                destination_url: None,
                states: BTreeMap::new(),
                frame: identified_main_frame(),
                visibility: BrowserVisibility::InViewport,
                actions: Vec::new(),
                document_order: i as usize + 1,
            });
        }
        for i in (5..60).step_by(6) {
            nodes.push(SemanticNode {
                ax_id: format!("wrapper-{i}"),
                parent_ax_id: Some("list".into()),
                child_ax_ids: Vec::new(),
                backend_node_id: None,
                role: "generic".into(),
                name: None,
                value: None,
                destination_url: None,
                states: BTreeMap::new(),
                frame: identified_main_frame(),
                visibility: BrowserVisibility::InViewport,
                actions: Vec::new(),
                document_order: 100 + i,
            });
        }
        let document = SemanticDocument {
            nodes,
            complete: true,
            ..Default::default()
        };
        let page = document.page(0, DEFAULT_SEMANTIC_NODE_BUDGET, Some("needle"), None);
        let first = document.query_contexts(&page).remove(0);
        assert_eq!(
            first.before_omitted
                + first.selected_nodes
                + first.after_omitted
                + first.projected_out_nodes,
            first.source_member_nodes
        );
        assert_eq!(first.member_projection, "semantic_evidence_v1");
        assert_eq!(first.projected_out_nodes, 10);
        assert!(first.before_omitted > 0 && first.after_omitted > 0);
        assert!(first.outline.len() <= QUERY_CONTEXT_OUTLINE_MAX_BYTES);
        assert!(
            first.range_end - first.range_start < CONTEXT_BEFORE_NODES + 1 + CONTEXT_AFTER_NODES,
            "multibyte labels must exercise character-budget shrinking"
        );
        let next = document
            .context_window(
                &first.anchor.to_context_ref_entry(),
                Some(&first.group.identity()),
                SemanticContextWindow::Forward {
                    start: first.range_end,
                },
                CONTEXT_BEFORE_NODES + 1 + CONTEXT_AFTER_NODES,
                CONTEXT_OUTLINE_MAX_BYTES,
            )
            .unwrap();
        assert_eq!(next.range_start, first.range_end);
        assert!(next.outline.len() <= CONTEXT_OUTLINE_MAX_BYTES);
        assert_eq!(next.before_omitted, first.range_end);

        let mut covered = (first.range_start..first.range_end).collect::<HashSet<_>>();
        let mut cursor = first.range_end;
        while cursor < first.total_nodes {
            let page = document
                .context_window(
                    &first.anchor.to_context_ref_entry(),
                    Some(&first.group.identity()),
                    SemanticContextWindow::Forward { start: cursor },
                    CONTEXT_BEFORE_NODES + 1 + CONTEXT_AFTER_NODES,
                    CONTEXT_OUTLINE_MAX_BYTES,
                )
                .unwrap();
            assert_eq!(
                page.before_omitted
                    + page.selected_nodes
                    + page.after_omitted
                    + page.projected_out_nodes,
                page.source_member_nodes
            );
            assert_eq!(page.range_start, cursor);
            assert!(page.range_end > cursor);
            for idx in page.range_start..page.range_end {
                assert!(
                    covered.insert(idx),
                    "forward context pages must not overlap"
                );
            }
            cursor = page.range_end;
        }
        let mut cursor = first.range_start;
        while cursor > 0 {
            let page = document
                .context_window(
                    &first.anchor.to_context_ref_entry(),
                    Some(&first.group.identity()),
                    SemanticContextWindow::Backward { end: cursor },
                    CONTEXT_BEFORE_NODES + 1 + CONTEXT_AFTER_NODES,
                    CONTEXT_OUTLINE_MAX_BYTES,
                )
                .unwrap();
            assert_eq!(
                page.before_omitted
                    + page.selected_nodes
                    + page.after_omitted
                    + page.projected_out_nodes,
                page.source_member_nodes
            );
            assert_eq!(page.range_end, cursor);
            assert!(page.range_start < cursor);
            for idx in page.range_start..page.range_end {
                assert!(
                    covered.insert(idx),
                    "backward context pages must not overlap"
                );
            }
            cursor = page.range_start;
        }
        assert_eq!(covered.len(), first.total_nodes);
        assert_eq!(covered.iter().copied().min(), Some(0));
        assert_eq!(covered.iter().copied().max(), Some(first.total_nodes - 1));
    }

    #[test]
    fn semantic_context_projects_only_disclosed_transparent_generic_members() {
        let mut document = context_fixture(0);
        document.nodes[1].child_ax_ids = [
            "transparent-parent",
            "transparent-leaf",
            "named",
            "valued",
            "destination",
            "stateful",
            "actionable",
            "hidden",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        let generic = |ax_id: &str, order: usize| SemanticNode {
            ax_id: ax_id.to_owned(),
            parent_ax_id: Some("node-1".into()),
            child_ax_ids: Vec::new(),
            backend_node_id: Some(100 + order as i64),
            role: "generic".into(),
            name: None,
            value: None,
            destination_url: None,
            states: BTreeMap::new(),
            frame: identified_main_frame(),
            visibility: BrowserVisibility::InViewport,
            actions: Vec::new(),
            document_order: order,
        };
        let mut transparent_parent = generic("transparent-parent", 2);
        transparent_parent.child_ax_ids = vec!["meaningful-text".into()];
        let transparent_leaf = generic("transparent-leaf", 3);
        let mut named = generic("named", 5);
        named.name = Some("qualifier".into());
        let mut valued = generic("valued", 6);
        valued.value = Some("42".into());
        let mut destination = generic("destination", 7);
        destination.destination_url = Some("https://example.test/destination".into());
        let mut stateful = generic("stateful", 8);
        stateful.states.insert("expanded".into(), json!(false));
        stateful.states.insert("level".into(), json!(0));
        let mut actionable = generic("actionable", 9);
        actionable.actions.push(BrowserActionKind::Click);
        let mut hidden = generic("hidden", 10);
        hidden.visibility = BrowserVisibility::CssHidden;
        document.nodes.extend([
            transparent_parent,
            transparent_leaf,
            SemanticNode {
                ax_id: "meaningful-text".into(),
                parent_ax_id: Some("transparent-parent".into()),
                child_ax_ids: Vec::new(),
                backend_node_id: None,
                role: "statictext".into(),
                name: Some("detail".into()),
                value: None,
                destination_url: None,
                states: BTreeMap::new(),
                frame: identified_main_frame(),
                visibility: BrowserVisibility::InViewport,
                actions: Vec::new(),
                document_order: 4,
            },
            named,
            valued,
            destination,
            stateful,
            actionable,
            hidden,
        ]);

        let group_page = document
            .context(&document.nodes[1].to_context_ref_entry())
            .unwrap();
        let transparent_anchor = document
            .nodes
            .iter()
            .find(|node| node.ax_id == "transparent-parent")
            .unwrap();
        let anchor_page = document
            .context(&transparent_anchor.to_context_ref_entry())
            .unwrap();
        let member_ids = group_page
            .nodes
            .iter()
            .map(|node| node.ax_id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(group_page.member_projection, "semantic_evidence_v1");
        assert_eq!(group_page.source_member_nodes, 9);
        assert_eq!(group_page.projected_out_nodes, 2);
        assert_eq!(group_page.total_nodes, 7);
        assert!(group_page.group_complete);
        assert_eq!(
            group_page.before_omitted
                + group_page.selected_nodes
                + group_page.after_omitted
                + group_page.projected_out_nodes,
            group_page.source_member_nodes
        );
        assert!(!member_ids.contains(&"transparent-parent"));
        assert!(!member_ids.contains(&"transparent-leaf"));
        assert!(!member_ids.contains(&"hidden"));
        for retained in [
            "node-1",
            "meaningful-text",
            "named",
            "valued",
            "destination",
            "stateful",
            "actionable",
        ] {
            assert!(member_ids.contains(&retained), "missing {retained}");
        }
        assert!(group_page.outline.contains("detail"));
        assert!(group_page.outline.contains("- generic\n"));
        assert!(!group_page.outline.contains("transparent-leaf"));

        assert_eq!(anchor_page.anchor.ax_id, "transparent-parent");
        assert_eq!(anchor_page.total_nodes, group_page.total_nodes);
        assert_eq!(
            anchor_page.source_member_nodes,
            group_page.source_member_nodes
        );
        assert_eq!(
            anchor_page.projected_out_nodes,
            group_page.projected_out_nodes
        );
        assert_eq!(
            anchor_page
                .nodes
                .iter()
                .map(|node| node.ax_id.as_str())
                .collect::<Vec<_>>(),
            member_ids,
            "projection must stay stable when an omitted generic is the exact anchor"
        );

        document.nodes[1].child_ax_ids.push("missing".into());
        let malformed = document
            .context(&document.nodes[1].to_context_ref_entry())
            .unwrap();
        assert!(!malformed.group_complete);
        assert_eq!(malformed.projected_out_nodes, 2);
    }

    #[test]
    fn semantic_projection_offsets_are_stable_for_omitted_middle_and_tail_anchors() {
        let mut document = context_fixture(0);
        let mut children = Vec::new();
        let mut added = Vec::new();
        for index in 0..30 {
            let ax_id = format!("evidence-{index}");
            children.push(ax_id.clone());
            added.push(SemanticNode {
                ax_id,
                parent_ax_id: Some("node-1".into()),
                child_ax_ids: Vec::new(),
                backend_node_id: Some(200 + index),
                role: "generic".into(),
                name: Some(format!("evidence {index}")),
                value: None,
                destination_url: None,
                states: BTreeMap::new(),
                frame: identified_main_frame(),
                visibility: BrowserVisibility::InViewport,
                actions: Vec::new(),
                document_order: index as usize + 2,
            });
            if index == 14 {
                children.push("transparent-middle".into());
                added.push(SemanticNode {
                    ax_id: "transparent-middle".into(),
                    parent_ax_id: Some("node-1".into()),
                    child_ax_ids: Vec::new(),
                    backend_node_id: None,
                    role: "generic".into(),
                    name: None,
                    value: None,
                    destination_url: None,
                    states: BTreeMap::new(),
                    frame: identified_main_frame(),
                    visibility: BrowserVisibility::InViewport,
                    actions: Vec::new(),
                    document_order: 100,
                });
            }
        }
        children.push("transparent-tail".into());
        added.push(SemanticNode {
            ax_id: "transparent-tail".into(),
            parent_ax_id: Some("node-1".into()),
            child_ax_ids: Vec::new(),
            backend_node_id: None,
            role: "generic".into(),
            name: None,
            value: None,
            destination_url: None,
            states: BTreeMap::new(),
            frame: identified_main_frame(),
            visibility: BrowserVisibility::InViewport,
            actions: Vec::new(),
            document_order: 101,
        });
        document.nodes[1].child_ax_ids = children;
        document.nodes.extend(added);

        let group = document.nodes[1].to_context_ref_entry();
        let group_identity = document.nodes[1].identity();
        let middle = document
            .nodes
            .iter()
            .find(|node| node.ax_id == "transparent-middle")
            .unwrap()
            .to_context_ref_entry();
        let tail = document
            .nodes
            .iter()
            .find(|node| node.ax_id == "transparent-tail")
            .unwrap()
            .to_context_ref_entry();
        let collect = |anchor: &RefEntry| {
            let mut ids = Vec::new();
            let mut cursor = 0;
            let mut expected_total = None;
            loop {
                let page = document
                    .context_window(
                        anchor,
                        Some(&group_identity),
                        SemanticContextWindow::Forward { start: cursor },
                        1,
                        CONTEXT_OUTLINE_MAX_BYTES,
                    )
                    .unwrap();
                assert_eq!(page.range_start, cursor);
                assert_eq!(page.member_projection, "semantic_evidence_v1");
                assert_eq!(page.source_member_nodes, 33);
                assert_eq!(page.projected_out_nodes, 2);
                assert_eq!(page.total_nodes, 31);
                assert_eq!(
                    page.before_omitted
                        + page.selected_nodes
                        + page.after_omitted
                        + page.projected_out_nodes,
                    page.source_member_nodes
                );
                expected_total.get_or_insert(page.total_nodes);
                ids.extend(page.nodes.iter().map(|node| node.ax_id.clone()));
                assert!(page.range_end > cursor);
                cursor = page.range_end;
                if cursor == page.total_nodes {
                    break;
                }
            }
            assert_eq!(ids.len(), expected_total.unwrap());
            ids
        };

        let group_ids = collect(&group);
        assert_eq!(collect(&middle), group_ids);
        assert_eq!(collect(&tail), group_ids);
        assert!(!group_ids.iter().any(|id| id.starts_with("transparent-")));

        for anchor in [&middle, &tail] {
            let around = document
                .context_window(
                    anchor,
                    Some(&group_identity),
                    SemanticContextWindow::Around,
                    1,
                    CONTEXT_OUTLINE_MAX_BYTES,
                )
                .unwrap();
            assert_eq!(around.selected_nodes, 1);
            assert_eq!(around.source_member_nodes, 33);
            assert_eq!(around.projected_out_nodes, 2);
            assert_eq!(around.total_nodes, 31);
        }
    }

    #[test]
    fn ignored_ax_rowgroup_is_collapsed_without_losing_table_context() {
        let ax = json!({"nodes": [
            {"nodeId":"root","ignored":false,"role":{"value":"RootWebArea"},"childIds":["main"]},
            {"nodeId":"main","parentId":"root","ignored":false,"role":{"value":"main"},"childIds":["table"]},
            {"nodeId":"table","parentId":"main","ignored":false,"role":{"value":"table"},"name":{"value":"Records"},"childIds":["rowgroup"]},
            {"nodeId":"rowgroup","parentId":"table","ignored":true,"childIds":["earlier","current"]},
            {"nodeId":"earlier","parentId":"rowgroup","ignored":false,"role":{"value":"row"},"childIds":["earlier-cell"]},
            {"nodeId":"earlier-cell","parentId":"earlier","ignored":false,"role":{"value":"cell"},"name":{"value":"Earlier"},"childIds":[]},
            {"nodeId":"current","parentId":"rowgroup","ignored":false,"role":{"value":"row"},"childIds":["current-cell"]},
            {"nodeId":"current-cell","parentId":"current","ignored":false,"role":{"value":"cell"},"name":{"value":"Inspect"},"childIds":["button"]},
            {"nodeId":"button","parentId":"current-cell","ignored":false,"backendDOMNodeId":42,"role":{"value":"button"},"name":{"value":"Inspect"},"childIds":[]}
        ]});
        let document = compose_accessibility_tree(
            &ax,
            &DomIndex::default(),
            &LayoutIndex::default(),
            &Viewport::default(),
            identified_main_frame(),
        );
        let button = document
            .nodes
            .iter()
            .find(|node| node.ax_id == "button")
            .unwrap()
            .to_ref_entry()
            .unwrap();

        let row = document.context(&button).unwrap();

        assert_eq!(row.group.ax_id, "current");
        assert_eq!(
            row.parent_group.as_ref().map(|node| node.ax_id.as_str()),
            Some("table")
        );
        assert!(row.group_complete);

        let table = document
            .nodes
            .iter()
            .find(|node| node.ax_id == "table")
            .unwrap()
            .to_context_ref_entry();
        let table_page = document.context(&table).unwrap();
        let earlier = table_page.outline.find("Earlier").unwrap();
        let inspect = table_page.outline.find("Inspect").unwrap();
        assert!(earlier < inspect);
        assert!(table_page.group_complete);
    }

    #[test]
    fn redundant_static_text_pruning_repairs_only_proven_leaf_edges() {
        let ax = json!({"nodes":[
            {"nodeId":"main","ignored":false,"role":{"value":"main"},"childIds":["article"]},
            {"nodeId":"article","parentId":"main","ignored":false,"role":{"value":"article"},"name":{"value":"Reference"},"childIds":["heading","paragraph","button"]},
            {"nodeId":"heading","parentId":"article","ignored":false,"role":{"value":"heading"},"name":{"value":"Methods"},"childIds":["heading-text"]},
            {"nodeId":"heading-text","parentId":"heading","ignored":false,"role":{"value":"StaticText"},"name":{"value":"Methods"},"childIds":["heading-inline"]},
            {"nodeId":"heading-inline","parentId":"heading-text","ignored":false,"role":{"value":"InlineTextBox"},"name":{"value":"Methods"},"childIds":[]},
            {"nodeId":"paragraph","parentId":"article","ignored":false,"role":{"value":"paragraph"},"childIds":["qualifier"]},
            {"nodeId":"qualifier","parentId":"paragraph","ignored":false,"role":{"value":"StaticText"},"name":{"value":"Context qualifier: preliminary."},"childIds":[]},
            {"nodeId":"button","parentId":"article","ignored":false,"backendDOMNodeId":42,"role":{"value":"button"},"name":{"value":"Inspect methods"},"childIds":["button-text"]},
            {"nodeId":"button-text","parentId":"button","ignored":false,"role":{"value":"StaticText"},"name":{"value":"Inspect methods"},"childIds":[]}
        ]});
        let document = compose_accessibility_tree(
            &ax,
            &DomIndex::default(),
            &LayoutIndex::default(),
            &Viewport::default(),
            identified_main_frame(),
        );
        let article = document
            .nodes
            .iter()
            .find(|node| node.ax_id == "article")
            .unwrap();
        let heading = document
            .nodes
            .iter()
            .find(|node| node.ax_id == "heading")
            .unwrap();
        let button = document
            .nodes
            .iter()
            .find(|node| node.ax_id == "button")
            .unwrap();

        assert!(heading.child_ax_ids.is_empty());
        assert!(button.child_ax_ids.is_empty());
        assert!(document.nodes.iter().any(|node| node.ax_id == "qualifier"));
        assert!(!document
            .nodes
            .iter()
            .any(|node| matches!(node.ax_id.as_str(), "heading-text" | "button-text")));
        let context = document.context(&article.to_context_ref_entry()).unwrap();
        assert!(context.outline.contains("Context qualifier: preliminary."));
        assert!(context.group_complete);

        let unknown_child = json!({"nodes":[
            {"nodeId":"article","ignored":false,"role":{"value":"article"},"childIds":["heading"]},
            {"nodeId":"heading","parentId":"article","ignored":false,"role":{"value":"heading"},"name":{"value":"Methods"},"childIds":["heading-text"]},
            {"nodeId":"heading-text","parentId":"heading","ignored":false,"role":{"value":"StaticText"},"name":{"value":"Methods"},"childIds":["unknown"]}
        ]});
        let document = compose_accessibility_tree(
            &unknown_child,
            &DomIndex::default(),
            &LayoutIndex::default(),
            &Viewport::default(),
            identified_main_frame(),
        );
        let article = document
            .nodes
            .iter()
            .find(|node| node.ax_id == "article")
            .unwrap();
        assert!(document
            .nodes
            .iter()
            .any(|node| node.ax_id == "heading-text"));
        assert!(
            !document
                .context(&article.to_context_ref_entry())
                .unwrap()
                .group_complete
        );
    }

    #[test]
    fn ignored_ax_normalization_is_iterative_and_document_bounded() {
        const IGNORED_DEPTH: usize = 10_000;
        const RETAINED_LEAVES: usize = 10_000;
        let mut nodes = Vec::with_capacity(IGNORED_DEPTH + RETAINED_LEAVES + 1);
        nodes.push(json!({
            "nodeId":"group",
            "ignored":false,
            "role":{"value":"table"},
            "childIds":["ignored-0"]
        }));
        for index in 0..IGNORED_DEPTH {
            let parent = if index == 0 {
                "group".to_owned()
            } else {
                format!("ignored-{}", index - 1)
            };
            let children = if index + 1 == IGNORED_DEPTH {
                (0..RETAINED_LEAVES)
                    .map(|leaf| Value::String(format!("row-{leaf}")))
                    .collect::<Vec<_>>()
            } else {
                vec![Value::String(format!("ignored-{}", index + 1))]
            };
            nodes.push(json!({
                "nodeId":format!("ignored-{index}"),
                "parentId":parent,
                "ignored":true,
                "childIds":children
            }));
        }
        for leaf in 0..RETAINED_LEAVES {
            nodes.push(json!({
                "nodeId":format!("row-{leaf}"),
                "parentId":format!("ignored-{}", IGNORED_DEPTH - 1),
                "ignored":false,
                "role":{"value":"row"},
                "childIds":[]
            }));
        }

        let document = compose_accessibility_tree(
            &json!({"nodes":nodes}),
            &DomIndex::default(),
            &LayoutIndex::default(),
            &Viewport::default(),
            identified_main_frame(),
        );
        let group = document
            .nodes
            .iter()
            .find(|node| node.ax_id == "group")
            .unwrap();
        let last_row = document
            .nodes
            .iter()
            .find(|node| node.ax_id == format!("row-{}", RETAINED_LEAVES - 1))
            .unwrap();

        assert_eq!(group.child_ax_ids.len(), RETAINED_LEAVES);
        assert_eq!(
            group.child_ax_ids.first().map(String::as_str),
            Some("row-0")
        );
        assert_eq!(
            group.child_ax_ids.last().map(String::as_str),
            Some(format!("row-{}", RETAINED_LEAVES - 1).as_str())
        );
        assert_eq!(last_row.parent_ax_id.as_deref(), Some("group"));
    }

    #[test]
    fn ignored_ax_normalization_preserves_dag_and_edge_contradictions_as_incomplete() {
        let ax = json!({"nodes":[
            {"nodeId":"group","ignored":false,"role":{"value":"table"},"childIds":["i1","i2"]},
            {"nodeId":"i1","parentId":"group","ignored":true,"childIds":["shared"]},
            {"nodeId":"i2","parentId":"group","ignored":true,"childIds":["shared"]},
            {"nodeId":"shared","parentId":"i1","ignored":true,"childIds":["row"]},
            {"nodeId":"row","parentId":"shared","ignored":false,"role":{"value":"row"},"childIds":[]}
        ]});
        let document = compose_accessibility_tree(
            &ax,
            &DomIndex::default(),
            &LayoutIndex::default(),
            &Viewport::default(),
            identified_main_frame(),
        );
        let group = document
            .nodes
            .iter()
            .find(|node| node.ax_id == "group")
            .unwrap();

        assert_eq!(group.child_ax_ids.first().map(String::as_str), Some("row"));
        assert_eq!(group.child_ax_ids.len(), 2);
        assert!(
            !document
                .context(&group.to_context_ref_entry())
                .unwrap()
                .group_complete
        );

        let contradictory = json!({"nodes":[
            {"nodeId":"group","ignored":false,"role":{"value":"table"},"childIds":["i1"]},
            {"nodeId":"i1","parentId":"group","ignored":true,"childIds":["row"]},
            {"nodeId":"i2","parentId":"group","ignored":true,"childIds":[]},
            {"nodeId":"row","parentId":"i2","ignored":false,"role":{"value":"row"},"childIds":[]}
        ]});
        let document = compose_accessibility_tree(
            &contradictory,
            &DomIndex::default(),
            &LayoutIndex::default(),
            &Viewport::default(),
            identified_main_frame(),
        );
        let group = document
            .nodes
            .iter()
            .find(|node| node.ax_id == "group")
            .unwrap();
        let row = document
            .nodes
            .iter()
            .find(|node| node.ax_id == "row")
            .unwrap();

        assert_ne!(group.child_ax_ids, ["row"]);
        assert_ne!(row.parent_ax_id.as_deref(), Some("group"));
        assert!(!document.page(0, 20, Some("row"), None).hierarchy_complete);
    }

    #[test]
    fn ignored_ax_normalization_keeps_malformed_and_duplicate_ids_unproven() {
        let malformed = json!({"nodes":[
            {"nodeId":"group","ignored":false,"role":{"value":"table"},"childIds":["row",42]},
            {"nodeId":"row","parentId":"group","ignored":false,"role":{"value":"row"},"childIds":[]}
        ]});
        let document = compose_accessibility_tree(
            &malformed,
            &DomIndex::default(),
            &LayoutIndex::default(),
            &Viewport::default(),
            identified_main_frame(),
        );
        let group = document
            .nodes
            .iter()
            .find(|node| node.ax_id == "group")
            .unwrap();
        assert_eq!(group.child_ax_ids.first().map(String::as_str), Some("row"));
        assert_eq!(group.child_ax_ids.len(), 2);
        assert!(
            !document
                .context(&group.to_context_ref_entry())
                .unwrap()
                .group_complete
        );

        for invalid_parent in [
            json!([
                {"nodeId":"outer","ignored":true,"childIds":[]},
                {"nodeId":"group","parentId":"outer","ignored":false,"role":{"value":"table"},"childIds":["row"]},
                {"nodeId":"row","parentId":"group","ignored":false,"role":{"value":"row"},"childIds":[]}
            ]),
            json!([
                {"nodeId":"group","parentId":42,"ignored":false,"role":{"value":"table"},"childIds":["row"]},
                {"nodeId":"row","parentId":"group","ignored":false,"role":{"value":"row"},"childIds":[]}
            ]),
        ] {
            let document = compose_accessibility_tree(
                &json!({"nodes":invalid_parent}),
                &DomIndex::default(),
                &LayoutIndex::default(),
                &Viewport::default(),
                identified_main_frame(),
            );
            let group = document
                .nodes
                .iter()
                .find(|node| node.ax_id == "group")
                .unwrap();
            assert!(matches!(
                document.context(&group.to_context_ref_entry()),
                Err(SemanticContextError::AnchorAmbiguous)
            ));
        }

        for duplicate_nodes in [
            json!([
                {"nodeId":"dup","parentId":"group","ignored":false,"role":{"value":"row"},"childIds":[]},
                {"nodeId":"dup","parentId":"group","ignored":true,"childIds":[]}
            ]),
            json!([
                {"nodeId":"dup","parentId":"group","ignored":true,"childIds":[]},
                {"nodeId":"dup","parentId":"group","ignored":true,"childIds":[]}
            ]),
        ] {
            let mut nodes = vec![json!({
                "nodeId":"group",
                "ignored":false,
                "role":{"value":"table"},
                "childIds":["dup"]
            })];
            nodes.extend(duplicate_nodes.as_array().unwrap().iter().cloned());
            let document = compose_accessibility_tree(
                &json!({"nodes":nodes}),
                &DomIndex::default(),
                &LayoutIndex::default(),
                &Viewport::default(),
                identified_main_frame(),
            );
            let group = document
                .nodes
                .iter()
                .find(|node| node.ax_id == "group")
                .unwrap();
            assert!(!document.nodes.iter().any(|node| node.ax_id == "dup"));
            assert_ne!(group.child_ax_ids, ["dup"]);
            assert!(
                !document
                    .context(&group.to_context_ref_entry())
                    .unwrap()
                    .group_complete
            );
        }
    }

    fn context_fixture(count: usize) -> SemanticDocument {
        let mut labels = vec![("Document", BrowserVisibility::InViewport)];
        labels.push(("Results", BrowserVisibility::InViewport));
        for _ in 0..count {
            labels.push(("Row", BrowserVisibility::InViewport));
        }
        let mut document = query_fixture(&labels);
        for node in &mut document.nodes {
            node.frame = identified_main_frame();
        }
        document.nodes[0].role = "document".into();
        document.nodes[0].backend_node_id = None;
        document.nodes[0].child_ax_ids = vec!["node-1".into()];
        document.nodes[1].role = "list".into();
        document.nodes[1].backend_node_id = None;
        document.nodes[1].parent_ax_id = Some("node-0".into());
        document.nodes[1].child_ax_ids = (2..count + 2)
            .map(|index| format!("node-{index}"))
            .collect();
        for (index, node) in document.nodes.iter_mut().enumerate().skip(2) {
            node.role = "listitem".into();
            node.name = Some(format!("Row {index}"));
            node.parent_ax_id = Some("node-1".into());
        }
        document
    }

    #[test]
    fn semantic_context_returns_bounded_source_order_and_backendless_group_path() {
        let document = context_fixture(30);
        let anchor = document.nodes[20].to_ref_entry().unwrap();

        let page = document.context(&anchor).unwrap();

        assert_eq!(page.group.role, "listitem");
        assert_eq!(
            page.parent_group.as_ref().map(|n| n.role.as_str()),
            Some("list")
        );
        assert_eq!(
            page.nodes.first().and_then(|n| n.name.as_deref()),
            Some("Row 20")
        );
        // A group anchor starts at that group's beginning and doesn't escape it.
        assert_eq!(page.total_nodes, 1);
        assert!(page.group_complete);

        let list_anchor = document.nodes[1].to_context_ref_entry();
        let page = document.context(&list_anchor).unwrap();
        assert_eq!(page.nodes.len(), 25);
        assert_eq!(page.nodes[1].name.as_deref(), Some("Row 2"));
        assert_eq!(page.after_omitted, 6);
        assert!(!page.group_complete);
        assert_eq!(
            page.parent_group.as_ref().map(|n| n.role.as_str()),
            Some("document")
        );
    }

    #[test]
    fn semantic_context_uses_snapshot_identity_not_duplicate_backend_id() {
        let mut document = context_fixture(2);
        document.nodes[2].backend_node_id = Some(77);
        document.nodes[3].backend_node_id = Some(77);
        let anchor = document.nodes[3].to_ref_entry().unwrap();

        let page = document.context(&anchor).unwrap();

        assert_eq!(page.group.name.as_deref(), Some("Row 3"));
    }

    #[test]
    fn semantic_context_marks_unknown_group_structure_incomplete() {
        let mut document = context_fixture(2);
        document.nodes[1].child_ax_ids.push("missing-child".into());
        let anchor = document.nodes[1].to_context_ref_entry();

        let page = document.context(&anchor).unwrap();

        assert!(!page.group_complete);
        assert!(page.document_collection_complete);
    }

    #[test]
    fn semantic_context_marks_descendant_parent_contradictions_incomplete() {
        for contradictory_parent in ["node-2", "node-0"] {
            let mut document = context_fixture(1);
            document.nodes[2].parent_ax_id = Some(contradictory_parent.into());
            let anchor = document.nodes[1].to_context_ref_entry();

            let page = document.context(&anchor).unwrap();

            assert!(!page.group_complete);
            assert_eq!(page.total_nodes, 1);
            assert!(!page.outline.contains("Row 2"));
        }
    }

    #[test]
    fn semantic_context_uses_proven_ax_child_order_not_mixed_numeric_domains() {
        let mut document = context_fixture(2);
        // Simulate a backend-less AX fallback order that is numerically later
        // than an unrelated DOM preorder. The validated AX childIds remain the
        // only shared structural ordering evidence.
        document.nodes[2].backend_node_id = None;
        document.nodes[2].document_order = 20;
        document.nodes[3].document_order = 5;
        let anchor = document.nodes[1].to_context_ref_entry();

        let first = document.context(&anchor).unwrap();
        let second = document.context(&anchor).unwrap();

        assert_eq!(first.outline, second.outline);
        assert!(first.outline.find("Row 2") < first.outline.find("Row 3"));
        assert_eq!(
            first
                .nodes
                .iter()
                .filter_map(|node| node.name.as_deref())
                .collect::<Vec<_>>(),
            vec!["Results", "Row 2", "Row 3"]
        );
        assert!(first.group_complete);
    }

    #[test]
    fn semantic_context_refuses_cyclic_or_unproven_anchor_ancestry() {
        let mut cyclic = context_fixture(1);
        cyclic.nodes[2].parent_ax_id = Some("node-2".into());
        let anchor = cyclic.nodes[2].to_ref_entry().unwrap();
        assert_eq!(
            cyclic.context(&anchor).unwrap_err(),
            SemanticContextError::AnchorAmbiguous
        );

        let mut unproven = context_fixture(1);
        unproven.nodes[2].frame = FrameRef::main_unproven();
        let anchor = unproven.nodes[2].to_ref_entry().unwrap();
        assert_eq!(
            unproven.context(&anchor).unwrap_err(),
            SemanticContextError::AnchorUnproven
        );
    }

    #[test]
    fn semantic_outline_preserves_valid_tree_text_and_depth() {
        use BrowserVisibility::InViewport;
        let mut document = query_fixture(&[("Parent", InViewport), ("Child", InViewport)]);
        document.nodes[0].role = "generic".into();
        document.nodes[0].child_ax_ids = vec!["node-1".into()];
        document.nodes[1].parent_ax_id = Some("node-0".into());

        let page = document.page(0, 300, Some("Child"), None);

        assert_eq!(page.outline, "- generic \"Parent\"\n  - link \"Child\"");
        assert_eq!(page.selected_nodes, 1);
        assert_eq!(page.omissions.unknown, 0);
        assert!(page.hierarchy_complete);
    }

    #[test]
    fn semantic_outline_omits_self_cycle_without_unbounded_parent_walk() {
        use BrowserVisibility::InViewport;
        let mut document = query_fixture(&[("Loop", InViewport)]);
        document.nodes[0].parent_ax_id = Some("node-0".into());
        document.nodes[0].child_ax_ids = vec!["node-0".into()];

        // This page call never returned before ancestry traversal was bounded.
        let page = document.page(0, 300, Some("Loop"), None);

        assert!(page.outline.is_empty());
        assert!(page.selected.is_empty());
        assert_eq!(page.selected_nodes, 0);
        assert_eq!(page.total_nodes, 1);
        assert_eq!(page.omissions.unknown, 1);
        assert!(!page.hierarchy_complete);
    }

    #[test]
    fn semantic_outline_does_not_double_count_unknown_cycle_omission() {
        let mut document = query_fixture(&[("Loop", BrowserVisibility::Unknown)]);
        document.nodes[0].parent_ax_id = Some("node-0".into());

        let page = document.page(0, 300, Some("Loop"), None);

        assert!(page.outline.is_empty());
        assert_eq!(page.omissions.unknown, 1);
        assert!(!page.hierarchy_complete);
    }

    #[test]
    fn semantic_outline_omits_descendant_whose_ancestry_enters_cycle() {
        use BrowserVisibility::InViewport;
        let mut document = query_fixture(&[
            ("Cycle A", InViewport),
            ("Cycle B", InViewport),
            ("Descendant", InViewport),
        ]);
        document.nodes[0].parent_ax_id = Some("node-1".into());
        document.nodes[1].parent_ax_id = Some("node-0".into());
        document.nodes[2].parent_ax_id = Some("node-0".into());

        let page = document.page(0, 300, Some("Descendant"), None);

        assert!(page.outline.is_empty());
        assert!(page.selected.is_empty());
        assert_eq!(page.selected_nodes, 0);
        assert_eq!(page.omissions.unknown, 1);
        assert!(!page.hierarchy_complete);
    }

    #[test]
    fn semantic_outline_treats_dangling_parent_as_unavailable_context() {
        use BrowserVisibility::InViewport;
        let mut document = query_fixture(&[("Orphan", InViewport)]);
        document.nodes[0].parent_ax_id = Some("ignored-or-missing-parent".into());

        let page = document.page(0, 300, Some("Orphan"), None);

        assert_eq!(page.outline, "- link \"Orphan\"");
        assert_eq!(page.selected_nodes, 1);
        assert_eq!(page.omissions.unknown, 0);
        assert!(page.hierarchy_complete);
    }

    #[test]
    fn semantic_outline_resolves_repeated_ax_ids_within_their_own_frames() {
        use BrowserVisibility::InViewport;
        let mut first = query_fixture(&[
            ("Frame one parent", InViewport),
            ("Frame one child", InViewport),
        ]);
        first.nodes[0].ax_id = "parent".into();
        first.nodes[0].role = "generic".into();
        first.nodes[0].child_ax_ids = vec!["child".into()];
        first.nodes[1].ax_id = "child".into();
        first.nodes[1].parent_ax_id = Some("parent".into());
        for node in &mut first.nodes {
            node.frame = identified_frame("frame-one", "loader-one");
        }

        let mut second = query_fixture(&[
            ("Frame two parent", InViewport),
            ("Frame two child", InViewport),
        ]);
        second.nodes[0].ax_id = "parent".into();
        second.nodes[0].role = "generic".into();
        second.nodes[0].child_ax_ids = vec!["child".into()];
        second.nodes[1].ax_id = "child".into();
        second.nodes[1].parent_ax_id = Some("parent".into());
        for node in &mut second.nodes {
            node.frame = identified_frame("frame-two", "loader-two");
        }
        first.extend(second);

        let page = first.page(0, 300, Some("Frame one child"), None);

        assert_eq!(
            page.outline,
            "- generic \"Frame one parent\"\n  - link \"Frame one child\""
        );
        assert!(!page.outline.contains("Frame two parent"));
        assert_eq!(page.omissions.unknown, 0);
    }

    #[test]
    fn semantic_outline_resolves_repeated_ax_ids_within_distinct_oopif_targets() {
        use BrowserVisibility::InViewport;
        let mut first = query_fixture(&[
            ("Target one parent", InViewport),
            ("Target one child", InViewport),
        ]);
        first.nodes[0].ax_id = "parent".into();
        first.nodes[0].role = "generic".into();
        first.nodes[1].ax_id = "child".into();
        first.nodes[1].parent_ax_id = Some("parent".into());
        for node in &mut first.nodes {
            node.frame = oopif_frame("target-one", "frame", "loader");
        }

        let mut second = query_fixture(&[
            ("Target two parent", InViewport),
            ("Target two child", InViewport),
        ]);
        second.nodes[0].ax_id = "parent".into();
        second.nodes[0].role = "generic".into();
        second.nodes[1].ax_id = "child".into();
        second.nodes[1].parent_ax_id = Some("parent".into());
        for node in &mut second.nodes {
            node.frame = oopif_frame("target-two", "frame", "loader");
        }
        first.extend(second);

        let page = first.page(0, 300, Some("Target one child"), None);

        assert_eq!(
            page.outline,
            "- generic \"Target one parent\"\n  - link \"Target one child\""
        );
        assert!(!page.outline.contains("Target two parent"));
        assert_eq!(page.omissions.unknown, 0);
    }

    #[test]
    fn semantic_outline_does_not_choose_between_duplicate_ids_in_one_frame() {
        use BrowserVisibility::InViewport;
        let mut document = query_fixture(&[
            ("First ambiguous parent", InViewport),
            ("Second ambiguous parent", InViewport),
            ("Ambiguous child", InViewport),
        ]);
        document.nodes[0].ax_id = "parent".into();
        document.nodes[1].ax_id = "parent".into();
        document.nodes[2].ax_id = "child".into();
        document.nodes[2].parent_ax_id = Some("parent".into());

        let page = document.page(0, 300, Some("Ambiguous child"), None);

        assert!(page.outline.is_empty());
        assert!(page.selected.is_empty());
        assert_eq!(page.selected_nodes, 0);
        assert_eq!(page.omissions.unknown, 1);
        assert!(!page.hierarchy_complete);
    }

    #[test]
    fn semantic_outline_does_not_invent_authority_for_unidentified_iframe() {
        use BrowserVisibility::InViewport;
        let mut document = query_fixture(&[("Unidentified frame node", InViewport)]);
        document.nodes[0].frame = FrameRef {
            kind: FrameKind::Iframe,
            oopif_target_id: None,
            identity: None,
        };

        let page = document.page(0, 300, Some("Unidentified frame node"), None);

        assert!(page.outline.is_empty());
        assert!(page.selected.is_empty());
        assert_eq!(page.omissions.unknown, 1);
        assert!(!page.hierarchy_complete);
    }

    #[test]
    fn semantic_query_recall_retains_reordered_terms_across_label_families() {
        use BrowserVisibility::InViewport;
        for (query, reordered, phrase, partial) in [
            (
                "annual report",
                "Report: annual summary",
                "Annual report",
                "Report archive",
            ),
            (
                "keyboard navigation",
                "Navigation with a keyboard",
                "Keyboard navigation",
                "Keyboard",
            ),
            ("water bottle", "Bottle for water", "Water bottle", "Water"),
        ] {
            let document = query_fixture(&[
                (reordered, InViewport),
                (phrase, InViewport),
                (partial, InViewport),
            ]);
            let page = document.page(0, 300, Some(query), None);
            assert_eq!(query_names(&page), vec![phrase, reordered], "query={query}");
            assert_eq!(page.total_nodes, 2);
            assert!(page.next_offset.is_none());
        }
    }

    #[test]
    fn semantic_query_recall_ignores_excluded_phrases_when_selecting_fallback() {
        use BrowserVisibility::{CssHidden, InViewport, PageOccluded};
        for excluded in [CssHidden, PageOccluded] {
            for visible in ["Report: annual summary", "Annual summary"] {
                let document = query_fixture(&[("Annual report", excluded), (visible, InViewport)]);
                let page = document.page(0, 300, Some("annual report"), None);
                assert_eq!(query_names(&page), vec![visible], "excluded={excluded:?}");
                assert_eq!(page.total_nodes, 1);
            }
        }
    }

    #[test]
    fn semantic_query_recall_preserves_partial_fallback_without_a_phrase() {
        use BrowserVisibility::InViewport;
        let document = query_fixture(&[
            ("Archive item 304", InViewport),
            ("Reply", InViewport),
            ("Unrelated", InViewport),
        ]);
        let page = document.page(0, 300, Some("reply archive 304"), None);
        assert_eq!(query_names(&page), vec!["Archive item 304", "Reply"]);
        // Preserve fallback breadth even if a non-phrase node has every term.
        let page = document.page(0, 300, Some("304 archive"), None);
        assert_eq!(query_names(&page), vec!["Archive item 304"]);
        let document = query_fixture(&[
            ("Report: annual summary", InViewport),
            ("Annual summary", InViewport),
        ]);
        let page = document.page(0, 300, Some("annual report"), None);
        assert_eq!(
            query_names(&page),
            vec!["Report: annual summary", "Annual summary"]
        );
    }

    #[test]
    fn semantic_query_recall_applies_subtree_scope_before_phrase_preference() {
        use BrowserVisibility::InViewport;
        let mut document = query_fixture(&[
            ("Section", InViewport),
            ("Annual summary", InViewport),
            ("Annual report", InViewport),
        ]);
        document.nodes[0].child_ax_ids = vec!["node-1".into()];
        document.nodes[1].parent_ax_id = Some("node-0".into());
        let page = document.page(0, 300, Some("annual report"), Some(1));
        assert_eq!(query_names(&page), vec!["Annual summary"]);
        assert_eq!(page.total_nodes, 1);
        assert!(page.hierarchy_complete);
    }

    #[test]
    fn semantic_scope_marks_ambiguous_child_lookup_incomplete() {
        use BrowserVisibility::InViewport;
        let mut document = query_fixture(&[
            ("Scope root", InViewport),
            ("First duplicate", InViewport),
            ("Second duplicate", InViewport),
        ]);
        document.nodes[0].role = "generic".into();
        document.nodes[0].child_ax_ids = vec!["duplicate".into()];
        document.nodes[1].ax_id = "duplicate".into();
        document.nodes[2].ax_id = "duplicate".into();

        let page = document.page(0, 300, None, Some(1));

        assert_eq!(page.outline, "- generic \"Scope root\"");
        assert_eq!(page.total_nodes, 1);
        assert!(!page.hierarchy_complete);
    }

    #[test]
    fn semantic_scope_keeps_missing_collected_child_policy_complete() {
        use BrowserVisibility::InViewport;
        let mut document = query_fixture(&[("Scope root", InViewport)]);
        document.nodes[0].role = "generic".into();
        document.nodes[0].child_ax_ids = vec!["ignored-child".into()];

        let page = document.page(0, 300, None, Some(1));

        assert_eq!(page.outline, "- generic \"Scope root\"");
        assert!(page.hierarchy_complete);
    }

    #[test]
    fn semantic_scope_marks_unproven_child_authority_incomplete() {
        use BrowserVisibility::InViewport;
        let mut document = query_fixture(&[("Scope root", InViewport), ("Child", InViewport)]);
        document.nodes[0].role = "generic".into();
        document.nodes[0].child_ax_ids = vec!["child".into()];
        document.nodes[1].ax_id = "child".into();
        document.nodes[1].frame = FrameRef {
            kind: FrameKind::Iframe,
            oopif_target_id: None,
            identity: None,
        };

        let page = document.page(0, 300, None, Some(1));

        assert_eq!(page.outline, "- generic \"Scope root\"");
        assert!(!page.hierarchy_complete);
    }

    #[test]
    fn semantic_scope_refuses_duplicate_backend_ids_across_frames() {
        use BrowserVisibility::InViewport;
        let mut document =
            query_fixture(&[("Main match", InViewport), ("Frame match", InViewport)]);
        document.nodes[1].backend_node_id = Some(1);
        document.nodes[1].frame = identified_frame("frame-two", "loader-two");

        let page = document.page(0, 300, None, Some(1));

        assert!(page.outline.is_empty());
        assert!(page.selected.is_empty());
        assert_eq!(page.total_nodes, 0);
        assert!(!page.hierarchy_complete);
    }

    #[test]
    fn semantic_query_recall_preserves_normalization_and_empty_term_behavior() {
        use BrowserVisibility::InViewport;
        let document = query_fixture(&[
            ("ANNUAL report report", InViewport),
            ("Report: annual summary", InViewport),
            ("...", InViewport),
            ("unrelated", InViewport),
        ]);
        let page = document.page(0, 300, Some(" annual REPORT report "), None);
        assert_eq!(
            query_names(&page),
            vec!["ANNUAL report report", "Report: annual summary"]
        );
        let punctuation = document.page(0, 300, Some("..."), None);
        assert_eq!(query_names(&punctuation), vec!["..."]);
        assert_eq!(document.page(0, 300, Some("!!!"), None).total_nodes, 0);
        assert_eq!(document.page(0, 300, Some("   "), None).total_nodes, 4);
        assert_eq!(document.page(0, 300, None, None).total_nodes, 4);
    }

    #[test]
    fn semantic_query_recall_preserves_cross_field_substring_matching() {
        use BrowserVisibility::InViewport;
        let mut document = query_fixture(&[
            ("link annual", InViewport),
            ("Annually updated", InViewport),
            ("Other", InViewport),
        ]);
        document.nodes[2].value = Some("annual".into());
        let page = document.page(0, 300, Some("link annual"), None);
        assert_eq!(
            query_names(&page),
            vec!["link annual", "Annually updated", "Other"]
        );
    }

    #[test]
    fn semantic_query_recall_preserves_visibility_ranking_and_bounded_pages() {
        use BrowserVisibility::{InViewport, NoLayout, Offscreen, Unknown};
        let document = query_fixture(&[
            ("Report: annual summary", InViewport),
            ("Annual report", Offscreen),
            ("Report: annual draft", Unknown),
            ("Report: annual notes", NoLayout),
            ("Annual unrelated", InViewport),
        ]);
        let first = document.page(0, 1, Some("annual report"), None);
        assert_eq!(query_names(&first), vec!["Annual report"]);
        assert_eq!(first.selected[0].visibility, Offscreen);
        assert_eq!(first.total_nodes, 4);
        assert_eq!(first.omissions.budget, 3);
        let second = document.page(first.next_offset.unwrap(), 1, Some("annual report"), None);
        assert_eq!(query_names(&second), vec!["Report: annual summary"]);
        let last = document.page(second.next_offset.unwrap(), 2, Some("annual report"), None);
        assert_eq!(
            query_names(&last),
            vec!["Report: annual draft", "Report: annual notes"]
        );
        assert!(last.next_offset.is_none());
        assert_eq!(last.selected_nodes, 2);
    }

    #[test]
    fn hidden_dom_nodes_do_not_enter_the_visible_working_set() {
        let dom = build_dom_index(&json!({
            "nodeType": 9,
            "children": [
                {"nodeType": 1, "nodeName": "BUTTON", "backendNodeId": 1,
                 "attributes": ["aria-hidden", "true"]},
                {"nodeType": 1, "nodeName": "BUTTON", "backendNodeId": 2,
                 "attributes": ["aria-label", "Reply"]}
            ]
        }));
        let ax = json!({"nodes": [
            {"nodeId": "root", "ignored": false, "role": {"value": "RootWebArea"},
             "childIds": ["retained", "reply"]},
            {"nodeId": "retained", "parentId": "root", "ignored": false,
             "backendDOMNodeId": 1, "role": {"value": "button"},
             "name": {"value": "Retained"}},
            {"nodeId": "reply", "parentId": "root", "ignored": false,
             "backendDOMNodeId": 2, "role": {"value": "button"},
             "name": {"value": "Reply"}}
        ]});
        let document = compose_accessibility_tree(
            &ax,
            &dom,
            &LayoutIndex::default(),
            &Viewport::default(),
            frame(),
        );
        let page = document.page(0, 300, None, None);
        assert_eq!(page.omissions.css_hidden, 1);
        let actionable = page
            .selected
            .iter()
            .filter(|node| !node.actions.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(actionable.len(), 1);
        assert_eq!(actionable[0].name.as_deref(), Some("Reply"));
    }

    #[test]
    fn dom_supplement_adds_only_explicit_visible_custom_actions() {
        let dom = build_dom_index(&json!({
            "nodeType": 9,
            "frameId": "F_MAIN",
            "children": [
                {"nodeType": 1, "nodeName": "DIV", "backendNodeId": 1,
                 "attributes": ["aria-label", "Custom action", "onclick", "run()"]},
                {"nodeType": 1, "nodeName": "DIV", "backendNodeId": 2,
                 "attributes": ["aria-label", "Static panel"]}
            ]
        }));
        let layout = build_layout_index(&json!({
            "strings": ["block", "visible", "1", "auto"],
            "documents": [{
                "nodes": {"backendNodeId": [1, 2]},
                "layout": {
                    "nodeIndex": [0, 1],
                    "bounds": [[10, 10, 100, 30], [10, 50, 100, 30]],
                    "styles": [[0, 1, 2, 3, 3, 3, 3], [0, 1, 2, 3, 3, 3, 3]],
                    "paintOrders": [1, 2]
                }
            }]
        }));
        let viewport = parse_viewport(&json!({
            "cssVisualViewport": {"pageX": 0.0, "pageY": 0.0,
                                  "clientWidth": 800.0, "clientHeight": 600.0}
        }));
        let document = compose_accessibility_tree(
            &json!({"nodes": [{"nodeId": "root", "ignored": false,
                "role": {"value": "RootWebArea"}, "childIds": []}]}),
            &dom,
            &layout,
            &viewport,
            frame(),
        );
        let page = document.page(0, 300, None, None);
        assert!(page.selected.iter().any(|node| {
            node.name.as_deref() == Some("Custom action")
                && node.actions == vec![BrowserActionKind::Click, BrowserActionKind::Pointer]
        }));
        assert!(page
            .selected
            .iter()
            .all(|node| node.name.as_deref() != Some("Static panel")));
    }

    #[test]
    fn dom_supplement_exposes_scrollable_containers_without_click_authority() {
        let dom = build_dom_index(&json!({
            "nodeType": 9,
            "frameId": "F_MAIN",
            "children": [
                {"nodeType": 1, "nodeName": "DIV", "backendNodeId": 1,
                 "attributes": ["aria-label", "Scrollable archive"]},
                {"nodeType": 1, "nodeName": "BODY", "backendNodeId": 2,
                 "attributes": ["aria-label", "Scrollable document root"]},
                {"nodeType": 1, "nodeName": "HTML", "backendNodeId": 3,
                 "attributes": ["aria-label", "Duplicate HTML document root"]}
            ]
        }));
        let layout = build_layout_index(&json!({
            "strings": ["block", "visible", "1", "auto", "default", "static", "0", "scroll"],
            "documents": [{
                "nodes": {"backendNodeId": [1, 2, 3]},
                "layout": {
                    "nodeIndex": [0, 1, 2],
                    "bounds": [[10, 10, 100, 50], [10, 80, 100, 50], [10, 80, 100, 50]],
                    "clientRects": [[10, 10, 100, 50], [10, 80, 100, 50], [10, 80, 100, 50]],
                    "scrollRects": [[0, 0, 100, 250], [0, 0, 100, 250], [0, 0, 100, 250]],
                    "styles": [
                        [0, 1, 2, 3, 4, 5, 6, 3, 7],
                        [0, 1, 2, 3, 4, 5, 6, 1, 1],
                        [0, 1, 2, 3, 4, 5, 6, 1, 1]
                    ],
                    "paintOrders": [1, 2, 3]
                }
            }]
        }));
        let viewport = parse_viewport(&json!({
            "cssVisualViewport": {"pageX": 0.0, "pageY": 0.0,
                                  "clientWidth": 800.0, "clientHeight": 600.0}
        }));
        let document = compose_accessibility_tree(
            &json!({"nodes": [{"nodeId": "root", "ignored": false,
                "role": {"value": "RootWebArea"}, "childIds": []}]}),
            &dom,
            &layout,
            &viewport,
            frame(),
        );
        let page = document.page(0, 300, None, None);
        let scrollable = page
            .selected
            .iter()
            .find(|node| node.name.as_deref() == Some("Scrollable archive"))
            .expect("scrollable DOM supplement");
        assert_eq!(scrollable.actions, vec![BrowserActionKind::Scroll]);
        let document_root = page
            .selected
            .iter()
            .find(|node| node.name.as_deref() == Some("Scrollable document root"))
            .expect("overflow-visible body root must expose document scrolling");
        assert_eq!(document_root.actions, vec![BrowserActionKind::Scroll]);
        assert!(page
            .selected
            .iter()
            .all(|node| node.name.as_deref() != Some("Duplicate HTML document root")));
    }

    #[test]
    fn unnamed_scrollable_body_is_labeled_as_the_document() {
        let dom = build_dom_index(&json!({
            "nodeType": 9,
            "children": [{"nodeType": 1, "nodeName": "BODY", "backendNodeId": 1}]
        }));
        let layout = build_layout_index(&json!({
            "strings": ["block", "visible", "1", "auto", "default", "static", "0"],
            "documents": [{
                "nodes": {"backendNodeId": [1]},
                "layout": {
                    "nodeIndex": [0],
                    "bounds": [[0, 0, 800, 600]],
                    "clientRects": [[0, 0, 800, 600]],
                    "scrollRects": [[0, 0, 800, 1200]],
                    "styles": [[0, 1, 2, 3, 4, 5, 6, 1, 1]],
                    "paintOrders": [1]
                }
            }]
        }));
        let document = compose_accessibility_tree(
            &json!({"nodes": [{"nodeId": "root", "ignored": false,
                "role": {"value": "RootWebArea"}, "childIds": []}]}),
            &dom,
            &layout,
            &parse_viewport(&json!({"cssVisualViewport": {
                "pageX": 0.0, "pageY": 0.0, "clientWidth": 800.0, "clientHeight": 600.0
            }})),
            frame(),
        );
        let page = document.page(0, 300, None, None);
        let root = page
            .selected
            .iter()
            .find(|node| node.name.as_deref() == Some("Document"))
            .expect("scrollable body label");
        assert_eq!(root.actions, vec![BrowserActionKind::Scroll]);
    }

    #[test]
    fn layout_ranks_in_viewport_actions_before_offscreen_content() {
        let dom = build_dom_index(&json!({
            "nodeType": 9,
            "children": [
                {"nodeType": 1, "nodeName": "P", "backendNodeId": 1},
                {"nodeType": 1, "nodeName": "BUTTON", "backendNodeId": 2}
            ]
        }));
        let layout = build_layout_index(&json!({
            "strings": ["block", "visible", "1", "auto", "pointer"],
            "documents": [{
                "nodes": {"backendNodeId": [1, 2]},
                "layout": {
                    "nodeIndex": [0, 1],
                    "bounds": [[0, 5000, 100, 20], [10, 10, 100, 30]],
                    "styles": [[0, 1, 2, 3, 4], [0, 1, 2, 3, 4]],
                    "paintOrders": [1, 2]
                }
            }]
        }));
        let viewport = parse_viewport(&json!({
            "cssVisualViewport": {"pageX": 0.0, "pageY": 0.0,
                                  "clientWidth": 800.0, "clientHeight": 600.0}
        }));
        let ax = json!({"nodes": [
            {"nodeId": "root", "ignored": false, "role": {"value": "RootWebArea"},
             "childIds": ["text", "reply"]},
            {"nodeId": "text", "parentId": "root", "ignored": false,
             "backendDOMNodeId": 1, "role": {"value": "StaticText"},
             "name": {"value": "Old content"}},
            {"nodeId": "reply", "parentId": "root", "ignored": false,
             "backendDOMNodeId": 2, "role": {"value": "button"},
             "name": {"value": "Reply"}}
        ]});
        let document = compose_accessibility_tree(&ax, &dom, &layout, &viewport, frame());
        let page = document.page(0, 1, None, None);
        assert_eq!(page.selected[0].name.as_deref(), Some("Reply"));
        assert_eq!(page.next_offset, Some(1));
    }

    #[test]
    fn fixed_painted_overlay_marks_covered_action_as_page_occluded() {
        let dom = build_dom_index(&json!({
            "nodeType": 9,
            "frameId": "F_MAIN",
            "children": [
                {"nodeType": 1, "nodeName": "BUTTON", "backendNodeId": 1},
                {"nodeType": 1, "nodeName": "DIV", "backendNodeId": 2}
            ]
        }));
        let layout = build_layout_index(&json!({
            "strings": ["block", "visible", "1", "auto", "fixed", "100"],
            "documents": [{
                "nodes": {"backendNodeId": [1, 2]},
                "layout": {
                    "nodeIndex": [0, 1],
                    "bounds": [[10, 10, 100, 30], [0, 0, 800, 600]],
                    "styles": [[0, 1, 2, 3, 3, 3, 3], [0, 1, 2, 3, 3, 4, 5]],
                    "paintOrders": [1, 2]
                }
            }]
        }));
        let viewport = parse_viewport(&json!({
            "cssVisualViewport": {"pageX": 0.0, "pageY": 0.0,
                                  "clientWidth": 800.0, "clientHeight": 600.0}
        }));
        let ax = json!({"nodes": [
            {"nodeId": "root", "ignored": false, "role": {"value": "RootWebArea"},
             "childIds": ["button"]},
            {"nodeId": "button", "parentId": "root", "ignored": false,
             "backendDOMNodeId": 1, "role": {"value": "button"},
             "name": {"value": "Covered"}}
        ]});
        let document = compose_accessibility_tree(&ax, &dom, &layout, &viewport, frame());
        let page = document.page(0, 300, None, None);
        assert!(page
            .selected
            .iter()
            .all(|node| node.name.as_deref() != Some("Covered")));
        assert_eq!(page.omissions.page_occluded, 1);
    }

    #[test]
    fn page_occlusion_prefilters_large_irrelevant_layout_before_branch_checks() {
        let mut layout = LayoutIndex::default();
        for backend in 1..=10_000 {
            layout.nodes.insert(
                backend,
                LayoutMeta {
                    bounds: Some(Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 800.0,
                        height: 600.0,
                    }),
                    styles: HashMap::from([("position".to_owned(), "static".to_owned())]),
                    paint_order: Some(backend),
                    ..Default::default()
                },
            );
        }
        layout.nodes.insert(
            10_001,
            LayoutMeta {
                bounds: Some(Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 800.0,
                    height: 600.0,
                }),
                styles: HashMap::from([
                    ("position".to_owned(), "fixed".to_owned()),
                    ("pointer-events".to_owned(), "auto".to_owned()),
                ]),
                paint_order: Some(10_001),
                ..Default::default()
            },
        );

        // apply_page_occlusion only performs the expensive DOM-branch test on
        // this prefiltered set, independent of the number of ordinary nodes.
        let candidates = page_occlusion_candidates(&layout);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].0, 10_001);
    }

    #[test]
    fn dialog_container_does_not_occlude_its_own_actions() {
        let dom = build_dom_index(&json!({
            "nodeType": 9,
            "frameId": "F_MAIN",
            "children": [{
                "nodeType": 1,
                "nodeName": "DIV",
                "backendNodeId": 10,
                "attributes": ["role", "alertdialog", "aria-label", "Remove app"],
                "children": [
                    {"nodeType": 1, "nodeName": "BUTTON", "backendNodeId": 11,
                     "attributes": ["aria-label", "Cancel"]},
                    {"nodeType": 1, "nodeName": "BUTTON", "backendNodeId": 12,
                     "attributes": ["aria-label", "Remove"]}
                ]
            }]
        }));
        let layout = build_layout_index(&json!({
            "strings": ["block", "visible", "1", "auto", "default", "static", "0", "fixed", "100"],
            "documents": [{
                "nodes": {"backendNodeId": [10, 11, 12]},
                "layout": {
                    "nodeIndex": [0, 1, 2],
                    "bounds": [[0, 0, 800, 600], [250, 400, 120, 36], [390, 400, 120, 36]],
                    "styles": [
                        [0, 1, 2, 3, 4, 7, 8, 3, 3],
                        [0, 1, 2, 3, 4, 5, 6, 3, 3],
                        [0, 1, 2, 3, 4, 5, 6, 3, 3]
                    ],
                    "paintOrders": [100, 10, 11]
                }
            }]
        }));
        let viewport = parse_viewport(&json!({
            "cssVisualViewport": {"pageX": 0.0, "pageY": 0.0,
                                  "clientWidth": 800.0, "clientHeight": 600.0}
        }));
        let ax = json!({"nodes": [
            {"nodeId": "root", "ignored": false, "role": {"value": "RootWebArea"},
             "childIds": ["dialog"]},
            {"nodeId": "dialog", "parentId": "root", "ignored": false,
             "backendDOMNodeId": 10, "role": {"value": "alertdialog"},
             "name": {"value": "Remove app"}, "childIds": ["cancel", "remove"]},
            {"nodeId": "cancel", "parentId": "dialog", "ignored": false,
             "backendDOMNodeId": 11, "role": {"value": "button"},
             "name": {"value": "Cancel"}, "childIds": []},
            {"nodeId": "remove", "parentId": "dialog", "ignored": false,
             "backendDOMNodeId": 12, "role": {"value": "button"},
             "name": {"value": "Remove"}, "childIds": []}
        ]});
        let document = compose_accessibility_tree(&ax, &dom, &layout, &viewport, frame());
        let page = document.page(0, 300, None, None);
        let actions = page
            .selected
            .iter()
            .filter(|node| !node.actions.is_empty())
            .filter_map(|node| node.name.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(actions, vec!["Cancel", "Remove"]);
        assert_eq!(page.omissions.page_occluded, 0);
    }

    #[test]
    fn semantic_text_removes_icon_glyphs_and_nonbreaking_spaces() {
        assert_eq!(
            clean_semantic_text("\u{e001} Reply\u{00a0}now \u{f8ff}".to_owned()).as_deref(),
            Some("Reply now")
        );
    }
}
