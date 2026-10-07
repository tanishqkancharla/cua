//! AX action dispatch — the preferred click/interaction path for indexed elements.

use crate::ax::bindings::*;
use core_foundation::base::{CFEqual, CFRelease, CFRetain, CFTypeRef};

const MAX_SELECTION_ANCESTORS: usize = 8;

fn is_selectable_container_role(role: &str) -> bool {
    matches!(role, "AXRow" | "AXCell" | "AXListItem" | "AXImage")
}

/// File labels can expose AXSelected themselves while AXValue stays settable
/// for renaming. Native resource/action evidence keeps ordinary editors out.
fn is_collection_selection_role(role: &str, has_open: bool, resource: Option<&str>) -> bool {
    is_selectable_container_role(role)
        || (role == "AXTextField"
            && has_open
            && resource.is_some_and(|url| url.starts_with("file:")))
}

pub fn is_collection_selection_element(element: AXUIElementRef, role: &str) -> bool {
    if is_selectable_container_role(role) {
        return true;
    }
    if role != "AXTextField" {
        return false;
    }
    let has_open = unsafe { copy_action_names(element) }
        .iter()
        .any(|action| action == "AXOpen");
    let resource = if has_open {
        unsafe { crate::ax::open_document::resource_url(element) }
    } else {
        None
    };
    is_collection_selection_role(role, has_open, resource.as_deref())
}

/// Sidebar selection writes can change AXSelected without activating the
/// navigation destination. Identify only row/cell items in a reported sidebar;
/// ordinary file collections retain their existing selection semantics.
pub fn is_sidebar_navigation_item(element_ptr: usize) -> bool {
    let mut current = element_ptr as AXUIElementRef;
    let mut owns_current = false;
    let mut saw_item = false;
    for _ in 0..MAX_SELECTION_ANCESTORS {
        let role = unsafe { copy_string_attr(current, "AXRole") }.unwrap_or_default();
        saw_item |= matches!(role.as_str(), "AXRow" | "AXCell");
        let sidebar = saw_item
            && role == "AXOutline"
            && unsafe { copy_string_attr(current, "AXDescription") }
                .is_some_and(|description| description.eq_ignore_ascii_case("sidebar"));
        if sidebar {
            if owns_current {
                unsafe { CFRelease(current as CFTypeRef) };
            }
            return true;
        }
        if matches!(role.as_str(), "AXWindow" | "AXApplication") {
            break;
        }
        let parent = unsafe { copy_element_attr(current, "AXParent") };
        if owns_current {
            unsafe { CFRelease(current as CFTypeRef) };
        }
        let Some(parent) = parent else {
            return false;
        };
        current = parent;
        owns_current = true;
    }
    if owns_current {
        unsafe { CFRelease(current as CFTypeRef) };
    }
    false
}

/// Select the nearest list-like element at or above `element_ptr` and confirm
/// the write through `AXSelected` read-back.
///
/// Finder and other AppKit collection views commonly expose an item's label as
/// an actionable child (`AXTextField`) while the selectable object is its
/// parent `AXRow`. Neither object necessarily advertises `AXPress`, and Finder
/// can return `kAXErrorCannotComplete` for a press on the row. A pointer click
/// selects that row, so the AX equivalent is to set the row's `AXSelected`
/// attribute rather than treating the failed press as terminal.
///
/// Finder icon views expose each selectable file directly as an `AXImage` with
/// an `AXSelected` attribute, so that role is included alongside the standard
/// row-like containers. The fallback remains bounded and requires a successful
/// `AXSelected=true` read-back, so an arbitrary failed image/button press cannot
/// become a claimed success.
pub fn select_nearest_container(element_ptr: usize) -> Option<String> {
    let mut current = element_ptr as AXUIElementRef;
    let mut owns_current = false;

    for _ in 0..MAX_SELECTION_ANCESTORS {
        let role = unsafe { copy_string_attr(current, "AXRole") }.unwrap_or_default();
        if is_collection_selection_element(current, &role)
            && unsafe { copy_bool_attr(current, "AXSelected") }.is_some()
        {
            let err = unsafe { set_bool_attr_true(current, "AXSelected") };
            if err == kAXErrorSuccess
                && unsafe { copy_bool_attr(current, "AXSelected") } == Some(true)
            {
                if owns_current {
                    unsafe { CFRelease(current as CFTypeRef) };
                }
                return Some(role);
            }
        }

        let parent = unsafe { copy_element_attr(current, "AXParent") };
        if owns_current {
            unsafe { CFRelease(current as CFTypeRef) };
        }
        current = parent?;
        owns_current = true;
    }

    if owns_current {
        unsafe { CFRelease(current as CFTypeRef) };
    }
    None
}

