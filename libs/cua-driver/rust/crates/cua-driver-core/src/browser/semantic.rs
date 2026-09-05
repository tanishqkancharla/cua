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
const NEAR_VIEWPORT_MARGIN: f64 = 1_000.0;
const MAX_SEMANTIC_TEXT_CHARS: usize = 1_000;
const MAX_LINK_DESTINATION_CHARS: usize = 2_048;

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
    pub(crate) parent_group: Option<SemanticNode>,
    pub(crate) selected_nodes: usize,
    pub(crate) total_nodes: usize,
    pub(crate) before_omitted: usize,
    pub(crate) after_omitted: usize,
    pub(crate) group_complete: bool,
    pub(crate) document_collection_complete: bool,
    pub(crate) omissions: OmissionCounts,
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
        }
    }

    pub(crate) fn context(
        &self,
        anchor: &RefEntry,
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
        let group_idx = *groups
            .first()
            .ok_or(SemanticContextError::GroupUnavailable)?;
        let parent_group = groups.get(1).map(|idx| self.nodes[*idx].clone());

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
        let anchor_position = eligible
            .iter()
            .position(|idx| *idx == *anchor_idx)
            .ok_or(SemanticContextError::AnchorUnproven)?;
        let start = if group_idx == *anchor_idx {
            0
        } else {
            anchor_position.saturating_sub(CONTEXT_BEFORE_NODES)
        };
        let end = (if group_idx == *anchor_idx {
            CONTEXT_BEFORE_NODES + 1 + CONTEXT_AFTER_NODES
        } else {
            anchor_position + 1 + CONTEXT_AFTER_NODES
        })
        .min(eligible.len());
        let selected_indices = &eligible[start..end];
        let selected = with_ancestors(&self.nodes, selected_indices);
        let mut context_order =
            ancestor_indices(&self.nodes, &by_ax_id, group_idx).unwrap_or_default();
        context_order.reverse();
        context_order.extend(member_order);
        let outline = render_outline_ordered(&self.nodes, &selected, context_order);
        let before_omitted = start;
        let after_omitted = eligible.len().saturating_sub(end);
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
            parent_group,
            selected_nodes: selected_indices.len(),
            total_nodes: eligible.len(),
            before_omitted,
            after_omitted,
            group_complete,
            document_collection_complete: self.complete,
            omissions,
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

    let mut nodes = Vec::new();
    for (fallback_order, ax) in ax_nodes.iter().enumerate() {
        if ax.get("ignored").and_then(Value::as_bool) == Some(true) {
            continue;
        }
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
        if role == "inlinetextbox" {
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
        nodes.push(SemanticNode {
            ax_id,
            parent_ax_id: ax
                .get("parentId")
                .and_then(Value::as_str)
                .map(str::to_owned),
            child_ax_ids: ax
                .get("childIds")
                .and_then(Value::as_array)
                .map(|ids| {
                    ids.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
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
    let names: HashMap<String, String> = nodes
        .iter()
        .filter_map(|node| node.name.clone().map(|name| (node.ax_id.clone(), name)))
        .collect();
    nodes.retain(|node| {
        if node.role != "statictext" && node.role != "text" {
            return true;
        }
        let Some(name) = node.name.as_deref() else {
            return false;
        };
        node.parent_ax_id
            .as_ref()
            .and_then(|parent| names.get(parent))
            .is_none_or(|parent_name| parent_name != name)
    });
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
