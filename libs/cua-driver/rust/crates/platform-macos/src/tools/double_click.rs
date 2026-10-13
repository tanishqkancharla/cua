use async_trait::async_trait;
use core_foundation::base::{CFRelease, CFTypeRef};
use cua_driver_core::{
    protocol::ToolResult,
    tool::{Tool, ToolDef},
};
use serde_json::Value;
use std::sync::Arc;

use crate::ax::bindings::{
    copy_action_names, copy_children, copy_string_attr, element_screen_center, kAXErrorSuccess,
    perform_action, AXUIElementRef,
};

use super::ToolState;
use crate::ax::open_document::{
    self, normalize_resource as resource_url_for_comparison, DocumentIdentity,
};

pub struct DoubleClickTool {
    state: Arc<ToolState>,
}

fn background_action_for_element(
    has_ax_open: bool,
) -> cua_driver_core::background_input::BackgroundAction {
    if has_ax_open {
        cua_driver_core::background_input::BackgroundAction::AxSemantic
    } else {
        cua_driver_core::background_input::BackgroundAction::WindowPointer
    }
}

impl DoubleClickTool {
    pub fn new(state: Arc<ToolState>) -> Self {
        Self { state }
    }
}

static DEF: std::sync::OnceLock<ToolDef> = std::sync::OnceLock::new();

fn def() -> &'static ToolDef {
    DEF.get_or_init(|| ToolDef {
        name: "double_click".into(),
        description:
            "Double-click at (x, y) or on an AX element identified by element_token.\n\n\
             AX path (element_token provided): performs `AXOpen` when the element advertises it \
             (Finder items, openable list rows/cells); otherwise resolves the element's on-screen \
             center and falls back to a pixel double-click there.\n\n\
             Pixel path (x, y provided): two down/up pairs ~80 ms apart at the given coordinates."
            .into(),
        input_schema: serde_json::json!({
            "type": "object",
            "required": ["pid"],
            "properties": {
                "session": { "type": "string", "description": "For multi-call work, prefer a short public session label and repeat it on every call that accepts it. Omit it to use the authenticated transport's implicit lifecycle session." },
                "pid":           { "type": "integer", "description": "Target process ID." },
                "x":             { "type": "number",  "description": "Screen X coordinate (pixel path)." },
                "y":             { "type": "number",  "description": "Screen Y coordinate (pixel path)." },
                "window_id":     { "type": "integer", "description": "CGWindowID. Omit when element_token is supplied (the token carries it)." },
                "element_token": cua_driver_core::tool_schema::element_token_schema(),
                "delivery_mode": cua_driver_core::tool_schema::delivery_mode_schema()
            },
            "additionalProperties": false
        }),
        read_only:   false,
        destructive: true,
        idempotent:  false,
        open_world:  true,
    })
}

#[async_trait]
impl Tool for DoubleClickTool {
    fn def(&self) -> &ToolDef {
        def()
    }

