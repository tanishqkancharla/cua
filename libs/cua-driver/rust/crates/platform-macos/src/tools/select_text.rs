//! Verified native text selection for one cached macOS AX element.
//!
//! The tool writes `AXSelectedTextRange` once. It binds to a current snapshot
//! token, matches against a retained element's live `AXValue`, and reads that
//! exact range back. There is no whole-window search or keyboard fallback.

use async_trait::async_trait;
use cua_driver_core::{
    background_input::BackgroundAction,
    protocol::ToolResult,
    tool::{Tool, ToolDef},
    tool_args::ArgsExt,
};
use serde_json::Value;
use std::sync::Arc;

use core_foundation::base::{CFEqual, CFRelease, CFTypeRef};

use crate::ax::bindings::{
    copy_string_attr, copy_text_range_attr, is_attribute_settable, kAXErrorSuccess,
    set_text_range_attr, AXUIElementRef, CFRange,
};

use super::ToolState;

pub struct SelectTextTool {
    state: Arc<ToolState>,
}

impl SelectTextTool {
    pub fn new(state: Arc<ToolState>) -> Self {
        Self { state }
    }
}

static DEF: std::sync::OnceLock<ToolDef> = std::sync::OnceLock::new();

fn def() -> &'static ToolDef {
    DEF.get_or_init(|| ToolDef {
        name: "select_text".into(),
        description: "Select one uniquely matched text range in an exact macOS accessibility element. Requires an element_token, or an element_index with snapshot_id, from get_window_state. It matches the element's current AXValue, not snapshot text. prefix and suffix constrain repeated text. selection_type:\"text\" selects the match; cursor_before and cursor_after place a collapsed caret at its respective UTF-16 boundary. The driver writes AXSelectedTextRange once and succeeds only after the retained element reads back the requested range. It never searches a whole window and never falls back to keyboard selection. An uncertain write is effect:\"unknown\" and must not be replayed automatically.".into(),
        input_schema: serde_json::json!({
            "type":"object", "required":["pid","text"], "properties": {
                "session":{"type":"string"}, "pid":{"type":"integer"}, "window_id":{"type":"integer"},
                "element_index":cua_driver_core::tool_schema::element_index_schema(), "element_token":cua_driver_core::tool_schema::element_token_schema(), "snapshot_id":cua_driver_core::tool_schema::snapshot_id_schema(),
                "text":{"type":"string","description":"Exact non-empty text to match in the target's live AXValue."}, "prefix":{"type":"string"}, "suffix":{"type":"string"},
                "selection_type":{"type":"string","enum":["text","cursor_before","cursor_after"],"default":"text"}
            }, "additionalProperties":false
        }),
        read_only: false,
        destructive: true,
        idempotent: false,
        open_world: true,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectionType {
    Text,
    CursorBefore,
    CursorAfter,
}

impl SelectionType {
    fn parse(value: Option<&str>) -> Option<Self> {
        match value.unwrap_or("text") {
            "text" => Some(Self::Text),
            "cursor_before" => Some(Self::CursorBefore),
            "cursor_after" => Some(Self::CursorAfter),
            _ => None,
        }
    }
}

fn refusal(code: &str, reason: impl Into<String>, pid: i32, window_id: u32) -> ToolResult {
    let reason = reason.into();
    ToolResult::error(reason.clone()).with_structured(serde_json::json!({"status":"refused","effect":"refused","code":code,"pid":pid,"window_id":window_id,"reason":reason}))
}

fn unknown(reason: impl Into<String>, pid: i32, window_id: u32) -> ToolResult {
    let reason = reason.into();
    ToolResult::error(reason.clone()).with_structured(serde_json::json!({"status":"unknown","effect":"unknown","pid":pid,"window_id":window_id,"verified":false,"reason":reason,"do_not_replay":true}))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MatchResult {
    Missing,
    Ambiguous,
    Unique(usize, usize),
}

/// Find a match satisfying the immediate optional prefix/suffix. Repeated
/// candidates are materially different from absent text because callers can
/// safely recover by adding context, so preserve that refusal reason.
fn find_match(value: &str, text: &str, prefix: Option<&str>, suffix: Option<&str>) -> MatchResult {
    if text.is_empty() {
        return MatchResult::Missing;
    }
    let mut found = None;
    // `match_indices` skips overlaps (`"aaa"` / `"aa"`), which could
    // silently select the first of two valid ranges. Start at every UTF-8
    // character boundary and stop at the second context-valid candidate.
    for (start, _) in value.char_indices() {
        let tail = &value[start..];
        if !tail.starts_with(text) {
            continue;
        }
        let end = start + text.len();
        if !prefix.is_none_or(|prefix| value[..start].ends_with(prefix))
            || !suffix.is_none_or(|suffix| value[end..].starts_with(suffix))
        {
            continue;
        }
        if found.replace((start, end)).is_some() {
            return MatchResult::Ambiguous;
        }
    }
    found.map_or(MatchResult::Missing, |(start, end)| {
        MatchResult::Unique(start, end)
    })
}

fn utf16_offset(value: &str, byte: usize) -> Option<isize> {
    value
        .is_char_boundary(byte)
        .then(|| value[..byte].encode_utf16().count())
        .and_then(|units| isize::try_from(units).ok())
}

fn requested_range(
    value: &str,
    text: &str,
    prefix: Option<&str>,
    suffix: Option<&str>,
    selection_type: SelectionType,
) -> Result<CFRange, MatchResult> {
    let (start, end) = match find_match(value, text, prefix, suffix) {
        MatchResult::Unique(start, end) => (start, end),
        outcome => return Err(outcome),
    };
    let Some(start) = utf16_offset(value, start) else {
        return Err(MatchResult::Missing);
    };
    let Some(end) = utf16_offset(value, end) else {
        return Err(MatchResult::Missing);
    };
    match selection_type {
        SelectionType::Text => end
            .checked_sub(start)
            .map(|length| CFRange {
                location: start,
                length,
            })
            .ok_or(MatchResult::Missing),
        SelectionType::CursorBefore => Ok(CFRange {
            location: start,
            length: 0,
        }),
        SelectionType::CursorAfter => Ok(CFRange {
            location: end,
            length: 0,
        }),
    }
}

#[async_trait]
impl Tool for SelectTextTool {
    fn def(&self) -> &'static ToolDef {
        def()
    }

    async fn invoke(&self, args: Value) -> ToolResult {
        let pid = match args.require_i32("pid") {
            Ok(pid) if pid > 0 => pid,
            Ok(_) => return ToolResult::error("pid must be positive"),
            Err(error) => return error,
        };
        let text = match args.require_str("text") {
            Ok(text) if !text.is_empty() => text,
            Ok(_) => return ToolResult::error("text must be non-empty"),
            Err(error) => return error,
        };
        let Some(selection_type) = SelectionType::parse(args.opt_str("selection_type").as_deref())
        else {
            return ToolResult::error(
                "selection_type must be text, cursor_before, or cursor_after",
            );
        };
        let has_token = args.opt_str("element_token").is_some();
        let has_snapshot_binding =
            args.opt_u64("element_index").is_some() && args.opt_str("snapshot_id").is_some();
        if !has_token && !has_snapshot_binding {
            return ToolResult::error(
                "select_text requires element_token, or element_index together with snapshot_id, from get_window_state.",
            )
            .with_structured(serde_json::json!({
                "status": "refused", "effect": "refused", "code": "snapshot_binding_required"
            }));
        }
        let window_arg = match args.opt_u64("window_id") {
            Some(value) => match u32::try_from(value) {
                Ok(value) => Some(value),
                Err(_) => return ToolResult::error("window_id is out of range"),
            },
            None => None,
        };
        let resolved = match cua_driver_core::element_token::resolve_element_args(
            pid,
            args.opt_u64("element_index").map(|value| value as usize),
            args.opt_str("element_token").as_deref(),
            args.opt_str("snapshot_id").as_deref(),
            window_arg,
            "select_text",
        ) {
            Ok(resolved) => resolved,
            Err(error) => return error,
        };
        let cua_driver_core::element_token::ResolvedElement::Element {
            window_id: Some(window_id),
            element_index,
            ..
        } = resolved
        else {
            return ToolResult::error("select_text requires element_token, or element_index with snapshot_id, to bind an exact observed element.");
        };
        let element = match self.state.element_cache.get_element_retained(pid, window_id, element_index) { Some(element) => element, None => return refusal("element_not_found", format!("Element index {element_index} is no longer retained; call get_window_state again."), pid, window_id) };
        let element_ptr = element.as_ptr() as usize;
        let _mutation_lease = match super::gate_background_window_action(
            pid,
            window_id,
            Some(element_ptr),
            BackgroundAction::AxSemantic,
        )
        .await
        {
            Ok(lease) => lease,
            Err(result) => return result,
        };
        let preflight_element = element.duplicate();
        let preflight = tokio::task::spawn_blocking(move || unsafe {
            let _element = preflight_element;
            is_attribute_settable(element_ptr as AXUIElementRef, "AXSelectedTextRange")
        })
        .await
        .unwrap_or(false);
        if !preflight {
            return refusal("selection_range_not_settable", "The retained target does not expose a writable AXSelectedTextRange; no focus or selection was changed.", pid, window_id);
        }

        let prefix = args.opt_str("prefix");
        let suffix = args.opt_str("suffix");
        let preparation_element = element.duplicate();
        let preparation = tokio::task::spawn_blocking(move || unsafe {
            let _element = preparation_element;
            let element = element_ptr as AXUIElementRef;
            let value = copy_string_attr(element, "AXValue").ok_or(MatchResult::Missing)?;
            let range = requested_range(
                &value,
                &text,
                prefix.as_deref(),
                suffix.as_deref(),
                selection_type,
            )?;
            Ok::<_, MatchResult>((value, range))
        })
        .await;
        let (live_value, expected_range) = match preparation {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(MatchResult::Ambiguous)) => return refusal("ambiguous_match", "The target's live AXValue contains ambiguous matching text; add prefix or suffix context. No selection was changed.", pid, window_id),
            _ => return refusal("match_not_found", "The target's live AXValue did not contain one uniquely context-matched range; no selection was changed.", pid, window_id),
        };
        // Focusing an editable AX element can change application state. It is
        // intentionally after every refusal-only preflight, and any failure to
        // prove the retained exact element owns focus is an unknown outcome.
        let focus_element = element.duplicate();
        let prior_front = crate::apps::frontmost_pid();
        let focused = crate::focus_guard::with_focus_suppressed(
            Some(pid),
            prior_front,
            "select_text.AXFocused",
            || async move {
                tokio::task::spawn_blocking(move || unsafe {
                    let _element = focus_element;
                    let _ = crate::input::ax_actions::focus_element(element_ptr);
                    let Some(focused) =
                        crate::ax::exact_target::focused_element_in_window(pid, window_id)
                    else {
                        return false;
                    };
                    let same = CFEqual(focused as CFTypeRef, element_ptr as CFTypeRef) != 0;
                    CFRelease(focused as CFTypeRef);
                    same
                })
                .await
                .unwrap_or(false)
            },
        )
        .await;
        if !focused {
            return unknown("AX focus was attempted but the retained target could not be proven as the focused element in the requested window. Observe before deciding what to do next.", pid, window_id);
        }
        let value_for_write = live_value.clone();
        let write_element = element.duplicate();
        let write = tokio::task::spawn_blocking(move || unsafe {
            let _element = write_element;
            let element = element_ptr as AXUIElementRef;
            let focused = crate::ax::exact_target::focused_element_in_window(pid, window_id);
            let focus_matches = focused.is_some_and(|focused| {
                let same = CFEqual(focused as CFTypeRef, element_ptr as CFTypeRef) != 0;
                CFRelease(focused as CFTypeRef);
                same
            });
            if !focus_matches
                || copy_string_attr(element, "AXValue").as_deref() != Some(value_for_write.as_str())
            {
                return None;
            }
            Some(set_text_range_attr(
                element,
                "AXSelectedTextRange",
                expected_range,
            ))
        })
        .await;
        match write { Ok(Some(error)) if error == kAXErrorSuccess => {}, Ok(None) => return unknown("Focus was attempted, but exact focus or live text changed before the range write. The range was not written; observe the potentially changed focus or selection before continuing.", pid, window_id), Ok(Some(error)) => return unknown(format!("AXSelectedTextRange write returned {error} after selection mutation began."), pid, window_id), Err(error) => return unknown(format!("Selection worker ended after AXSelectedTextRange mutation began: {error}"), pid, window_id) }
        let verify_element = element.duplicate();
        let verified = tokio::task::spawn_blocking(move || unsafe {
            let _element = verify_element;
            let element = element_ptr as AXUIElementRef;
            let focused = crate::ax::exact_target::focused_element_in_window(pid, window_id);
            let focus_matches = focused.is_some_and(|focused| {
                let same = CFEqual(focused as CFTypeRef, element_ptr as CFTypeRef) != 0;
                CFRelease(focused as CFTypeRef);
                same
            });
            focus_matches
                && copy_string_attr(element, "AXValue").as_deref() == Some(live_value.as_str())
                && copy_text_range_attr(element, "AXSelectedTextRange") == Some(expected_range)
        })
        .await
        .unwrap_or(false);
        if !verified {
            return unknown("AXSelectedTextRange was written but the retained target did not read back the requested range, unchanged live value, and exact focused element. Observe before deciding what to do next.", pid, window_id);
        }
        ToolResult::text("Completed one verified native text selection.").with_structured(serde_json::json!({"status":"completed","effect":"completed","pid":pid,"window_id":window_id,"element_index":element_index,"selection_type":match selection_type { SelectionType::Text => "text", SelectionType::CursorBefore => "cursor_before", SelectionType::CursorAfter => "cursor_after" },"range_utf16":{"location":expected_range.location,"length":expected_range.length},"verified":true}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_match_uses_utf16_offsets_without_splitting_astral_text() {
        assert_eq!(
            requested_range(
                "😀 one needle.",
                "needle",
                Some("😀 one "),
                Some("."),
                SelectionType::Text
            ),
            Ok(CFRange {
                location: 7,
                length: 6
            })
        );
    }
    #[test]
    fn repeated_text_requires_context_or_refuses() {
        assert_eq!(
            find_match("one needle, two needle.", "needle", None, None),
            MatchResult::Ambiguous
        );
        assert_eq!(
            requested_range(
                "one needle, two needle.",
                "needle",
                Some("two "),
                Some("."),
                SelectionType::Text
            ),
            Ok(CFRange {
                location: 16,
                length: 6
            })
        );
    }

    #[test]
    fn overlapping_matches_are_ambiguous() {
        assert_eq!(find_match("aaa", "aa", None, None), MatchResult::Ambiguous);
    }
    #[test]
    fn caret_modes_use_match_edges() {
        let value = "prefix needle suffix";
        assert_eq!(
            requested_range(value, "needle", None, None, SelectionType::CursorBefore),
            Ok(CFRange {
                location: 7,
                length: 0
            })
        );
        assert_eq!(
            requested_range(value, "needle", None, None, SelectionType::CursorAfter),
            Ok(CFRange {
                location: 13,
                length: 0
            })
        );
    }
}
