//! Exact-target native plaintext paste for generic macOS accessibility controls.
//!
//! This intentionally has one clipboard policy: `leave`.  NSPasteboard has no
//! atomic compare-and-restore operation, so restoration could overwrite a copy
//! made by the user after this tool writes its payload.  The payload therefore
//! remains on the clipboard after every outcome.
//!
//! `changeCount` is an observation, not a compare-and-swap primitive. An
//! external clipboard writer can still race after the final pre-dispatch check;
//! the post-dispatch token check detects only overwrites that have become
//! observable and turns them into an unknown outcome rather than a replay.

use async_trait::async_trait;
use core_foundation::base::{CFEqual, CFRelease, CFTypeRef};
use cua_driver_core::{
    background_input::BackgroundAction,
    clipboard::CLIPBOARD_WRITE_LOCK,
    protocol::ToolResult,
    tool::{Tool, ToolDef},
    tool_args::ArgsExt,
};
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::NSString;
use serde_json::Value;

use crate::ax::{
    bindings::{copy_string_attr, copy_text_range_attr, AXUIElementRef, CFRange},
    exact_target::focused_element_in_window,
};

use super::gate_background_window_action;

const MAX_PLAINTEXT_BYTES: usize = 16 * 1024;

pub struct NativePasteTool;

impl NativePasteTool {
    pub fn new() -> Self {
        Self
    }
}

static DEF: std::sync::OnceLock<ToolDef> = std::sync::OnceLock::new();

fn def() -> &'static ToolDef {
    DEF.get_or_init(|| ToolDef {
        name: "native_paste".into(),
        description: "Paste plaintext into one exact macOS native accessibility control with Cmd+V. Requires an exact pid and window_id, verifies the same focused AX element before the clipboard write and key dispatch, and succeeds only when AXValue read-back equals the expected UTF-16-range insertion. Only clipboard_policy:\"leave\" is supported: the supplied plaintext remains on the system clipboard and is never restored. delivery_mode:\"background\" (default) never fronts the target. delivery_mode:\"foreground\" briefly fronts the exact target for one physical HID Cmd+V chord, revalidates the AX target and clipboard immediately before that chord, then restores the prior frontmost app. After dispatch, any unverified result is reported as effect:\"unknown\" and must not be replayed automatically.".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "required": ["pid", "window_id", "text", "clipboard_policy"],
            "properties": {
                "session": { "type": "string", "description": "Public session label for window-scope authorization." },
                "pid": { "type": "integer", "description": "Exact target process ID." },
                "window_id": { "type": "integer", "description": "Exact target CGWindowID." },
                "text": { "type": "string", "description": "Plaintext to paste (at most 16 KiB UTF-8)." },
                "format": { "type": "string", "enum": ["text", "md", "html"], "default": "text", "description": "Only text is implemented; md and html are refused before clipboard mutation." },
                "clipboard_policy": { "type": "string", "enum": ["leave"], "description": "Required. leave writes the plaintext to NSPasteboard and never restores prior clipboard content." },
                "delivery_mode": cua_driver_core::tool_schema::delivery_mode_schema_with("Best-effort-background ladder rung (default \"background\"). \"background\": posts one PID-routed Cmd+V without fronting the target. \"foreground\": briefly front the exact target, revalidate the retained AX target and clipboard immediately before one physical HID Cmd+V chord, then restore the prior frontmost app. Choose the delivery mode before dispatch; never replay an uncertain paste.")
            },
            "additionalProperties": false
        }),
        read_only: false,
        destructive: true,
        idempotent: false,
        open_world: true,
    })
}

struct RetainedElement(usize);

impl Drop for RetainedElement {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0 as AXUIElementRef as CFTypeRef) }
    }
}

struct PreparedPaste {
    element: RetainedElement,
    before: String,
    selection: CFRange,
    expected: String,
}

#[derive(Clone, Copy)]
struct PasteboardToken {
    change_count: isize,
}

enum ClipboardWriteFailure {
    MayHaveMutated(String),
}

enum PasteDispatch {
    Dispatched,
    NoInputAfterClipboardWrite(String),
    MayHaveDispatched(String),
}