    async fn invoke(&self, args: Value) -> ToolResult {
        use cua_driver_core::tool_args::ArgsExt;
        let pid = match args.require_i32("pid") {
            Ok(v) => v,
            Err(e) => return e,
        };
        // delivery_mode: foreground briefly fronts the window before the pixel
        // double-click (the explicit last resort for surfaces that drop
        // background CGEvents), via the same skylight assist click uses.
        let delivery_mode = super::DeliveryMode::parse(args.opt_str("delivery_mode").as_deref());
        let cursor_key = super::cursor_tools::resolve_cursor_key(&args);
        let window_id_arg = args.opt_u64("window_id");
        let resolved = match self.state.snapshots.resolve(pid, &args) {
            Ok(r) => r,
            Err(e) => return e,
        };
        let (element_index, window_id, element_guard) = resolved.into_parts(window_id_arg);
        let window_id = match super::native_window_id(window_id) {
            Ok(window_id) => window_id,
            Err(error) => return error,
        };

        // ── AX element path ──────────────────────────────────────────────────
        if let (Some(idx), Some(wid), Some(element_guard)) =
            (element_index, window_id, element_guard)
        {
            let element_ptr = element_guard.as_ptr();

            // Choose one background actuator before dispatch. An element that
            // advertises AXOpen uses the exact semantic route; all other
            // elements require the stricter routed-pointer proof. Do not let a
            // failed AXOpen silently cross into an ungated pointer fallback.
            let probe_guard = element_guard.clone();
            let has_ax_open = tokio::task::spawn_blocking(move || unsafe {
                copy_action_names(probe_guard.as_ptr() as AXUIElementRef)
                    .iter()
                    .any(|action| action == "AXOpen")
            })
            .await
            .unwrap_or(false);
            let _mutation_lease = if !delivery_mode.is_foreground() {
                let action = background_action_for_element(has_ax_open);
                match super::gate_background_window_action(pid, wid, Some(element_ptr), action)
                    .await
                {
                    Ok(lease) => Some(lease),
                    Err(refusal_result) => return refusal_result,
                }
            } else {
                None
            };

            // Thread the resolved session cursor key into the blocking AX path
            // so its ClickPulse lands on THIS session's cursor, not "default".
            let ck = cursor_key.clone();
            let result = tokio::task::spawn_blocking(move || {
                ax_double_click(
                    pid,
                    wid,
                    element_guard.as_ptr(),
                    idx,
                    &ck,
                    has_ax_open,
                    delivery_mode.is_foreground(),
                )
            })
            .await;

            return match result {
                Ok(Ok(msg)) => ToolResult::text(msg),
                Ok(Err(e)) => ToolResult::error(format!("double_click failed: {e}")),
                Err(e) => ToolResult::error(format!("Task error: {e}")),
            };
        }

        // ── Pixel path ───────────────────────────────────────────────────────
        let mut cx = match args.get("x").and_then(|v| v.as_f64()) {
            Some(v) => v,
            None => return ToolResult::error("Either element_token or x + y must be provided."),
        };
        let mut cy = match args.get("y").and_then(|v| v.as_f64()) {
            Some(v) => v,
            None => return ToolResult::error("Missing required parameter: y"),
        };

        // Scale back from downscaled-image space to native pixels when needed.
        let ratio = match super::screenshot_scale(&self.state, &args, pid, window_id) {
            Ok(ratio) => ratio,
            Err(refusal) => return refusal,
        };
        cx *= ratio;
        cy *= ratio;

        // Window-local → screen coordinate translation + win-local logical coords
        // for CGEventSetWindowLocation (shared with click.rs via px_frame, which
        // refuses a window with no live frame instead of silently treating the
        // local point as screen-absolute).
        let (screen_x, screen_y, win_local_x, win_local_y) = if let Some(wid) = window_id {
            match super::px_frame::resolve_or_refuse(wid).await {
                Ok(frame) => {
                    let translated = frame.to_screen(cx, cy);
                    if !delivery_mode.is_foreground()
                        && (translated.2 < 0.0
                            || translated.3 < 0.0
                            || translated.2 > frame.bounds.width
                            || translated.3 > frame.bounds.height)
                    {
                        return ToolResult::error(format!(
                            "double_click: window-local point ({:.1}, {:.1}) pt lies outside \
                             window {wid}'s {:.0}×{:.0} pt frame; background delivery refused",
                            translated.2, translated.3, frame.bounds.width, frame.bounds.height
                        ));
                    }
                    translated
                }
                Err(refusal) => return refusal,
            }
        } else {
            (cx, cy, cx, cy)
        };

        let _mutation_lease = if !delivery_mode.is_foreground() {
            if let Some(wid) = window_id {
                match super::gate_background_window_action(
                    pid,
                    wid,
                    None,
                    cua_driver_core::background_input::BackgroundAction::WindowPointer,
                )
                .await
                {
                    Ok(lease) => Some(lease),
                    Err(refusal_result) => return refusal_result,
                }
            } else {
                None
            }
        } else {
            None
        };

        // Pin overlay above the target window before animating.
        if let Some(wid) = window_id {
            crate::cursor::overlay::send_command(
                cursor_key.clone(),
                cursor_overlay::OverlayCommand::PinAbove(wid as u64),
            );
        }
        // Animate cursor to the click point; wait for arrival before firing.
        crate::cursor::overlay::animate_cursor_to(cursor_key.clone(), screen_x, screen_y).await;
        crate::cursor::overlay::send_command(
            cursor_key.clone(),
            cursor_overlay::OverlayCommand::ClickPulse {
                x: screen_x,
                y: screen_y,
            },
        );

        let fg = delivery_mode.is_foreground() && window_id.is_some();
        let route =
            match super::pixel_route::resolve(pid, fg, window_id, "mouse_double_click").await {
                Ok(route) => route,
                Err(refusal) => return refusal,
            };
        let result = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let do_click = move || -> anyhow::Result<()> {
                if route == super::pixel_route::PixelClickRoute::ForegroundHid {
                    // Warp the hardware pointer and post at the HID tap; see
                    // `pixel_route` for the cross-platform foreground contract.
                    return crate::input::mouse::click_at_xy_desktop_with_modifiers(
                        screen_x,
                        screen_y,
                        2,
                        "left",
                        &[],
                    );
                }
                if let Some(wid) = window_id {
                    crate::input::mouse::click_at_xy_with_window_local(
                        pid,
                        screen_x,
                        screen_y,
                        win_local_x,
                        win_local_y,
                        wid,
                        2,
                        &[],
                        crate::input::mouse::WindowClickDelivery::from_foreground(fg),
                    )
                } else {
                    crate::input::mouse::click_at_xy(pid, screen_x, screen_y, 2, &[])
                }
            };
            // Foreground rung: front the exact window → HID double-click →
            // restore the prior frontmost. No input is sent unless the exact
            // window is proven focused.
            match (route, window_id) {
                (super::pixel_route::PixelClickRoute::ForegroundHid, Some(wid)) => {
                    crate::input::skylight::with_foreground_hid_activation(
                        pid as libc::pid_t,
                        wid,
                        do_click,
                    )
                }
                _ => do_click(),
            }
        })
        .await;

        match result {
            Ok(Ok(())) => ToolResult::text(format!(
                "✅ Double-clicked at ({screen_x:.1}, {screen_y:.1}) ({}).",
                super::pixel_route::delivery_note(route)
            ))
            .with_structured(serde_json::json!({
                "path": super::pixel_route::path_label(route), "verified": false, "effect": "unverifiable"
            })),
            Ok(Err(e)) if route == super::pixel_route::PixelClickRoute::ForegroundHid => {
                super::pixel_route::foreground_unavailable(
                    "Double-click",
                    window_id.unwrap_or_default(),
                    &e.to_string(),
                )
            }
            Ok(Err(e)) => ToolResult::error(format!("Double-click failed: {e}")),
            Err(e)     => ToolResult::error(format!("Task error: {e}")),
        }
    }
}

