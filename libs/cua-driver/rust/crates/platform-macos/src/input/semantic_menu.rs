//! Exact semantic dispatch for a menu inside an attached AppKit sheet.
//!
//! The sheet can be key while its parent document remains the observed target.
//! Re-keying that document before AXPress dismisses the menu or cannot succeed.
//! This proof authorizes only the retained advertised AX action, never HID input.
use crate::ax::bindings::{
    ax_get_window_id, copy_action_names, copy_ax_windows, copy_bool_attr, copy_children,
    copy_element_attr, copy_string_attr, kAXErrorSuccess, AXUIElementCreateApplication,
    AXUIElementGetPid, AXUIElementRef, AXUIElementSetMessagingTimeout,
};
use crate::windows::{resolve_window_owner, WindowOwner};
use core_foundation::base::{CFEqual, CFRelease, CFTypeRef};

struct OwnedAx(AXUIElementRef);
impl Drop for OwnedAx {
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.0 as CFTypeRef);
        }
    }
}

/// The caller retains `element` throughout this proof and the subsequent action.
/// Unknown ancestry, a sibling sheet or an unadvertised action keeps the existing
/// foreground route. No title, coordinate or shared-process match grants scope.
pub unsafe fn is_exact_attached_sheet_menu(
    element: AXUIElementRef,
    pid: i32,
    window_id: u32,
    requested_action: &str,
) -> anyhow::Result<bool> {
    if copy_string_attr(element, "AXRole").as_deref() != Some("AXMenuItem") {
        return Ok(false);
    }
    let advertised = copy_action_names(element);
    let action = match requested_action {
        "press" => "AXPress",
        "pick" => "AXPick",
        "cancel" => "AXCancel",
        _ => return Ok(false),
    };
    if !advertised.iter().any(|name| name == action) {
        return Ok(false);
    }
    if copy_bool_attr(element, "AXEnabled") != Some(true) {
        return Ok(false);
    }
    let mut owner = 0;
    if AXUIElementGetPid(element, &mut owner) != kAXErrorSuccess
        || owner != pid
        || !matches!(resolve_window_owner(pid, window_id), WindowOwner::SamePid)
    {
        anyhow::bail!("menu owner or exact target window changed; no action was sent");
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
    let mut retained = Vec::<OwnedAx>::new();
    let mut current = element;
    let mut sheet = None;
    for _ in 0..40 {
        if std::time::Instant::now() >= deadline {
            return Ok(false);
        }
        match copy_string_attr(current, "AXRole").as_deref() {
            Some("AXSheet") => {
                sheet = Some(current);
                break;
            }
            Some("AXWindow" | "AXApplication") | None => return Ok(false),
            _ => {}
        }
        let Some(parent) = copy_element_attr(current, "AXParent") else {
            return Ok(false);
        };
        let _ = AXUIElementSetMessagingTimeout(parent, 0.1);
        retained.push(OwnedAx(parent));
        current = parent;
    }
    let Some(sheet) = sheet else {
        return Ok(false);
    };
    let mut sheet_owner = 0;
    if AXUIElementGetPid(sheet, &mut sheet_owner) != kAXErrorSuccess || sheet_owner != pid {
        return Ok(false);
    }
    let app = AXUIElementCreateApplication(pid);
    if app.is_null() {
        return Ok(false);
    }
    let app = OwnedAx(app);
    let _ = AXUIElementSetMessagingTimeout(app.0, 0.1);
    let windows: Vec<OwnedAx> = copy_ax_windows(app.0).into_iter().map(OwnedAx).collect();
    for window in windows {
        if std::time::Instant::now() >= deadline {
            return Ok(false);
        }
        let _ = AXUIElementSetMessagingTimeout(window.0, 0.1);
        if ax_get_window_id(window.0) != Some(window_id)
            || copy_string_attr(window.0, "AXRole").as_deref() != Some("AXWindow")
        {
            continue;
        }
        // AppKit exposes the attached sheet as an actual AXChildren object;
        // AXSheet is a role, not a document's element-valued attribute.
        let children: Vec<OwnedAx> = copy_children(window.0).into_iter().map(OwnedAx).collect();
        return Ok(children
            .iter()
            .any(|child| CFEqual(child.0 as CFTypeRef, sheet as CFTypeRef) != 0));
    }
    Ok(false)
}
