//! Session-scoped target / tab / page-ref capability store.
//!
//! Browser target ids (`bt-<uuid>`), tab ids (`tab-<uuid>`) and page refs
//! (`p<snapshot>:<index>`) are opaque, session-scoped capabilities:
//! they only resolve in the session that minted them, and the whole
//! namespace is dropped when that session ends. Ids are minted from a
//! process-global counter so they never collide across sessions — a
//! capability leaked into another session simply fails to resolve.
//!
//! Refs map internally to CDP `backendNodeId`s plus a [`FrameRef`]
//! recording which frame (main, same-process iframe, or OOPIF child
//! target) minted the node and that frame's document identity
//! (`frame_id` + `loader_id`) at snapshot time. Navigation invalidates
//! every snapshot of the navigated tab; stale refs refuse with
//! `browser_ref_stale`, and frame identity is re-proven against the
//! live frame tree before any mutation.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use uuid::Uuid;

pub(crate) const MAX_SEMANTIC_CONTINUATIONS: usize = 1_024;

use super::refusal::{BrowserRefusal, BrowserRefusalCode};
use super::semantic::SemanticDocument;
use super::types::{
    BindingQuality, EndpointAccessClass, EndpointTransport, ProcessFingerprint, Rect,
};

/// Browser action kinds proven for one semantic page ref.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserActionKind {
    Click,
    Type,
    Upload,
    Pointer,
    Scroll,
}

impl BrowserActionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Click => "click",
            Self::Type => "type",
            Self::Upload => "upload",
            Self::Pointer => "pointer",
            Self::Scroll => "scroll",
        }
    }
}

/// Browser-layout visibility. This is independent from native desktop
/// foreground or occlusion state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserVisibility {
    InViewport,
    NearViewport,
    Offscreen,
    CssHidden,
    NoLayout,
    PageOccluded,
    Unknown,
}

impl BrowserVisibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InViewport => "in_viewport",
            Self::NearViewport => "near_viewport",
            Self::Offscreen => "offscreen",
            Self::CssHidden => "css_hidden",
            Self::NoLayout => "no_layout",
            Self::PageOccluded => "page_occluded",
            Self::Unknown => "unknown",
        }
    }
}

/// Which frame kind a ref was minted in. Exposed on the wire as a
/// stable string via [`FrameKind::as_str`]; everything else about the
/// frame stays internal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    /// The tab's main frame (including composed shadow DOM inside it).
    Main,
    /// A same-process iframe walked via `contentDocument`.
    Iframe,
    /// An out-of-process iframe reached through a capability-tested
    /// child session beneath the tab's target.
    Oopif,
}

impl FrameKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Iframe => "iframe",
            Self::Oopif => "oopif",
        }
    }
}

/// CDP frame/document identity captured at snapshot time. The
/// `loader_id` changes on every document load, so equality against the
/// live frame tree proves the ref's document is still the one that was
/// snapshotted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameIdentity {
    pub frame_id: String,
    pub loader_id: String,
}

/// The frame a ref belongs to, with everything needed to re-prove that
/// frame before mutation. Invariants enforced at mint time:
/// - `kind != Main` ⇒ `identity` is `Some` (unprovable frames are
///   omitted from snapshots, never guessed).
/// - `kind == Oopif` ⇔ `oopif_target_id` is `Some`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameRef {
    pub kind: FrameKind,
    /// CDP target id of the OOPIF child target (contained beneath the
    /// bound tab), present only for `Oopif` refs.
    pub oopif_target_id: Option<String>,
    /// Document identity at snapshot time. `None` only on the
    /// v1-compat main-frame path where the endpoint cannot report a
    /// frame tree; node liveness checks remain the backstop there.
    pub identity: Option<FrameIdentity>,
}

impl FrameRef {
    /// The v1-compat main-frame ref: no frame tree available, identity
    /// unproven, mutation falls back to node-liveness checks only.
    pub fn main_unproven() -> Self {
        Self {
            kind: FrameKind::Main,
            oopif_target_id: None,
            identity: None,
        }
    }
}

