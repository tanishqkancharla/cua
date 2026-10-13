//! Read-only native collection membership; never changes the full driver snapshot.
use super::{bindings::*, tree::AXNode};
use core_foundation::{
    array::CFArray,
    base::{CFEqual, CFRelease, CFRetain, CFTypeRef, TCFType},
    string::CFString,
};
use serde_json::{json, Value};

struct Rows(Vec<AXUIElementRef>);
impl Drop for Rows {
    fn drop(&mut self) {
        for row in &self.0 {
            unsafe { CFRelease(*row as CFTypeRef) }
        }
    }
}
unsafe fn rows(element: AXUIElementRef, name: &str) -> Option<Rows> {
    let name = CFString::new(name);
    let mut value: CFTypeRef = std::ptr::null();
    let status = AXUIElementCopyAttributeValue(element, name.as_concrete_TypeRef(), &mut value);
    if value.is_null() {
        return None;
    }
    if status != kAXErrorSuccess
        || core_foundation::base::CFGetTypeID(value) != CFArray::<CFTypeRef>::type_id()
    {
        CFRelease(value);
        return None;
    }
    let array = CFArray::<CFTypeRef>::wrap_under_create_rule(value as _);
    let mut result = Rows(Vec::new());
    for i in 0..array.len() {
        let item = *array.get(i)?;
        if item.is_null() || core_foundation::base::CFGetTypeID(item) != AXUIElementGetTypeID() {
            return None;
        }
        CFRetain(item);
        result.0.push(item as AXUIElementRef);
    }
    Some(result)
}
fn coherent(all: &[usize], visible: &[usize], selected: &[usize]) -> bool {
    let unique = |ids: &[usize]| ids.iter().enumerate().all(|(i, id)| !ids[..i].contains(id));
    !all.is_empty()
        && unique(all)
        && unique(visible)
        && unique(selected)
        && visible.iter().chain(selected).all(|id| all.contains(id))
}
/// Observe only complete, directly mapped native rows. Unknown or inconsistent
/// membership is omitted, leaving clients on their existing full-evidence path.
/// # Safety
/// Nodes must still own live AX references from the same current window walk.
pub(crate) unsafe fn observe(nodes: &[AXNode]) -> Vec<Value> {
    let mut result = Vec::new();
    for collection in nodes
        .iter()
        .filter(|n| !n.in_web_content && matches!(n.role.as_str(), "AXTable" | "AXOutline"))
    {
        // The walker retains only indexed nodes. Display-only table pointers
        // are borrowed and must never be dereferenced after the walk. Resolve
        // their live table via a retained direct row and its owned AXParent.
        let Some(start) = nodes.iter().position(|n| std::ptr::eq(n, collection)) else {
            continue;
        };
        let descendants: Vec<_> = nodes[start + 1..]
            .iter()
            .take_while(|n| n.depth > collection.depth)
            .collect();
        let Some(row) = descendants.iter().find(|n| {
            n.role == "AXRow" && n.depth == collection.depth + 1 && n.element_index.is_some()
        }) else {
            continue;
        };
        let Some(element) = copy_element_attr(row.element_ptr as AXUIElementRef, "AXParent") else {
            continue;
        };
        struct Parent(AXUIElementRef);
        impl Drop for Parent {
            fn drop(&mut self) {
                unsafe { CFRelease(self.0 as CFTypeRef) }
            }
        }
        let _parent = Parent(element);
        if copy_string_attr(element, "AXRole").as_deref() != Some(collection.role.as_str())
            || copy_string_attr(element, "AXDescription") != collection.description
            || copy_string_attr(element, "AXTitle") != collection.title
        {
            continue;
        }

        let (Some(all), Some(visible), Some(selected)) = (
            rows(element, "AXRows"),
            rows(element, "AXVisibleRows"),
            rows(element, "AXSelectedRows"),
        ) else {
            continue;
        };
        let map = |items: &[AXUIElementRef]| -> Option<Vec<usize>> {
            items
                .iter()
                .map(|row| {
                    let matches: Vec<_> = descendants
                        .iter()
                        .filter(|n| {
                            n.element_index.is_some()
                                && n.role == "AXRow"
                                && n.depth == collection.depth + 1
                                && !n.in_web_content
                                && CFEqual(*row as CFTypeRef, n.element_ptr as CFTypeRef) != 0
                        })
                        .collect();
                    (matches.len() == 1)
                        .then(|| matches[0].element_index)
                        .flatten()
                })
                .collect()
        };
        let (Some(all), Some(visible), Some(selected)) =
            (map(&all.0), map(&visible.0), map(&selected.0))
        else {
            continue;
        };
        if !coherent(&all, &visible, &selected) {
            continue;
        }
        result.push(json!({"source":"AXVisibleRows", "web_content":false, "role":collection.role,
            "depth":collection.depth, "row_element_indices":all, "visible_row_element_indices":visible,
            "selected_row_element_indices":selected}));
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visible_and_offscreen_selected_rows_belong_to_the_full_observed_set() {
        assert!(coherent(&[1, 4, 7], &[1, 4], &[7]));
        assert!(coherent(&[1, 4], &[], &[]));
    }
    #[test]
    fn stale_ambiguous_or_missing_membership_never_grants_a_projection() {
        for (all, visible, selected) in [
            (vec![], vec![], vec![]),
            (vec![1, 1], vec![1], vec![]),
            (vec![1, 4], vec![1, 1], vec![]),
            (vec![1, 4], vec![7], vec![]),
            (vec![1, 4], vec![1], vec![7]),
            (vec![1, 4], vec![1], vec![4, 4]),
        ] {
            assert!(!coherent(&all, &visible, &selected));
        }
    }
}