fn refusal(code: &str, reason: impl Into<String>, pid: i32, window_id: u32) -> ToolResult {
    let reason = reason.into();
    ToolResult::error(reason.clone()).with_structured(serde_json::json!({
        "status": "refused",
        "effect": "refused",
        "code": code,
        "pid": pid,
        "window_id": window_id,
        "clipboard_policy": "leave",
        "clipboard_restored": false,
        "reason": reason,
    }))
}

fn unknown(reason: impl Into<String>, pid: i32, window_id: u32) -> ToolResult {
    let reason = reason.into();
    ToolResult::error(reason.clone()).with_structured(serde_json::json!({
        "status": "unknown",
        "effect": "unknown",
        "pid": pid,
        "window_id": window_id,
        "clipboard_policy": "leave",
        "clipboard_restored": false,
        "transfer_verified": false,
        "reason": reason,
        "do_not_replay": true,
    }))
}

fn clipboard_mutation_unknown(reason: impl Into<String>, pid: i32, window_id: u32) -> ToolResult {
    let reason = reason.into();
    ToolResult::error(reason.clone()).with_structured(serde_json::json!({
        "status": "incomplete",
        "effect": "clipboard_mutation_unknown",
        "code": "clipboard_write_incomplete",
        "pid": pid,
        "window_id": window_id,
        "clipboard_policy": "leave",
        "clipboard_restored": false,
        "clipboard_may_have_changed": true,
        "input_dispatched": false,
        "reason": reason,
    }))
}

fn input_refused_after_clipboard_write(
    reason: impl Into<String>,
    pid: i32,
    window_id: u32,
) -> ToolResult {
    let reason = reason.into();
    ToolResult::error(reason.clone()).with_structured(serde_json::json!({
        "status": "incomplete",
        "effect": "input_refused_after_clipboard_write",
        "code": "target_or_clipboard_changed_before_dispatch",
        "pid": pid,
        "window_id": window_id,
        "clipboard_policy": "leave",
        "clipboard_restored": false,
        "clipboard_changed": true,
        "clipboard_may_have_changed": true,
        "input_dispatched": false,
        "reason": reason,
    }))
}

/// Convert a UTF-16 offset only when it falls on a Rust UTF-8 character
/// boundary. AXSelectedTextRange is specified in UTF-16 code units.
fn utf16_offset_to_byte(value: &str, offset: usize) -> Option<usize> {
    if offset == 0 {
        return Some(0);
    }
    let mut seen = 0;
    for (byte, ch) in value.char_indices() {
        seen += ch.len_utf16();
        if seen == offset {
            return Some(byte + ch.len_utf8());
        }
        if seen > offset {
            return None;
        }
    }
    (seen == offset).then_some(value.len())
}

fn expected_insertion(before: &str, selection: CFRange, text: &str) -> Option<String> {
    let location = usize::try_from(selection.location).ok()?;
    let length = usize::try_from(selection.length).ok()?;
    let end = location.checked_add(length)?;
    let start_byte = utf16_offset_to_byte(before, location)?;
    let end_byte = utf16_offset_to_byte(before, end)?;
    let mut expected = String::with_capacity(before.len() - (end_byte - start_byte) + text.len());
    expected.push_str(&before[..start_byte]);
    expected.push_str(text);
    expected.push_str(&before[end_byte..]);
    Some(expected)
}

fn prepare_target(pid: i32, window_id: u32, text: &str) -> Result<PreparedPaste, String> {
    unsafe {
        let element = focused_element_in_window(pid, window_id).ok_or_else(|| {
            "The requested window does not have a focused AX element that can be proven local to it."
                .to_owned()
        })?;
        let before = match copy_string_attr(element, "AXValue") {
            Some(value) => value,
            None => {
                CFRelease(element as CFTypeRef);
                return Err("Focused target has no readable plaintext AXValue.".into());
            }
        };
        let selection = match copy_text_range_attr(element, "AXSelectedTextRange") {
            Some(range) => range,
            None => {
                CFRelease(element as CFTypeRef);
                return Err("Focused target has no readable AXSelectedTextRange.".into());
            }
        };
        let expected = match expected_insertion(&before, selection, text) {
            Some(expected) => expected,
            None => {
                CFRelease(element as CFTypeRef);
                return Err(
                    "AXSelectedTextRange is invalid or splits a UTF-16 surrogate pair.".into(),
                );
            }
        };
        Ok(PreparedPaste {
            element: RetainedElement(element as usize),
            before,
            selection,
            expected,
        })
    }
}

