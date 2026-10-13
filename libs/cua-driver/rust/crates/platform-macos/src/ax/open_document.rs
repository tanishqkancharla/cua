//! Native document identity for one exact AX/WindowServer owner/window.
//! NoValue is a known absence; unsupported, failed and malformed reads are unknown.
use super::bindings::*;
use core_foundation::{
    base::{CFGetTypeID, CFRelease, CFTypeRef, TCFType},
    string::CFString,
    url::*,
};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentIdentity {
    Absent,
    Url(String),
    Unknown,
}

fn classify(error: AXError, observed: Option<String>) -> DocumentIdentity {
    match (error, observed) {
        (kAXErrorNoValue, _) => DocumentIdentity::Absent,
        (kAXErrorSuccess, Some(url)) if !url.is_empty() => DocumentIdentity::Url(url),
        _ => DocumentIdentity::Unknown,
    }
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFURLIsFileReferenceURL(url: CFURLRef) -> u8;
}

pub fn normalize_resource(observed: &str) -> Option<String> {
    if observed.is_empty() {
        return None;
    }
    unsafe {
        let string = CFString::new(observed);
        let raw = CFURLCreateWithString(
            std::ptr::null(),
            string.as_concrete_TypeRef(),
            std::ptr::null(),
        );
        if raw.is_null() {
            return None;
        }
        let url = CFURL::wrap_under_create_rule(raw);
        if CFURLIsFileReferenceURL(url.as_concrete_TypeRef()) == 0 {
            return Some(observed.into());
        }
        let mut error = std::ptr::null_mut();
        let path = CFURLCreateFilePathURL(std::ptr::null(), url.as_concrete_TypeRef(), &mut error);
        if !error.is_null() {
            CFRelease(error as CFTypeRef);
        }
        if path.is_null() {
            return None;
        }
        Some(CFURL::wrap_under_create_rule(path).get_string().to_string())
    }
}

unsafe fn observe(element: AXUIElementRef, attribute: &str) -> DocumentIdentity {
    let name = CFString::new(attribute);
    let mut value: CFTypeRef = std::ptr::null();
    let error = AXUIElementCopyAttributeValue(element, name.as_concrete_TypeRef(), &mut value);
    let observed = if error == kAXErrorSuccess && !value.is_null() {
        let kind = CFGetTypeID(value);
        if kind == CFString::type_id() {
            normalize_resource(&CFString::wrap_under_get_rule(value as _).to_string())
        } else if kind == CFURL::type_id() {
            normalize_resource(
                &CFURL::wrap_under_get_rule(value as _)
                    .get_string()
                    .to_string(),
            )
        } else {
            None
        }
    } else {
        None
    };
    if !value.is_null() {
        CFRelease(value);
    }
    classify(error, observed)
}

pub unsafe fn resource_url(element: AXUIElementRef) -> Option<String> {
    match observe(element, "AXURL") {
        DocumentIdentity::Url(url) => Some(url),
        _ => None,
    }
}

pub fn snapshot(
    pid: i32,
    windows: &[crate::windows::WindowInfo],
) -> HashMap<u32, DocumentIdentity> {
    // Keep every existing CG window in the baseline, including those whose AX
    // object cannot be read. Missing metadata must never masquerade as newness.
    let mut result: HashMap<_, _> = windows
        .iter()
        .filter(|w| w.pid == pid)
        .map(|w| (w.window_id, DocumentIdentity::Unknown))
        .collect();
    unsafe {
        let app = AXUIElementCreateApplication(pid);
        if app.is_null() {
            return result;
        }
        AXUIElementSetMessagingTimeout(app, 0.5);
        for window in copy_ax_windows(app) {
            let mut owner = 0;
            if AXUIElementGetPid(window, &mut owner) == kAXErrorSuccess
                && owner == pid
                && copy_string_attr(window, "AXRole").as_deref() == Some("AXWindow")
            {
                if let Some(id) = ax_get_window_id(window) {
                    if result.contains_key(&id) {
                        result.insert(id, observe(window, "AXDocument"));
                    }
                }
            }
            CFRelease(window as CFTypeRef);
        }
        CFRelease(app as CFTypeRef);
    }
    result
}

pub fn document_urls(pid: i32, windows: &[crate::windows::WindowInfo]) -> HashMap<u32, String> {
    snapshot(pid, windows)
        .into_iter()
        .filter_map(|(id, state)| match state {
            DocumentIdentity::Url(url) => Some((id, url)),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_explicit_no_value_is_known_absence() {
        assert_eq!(classify(kAXErrorNoValue, None), DocumentIdentity::Absent);
        for error in [-25205, -25204, -25201] {
            assert_eq!(classify(error, None), DocumentIdentity::Unknown);
        }
        assert_eq!(classify(kAXErrorSuccess, None), DocumentIdentity::Unknown);
        assert_eq!(
            classify(kAXErrorSuccess, Some(String::new())),
            DocumentIdentity::Unknown
        );
        assert_eq!(
            classify(kAXErrorSuccess, Some("file:///owned/a.rtf".into())),
            DocumentIdentity::Url("file:///owned/a.rtf".into())
        );
    }
}