// ── Blocking AX path ─────────────────────────────────────────────────────────

/// Resource identity must come from the actionable element (or its unique
/// openable child), never from a filename/title converted into a path.
unsafe fn open_destination_url(element: AXUIElementRef) -> Option<String> {
    if let Some(url) = open_document::resource_url(element).filter(|url| !url.is_empty()) {
        return resource_url_for_comparison(&url);
    }
    let mut urls = Vec::new();
    for child in copy_children(element) {
        if copy_action_names(child)
            .iter()
            .any(|action| action == "AXOpen")
        {
            if let Some(url) = open_document::resource_url(child).filter(|url| !url.is_empty()) {
                urls.push(url);
            }
        }
        CFRelease(child as CFTypeRef);
    }
    if urls.len() == 1 {
        urls.pop().and_then(|url| resource_url_for_comparison(&url))
    } else {
        None
    }
}

/// A previously open matching document is not evidence that this input worked.
/// Require a new/changed document on the exact newly focused, visible window.
fn opened_document_window(
    pid: i32,
    destination: &str,
    before: &std::collections::HashMap<u32, DocumentIdentity>,
    windows: &[crate::windows::WindowInfo],
    documents: &std::collections::HashMap<u32, String>,
    focused: Option<u32>,
) -> Option<u32> {
    windows
        .iter()
        .find(|window| {
            window.pid == pid
                && window.layer == 0
                && window.is_on_screen
                && window.on_current_space != Some(false)
                && focused == Some(window.window_id)
                && documents.get(&window.window_id).map(String::as_str) == Some(destination)
                && match before.get(&window.window_id) {
                    None => true,
                    Some(DocumentIdentity::Url(old)) => old != destination,
                    Some(DocumentIdentity::Absent) => true,
                    // An existing window without readable document identity does
                    // not establish a transition to this resource.
                    Some(DocumentIdentity::Unknown) => false,
                }
        })
        .map(|window| window.window_id)
}