/// Read the selection state of the nearest collection-like element without
/// mutating it. This is used to verify a coordinate fallback when AppKit
/// exposes `AXSelected` but refuses to set it directly (notably Finder icon
/// views).
pub fn nearest_container_selection_state(element_ptr: usize) -> Option<(String, bool)> {
    let mut current = element_ptr as AXUIElementRef;
    let mut owns_current = false;

    for _ in 0..MAX_SELECTION_ANCESTORS {
        let role = unsafe { copy_string_attr(current, "AXRole") }.unwrap_or_default();
        if is_collection_selection_element(current, &role) {
            if let Some(selected) = unsafe { copy_bool_attr(current, "AXSelected") } {
                if owns_current {
                    unsafe { CFRelease(current as CFTypeRef) };
                }
                return Some((role, selected));
            }
        }

        let parent = unsafe { copy_element_attr(current, "AXParent") };
        if owns_current {
            unsafe { CFRelease(current as CFTypeRef) };
        }
        current = parent?;
        owns_current = true;
    }

    if owns_current {
        unsafe { CFRelease(current as CFTypeRef) };
    }
    None
}

/// A retained selection context for proving the settled result of a modified
/// pointer click.
///
/// Reading only the target's `AXSelected` bit is insufficient for multi-select:
/// AppKit can expose a transient target transition while it is still resolving
/// the synthetic gesture, and a modifier-less outcome can replace the prior
/// selection with the target. Keep the target and every selected sibling alive
/// across delivery so callers can require both the intended target transition
/// and preservation of the pre-existing selection.
pub struct SelectionReadback {
    role: String,
    target: AXUIElementRef,
    peer_model_observed: bool,
    previously_selected_peers: Vec<AXUIElementRef>,
}

impl SelectionReadback {
    pub fn role(&self) -> &str {
        &self.role
    }

    pub fn observe(&self) -> Option<(bool, bool)> {
        let target_selected = unsafe { copy_bool_attr(self.target, "AXSelected") }?;
        let peers_preserved = self.peer_model_observed
            && self
                .previously_selected_peers
                .iter()
                .all(|&peer| unsafe { copy_bool_attr(peer, "AXSelected") } == Some(true));
        Some((target_selected, peers_preserved))
    }
}

impl Drop for SelectionReadback {
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.target as CFTypeRef);
            for peer in self.previously_selected_peers.drain(..) {
                CFRelease(peer as CFTypeRef);
            }
        }
    }
}

/// Capture the nearest selectable container and its currently-selected peers.
/// Returns `None` when the platform does not expose a readable selection model;
/// callers must then leave the action unverifiable instead of inventing proof.
pub fn capture_nearest_container_selection(element_ptr: usize) -> Option<SelectionReadback> {
    let mut current = element_ptr as AXUIElementRef;
    let mut owns_current = false;

    for _ in 0..MAX_SELECTION_ANCESTORS {
        let role = unsafe { copy_string_attr(current, "AXRole") }.unwrap_or_default();
        if is_collection_selection_element(current, &role)
            && unsafe { copy_bool_attr(current, "AXSelected") }.is_some()
        {
            if !owns_current {
                unsafe { CFRetain(current as CFTypeRef) };
            }
            let target = current;
            let parent = unsafe { copy_element_attr(target, "AXParent") };
            let mut peers = Vec::new();
            let mut peer_model_observed = false;
            if let Some(parent) = parent {
                for child in unsafe { copy_children(parent) } {
                    let is_target =
                        unsafe { CFEqual(child as CFTypeRef, target as CFTypeRef) != 0 };
                    peer_model_observed |= is_target;
                    if !is_target && unsafe { copy_bool_attr(child, "AXSelected") } == Some(true) {
                        peers.push(child);
                    } else {
                        unsafe { CFRelease(child as CFTypeRef) };
                    }
                }
                unsafe { CFRelease(parent as CFTypeRef) };
            }
            return Some(SelectionReadback {
                role,
                target,
                peer_model_observed,
                previously_selected_peers: peers,
            });
        }

        let parent = unsafe { copy_element_attr(current, "AXParent") };
        if owns_current {
            unsafe { CFRelease(current as CFTypeRef) };
        }
        current = parent?;
        owns_current = true;
    }

    if owns_current {
        unsafe { CFRelease(current as CFTypeRef) };
    }
    None
}

fn ensure_ax_enabled(enabled: Option<bool>, action: &str) -> anyhow::Result<()> {
    if enabled == Some(false) {
        anyhow::bail!(
            "refusing {action}: the target reports AXEnabled=false. \
             Retry this action with delivery_mode:\"foreground\" or call bring_to_front first"
        );
    }
    Ok(())
}

