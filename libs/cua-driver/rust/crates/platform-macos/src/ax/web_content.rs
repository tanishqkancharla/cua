//! Conservative ancestry classification for renderer focus settling.
//! Absence of AXWebArea is insufficient: only the exact addressed AXWindow
//! boundary proves native content. Missing/detached/capped chains stay unknown.

use super::bindings::{
    ax_get_window_id, copy_element_attr, copy_string_attr, kAXErrorSuccess, AXUIElementGetPid,
    AXUIElementRef,
};
use core_foundation::base::CFRelease;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Surface {
    NativeWindow,
    Web,
    Unknown,
}

fn boundary(role: Option<&str>, exact_window: bool) -> Option<Surface> {
    match role {
        Some("AXWebArea") => Some(Surface::Web),
        Some("AXWindow") if exact_window => Some(Surface::NativeWindow),
        Some("AXWindow") | Some("AXApplication") | None => Some(Surface::Unknown),
        _ => None,
    }
}

/// Classify a live borrowed addressed element without changing focus/cache.
///
/// # Safety
/// The caller must retain `element` throughout this bounded blocking AX walk.
/// Only copied parents are released; the caller's reference remains borrowed.
pub(crate) unsafe fn classify(element: AXUIElementRef, pid: i32, window_id: u32) -> Surface {
    if element.is_null() {
        return Surface::Unknown;
    }
    let mut current = element;
    let mut owned = false;
    let mut result = Surface::Unknown;
    for _ in 0..40 {
        let role = copy_string_attr(current, "AXRole");
        let exact_window = if role.as_deref() == Some("AXWindow") {
            let mut owner_pid = 0;
            ax_get_window_id(current) == Some(window_id)
                && AXUIElementGetPid(current, &mut owner_pid) == kAXErrorSuccess
                && owner_pid == pid
        } else {
            false
        };
        if let Some(surface) = boundary(role.as_deref(), exact_window) {
            result = surface;
            break;
        }
        let parent = copy_element_attr(current, "AXParent");
        if owned {
            CFRelease(current as _);
        }
        owned = false;
        match parent {
            Some(parent) => {
                current = parent;
                owned = true;
            }
            None => break,
        }
    }
    if owned {
        CFRelease(current as _);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_exact_window_boundary_proves_native() {
        assert_eq!(
            boundary(Some("AXWindow"), true),
            Some(Surface::NativeWindow)
        );
        assert_eq!(boundary(Some("AXWindow"), false), Some(Surface::Unknown));
    }

    #[test]
    fn web_boundary_keeps_renderer_settling() {
        assert_eq!(boundary(Some("AXWebArea"), false), Some(Surface::Web));
        assert_ne!(
            boundary(Some("AXWebArea"), true),
            Some(Surface::NativeWindow)
        );
    }

    #[test]
    fn missing_role_or_application_without_window_remains_unknown() {
        assert_eq!(boundary(None, true), Some(Surface::Unknown));
        assert_eq!(
            boundary(Some("AXApplication"), true),
            Some(Surface::Unknown)
        );
    }

    #[test]
    fn control_and_container_roles_do_not_prove_surface() {
        for role in ["AXTextField", "AXTextArea", "AXGroup", "AXScrollArea"] {
            assert_eq!(boundary(Some(role), true), None);
        }
    }
}
