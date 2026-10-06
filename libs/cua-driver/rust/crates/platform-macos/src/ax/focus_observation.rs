//! Exact-window focus metadata for already indexed snapshot objects only.
use super::{bindings::*, tree::AXNode};
use core_foundation::base::{CFEqual, CFRelease, CFRetain, CFTypeRef};

struct OwnedElement(AXUIElementRef);
impl Drop for OwnedElement {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0 as CFTypeRef) }
    }
}

/// Independent retains keep the walk's objects alive if optional observation
/// times out and another request replaces the element cache in the meantime.
pub(crate) struct Candidates(Vec<(usize, usize, String)>);
unsafe impl Send for Candidates {}
impl Candidates {
    pub(crate) fn retain(nodes: &[AXNode]) -> Self {
        Self(
            nodes
                .iter()
                .filter_map(|node| {
                    let index = node.element_index?;
                    if node.element_ptr == 0 {
                        return None;
                    }
                    unsafe {
                        CFRetain(node.element_ptr as CFTypeRef);
                    }
                    Some((index, node.element_ptr, node.role.clone()))
                })
                .collect(),
        )
    }
}
impl Drop for Candidates {
    fn drop(&mut self) {
        for (_, ptr, _) in &self.0 {
            unsafe {
                CFRelease(*ptr as CFTypeRef);
            }
        }
    }
}

fn window_matches(
    direct: Option<u32>,
    associated: Option<(bool, i32, u32)>,
    pid: i32,
    window_id: u32,
) -> bool {
    match direct {
        Some(actual) => actual == window_id,
        None => associated.is_some_and(|(ordinary, owner, actual)| {
            ordinary && owner == pid && actual == window_id
        }),
    }
}

unsafe fn owned(element: AXUIElementRef, pid: i32) -> bool {
    let mut actual = 0;
    AXUIElementGetPid(element, &mut actual) == kAXErrorSuccess && actual == pid
}
unsafe fn exact_window(element: AXUIElementRef, pid: i32, window_id: u32) -> bool {
    let direct = ax_get_window_id(element);
    if direct.is_some() {
        return window_matches(direct, None, pid, window_id);
    }
    let Some(window) = copy_element_attr(element, "AXWindow") else {
        return false;
    };
    let window = OwnedElement(window);
    AXUIElementSetMessagingTimeout(window.0, 0.1);
    let mut owner = 0;
    if AXUIElementGetPid(window.0, &mut owner) != kAXErrorSuccess {
        return false;
    }
    let Some(actual) = ax_get_window_id(window.0) else {
        return false;
    };
    window_matches(
        None,
        Some((
            copy_string_attr(window.0, "AXRole").as_deref() == Some("AXWindow"),
            owner,
            actual,
        )),
        pid,
        window_id,
    )
}
unsafe fn focused_window(app: AXUIElementRef, pid: i32, window_id: u32) -> Option<OwnedElement> {
    let window = OwnedElement(copy_element_attr(app, "AXFocusedWindow")?);
    AXUIElementSetMessagingTimeout(window.0, 0.1);
    (owned(window.0, pid)
        && copy_string_attr(window.0, "AXRole").as_deref() == Some("AXWindow")
        && ax_get_window_id(window.0) == Some(window_id))
    .then_some(window)
}

/// Unknown, changing, protected or ambiguously indexed focus is omitted. Never
/// substitute role/value/label similarity for equality of native AX objects.
pub(crate) fn observe(pid: i32, window_id: u32, candidates: Candidates) -> Option<usize> {
    unsafe {
        let ptr = AXUIElementCreateApplication(pid);
        if ptr.is_null() {
            return None;
        }
        let app = OwnedElement(ptr);
        AXUIElementSetMessagingTimeout(app.0, 0.1);
        let before_window = focused_window(app.0, pid, window_id)?;
        let focus = OwnedElement(copy_element_attr(app.0, "AXFocusedUIElement")?);
        AXUIElementSetMessagingTimeout(focus.0, 0.1);
        if !owned(focus.0, pid) || !exact_window(focus.0, pid, window_id) {
            return None;
        }
        let role = copy_string_attr(focus.0, "AXRole")?;
        let subrole = copy_string_attr(focus.0, "AXSubrole").unwrap_or_default();
        if role.contains("Secure") || subrole.contains("Secure") {
            return None;
        }
        let mut matches = candidates.0.iter().filter(|(_, ptr, candidate_role)| {
            candidate_role == &role && CFEqual(*ptr as CFTypeRef, focus.0 as CFTypeRef) != 0
        });
        let index = matches.next()?.0;
        if matches.next().is_some() {
            return None;
        }
        let after_focus = OwnedElement(copy_element_attr(app.0, "AXFocusedUIElement")?);
        AXUIElementSetMessagingTimeout(after_focus.0, 0.1);
        let after_window = focused_window(app.0, pid, window_id)?;
        if CFEqual(focus.0 as CFTypeRef, after_focus.0 as CFTypeRef) == 0
            || CFEqual(before_window.0 as CFTypeRef, after_window.0 as CFTypeRef) == 0
            || !owned(after_focus.0, pid)
            || !exact_window(after_focus.0, pid, window_id)
            || copy_string_attr(after_focus.0, "AXRole").as_deref() != Some(role.as_str())
            || copy_string_attr(after_focus.0, "AXSubrole").unwrap_or_default() != subrole
        {
            return None;
        }
        Some(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_direct_mismatch_never_uses_associated_window() {
        assert!(window_matches(Some(7), None, 42, 7));
        assert!(!window_matches(Some(8), Some((true, 42, 7)), 42, 7));
    }
    #[test]
    fn unknown_direct_window_requires_ordinary_exact_owner_and_id() {
        assert!(window_matches(None, Some((true, 42, 7)), 42, 7));
        assert!(!window_matches(None, None, 42, 7));
        assert!(!window_matches(None, Some((false, 42, 7)), 42, 7));
        assert!(!window_matches(None, Some((true, 43, 7)), 42, 7));
        assert!(!window_matches(None, Some((true, 42, 8)), 42, 7));
    }
}