fn target_still_matches_prepared(
    retained: usize,
    pid: i32,
    window_id: u32,
    before: &str,
    selection: CFRange,
) -> bool {
    unsafe {
        let Some(current) = focused_element_in_window(pid, window_id) else {
            return false;
        };
        let same = CFEqual(retained as CFTypeRef, current as CFTypeRef) != 0;
        CFRelease(current as CFTypeRef);
        same && copy_string_attr(retained as AXUIElementRef, "AXValue").as_deref() == Some(before)
            && copy_text_range_attr(retained as AXUIElementRef, "AXSelectedTextRange")
                == Some(selection)
    }
}

fn write_plaintext(payload: &str) -> Result<PasteboardToken, ClipboardWriteFailure> {
    unsafe {
        let board = NSPasteboard::generalPasteboard();
        board.clearContents();
        let text = NSString::from_str(payload);
        if !board.setString_forType(&text, NSPasteboardTypeString) {
            return Err(ClipboardWriteFailure::MayHaveMutated(
                "NSPasteboard cleared or may have changed, then refused the plaintext payload."
                    .into(),
            ));
        }
        Ok(PasteboardToken {
            change_count: board.changeCount(),
        })
    }
}

fn expected_post_selection(selection: CFRange, text: &str) -> Option<CFRange> {
    let inserted_utf16 = isize::try_from(text.encode_utf16().count()).ok()?;
    Some(CFRange {
        location: selection.location.checked_add(inserted_utf16)?,
        length: 0,
    })
}

/// A collapsed empty insertion does not change either AXValue or the selected
/// range. Generic AX read-back cannot prove that Cmd+V was consumed, so this
/// candidate is refused before any clipboard or input mutation.
fn is_unverifiable_noop(before: &str, selection: CFRange, text: &str) -> bool {
    expected_insertion(before, selection, text) == Some(before.to_owned())
        && expected_post_selection(selection, text) == Some(selection)
}

fn observe_after_dispatch(
    element: usize,
    expected: &str,
    expected_selection: Option<CFRange>,
    require_selection_proof: bool,
    token: PasteboardToken,
    payload: &str,
) -> bool {
    // Give the target a bounded, observation-only settle interval. No input is
    // replayed while waiting: a delayed mutation remains one dispatched Cmd+V.
    for delay_ms in [20, 50, 100] {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        let value_matches = unsafe {
            copy_string_attr(element as AXUIElementRef, "AXValue").as_deref() == Some(expected)
        };
        let selection_matches = !require_selection_proof
            || expected_selection.is_some_and(|selection| unsafe {
                copy_text_range_attr(element as AXUIElementRef, "AXSelectedTextRange")
                    == Some(selection)
            });
        if value_matches && selection_matches && pasteboard_still_has(token, payload) {
            return true;
        }
    }
    false
}

fn pasteboard_still_has(token: PasteboardToken, payload: &str) -> bool {
    unsafe {
        let board = NSPasteboard::generalPasteboard();
        board.changeCount() == token.change_count
            && board
                .stringForType(NSPasteboardTypeString)
                .is_some_and(|value| value.to_string() == payload)
    }
}

