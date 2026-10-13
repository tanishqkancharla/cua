//! A distinct Mac collection selection capability; never a fabricated AX action.
//! Revalidate live containment, exact scope and enabled/writable parent before input.
use super::bindings::*;
use core_foundation::{
    array::CFArray,
    base::{CFEqual, CFGetTypeID, CFRelease, CFType, CFTypeRef, TCFType},
    string::CFString,
};
use std::time::{Duration, Instant};

const MAX_ARRAY_ITEMS: isize = 4096;
struct Owned(AXUIElementRef);
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0 as CFTypeRef) }
    }
}
struct Context {
    item: Owned,
    section: Owned,
    collection: Owned,
}

unsafe fn parent(element: AXUIElementRef) -> Option<Owned> {
    let parent = Owned(copy_element_attr(element, "AXParent")?);
    AXUIElementSetMessagingTimeout(parent.0, 0.1);
    Some(parent)
}
unsafe fn array(element: AXUIElementRef, name: &str) -> Option<CFArray<CFTypeRef>> {
    let attr = CFString::new(name);
    let mut value = std::ptr::null();
    if AXUIElementCopyAttributeValue(element, attr.as_concrete_TypeRef(), &mut value)
        != kAXErrorSuccess
        || value.is_null()
    {
        return None;
    }
    if CFGetTypeID(value) != CFArray::<CFTypeRef>::type_id() {
        CFRelease(value);
        return None;
    }
    let array = CFArray::<CFTypeRef>::wrap_under_create_rule(value as _);
    if array.len() > MAX_ARRAY_ITEMS
        || array
            .iter()
            .any(|item| CFGetTypeID(*item) != AXUIElementGetTypeID())
    {
        return None;
    }
    Some(array)
}
unsafe fn contains(parent: AXUIElementRef, attr: &str, child: AXUIElementRef) -> Option<bool> {
    Some(
        array(parent, attr)?
            .iter()
            .any(|item| CFEqual(*item, child as CFTypeRef) != 0),
    )
}
unsafe fn scope(element: AXUIElementRef) -> Option<(i32, u32)> {
    let mut pid = 0;
    if AXUIElementGetPid(element, &mut pid) != kAXErrorSuccess {
        return None;
    }
    Some((pid, ax_get_window_id(element)?))
}
unsafe fn context(element: AXUIElementRef, expected: Option<(i32, u32)>) -> Option<Context> {
    let deadline = Instant::now() + Duration::from_millis(500);
    AXUIElementSetMessagingTimeout(element, 0.1);
    if copy_string_attr(element, "AXRole").as_deref() != Some("AXGroup")
        || copy_string_attr(element, "AXSubrole").as_deref() != Some("AXHostingView")
        || copy_bool_attr(element, "AXEnabled") == Some(false)
    {
        return None;
    }
    let exact = scope(element)?;
    if expected.is_some_and(|expected| exact != expected) {
        return None;
    }
    let item = parent(element)?;
    if copy_string_attr(item.0, "AXRole").as_deref() != Some("AXGroup")
        || scope(item.0) != Some(exact)
        || contains(item.0, "AXChildren", element) != Some(true)
        || Instant::now() >= deadline
    {
        return None;
    }
    let section = parent(item.0)?;
    if copy_string_attr(section.0, "AXRole").as_deref() != Some("AXList")
        || copy_string_attr(section.0, "AXSubrole").as_deref() != Some("AXSectionList")
        || scope(section.0) != Some(exact)
        || contains(section.0, "AXChildren", item.0) != Some(true)
        || Instant::now() >= deadline
    {
        return None;
    }
    let collection = parent(section.0)?;
    if copy_string_attr(collection.0, "AXRole").as_deref() != Some("AXList")
        || copy_string_attr(collection.0, "AXSubrole").as_deref() != Some("AXCollectionList")
        || scope(collection.0) != Some(exact)
        || contains(collection.0, "AXChildren", section.0) != Some(true)
        || copy_bool_attr(collection.0, "AXEnabled") != Some(true)
        || attribute_settable(collection.0, "AXSelectedChildren") != Some(true)
        || Instant::now() >= deadline
    {
        return None;
    }
    Some(Context {
        item,
        section,
        collection,
    })
}

/// Returned state is membership in the parent's selected children, independent
/// of a layout item's own AXSelected/AXEnabled. Unknown capability is omitted.
pub(crate) unsafe fn observe(element: AXUIElementRef) -> Option<bool> {
    let context = context(element, None)?;
    contains(context.collection.0, "AXSelectedChildren", context.item.0)
}

/// One setter, no fallback or replay. A successful readback confirms selection,
/// not keyboard focus. Errors after the setter preserve an uncertain outcome.
pub(crate) fn select(element: usize, pid: i32, window_id: u32) -> anyhow::Result<()> {
    unsafe {
        let element = element as AXUIElementRef;
        let context = context(element, Some((pid,window_id)))
            .ok_or_else(|| anyhow::anyhow!("collection selection capability unavailable in this exact window; no input was sent"))?;
        // Recheck authority and containment immediately before the one write.
        if copy_bool_attr(element, "AXEnabled") == Some(false)
            || copy_string_attr(element, "AXRole").as_deref() != Some("AXGroup")
            || copy_string_attr(element, "AXSubrole").as_deref() != Some("AXHostingView")
            || copy_bool_attr(context.collection.0, "AXEnabled") != Some(true)
            || attribute_settable(context.collection.0, "AXSelectedChildren") != Some(true)
            || scope(element) != Some((pid, window_id))
            || scope(context.item.0) != Some((pid, window_id))
            || scope(context.section.0) != Some((pid, window_id))
            || scope(context.collection.0) != Some((pid, window_id))
            || contains(context.item.0, "AXChildren", element) != Some(true)
            || contains(context.section.0, "AXChildren", context.item.0) != Some(true)
            || contains(context.collection.0, "AXChildren", context.section.0) != Some(true)
        {
            anyhow::bail!("collection selection capability changed; no input was sent");
        }
        let item = CFType::wrap_under_get_rule(context.item.0 as CFTypeRef);
        let selection = CFArray::from_CFTypes(&[item]);
        let attr = CFString::new("AXSelectedChildren");
        let error = AXUIElementSetAttributeValue(
            context.collection.0,
            attr.as_concrete_TypeRef(),
            selection.as_CFTypeRef(),
        );
        if error != kAXErrorSuccess {
            anyhow::bail!("AXSelectedChildren write returned AX error {error}; outcome unknown, observe before any new input");
        }
        for _ in 0..2 {
            if scope(element) != Some((pid, window_id))
                || scope(context.collection.0) != Some((pid, window_id))
                || array(context.collection.0, "AXSelectedChildren").is_none_or(|selected| {
                    selected.len() != 1
                        || selected
                            .get(0)
                            .is_none_or(|item| CFEqual(*item, context.item.0 as CFTypeRef) == 0)
                })
            {
                anyhow::bail!("AXSelectedChildren write dispatched, but selection readback is unknown; observe before any new input");
            }
        }
        Ok(())
    }
}