/// Refuse AX actions that macOS reports as disabled.
///
/// This must be checked immediately before dispatch rather than trusting the
/// cached snapshot value: foreground delivery can make a menu item live after
/// it was resolved, while backgrounding can disable it in the other direction.
pub fn ensure_ax_action_enabled(element_ptr: usize, action: &str) -> anyhow::Result<()> {
    let enabled = unsafe { copy_bool_attr(element_ptr as AXUIElementRef, "AXEnabled") };
    ensure_ax_enabled(enabled, action)
}

/// Perform an AX action on a cached element.
pub fn perform_ax_action(element_ptr: usize, action: &str) -> anyhow::Result<()> {
    let advertised = unsafe { copy_action_names(element_ptr as AXUIElementRef) };
    let ax_action = resolve_action(action, &advertised)?;
    ensure_ax_action_enabled(element_ptr, &ax_action)?;
    let err = unsafe { perform_action(element_ptr as AXUIElementRef, &ax_action) };

    if err == kAXErrorSuccess {
        Ok(())
    } else {
        anyhow::bail!("AXUIElementPerformAction({action}) failed with error {err}")
    }
}

/// Resolve published AX actions without substituting a press for an unknown action.
/// Custom action names are copied verbatim from this live element's inventory.
pub fn resolve_action(action: &str, advertised: &[String]) -> anyhow::Result<String> {
    let normalized = action.trim().to_lowercase();
    let canonical = match normalized.as_str() {
        "press" | "click" | "axpress" => Some("AXPress"),
        "show_menu" | "show menu" | "right_click" | "rightclick" | "axshowmenu" => {
            Some("AXShowMenu")
        }
        "pick" | "axpick" => Some("AXPick"),
        "confirm" | "axconfirm" => Some("AXConfirm"),
        "cancel" | "axcancel" => Some("AXCancel"),
        "open" | "axopen" => Some("AXOpen"),
        _ => None,
    };
    if let Some(name) = canonical {
        return Ok(name.to_owned());
    }
    let mut matches: Vec<_> = advertised
        .iter()
        .filter(|name| name.to_lowercase() == normalized || public_action_name(name) == normalized)
        .collect();
    // AppKit can repeat the exact same custom token in one element's action
    // inventory. Repetition does not introduce a second action; distinct raw
    // tokens with the same public name remain ambiguous and must be refused.
    matches.dedup();
    if matches.len() == 1 {
        return Ok(matches[0].to_string());
    }
    anyhow::bail!(
        "Action {action:?} is not uniquely advertised by this element (live actions: {advertised:?}); no action was sent"
    )
}

// AppKit custom names encode their visible name plus opaque target/selector
// lines. Match the published first-line name and dispatch the full live token.
fn public_action_name(name: &str) -> String {
    let first = name.lines().next().unwrap_or(name);
    let visible = first.strip_prefix("Name:").unwrap_or(first).trim();
    visible
        .strip_prefix("AX")
        .unwrap_or(visible)
        .trim()
        .to_lowercase()
}

#[cfg(test)]
mod advertised_action_tests {
    use super::*;

    #[test]
    fn repeated_exact_custom_token_is_unique_but_distinct_tokens_are_ambiguous() {
        let action = "Name:Customize info\nTarget:0x0\nSelector:(null)".to_owned();
        assert_eq!(
            resolve_action(
                "customize info",
                &[action.clone(), "Pin List".into(), action.clone()]
            )
            .unwrap(),
            action
        );
        let other = "Name:Customize info\nTarget:0x1\nSelector:(null)".to_owned();
        assert!(resolve_action("customize info", &[action, other]).is_err());
    }

    #[test]
    fn advertised_custom_actions_keep_their_exact_name() {
        let actions = vec![
            "AXPress".to_owned(),
            "Close Tab".to_owned(),
            "AXZoomWindow".to_owned(),
        ];
        assert_eq!(resolve_action("close tab", &actions).unwrap(), "Close Tab");
        assert_eq!(
            resolve_action("zoomwindow", &actions).unwrap(),
            "AXZoomWindow"
        );
        assert_eq!(resolve_action("confirm", &actions).unwrap(), "AXConfirm");
        assert!(resolve_action("unknown", &actions).is_err());
        let opaque = "Name:close tab\nTarget:0x123\nSelector:_closeButtonClicked:".to_owned();
        assert_eq!(
            resolve_action("close tab", &[opaque.clone()]).unwrap(),
            opaque
        );
    }

