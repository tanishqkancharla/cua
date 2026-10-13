use super::bindings::AXUIElementRef;
use super::tree::AXNode;
use core_foundation::base::{CFEqual, CFHash, CFRelease, CFRetain, CFTypeRef};
use cua_driver_core::snapshot_store::{SnapshotPayload, SnapshotStore};
use std::collections::HashMap;

pub struct RetainedElement(usize);

impl RetainedElement {
    pub fn as_ptr(&self) -> usize {
        self.0
    }

    /// Take a +1 reference on an AX element pointer (0 is kept as null).
    ///
    /// # Safety
    ///
    /// A nonzero `ptr` must be a live `AXUIElementRef`.
    pub unsafe fn retain(ptr: usize) -> Self {
        if ptr != 0 {
            unsafe { CFRetain(ptr as AXUIElementRef as CFTypeRef) };
        }
        Self(ptr)
    }
}

impl Clone for RetainedElement {
    fn clone(&self) -> Self {
        unsafe { Self::retain(self.0) }
    }
}

impl Drop for RetainedElement {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe { CFRelease(self.0 as AXUIElementRef as CFTypeRef) };
        }
    }
}

pub struct AxSnapshot {
    pub elements: Vec<usize>,
    pub object_ids: Vec<Option<String>>,
}

impl AxSnapshot {
    pub fn from_nodes(nodes: &[AXNode]) -> Self {
        Self {
            elements: nodes
                .iter()
                .filter(|node| node.element_index.is_some())
                .map(|node| node.element_ptr)
                .collect(),
            object_ids: Vec::new(),
        }
    }
}

impl SnapshotPayload for AxSnapshot {
    type Element = RetainedElement;
    fn len(&self) -> usize {
        self.elements.len()
    }
    fn prepare_observation_identity(&mut self, previous: Option<&Self>) {
        // CFHash only narrows candidates; CFEqual proves native continuity.
        // Previous/current refs already belong to retained snapshot payloads.
        let mut old_by_hash: HashMap<usize, Vec<usize>> = HashMap::new();
        if let Some(old) = previous {
            for (index, &ptr) in old.elements.iter().enumerate() {
                if ptr != 0 && old.object_ids.get(index).and_then(Option::as_ref).is_some() {
                    old_by_hash
                        .entry(unsafe { CFHash(ptr as CFTypeRef) } as usize)
                        .or_default()
                        .push(index);
                }
            }
        }
        let mut current_by_hash: HashMap<usize, Vec<usize>> = HashMap::new();
        self.object_ids.clear();
        for (index, &ptr) in self.elements.iter().enumerate() {
            if ptr == 0 {
                self.object_ids.push(None);
                continue;
            }
            let hash = unsafe { CFHash(ptr as CFTypeRef) } as usize;
            let prior_id = previous.and_then(|old| {
                old_by_hash.get(&hash)?.iter().find_map(|&prior| {
                    (unsafe { CFEqual(ptr as CFTypeRef, old.elements[prior] as CFTypeRef) } != 0)
                        .then(|| old.object_ids[prior].clone())
                        .flatten()
                })
            });
            // Aliases share observation identity; duplicate IDs are ambiguous
            // to consumers and do not acquire additional input authority.
            let alias_id = current_by_hash.get(&hash).and_then(|candidates| {
                candidates.iter().find_map(|&prior| {
                    (unsafe { CFEqual(ptr as CFTypeRef, self.elements[prior] as CFTypeRef) } != 0)
                        .then(|| self.object_ids[prior].clone())
                        .flatten()
                })
            });
            self.object_ids.push(Some(
                prior_id
                    .or(alias_id)
                    .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string()),
            ));
            current_by_hash.entry(hash).or_default().push(index);
        }
    }
    fn retain(&self, index: usize) -> Option<RetainedElement> {
        self.elements
            .get(index)
            .map(|ptr| unsafe { RetainedElement::retain(*ptr) })
    }
}

impl Drop for AxSnapshot {
    fn drop(&mut self) {
        for ptr in &self.elements {
            if *ptr != 0 {
                unsafe { CFRelease(*ptr as AXUIElementRef as CFTypeRef) };
            }
        }
    }
}

pub type Snapshots = SnapshotStore<AxSnapshot>;

#[cfg(test)]
mod tests {
    use super::*;
    use core_foundation::base::{CFGetRetainCount, TCFType};
    use core_foundation::string::CFString;
    use cua_driver_core::element_token::{token_for, ResolvedElement};

    fn resolve(cache: &Snapshots, snapshot: u32, index: usize) -> Option<RetainedElement> {
        match cache
            .resolve(
                1,
                &serde_json::json!({ "element_token": token_for(snapshot, index) }),
            )
            .ok()?
        {
            ResolvedElement::Element { element, .. } => Some(element),
            _ => None,
        }
    }

    fn payload(ptr: usize) -> AxSnapshot {
        unsafe { CFRetain(ptr as CFTypeRef) };
        AxSnapshot {
            elements: vec![ptr],
            object_ids: Vec::new(),
        }
    }