/// A destination label read before AXOpen invalidates Finder's old row refs.
unsafe fn open_destination_label(element: AXUIElementRef) -> Option<String> {
    for attribute in ["AXTitle", "AXValue"] {
        if let Some(value) = copy_string_attr(element, attribute).filter(|s| !s.trim().is_empty()) {
            return Some(value);
        }
    }
    let mut labels = Vec::new();
    for child in copy_children(element) {
        let role = copy_string_attr(child, "AXRole");
        // A collection cell can expose its filename through an openable
        // text field instead of a static label. Read it before AXOpen may
        // invalidate the cell, and still refuse multiple candidate labels.
        // Ordinary editable controls do not establish an Open destination.
        let is_label = role.as_deref() == Some("AXStaticText")
            || (role.as_deref() == Some("AXTextField")
                && copy_action_names(child)
                    .iter()
                    .any(|action| action == "AXOpen"));
        if is_label {
            if let Some(value) = copy_string_attr(child, "AXValue").filter(|s| !s.trim().is_empty())
            {
                labels.push(value);
            }
        }
        CFRelease(child as CFTypeRef);
    }
    if labels.len() == 1 {
        labels.pop()
    } else {
        None
    }
}

fn ax_double_click(
    pid: i32,
    wid: u32,
    element_ptr: usize,
    idx: usize,
    cursor_key: &str,
    has_ax_open: bool,
    foreground: bool,
) -> anyhow::Result<String> {
    let element = element_ptr as AXUIElementRef;
    let mut open_error = None;

    // Try AXOpen first (Finder items, openable list rows, document cells).
    if has_ax_open {
        let before = crate::windows::window_info_by_id(wid)
            .filter(|window| window.pid == pid)
            .map(|window| window.title);
        let destination = unsafe { open_destination_label(element) };
        let resource = unsafe { open_destination_url(element) };
        let before_documents = resource.as_ref().map(|_| {
            let windows = crate::windows::all_windows();
            open_document::snapshot(pid, &windows)
        });
        let err = unsafe { perform_action(element, "AXOpen") };
        if err == kAXErrorSuccess {
            return Ok(format!("AXOpen performed on element [{idx}]."));
        }
        open_error = Some(err);
        // Finder can navigate and invalidate this AX row while returning an
        // AX error. Observe the exact window before any fallback actuator;
        // only its requested destination title proves this Open completed.
        let deadline = std::time::Instant::now()
            + std::time::Duration::from_millis(if resource.is_some() { 1_000 } else { 500 });
        let mut transitioned = false;
        loop {
            if let (Some(resource), Some(before_documents)) = (&resource, &before_documents) {
                let windows = crate::windows::all_windows();
                let documents = open_document::document_urls(pid, &windows)
                    .into_iter()
                    .filter_map(|(id, url)| resource_url_for_comparison(&url).map(|url| (id, url)))
                    .collect::<std::collections::HashMap<_, _>>();
                if let Some(opened) = opened_document_window(
                    pid,
                    resource,
                    before_documents,
                    &windows,
                    &documents,
                    crate::ax::bindings::focused_window_id_of_pid(pid),
                ) {
                    return Ok(format!("Opened element [{idx}]; exact focused window {opened} now has requested document URL {resource:?} despite AXOpen receipt {err}. No fallback click was sent."));
                }
            }
            let after = crate::windows::window_info_by_id(wid)
                .filter(|window| window.pid == pid)
                .map(|window| window.title);
            if let (Some(before), Some(after), Some(destination)) = (&before, &after, &destination)
            {
                if before != after && after == destination {
                    return Ok(format!("Opened element [{idx}]; exact window {wid} now shows {destination:?} despite AXOpen receipt {err}."));
                }
            }
            transitioned |= after != before;
            if std::time::Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if transitioned {
            anyhow::bail!("AXOpen returned {err} and changed window {wid}; requested resource={resource:?} is unverified, so no fallback click was sent");
        }
        if !foreground {
            anyhow::bail!(
                "AXOpen returned {err} for element [{idx}]; background delivery will not \
                 improvise a pointer fallback after choosing the semantic route"
            );
        }
        tracing::debug!(
            "AXOpen returned {err} for element [{idx}], falling back to pixel double-click"
        );
    }

    // Resolve screen center and fall back to pixel double-click.
    let (cx, cy) = unsafe { element_screen_center(element) }
        .ok_or_else(|| match open_error {
            Some(err) => anyhow::anyhow!("AXOpen returned {err} for element [{idx}]; document outcome is unverified and the old element has no screen center. Observe the current app before another input; no fallback click was sent."),
            None => anyhow::anyhow!("Cannot resolve screen center for element [{idx}]"),
        })?;

    // Drive THIS session's cursor (threaded in via `cursor_key`), not "default".
    crate::cursor::overlay::send_command(
        cursor_key.to_owned(),
        cursor_overlay::OverlayCommand::ClickPulse { x: cx, y: cy },
    );
    // Use the window-local primitive (not bare click_at_xy): a plain
    // click_at_xy does NOT reliably reach a backgrounded / non-key window — it
    // no-ops on AppKit controls that hit-test the window-local stamp. Mirror the
    // delivering path the `click` pixel branch uses so the double-click actually
    // lands without foregrounding the app.
    // Window-routed target: we MUST have the window's bounds to translate the
    // screen center into a window-local stamp. If bounds are missing, refuse
    // rather than stamping screen coords as window-local — that no-ops or hits
    // the wrong location while still routing to `wid`.
    let (wx, wy) = crate::windows::window_bounds_by_id(wid)
        .map(|b| (cx - b.x, cy - b.y))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Cannot resolve window bounds for window_id {wid}; refusing to stamp \
             screen coordinates as window-local for element [{idx}]."
            )
        })?;
    crate::input::mouse::click_at_xy_with_window_local(
        pid,
        cx,
        cy,
        wx,
        wy,
        wid,
        2,
        &[],
        crate::input::mouse::WindowClickDelivery::from_foreground(foreground),
    )?;
    Ok(format!(
        "✅ Double-clicked element [{idx}] at ({cx:.1}, {cy:.1})."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_foundation::base::TCFType;

    #[test]
    fn file_reference_identity_follows_rename_without_guessing_a_filename() {
        use core_foundation::url::*;
        let directory = tempfile::tempdir().unwrap();
        let directory = directory.path().canonicalize().unwrap();
        let path = directory.join("requested α.pdf");
        std::fs::write(&path, b"controlled URL identity").unwrap();
        let url = CFURL::from_path(&path, false).unwrap();
        let reference = unsafe {
            let raw = CFURLCreateFileReferenceURL(
                std::ptr::null(),
                url.as_concrete_TypeRef(),
                std::ptr::null_mut(),
            );
            assert!(!raw.is_null());
            CFURL::wrap_under_create_rule(raw).get_string().to_string()
        };
        assert_eq!(
            resource_url_for_comparison(&reference),
            Some(url.get_string().to_string())
        );
        let renamed = directory.join("same actual resource β.pdf");
        std::fs::rename(path, &renamed).unwrap();
        let expected = CFURL::from_path(&renamed, false)
            .unwrap()
            .get_string()
            .to_string();
        assert_eq!(resource_url_for_comparison(&reference), Some(expected));
        assert_eq!(
            resource_url_for_comparison("https://example.test/path?q=1"),
            Some("https://example.test/path?q=1".into())
        );
        assert_eq!(resource_url_for_comparison("file:///.file/id=0.0"), None);
    }

    fn document_window(pid: i32, id: u32) -> crate::windows::WindowInfo {
        crate::windows::WindowInfo {
            pid,
            window_id: id,
            app_name: "Document app".into(),
            title: "same display label".into(),
            bounds: crate::windows::WindowBounds {
                x: 0.0,
                y: 0.0,
                width: 600.0,
                height: 400.0,
            },
            layer: 0,
            z_index: 1,
            is_on_screen: true,
            on_current_space: Some(true),
            current_space_id: Some(1),
            space_ids: Some(vec![1]),
        }
    }

    #[test]
    fn open_receipt_requires_new_or_changed_exact_focused_resource() {
        use std::collections::HashMap;
        let destination = "file:///controlled/requested.pdf";
        let windows = vec![document_window(123, 42)];
        let documents = HashMap::from([(42, destination.to_owned())]);
        assert_eq!(
            opened_document_window(
                123,
                destination,
                &HashMap::new(),
                &windows,
                &documents,
                Some(42)
            ),
            Some(42)
        );
        let changed = HashMap::from([(
            42,
            DocumentIdentity::Url("file:///controlled/old.pdf".to_owned()),
        )]);
        assert_eq!(
            opened_document_window(123, destination, &changed, &windows, &documents, Some(42)),
            Some(42)
        );
        for before in [
            HashMap::from([(42, DocumentIdentity::Url(destination.to_owned()))]),
            HashMap::from([(42, DocumentIdentity::Unknown)]),
        ] {
            assert_eq!(
                opened_document_window(123, destination, &before, &windows, &documents, Some(42)),
                None
            );
        }
        let absent = HashMap::from([(42, DocumentIdentity::Absent)]);
        assert_eq!(
            opened_document_window(123, destination, &absent, &windows, &documents, Some(42)),
            Some(42)
        );
        for focused in [None, Some(41)] {
            assert_eq!(
                opened_document_window(
                    123,
                    destination,
                    &HashMap::new(),
                    &windows,
                    &documents,
                    focused
                ),
                None
            );
        }
    }

    #[test]
    fn open_receipt_refuses_wrong_identity_hidden_and_other_space_windows() {
        use std::collections::HashMap;
        let destination = "file:///controlled/requested.pdf";
        let documents = HashMap::from([(42, destination.to_owned())]);
        let mut wrong_pid = document_window(124, 42);
        let mut hidden = document_window(123, 42);
        hidden.is_on_screen = false;
        let mut other_space = document_window(123, 42);
        other_space.on_current_space = Some(false);
        let mut accessory = document_window(123, 42);
        accessory.layer = 3;
        wrong_pid.title = "requested.pdf".into();
        for window in [wrong_pid, hidden, other_space, accessory] {
            assert_eq!(
                opened_document_window(
                    123,
                    destination,
                    &HashMap::new(),
                    &[window],
                    &documents,
                    Some(42)
                ),
                None
            );
        }
        let wrong_resource = HashMap::from([(42, "file:///other/requested.pdf".to_owned())]);
        assert_eq!(
            opened_document_window(
                123,
                destination,
                &HashMap::new(),
                &[document_window(123, 42)],
                &wrong_resource,
                Some(42)
            ),
            None
        );
    }

    #[test]
    fn background_element_route_does_not_guess_across_actuator_classes() {
        assert_eq!(
            background_action_for_element(true),
            cua_driver_core::background_input::BackgroundAction::AxSemantic
        );
        assert_eq!(
            background_action_for_element(false),
            cua_driver_core::background_input::BackgroundAction::WindowPointer
        );
    }
}