    #[test]
    fn disabled_elements_are_refused_before_dispatch() {
        let error = ensure_ax_enabled(Some(false), "AXPick").unwrap_err();
        let message = error.to_string();
        assert!(message.contains("AXEnabled=false"));
        assert!(message.contains("delivery_mode:\"foreground\""));
        assert!(message.contains("bring_to_front"));
    }

    #[test]
    fn enabled_or_unreported_state_is_allowed() {
        assert!(ensure_ax_enabled(Some(true), "AXPress").is_ok());
        assert!(ensure_ax_enabled(None, "AXPress").is_ok());
    }

    #[test]
    fn filename_selection_requires_both_native_file_resource_and_open_action() {
        assert!(is_collection_selection_role(
            "AXTextField",
            true,
            Some("file:///owned/request.rtf")
        ));
        assert!(!is_collection_selection_role(
            "AXTextField",
            false,
            Some("file:///owned/request.rtf")
        ));
        assert!(!is_collection_selection_role("AXTextField", true, None));
        assert!(!is_collection_selection_role(
            "AXTextField",
            true,
            Some("https://example.org")
        ));
        assert!(!is_collection_selection_role(
            "AXTextArea",
            true,
            Some("file:///owned/request.rtf")
        ));
        assert!(!is_collection_selection_role(
            "AXComboBox",
            true,
            Some("file:///owned/request.rtf")
        ));
    }

    #[test]
    fn selection_fallback_is_limited_to_collection_item_roles() {
        for role in ["AXRow", "AXCell", "AXListItem", "AXImage"] {
            assert!(is_selectable_container_role(role), "{role}");
        }
        for role in ["AXButton", "AXTextField", "AXWindow", "AXOutline"] {
            assert!(!is_selectable_container_role(role), "{role}");
        }
    }
}

/// Set AXFocused=true on an element (for pre-focusing before key press).
pub fn focus_element(element_ptr: usize) -> anyhow::Result<()> {
    let err = unsafe { set_bool_attr_true(element_ptr as AXUIElementRef, "AXFocused") };
    if err == kAXErrorSuccess {
        Ok(())
    } else {
        // Focus errors are often benign (element doesn't support focus).
        tracing::warn!("AXSetAttribute(AXFocused) returned {err}");
        Ok(())
    }
}

/// Report whether `element_ptr` is the application's currently focused element.
///
/// This is a read-only confirmation for the foreground typing rung: an
/// `AXFocused` write can be accepted by the element and then immediately
/// clobbered when AppKit installs the window's remembered first responder, so
/// "the write returned success" is not evidence that focus stuck. Identity is
/// compared with `CFEqual` because the app hands back a fresh `AXUIElementRef`
/// for the same underlying element.
///
/// A `false` return is deliberately conservative: an app whose
/// `AXFocusedUIElement` is unreadable reports not-focused, which at worst costs
/// one extra re-apply.
pub fn is_element_focused(pid: i32, element_ptr: usize) -> bool {
    unsafe {
        let Some(focused) = crate::ax::bindings::focused_element_of_pid(pid) else {
            return false;
        };
        let same = CFEqual(focused as CFTypeRef, element_ptr as CFTypeRef) != 0;
        CFRelease(focused as CFTypeRef);
        same
    }
}

/// Set the AXValue of an element (for dropdowns, text fields, etc.).
pub fn set_ax_value(element_ptr: usize, value: &str) -> anyhow::Result<()> {
    let err = unsafe { set_string_attr(element_ptr as AXUIElementRef, "AXValue", value) };
    if err == kAXErrorSuccess {
        Ok(())
    } else {
        anyhow::bail!("AXUIElementSetAttributeValue(AXValue) failed with error {err}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_elements_are_refused_before_dispatch() {
        let error = ensure_ax_enabled(Some(false), "AXPick").unwrap_err();
        let message = error.to_string();
        assert!(message.contains("AXEnabled=false"));
        assert!(message.contains("delivery_mode:\"foreground\""));
        assert!(message.contains("bring_to_front"));
    }

    #[test]
    fn enabled_or_unreported_state_is_allowed() {
        assert!(ensure_ax_enabled(Some(true), "AXPress").is_ok());
        assert!(ensure_ax_enabled(None, "AXPress").is_ok());
    }

    #[test]
    fn selection_fallback_is_limited_to_collection_item_roles() {
        for role in ["AXRow", "AXCell", "AXListItem", "AXImage"] {
            assert!(is_selectable_container_role(role), "{role}");
        }
        for role in ["AXButton", "AXTextField", "AXWindow", "AXOutline"] {
            assert!(!is_selectable_container_role(role), "{role}");
        }
    }
}
