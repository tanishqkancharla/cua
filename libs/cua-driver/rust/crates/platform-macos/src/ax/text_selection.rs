//! Bounded, read-only selection evidence, never an element/action selector.
use super::bindings::*;
use core_foundation::base::{CFEqual, CFRelease, CFTypeRef};
use serde_json::{json, Value};

const MAX_TEXT_UTF16: usize = 2048;

struct OwnedElement(AXUIElementRef);
impl Drop for OwnedElement {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0 as CFTypeRef) }
    }
}

// Prefer the direct mapping, but an embedded editor can report an unsupported
// SPI while publishing its real AXWindow. Never override a known mismatch.
fn window_matches(
    direct: Option<u32>,
    associated: Option<(bool, i32, u32)>,
    pid: i32,
    window_id: u32,
) -> bool {
    match direct {
        Some(actual) => actual == window_id,
        None => associated.is_some_and(|(ordinary_window, owner, actual)| {
            ordinary_window && owner == pid && actual == window_id
        }),
    }
}

unsafe fn element_matches_window(element: AXUIElementRef, pid: i32, window_id: u32) -> bool {
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

/// Omit unknown, changing, protected or differently scoped selection state.
/// A zero-length observed range is affirmative caret/cleared-selection evidence.
pub(crate) fn observe(pid: i32, window_id: u32) -> Option<Value> {
    unsafe {
        let app = AXUIElementCreateApplication(pid);
        if app.is_null() {
            return None;
        }
        let app = OwnedElement(app);
        AXUIElementSetMessagingTimeout(app.0, 0.1);
        let window = OwnedElement(copy_element_attr(app.0, "AXFocusedWindow")?);
        if ax_get_window_id(window.0) != Some(window_id) {
            return None;
        }
        let element = OwnedElement(copy_element_attr(app.0, "AXFocusedUIElement")?);
        AXUIElementSetMessagingTimeout(element.0, 0.1);
        let mut owner = 0;
        if AXUIElementGetPid(element.0, &mut owner) != kAXErrorSuccess
            || owner != pid
            || !element_matches_window(element.0, pid, window_id)
        {
            return None;
        }
        let role = copy_string_attr(element.0, "AXRole")?;
        let subrole = copy_string_attr(element.0, "AXSubrole").unwrap_or_default();
        if role.contains("Secure") || subrole.contains("Secure") {
            return None;
        }
        let range = copy_text_range_attr(element.0)?;
        let start = usize::try_from(range.location).ok()?;
        let length = usize::try_from(range.length).ok()?;
        start.checked_add(length)?;
        let text = if length == 0 {
            String::new()
        } else {
            copy_string_attr(element.0, "AXSelectedText")?
        };
        let (text, truncated) = bounded_selection(&text, length)?;
        // A changing native selection cannot be represented by mixing reads.
        let after = copy_text_range_attr(element.0)?;
        let focused = OwnedElement(copy_element_attr(app.0, "AXFocusedUIElement")?);
        let window_after = OwnedElement(copy_element_attr(app.0, "AXFocusedWindow")?);
        if after.location != range.location
            || after.length != range.length
            || CFEqual(element.0 as CFTypeRef, focused.0 as CFTypeRef) == 0
            || ax_get_window_id(window_after.0) != Some(window_id)
            || !element_matches_window(element.0, pid, window_id)
        {
            return None;
        }
        Some(json!({"pid": pid, "window_id": window_id, "role": role,
            "range": {"start_utf16": start, "length_utf16": length},
            "text": text, "text_truncated": truncated}))
    }
}

fn bounded_selection(text: &str, expected_utf16: usize) -> Option<(String, bool)> {
    if text.encode_utf16().count() != expected_utf16 {
        return None;
    }
    let mut units = 0;
    let prefix: String = text
        .chars()
        .take_while(|ch| {
            units += ch.len_utf16();
            units <= MAX_TEXT_UTF16
        })
        .collect();
    let truncated = prefix.len() != text.len();
    Some((prefix, truncated))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn associated_window_requires_exact_scope_and_never_overrides_a_direct_mismatch() {
        assert!(window_matches(Some(7), None, 42, 7));
        assert!(window_matches(None, Some((true, 42, 7)), 42, 7));
        assert!(!window_matches(Some(8), Some((true, 42, 7)), 42, 7));
        assert!(!window_matches(None, None, 42, 7));
        assert!(!window_matches(None, Some((false, 42, 7)), 42, 7));
        assert!(!window_matches(None, Some((true, 41, 7)), 42, 7));
        assert!(!window_matches(None, Some((true, 42, 8)), 42, 7));
    }
    #[test]
    fn selection_length_uses_utf16_and_rejects_mixed_reads() {
        assert_eq!(
            bounded_selection("café 😀", 7),
            Some(("café 😀".into(), false))
        );
        assert!(bounded_selection("café 😀", 6).is_none());
        assert_eq!(bounded_selection("", 0), Some(("".into(), false)));
    }
    #[test]
    fn cap_never_splits_a_surrogate_pair() {
        let text = format!("{}😀tail", "a".repeat(2047));
        let (prefix, truncated) = bounded_selection(&text, 2053).unwrap();
        assert_eq!(prefix, "a".repeat(2047));
        assert!(truncated);
    }
}
