//! Snapshot-bound, sparse AT-SPI object identities for X11 indexed clicks.
//! Integer indices are observation addresses, never native object identities.

use super::{AtspiIdentity, AtspiNode};
use cua_driver_core::element_cache::ElementCacheCore;
use cua_driver_core::element_token::{token_for, STALE_TOKEN_ERROR};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub runtime_scope: String,
    pub pid: u32,
    pub xid: u64,
}

pub struct CachedSnapshot {
    pub snapshot_id: u32,
    pub elements: HashMap<usize, AtspiIdentity>,
}

pub struct ElementCache {
    core: ElementCacheCore<CacheKey, CachedSnapshot>,
}

impl ElementCache {
    pub fn new() -> Self {
        Self {
            core: ElementCacheCore::new(),
        }
    }

    fn key(pid: u32, xid: u64) -> CacheKey {
        CacheKey {
            runtime_scope: cua_driver_core::tool::current_dispatch_runtime_scope()
                .unwrap_or_else(|| "legacy".to_owned()),
            pid,
            xid,
        }
    }

    pub fn update(&self, pid: u32, xid: u64, snapshot_id: u32, nodes: &[AtspiNode]) {
        let elements = nodes
            .iter()
            .filter_map(|node| Some((node.element_index?, node.identity.clone()?)))
            .collect();
        self.core.insert(
            Self::key(pid, xid),
            CachedSnapshot {
                snapshot_id,
                elements,
            },
        );
    }

    /// Require the exact caller snapshot while retaining an owned identity.
    /// A racing observation must never replace an old token's target map.
    pub fn observed_identity(
        &self,
        pid: u32,
        xid: u64,
        idx: usize,
        token: &str,
    ) -> Result<AtspiIdentity, String> {
        self.core
            .with_snapshot(&Self::key(pid, xid), |snapshot| {
                (token_for(snapshot.snapshot_id, idx) == token)
                    .then(|| snapshot.elements.get(&idx).cloned())
                    .flatten()
            })
            .flatten()
            .ok_or_else(|| format!("{STALE_TOKEN_ERROR}: observed AT-SPI identity unavailable"))
    }
}

impl Default for ElementCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(path: &str) -> AtspiIdentity {
        AtspiIdentity {
            bus_name: ":1.9".into(),
            path: path.into(),
            frame_bus_name: ":1.9".into(),
            frame_path: "/frame".into(),
        }
    }

    #[test]
    fn sparse_snapshot_lookup_never_substitutes_replacement_or_other_scope() {
        let cache = ElementCache::new();
        cua_driver_core::tool::with_runtime_scope("index-a".into(), || {
            cache.core.insert(
                ElementCache::key(123, 456),
                CachedSnapshot {
                    snapshot_id: 1,
                    elements: HashMap::from([(1578, identity("/ok"))]),
                },
            );
            assert_eq!(
                cache
                    .observed_identity(123, 456, 1578, &token_for(1, 1578))
                    .unwrap(),
                identity("/ok")
            );
            assert!(cache
                .observed_identity(123, 456, 0, &token_for(1, 0))
                .is_err());
            assert!(cache
                .observed_identity(123, 457, 1578, &token_for(1, 1578))
                .is_err());
            cache.core.insert(
                ElementCache::key(123, 456),
                CachedSnapshot {
                    snapshot_id: 2,
                    elements: HashMap::from([(1578, identity("/cancel"))]),
                },
            );
            assert!(cache
                .observed_identity(123, 456, 1578, &token_for(1, 1578))
                .is_err());
        });
        cua_driver_core::tool::with_runtime_scope("index-b".into(), || {
            assert!(cache
                .observed_identity(123, 456, 1578, &token_for(2, 1578))
                .is_err());
        });
    }
}