    #[test]
    fn cf_object_identity_survives_reordering_without_extra_retains_or_cross_window_reuse() {
        let first = CFString::new("cua-driver-native-object-first-placeholder-long-value");
        let second = CFString::new("cua-driver-native-object-second-placeholder-long-value");
        let first_again = CFString::new("cua-driver-native-object-first-placeholder-long-value");
        let a = first.as_concrete_TypeRef() as usize;
        let b = second.as_concrete_TypeRef() as usize;
        let a_again = first_again.as_concrete_TypeRef() as usize;
        assert_ne!(a, a_again);
        let produce = |ptrs: &[usize]| {
            for &ptr in ptrs {
                unsafe { CFRetain(ptr as CFTypeRef) };
            }
            AxSnapshot {
                elements: ptrs.to_vec(),
                object_ids: Vec::new(),
            }
        };
        let store = Snapshots::new();
        let ids = |pid, window, snapshot| {
            store
                .with_current_payload(pid, window, snapshot, |s| s.object_ids.clone())
                .unwrap()
        };
        let base = unsafe { CFGetRetainCount(a as CFTypeRef) };
        let old = store.publish(1, 2, produce(&[a, b]));
        let original = ids(1, 2, old);
        assert_ne!(original[0], original[1]);
        assert_eq!(unsafe { CFGetRetainCount(a as CFTypeRef) }, base + 1);
        let new = store.publish(1, 2, produce(&[b, a_again]));
        assert_eq!(
            ids(1, 2, new),
            vec![original[1].clone(), original[0].clone()]
        );
        assert!(resolve(&store, old, 0).is_none());
        assert_eq!(unsafe { CFGetRetainCount(a as CFTypeRef) }, base);
        let alias = store.publish(1, 2, produce(&[a_again, a_again]));
        assert_eq!(
            ids(1, 2, alias),
            vec![original[0].clone(), original[0].clone()]
        );
        let sibling = store.publish(1, 3, produce(&[a_again]));
        assert_ne!(ids(1, 3, sibling)[0], original[0]);
        let other_pid = store.publish(9, 2, produce(&[a_again]));
        assert_ne!(ids(9, 2, other_pid)[0], original[0]);
        store.publish(1, 2, AxSnapshot::from_nodes(&[]));
        let recreated = store.publish(1, 2, produce(&[a_again]));
        assert_ne!(ids(1, 2, recreated)[0], original[0]);
        drop(store);
        assert_eq!(unsafe { CFGetRetainCount(a as CFTypeRef) }, base);
    }

    #[test]
    fn retained_element_survives_concurrent_snapshot_replace() {
        let value = CFString::new("cua-driver-uaf-test-element-placeholder");
        let ptr = value.as_concrete_TypeRef() as usize;
        let base = unsafe { CFGetRetainCount(ptr as CFTypeRef) };
        let cache = Snapshots::new();
        let snapshot = cache.publish(1, 2, payload(ptr));
        assert_eq!(unsafe { CFGetRetainCount(ptr as CFTypeRef) }, base + 1);
        let guard = resolve(&cache, snapshot, 0).unwrap();
        assert_eq!(unsafe { CFGetRetainCount(ptr as CFTypeRef) }, base + 2);
        cache.publish(1, 2, AxSnapshot::from_nodes(&[]));
        assert_eq!(unsafe { CFGetRetainCount(ptr as CFTypeRef) }, base + 1);
        assert!(resolve(&cache, snapshot, 0).is_none());
        drop(guard);
        assert_eq!(unsafe { CFGetRetainCount(ptr as CFTypeRef) }, base);
    }

    #[test]
    fn admitted_element_survives_cache_destruction_until_native_work_finishes() {
        let value = CFString::new("cua-driver-invariant-admitted-native-work");
        let ptr = value.as_concrete_TypeRef() as usize;
        let base = unsafe { CFGetRetainCount(ptr as CFTypeRef) };
        let cache = Snapshots::new();
        let snapshot = cache.publish(1, 2, payload(ptr));
        let guard = resolve(&cache, snapshot, 0).unwrap();
        let (finish_tx, finish_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            finish_rx.recv().unwrap();
            assert_eq!(guard.as_ptr(), ptr);
            drop(guard);
        });
        drop(cache);
        let retained = unsafe { CFGetRetainCount(ptr as CFTypeRef) };
        finish_tx.send(()).unwrap();
        worker.join().unwrap();
        assert_eq!(retained, base + 1);
        assert_eq!(unsafe { CFGetRetainCount(ptr as CFTypeRef) }, base);
    }

    #[test]
    fn missing_index_returns_none() {
        let cache = Snapshots::new();
        assert!(resolve(&cache, 0, 0).is_none());
        let snapshot = cache.publish(1, 2, AxSnapshot::from_nodes(&[]));
        assert!(resolve(&cache, snapshot, 0).is_none());
        assert!(resolve(&cache, snapshot, 5).is_none());
    }

    #[test]
    fn abandoned_preparation_releases_native_payload_without_replacing_snapshot() {
        let original = CFString::new("cua-driver-original-published-native-work");
        let replacement = CFString::new("cua-driver-abandoned-prepared-native-work");
        let original_ptr = original.as_concrete_TypeRef() as usize;
        let replacement_ptr = replacement.as_concrete_TypeRef() as usize;
        let base = unsafe { CFGetRetainCount(replacement_ptr as CFTypeRef) };
        let cache = Snapshots::new();
        let snapshot = cache.publish(1, 2, payload(original_ptr));
        let prepared = payload(replacement_ptr);
        assert_eq!(resolve(&cache, snapshot, 0).unwrap().as_ptr(), original_ptr);
        drop(prepared);
        assert_eq!(
            unsafe { CFGetRetainCount(replacement_ptr as CFTypeRef) },
            base
        );
        assert_eq!(resolve(&cache, snapshot, 0).unwrap().as_ptr(), original_ptr);
    }
}