#[async_trait]
impl Tool for NativePasteTool {
    fn def(&self) -> &'static ToolDef {
        def()
    }

    async fn invoke(&self, args: Value) -> ToolResult {
        let pid = match args.require_i32("pid") {
            Ok(pid) if pid > 0 => pid,
            Ok(_) => return ToolResult::error("pid must be positive"),
            Err(error) => return error,
        };
        let window_id = match args.require_u64("window_id").and_then(|id| {
            u32::try_from(id).map_err(|_| ToolResult::error("window_id is out of range"))
        }) {
            Ok(window_id) if window_id > 0 => window_id,
            Ok(_) => return ToolResult::error("window_id must be positive"),
            Err(error) => return error,
        };
        let text = match args.require_str("text") {
            Ok(text) => text,
            Err(error) => return error,
        };
        if !matches!(args.opt_str("format").as_deref(), None | Some("text")) {
            return refusal(
                "unsupported_format",
                "native_paste currently supports only format:\"text\"; md and html are refused before clipboard mutation.",
                pid,
                window_id,
            );
        }
        if args.opt_str("clipboard_policy").as_deref() != Some("leave") {
            return refusal(
                "clipboard_policy_required",
                "native_paste requires clipboard_policy:\"leave\"; no other policy is implemented.",
                pid,
                window_id,
            );
        }
        if text.len() > MAX_PLAINTEXT_BYTES {
            return refusal(
                "payload_too_large",
                format!("native_paste accepts at most {MAX_PLAINTEXT_BYTES} UTF-8 bytes."),
                pid,
                window_id,
            );
        }
        let delivery_mode = super::DeliveryMode::parse(args.opt_str("delivery_mode").as_deref());

        let text_for_prepare = text.clone();
        let prepared = match tokio::task::spawn_blocking(move || {
            prepare_target(pid, window_id, &text_for_prepare)
        })
        .await
        {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(reason)) => return refusal("target_unverifiable", reason, pid, window_id),
            Err(error) => return refusal("target_unverifiable", error.to_string(), pid, window_id),
        };
        if is_unverifiable_noop(&prepared.before, prepared.selection, &text) {
            return refusal(
                "unverifiable_noop",
                "native_paste refuses an empty insertion at a collapsed selection because AX read-back cannot prove Cmd+V was consumed; no clipboard or input mutation occurred.",
                pid,
                window_id,
            );
        }

        let _mutation_lease = if delivery_mode.is_foreground() {
            // Foreground intentionally bypasses the background eligibility
            // decision, but still serializes mutations to this process while
            // activation, revalidation, and the single chord are in flight.
            Some(super::acquire_background_mutation(pid).await)
        } else {
            match gate_background_window_action(
                pid,
                window_id,
                Some(prepared.element.0),
                BackgroundAction::GenericKey,
            )
            .await
            {
                Ok(lease) => Some(lease),
                Err(result) => return result,
            }
        };
        let _clipboard_guard = CLIPBOARD_WRITE_LOCK.lock().await;

        let element = prepared.element.0;
        let before = prepared.before.clone();
        let selection = prepared.selection;
        let ready_before_write = tokio::task::spawn_blocking(move || {
            target_still_matches_prepared(element, pid, window_id, &before, selection)
        })
        .await
        .unwrap_or(false);
        if !ready_before_write {
            return refusal(
                "target_changed_before_clipboard_write",
                "Focused AX target, value, or selection changed before clipboard mutation; no paste was dispatched.",
                pid,
                window_id,
            );
        }

        let payload = text.clone();
        let token = match tokio::task::spawn_blocking(move || write_plaintext(&payload)).await {
            Ok(Ok(token)) => token,
            Ok(Err(ClipboardWriteFailure::MayHaveMutated(reason))) => {
                return clipboard_mutation_unknown(reason, pid, window_id)
            }
            Err(error) => {
                return clipboard_mutation_unknown(
                    format!("Clipboard worker ended after clipboard mutation began: {error}"),
                    pid,
                    window_id,
                )
            }
        };

        let payload = text.clone();
        let element = prepared.element.0;
        let before = prepared.before.clone();
        let selection = prepared.selection;
        let ready = tokio::task::spawn_blocking(move || {
            target_still_matches_prepared(element, pid, window_id, &before, selection)
                && pasteboard_still_has(token, &payload)
        })
        .await
        .unwrap_or(false);
        if !ready {
            return input_refused_after_clipboard_write(
                "Target focus, value, selection, or clipboard content changed before Cmd+V; no paste was dispatched and newer clipboard content was preserved.",
                pid,
                window_id,
            );
        }

        // Background posts exactly one PID-routed chord guarded above. The
        // explicit foreground rung instead performs exactly one physical HID
        // chord inside the exact-window activation interval. It does not try
        // the background transport first and never replays either transport.
        let payload = text.clone();
        let element = prepared.element.0;
        let before = prepared.before.clone();
        let selection = prepared.selection;
        let foreground = delivery_mode.is_foreground();
        let dispatch = tokio::task::spawn_blocking(move || {
            if !foreground {
                return match crate::input::keyboard::press_key(pid, "v", &["cmd"]) {
                    Ok(()) => PasteDispatch::Dispatched,
                    Err(error) => PasteDispatch::MayHaveDispatched(error.to_string()),
                };
            }

            let mut input_started = false;
            let activation = crate::input::skylight::with_foreground_hid_activation(
                pid as libc::pid_t,
                window_id,
                || {
                    // Activation can change the first responder or selection.
                    // This is deliberately the final check before the only
                    // global HID chord, while the activation guard still owns
                    // the exact target window.
                    if !target_still_matches_prepared(element, pid, window_id, &before, selection)
                        || !pasteboard_still_has(token, &payload)
                    {
                        anyhow::bail!(
                            "Target focus, value, selection, or clipboard content changed during foreground activation"
                        );
                    }
                    // The chord can fail after posting some transitions.
                    // Mark the attempt before entering the input primitive.
                    input_started = true;
                    crate::input::keyboard::press_key_bare_global("v", &["cmd"])?;
                    Ok(())
                },
            );
            match activation {
                Ok(()) => PasteDispatch::Dispatched,
                Err(error) if input_started => PasteDispatch::MayHaveDispatched(error.to_string()),
                Err(error) => PasteDispatch::NoInputAfterClipboardWrite(error.to_string()),
            }
        })
        .await;
        match dispatch {
            Ok(PasteDispatch::Dispatched) => {}
            Ok(PasteDispatch::NoInputAfterClipboardWrite(reason)) => {
                return input_refused_after_clipboard_write(reason, pid, window_id)
            }
            Ok(PasteDispatch::MayHaveDispatched(reason)) => {
                return unknown(
                    format!("Cmd+V may have been dispatched: {reason}"),
                    pid,
                    window_id,
                )
            }
            Err(error) => {
                return unknown(
                    format!("Cmd+V worker failed after dispatch began: {error}"),
                    pid,
                    window_id,
                )
            }
        }

        let element = prepared.element.0;
        let expected = prepared.expected.clone();
        let expected_selection = expected_post_selection(prepared.selection, &text);
        let require_selection_proof = prepared.expected == prepared.before;
        let payload = text.clone();
        let verified = tokio::task::spawn_blocking(move || {
            observe_after_dispatch(
                element,
                &expected,
                expected_selection,
                require_selection_proof,
                token,
                &payload,
            )
        })
        .await
        .unwrap_or(false);
        if !verified {
            return unknown(
                "Cmd+V was dispatched but the retained AX target and clipboard token did not provide the expected bounded observation. Observe before deciding what to do next.",
                pid,
                window_id,
            );
        }

        ToolResult::text("Completed one verified native plaintext paste.").with_structured(
            serde_json::json!({
                "status": "completed",
                "effect": "completed",
                "pid": pid,
                "window_id": window_id,
                "clipboard_policy": "leave",
                "clipboard_restored": false,
                "target_value_verified": true,
                "clipboard_payload_observed_before_and_after_dispatch": true,
                "transfer_verified": false,
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_offsets_respect_astral_character_boundaries() {
        assert_eq!(utf16_offset_to_byte("a😀b", 0), Some(0));
        assert_eq!(utf16_offset_to_byte("a😀b", 1), Some(1));
        assert_eq!(utf16_offset_to_byte("a😀b", 2), None);
        assert_eq!(utf16_offset_to_byte("a😀b", 3), Some(5));
        assert_eq!(utf16_offset_to_byte("a😀b", 4), Some(6));
    }

    #[test]
    fn expected_insertion_uses_utf16_selection_range() {
        assert_eq!(
            expected_insertion(
                "a😀b",
                CFRange {
                    location: 1,
                    length: 2
                },
                "X"
            ),
            Some("aXb".into())
        );
        assert_eq!(
            expected_insertion(
                "a😀b",
                CFRange {
                    location: 2,
                    length: 1
                },
                "X"
            ),
            None
        );
    }

    #[test]
    fn only_unchanged_value_and_selection_is_an_unverifiable_noop() {
        assert!(is_unverifiable_noop(
            "value",
            CFRange {
                location: 2,
                length: 0,
            },
            ""
        ));
        assert!(!is_unverifiable_noop(
            "value",
            CFRange {
                location: 0,
                length: 5,
            },
            "value"
        ));
    }

    #[test]
    fn schema_exposes_background_and_foreground_delivery_modes() {
        let mode = &def().input_schema["properties"]["delivery_mode"];
        assert_eq!(
            mode["enum"],
            serde_json::json!(["background", "foreground"])
        );
    }
}