/// One interactive element captured in a page snapshot.
#[derive(Debug, Clone, Serialize)]
pub struct RefEntry {
    /// CDP backendNodeId — internal only, never exposed to callers.
    /// Valid in the tab's own session, or in the OOPIF child session
    /// named by `frame.oopif_target_id`.
    #[serde(skip_serializing)]
    pub backend_node_id: i64,
    pub node_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Semantic action kinds. Empty for legacy DOM refs, whose existing
    /// mutation behavior remains compatible during the v2 migration.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<BrowserActionKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<BrowserVisibility>,
    /// Semantic refs enforce their declared action set. Legacy DOM refs keep
    /// their existing permissive behavior during the versioned migration.
    #[serde(skip_serializing)]
    pub semantic: bool,
    /// Context-only refs are read capabilities into the stored semantic
    /// document. They never resolve through the mutation/scope namespace.
    #[serde(skip_serializing)]
    pub(crate) context_only: bool,
    /// Exact snapshot-local semantic identity. Unlike backend ids this remains
    /// unambiguous for backend-less AX nodes and duplicate ids across frames.
    #[serde(skip_serializing)]
    pub(crate) semantic_node: Option<SemanticNodeIdentity>,
    /// Frame identity — internal; only the kind string is surfaced.
    #[serde(skip_serializing)]
    pub frame: FrameRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticNodeIdentity {
    pub(crate) ax_id: String,
    pub(crate) document_order: usize,
    pub(crate) frame: FrameRef,
}

#[derive(Debug, Clone)]
pub struct SnapshotRecord {
    pub id: u64,
    pub generation: u64,
    pub url: String,
    /// index → entry; the external ref is `p<id>:<index>`.
    pub refs: HashMap<u32, RefEntry>,
    pub(crate) next_ref_index: u32,
    pub(crate) semantic: Option<SemanticDocument>,
    pub(crate) semantic_root_identity: Option<FrameIdentity>,
    pub(crate) semantic_oopif_supported: bool,
    pub(crate) semantic_oopif_frames: usize,
    pub(crate) continuations: HashMap<String, SemanticContinuation>,
}

#[derive(Debug, Clone)]
pub(crate) enum SemanticContinuation {
    Matches {
        offset: usize,
        query: Option<String>,
        scope_backend_node_id: Option<i64>,
        oopif_supported: bool,
        oopif_frames: usize,
    },
    Context {
        anchor_ref: String,
        group: SemanticNodeIdentity,
        group_ref: String,
        start: usize,
        backwards: bool,
        oopif_supported: bool,
        oopif_frames: usize,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedSemanticRef {
    pub(crate) snapshot: SnapshotRecord,
    pub(crate) entry: RefEntry,
}

#[derive(Debug, Clone)]
pub struct TabRecord {
    pub tab_id: String,
    pub cdp_target_id: String,
    pub title: String,
    pub url: String,
    /// Native-window selection proof captured at bind time. `None` means the
    /// selected tab could not be proven without activating or foregrounding a
    /// page, so the public `active` field must be JSON null.
    pub active: Option<bool>,
    pub generation: u64,
    pub snapshots: HashMap<u64, SnapshotRecord>,
}

/// One bound browser target: the full evidence set captured at bind
/// time, revalidated before every mutation.
#[derive(Debug, Clone)]
pub struct TargetRecord {
    pub target_id: String,
    pub pid: i64,
    pub window_id: u64,
    pub ws_url: String,
    pub endpoint_owner_pid: i64,
    pub endpoint_transport: EndpointTransport,
    pub endpoint_access_class: EndpointAccessClass,
    /// CDP connection generation that minted this capability. Zero denotes
    /// the legacy/non-grant route.
    pub generation: u64,
    /// Internal transport owner that proved an existing-profile grant or a
    /// driver-owned browser lifecycle. The public session remains the target
    /// namespace and is checked independently.
    pub transport_session: Option<String>,
    pub fingerprint: ProcessFingerprint,
    pub native_title: String,
    pub native_bounds: Rect,
    pub cdp_target_id: String,
    /// CDP browser window id, or None for the exact single-page embedded route.
    pub cdp_window_id: Option<i64>,
    pub quality: BindingQuality,
    pub tabs: HashMap<String, TabRecord>,
}

#[derive(Default)]
struct SessionTargets {
    targets: HashMap<String, TargetRecord>,
}

/// Parse an external page ref of the form `p<snapshot>:<index>`.
/// Anything else — including refs from other namespaces such as the
/// accessibility `element_index` / element-token space — is rejected.
pub fn parse_ref(external: &str) -> Option<(u64, u32)> {
    let rest = external.strip_prefix('p')?;
    let (snap, idx) = rest.split_once(':')?;
    // Reject leading '+', whitespace, empty parts: only plain digits.
    if snap.is_empty()
        || idx.is_empty()
        || !snap.bytes().all(|b| b.is_ascii_digit())
        || !idx.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    Some((snap.parse().ok()?, idx.parse().ok()?))
}

/// Format the external ref for a snapshot/index pair.
pub fn format_ref(snapshot_id: u64, index: u32) -> String {
    format!("p{snapshot_id}:{index}")
}

pub struct BrowserStore {
    inner: Mutex<HashMap<String, SessionTargets>>,
}

impl BrowserStore {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    fn next(&self) -> u64 {
        static NEXT_BROWSER_SNAPSHOT_ID: AtomicU64 = AtomicU64::new(1);
        NEXT_BROWSER_SNAPSHOT_ID.fetch_add(1, Ordering::Relaxed)
    }

    /// Mint a target id and insert the record under `session`.
    /// Panics never; the caller has already enforced that `session` is
    /// an explicit (non-default) session.
    pub fn mint_target(&self, session: &str, mut record: TargetRecord) -> String {
        let id = format!("bt-{}", Uuid::new_v4());
        record.target_id = id.clone();
        self.inner
            .lock()
            .unwrap()
            .entry(session.to_owned())
            .or_default()
            .targets
            .insert(id.clone(), record);
        id
    }

    /// Mint a fresh tab id (caller stores it via [`Self::update_target`]).
    pub fn mint_tab_id(&self) -> String {
        format!("tab-{}", Uuid::new_v4())
    }

    /// Mint a fresh snapshot id.
    pub fn mint_snapshot_id(&self) -> u64 {
        self.next()
    }

    /// Look up a target capability. Unknown ids — including ids minted
    /// by a *different* session — refuse with `browser_binding_stale`:
    /// the capability is simply not valid here.
    pub fn get_target(
        &self,
        session: &str,
        target_id: &str,
    ) -> Result<TargetRecord, BrowserRefusal> {
        self.inner
            .lock()
            .unwrap()
            .get(session)
            .and_then(|s| s.targets.get(target_id))
            .cloned()
            .ok_or_else(|| {
                BrowserRefusal::new(
                    BrowserRefusalCode::BrowserBindingStale,
                    format!(
                        "target {target_id} is not a live binding in this session — \
                         re-run get_browser_state with pid + window_id"
                    ),
                )
            })
    }

    /// Mutate a stored target in place. No-op if it disappeared.
    pub fn update_target(&self, session: &str, target_id: &str, f: impl FnOnce(&mut TargetRecord)) {
        if let Some(rec) = self
            .inner
            .lock()
            .unwrap()
            .get_mut(session)
            .and_then(|s| s.targets.get_mut(target_id))
        {
            f(rec);
        }
    }

    /// Resolve an external page ref to a backendNodeId within one tab's
    /// live snapshot namespace.
    pub fn resolve_ref(
        &self,
        session: &str,
        target_id: &str,
        tab_id: &str,
        external: &str,
    ) -> Result<RefEntry, BrowserRefusal> {
        let (snap, idx) = parse_ref(external).ok_or_else(|| {
            BrowserRefusal::new(
                BrowserRefusalCode::BrowserRefStale,
                format!(
                    "ref {external:?} is not a browser page ref — expected the \
                     p<snapshot>:<index> namespace from get_browser_state"
                ),
            )
        })?;
        let target = self.get_target(session, target_id)?;
        let tab = target.tabs.get(tab_id).ok_or_else(|| {
            BrowserRefusal::new(
                BrowserRefusalCode::BrowserTabNotFound,
                format!("tab {tab_id} is not known for target {target_id}"),
            )
        })?;
        tab.snapshots
            .get(&snap)
            .filter(|snapshot| snapshot.generation == target.generation)
            .and_then(|snapshot| snapshot.refs.get(&idx))
            .filter(|entry| !entry.context_only)
            .cloned()
            .ok_or_else(|| {
                BrowserRefusal::new(
                    BrowserRefusalCode::BrowserRefStale,
                    format!(
                        "ref {external} is stale — the page navigated or the snapshot \
                         was superseded; re-run get_browser_state to re-snapshot"
                    ),
                )
            })
    }

    pub(crate) fn resolve_semantic_ref(
        &self,
        session: &str,
        target_id: &str,
        tab_id: &str,
        external: &str,
    ) -> Result<ResolvedSemanticRef, BrowserRefusal> {
        let (snap, idx) = parse_ref(external).ok_or_else(|| {
            BrowserRefusal::new(
                BrowserRefusalCode::BrowserRefStale,
                "context_ref must be an issued p<snapshot>:<index> semantic ref",
            )
        })?;
        let target = self.get_target(session, target_id)?;
        let tab = target.tabs.get(tab_id).ok_or_else(|| {
            BrowserRefusal::new(
                BrowserRefusalCode::BrowserTabNotFound,
                format!("tab {tab_id} is not known for target {target_id}"),
            )
        })?;
        let snapshot = tab
            .snapshots
            .get(&snap)
            .filter(|snapshot| {
                snapshot.generation == target.generation && snapshot.semantic.is_some()
            })
            .ok_or_else(|| {
                BrowserRefusal::new(
                    BrowserRefusalCode::BrowserRefStale,
                    "context_ref is stale, foreign, or does not name a semantic snapshot",
                )
            })?;
        let entry = snapshot
            .refs
            .get(&idx)
            .filter(|entry| entry.semantic && entry.semantic_node.is_some())
            .ok_or_else(|| {
                BrowserRefusal::new(
                    BrowserRefusalCode::BrowserRefStale,
                    "context_ref was not issued as a semantic/content ref in this snapshot",
                )
            })?;
        Ok(ResolvedSemanticRef {
            snapshot: snapshot.clone(),
            entry: entry.clone(),
        })
    }

    /// Resolve an opaque continuation within the same session, target, tab,
    /// snapshot generation, and currently-live snapshot namespace.
    pub(crate) fn take_semantic_continuation(
        &self,
        session: &str,
        target_id: &str,
        tab_id: &str,
        token: &str,
    ) -> Result<(SnapshotRecord, SemanticContinuation), BrowserRefusal> {
        let mut store = self.inner.lock().unwrap();
        let target = store
            .get_mut(session)
            .and_then(|session| session.targets.get_mut(target_id))
            .ok_or_else(|| {
                BrowserRefusal::new(
                    BrowserRefusalCode::BrowserBindingStale,
                    "the continuation target is not live in this session",
                )
            })?;
        let generation = target.generation;
        let tab = target.tabs.get_mut(tab_id).ok_or_else(|| {
            BrowserRefusal::new(
                BrowserRefusalCode::BrowserTabNotFound,
                format!("tab {tab_id} is not known for target {target_id}"),
            )
        })?;
        let snapshot = tab
            .snapshots
            .values_mut()
            .find(|snapshot| {
                snapshot.generation == generation
                    && snapshot.semantic.is_some()
                    && snapshot.continuations.contains_key(token)
            })
            .ok_or_else(|| {
                BrowserRefusal::new(
                    BrowserRefusalCode::BrowserRefStale,
                    "the semantic continuation is stale or does not belong to this session and tab",
                )
            })?;
        let continuation = snapshot
            .continuations
            .remove(token)
            .expect("located continuation must exist");
        Ok((snapshot.clone(), continuation))
    }

    pub(crate) fn reserve_semantic_ref_indices(
        &self,
        session: &str,
        target_id: &str,
        tab_id: &str,
        snapshot_id: u64,
        count: usize,
    ) -> Result<u32, BrowserRefusal> {
        let mut store = self.inner.lock().unwrap();
        let target = store
            .get_mut(session)
            .and_then(|session| session.targets.get_mut(target_id))
            .ok_or_else(|| {
                BrowserRefusal::new(
                    BrowserRefusalCode::BrowserBindingStale,
                    "semantic target is no longer live",
                )
            })?;
        let generation = target.generation;
        let snapshot = target
            .tabs
            .get_mut(tab_id)
            .and_then(|tab| tab.snapshots.get_mut(&snapshot_id))
            .filter(|snapshot| snapshot.generation == generation && snapshot.semantic.is_some())
            .ok_or_else(|| {
                BrowserRefusal::new(
                    BrowserRefusalCode::BrowserRefStale,
                    "semantic snapshot is no longer live",
                )
            })?;
        let count = u32::try_from(count).map_err(|_| {
            BrowserRefusal::new(
                BrowserRefusalCode::BrowserRefStale,
                "semantic ref reservation is too large",
            )
        })?;
        let start = snapshot.next_ref_index;
        snapshot.next_ref_index = start.checked_add(count).ok_or_else(|| {
            BrowserRefusal::new(
                BrowserRefusalCode::BrowserRefStale,
                "semantic ref namespace is exhausted",
            )
        })?;
        Ok(start)
    }

    pub(crate) fn commit_reserved_semantic_refs(
        &self,
        session: &str,
        target_id: &str,
        tab_id: &str,
        snapshot_id: u64,
        refs: HashMap<u32, RefEntry>,
        continuations: Vec<(String, SemanticContinuation)>,
    ) -> Result<(), BrowserRefusal> {
        let mut store = self.inner.lock().unwrap();
        let target = store
            .get_mut(session)
            .and_then(|session| session.targets.get_mut(target_id))
            .ok_or_else(|| {
                BrowserRefusal::new(
                    BrowserRefusalCode::BrowserBindingStale,
                    "semantic target is no longer live",
                )
            })?;
        let generation = target.generation;
        let snapshot = target
            .tabs
            .get_mut(tab_id)
            .and_then(|tab| tab.snapshots.get_mut(&snapshot_id))
            .filter(|snapshot| snapshot.generation == generation && snapshot.semantic.is_some())
            .ok_or_else(|| {
                BrowserRefusal::new(
                    BrowserRefusalCode::BrowserRefStale,
                    "semantic snapshot changed before continuation commit",
                )
            })?;
        if snapshot
            .continuations
            .len()
            .checked_add(continuations.len())
            .is_none_or(|count| count > MAX_SEMANTIC_CONTINUATIONS)
        {
            return Err(BrowserRefusal::new(
                BrowserRefusalCode::BrowserRefStale,
                "the semantic snapshot continuation capacity is exhausted",
            ));
        }
        if refs
            .keys()
            .any(|idx| *idx >= snapshot.next_ref_index || snapshot.refs.contains_key(idx))
        {
            return Err(BrowserRefusal::new(
                BrowserRefusalCode::BrowserRefStale,
                "reserved semantic ref range is no longer exclusive",
            ));
        }
        snapshot.refs.extend(refs);
        for (token, continuation) in continuations {
            snapshot.continuations.insert(token, continuation);
        }
        Ok(())
    }

    /// Drop every snapshot of one tab (navigation invalidates refs).
    pub fn invalidate_tab_snapshots(&self, session: &str, target_id: &str, tab_id: &str) {
        self.update_target(session, target_id, |rec| {
            if let Some(tab) = rec.tabs.get_mut(tab_id) {
                tab.snapshots.clear();
            }
        });
    }

    /// Drop the whole namespace for an ended session. Wired to
    /// `session::register_session_end_hook` by the engine.
    pub fn remove_session(&self, session: &str) {
        self.inner.lock().unwrap().remove(session);
    }

    /// Invalidate every capability minted for one browser endpoint before a
    /// reconnect generation becomes visible.
    pub fn invalidate_endpoint_generation(&self, pid: i64, generation: u64) -> usize {
        let mut removed = 0;
        for session in self.inner.lock().unwrap().values_mut() {
            let before = session.targets.len();
            session
                .targets
                .retain(|_, target| !(target.pid == pid && target.generation == generation));
            removed += before - session.targets.len();
        }
        removed
    }

    /// Number of live targets in a session (diagnostics/tests).
    pub fn target_count(&self, session: &str) -> usize {
        self.inner
            .lock()
            .unwrap()
            .get(session)
            .map_or(0, |s| s.targets.len())
    }
}

impl Default for BrowserStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::types::BindingQuality;

    fn record() -> TargetRecord {
        TargetRecord {
            target_id: String::new(),
            pid: 42,
            window_id: 7,
            ws_url: "ws://127.0.0.1:9222/devtools/browser/x".into(),
            endpoint_owner_pid: 42,
            endpoint_transport: EndpointTransport::LegacyJsonVersion,
            endpoint_access_class: EndpointAccessClass::EmbeddedApplication,
            generation: 0,
            transport_session: None,
            fingerprint: ProcessFingerprint {
                pid: 42,
                start_time: Some(1),
                executable: None,
            },
            native_title: "Docs - Chrome".into(),
            native_bounds: Rect::new(0.0, 0.0, 800.0, 600.0),
            cdp_target_id: "CDP1".into(),
            cdp_window_id: Some(11),
            quality: BindingQuality::Exact,
            tabs: HashMap::new(),
        }
    }

    fn store_with_ref() -> (BrowserStore, String, String, String) {
        let store = BrowserStore::new();
        let tid = store.mint_target("sess-a", record());
        let tab_id = store.mint_tab_id();
        let snap_id = store.mint_snapshot_id();
        let ext = format_ref(snap_id, 0);
        store.update_target("sess-a", &tid, |rec| {
            let mut refs = HashMap::new();
            refs.insert(
                0,
                RefEntry {
                    backend_node_id: 555,
                    node_name: "button".into(),
                    label: Some("Submit".into()),
                    actions: Vec::new(),
                    visibility: None,
                    semantic: false,
                    context_only: false,
                    semantic_node: None,
                    frame: FrameRef {
                        kind: FrameKind::Main,
                        oopif_target_id: None,
                        identity: Some(FrameIdentity {
                            frame_id: "F_MAIN".into(),
                            loader_id: "L1".into(),
                        }),
                    },
                },
            );
            rec.tabs.insert(
                tab_id.clone(),
                TabRecord {
                    tab_id: tab_id.clone(),
                    cdp_target_id: "CDP1".into(),
                    title: "Example".into(),
                    url: "https://example.test".into(),
                    active: Some(true),
                    generation: 0,
                    snapshots: HashMap::from([(
                        snap_id,
                        SnapshotRecord {
                            id: snap_id,
                            generation: 0,
                            url: "https://example.test".into(),
                            refs,
                            next_ref_index: 1,
                            semantic: None,
                            semantic_root_identity: None,
                            semantic_oopif_supported: false,
                            semantic_oopif_frames: 0,
                            continuations: HashMap::new(),
                        },
                    )]),
                },
            );
        });
        (store, tid, tab_id, ext)
    }

    #[test]
    fn ref_parsing_accepts_only_the_p_namespace() {
        assert_eq!(parse_ref("p12:5"), Some((12, 5)));
        assert_eq!(parse_ref("p0:0"), Some((0, 0)));
        for bad in [
            "e12", "12:5", "p:5", "p12:", "p-1:2", "p 1:2", "p1:+2", "p1", "",
        ] {
            assert_eq!(parse_ref(bad), None, "must reject {bad:?}");
        }
    }

    #[test]
    fn resolve_ref_happy_path() {
        let (store, tid, tab, ext) = store_with_ref();
        let entry = store.resolve_ref("sess-a", &tid, &tab, &ext).unwrap();
        assert_eq!(entry.backend_node_id, 555);
        assert_eq!(entry.node_name, "button");
        assert_eq!(entry.frame.kind, FrameKind::Main);
        assert_eq!(
            entry.frame.identity.as_ref().map(|i| i.loader_id.as_str()),
            Some("L1")
        );
    }

    #[test]
    fn ref_entry_serialization_hides_internal_identifiers() {
        let (store, tid, tab, ext) = store_with_ref();
        let entry = store.resolve_ref("sess-a", &tid, &tab, &ext).unwrap();
        let v = serde_json::to_value(&entry).unwrap();
        assert!(v.get("backend_node_id").is_none(), "{v}");
        assert!(v.get("frame").is_none(), "frame identity is internal: {v}");
    }

    #[test]
    fn target_ids_do_not_resolve_in_a_foreign_session() {
        let (store, tid, tab, ext) = store_with_ref();
        let err = store.resolve_ref("sess-b", &tid, &tab, &ext).unwrap_err();
        assert_eq!(err.code, BrowserRefusalCode::BrowserBindingStale);
        let err = store.get_target("sess-b", &tid).unwrap_err();
        assert_eq!(err.code, BrowserRefusalCode::BrowserBindingStale);
    }

    #[test]
    fn page_refs_do_not_alias_across_runtime_owned_stores() {
        let (_store_a, _target_a, _tab_a, ref_a) = store_with_ref();
        let (store_b, target_b, tab_b, ref_b) = store_with_ref();
        assert_ne!(ref_a, ref_b);
        let error = store_b
            .resolve_ref("sess-a", &target_b, &tab_b, &ref_a)
            .unwrap_err();
        assert_eq!(error.code, BrowserRefusalCode::BrowserRefStale);
    }

    #[test]
    fn foreign_namespace_refs_are_refused_as_stale() {
        let (store, tid, tab, _) = store_with_ref();
        // An accessibility element_index-style ref must not resolve.
        let err = store.resolve_ref("sess-a", &tid, &tab, "e42").unwrap_err();
        assert_eq!(err.code, BrowserRefusalCode::BrowserRefStale);
    }

    #[test]
    fn unknown_snapshot_or_index_is_stale() {
        let (store, tid, tab, _) = store_with_ref();
        let err = store
            .resolve_ref("sess-a", &tid, &tab, "p999999:0")
            .unwrap_err();
        assert_eq!(err.code, BrowserRefusalCode::BrowserRefStale);
    }

    #[test]
    fn unknown_tab_is_tab_not_found() {
        let (store, tid, _, ext) = store_with_ref();
        let err = store
            .resolve_ref("sess-a", &tid, "tab999", &ext)
            .unwrap_err();
        assert_eq!(err.code, BrowserRefusalCode::BrowserTabNotFound);
    }

    #[test]
    fn navigation_invalidates_tab_refs() {
        let (store, tid, tab, ext) = store_with_ref();
        assert!(store.resolve_ref("sess-a", &tid, &tab, &ext).is_ok());
        store.invalidate_tab_snapshots("sess-a", &tid, &tab);
        let err = store.resolve_ref("sess-a", &tid, &tab, &ext).unwrap_err();
        assert_eq!(err.code, BrowserRefusalCode::BrowserRefStale);
    }

    #[test]
    fn remove_session_drops_the_whole_namespace() {
        let (store, tid, tab, ext) = store_with_ref();
        assert_eq!(store.target_count("sess-a"), 1);
        store.remove_session("sess-a");
        assert_eq!(store.target_count("sess-a"), 0);
        let err = store.resolve_ref("sess-a", &tid, &tab, &ext).unwrap_err();
        assert_eq!(err.code, BrowserRefusalCode::BrowserBindingStale);
    }

    #[test]
    fn semantic_continuation_capacity_refuses_atomically_without_eviction() {
        let (store, tid, tab, ext) = store_with_ref();
        let (snapshot_id, _) = parse_ref(&ext).unwrap();
        store.update_target("sess-a", &tid, |target| {
            let snapshot = target
                .tabs
                .get_mut(&tab)
                .unwrap()
                .snapshots
                .get_mut(&snapshot_id)
                .unwrap();
            snapshot.semantic = Some(SemanticDocument::default());
            let entry = snapshot.refs.get_mut(&0).unwrap();
            entry.semantic = true;
            entry.semantic_node = Some(SemanticNodeIdentity {
                ax_id: "root".into(),
                document_order: 0,
                frame: entry.frame.clone(),
            });
        });
        let continuations = (0..MAX_SEMANTIC_CONTINUATIONS)
            .map(|idx| {
                (
                    format!("bc-{idx}"),
                    SemanticContinuation::Matches {
                        offset: idx,
                        query: None,
                        scope_backend_node_id: None,
                        oopif_supported: false,
                        oopif_frames: 0,
                    },
                )
            })
            .collect();
        store
            .commit_reserved_semantic_refs(
                "sess-a",
                &tid,
                &tab,
                snapshot_id,
                HashMap::new(),
                continuations,
            )
            .unwrap();

        let error = store
            .commit_reserved_semantic_refs(
                "sess-a",
                &tid,
                &tab,
                snapshot_id,
                HashMap::new(),
                vec![(
                    "bc-overflow".into(),
                    SemanticContinuation::Matches {
                        offset: usize::MAX,
                        query: None,
                        scope_backend_node_id: None,
                        oopif_supported: false,
                        oopif_frames: 0,
                    },
                )],
            )
            .unwrap_err();
        assert_eq!(error.code, BrowserRefusalCode::BrowserRefStale);
        assert!(store
            .take_semantic_continuation("sess-a", &tid, &tab, "bc-0")
            .is_ok());
        assert!(store
            .take_semantic_continuation("sess-a", &tid, &tab, "bc-overflow")
            .is_err());
    }

    #[test]
    fn generation_invalidation_is_browser_wide_and_exact() {
        let store = BrowserStore::new();
        let mut old = record();
        old.generation = 1;
        let mut current = record();
        current.generation = 2;
        let mut other_process = record();
        other_process.pid = 99;
        other_process.endpoint_owner_pid = 99;
        other_process.fingerprint.pid = 99;
        other_process.generation = 1;
        store.mint_target("session-a", old);
        store.mint_target("session-b", current);
        store.mint_target("session-c", other_process);

        assert_eq!(store.invalidate_endpoint_generation(42, 1), 1);
        assert_eq!(store.target_count("session-a"), 0);
        assert_eq!(store.target_count("session-b"), 1);
        assert_eq!(store.target_count("session-c"), 1);
    }

    #[test]
    fn minted_ids_are_unique_across_sessions() {
        let store = BrowserStore::new();
        let a = store.mint_target("s1", record());
        let b = store.mint_target("s2", record());
        assert_ne!(a, b, "capability ids must never collide across sessions");
    }
}
