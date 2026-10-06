//! Exact-window AX text selection, with a verified UTF-16 range read-back.

use async_trait::async_trait;
use cua_driver_core::{
    protocol::ToolResult,
    tool::{Tool, ToolDef},
};
use serde_json::Value;
use std::sync::Arc;

use super::ToolState;
use crate::ax::bindings::{
    copy_string_attr, copy_text_range_attr, is_attribute_settable, kAXErrorSuccess,
    set_text_range_attr, AXTextRange, AXUIElementRef,
};

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
        description: "Select one unambiguous occurrence of text in a native editable element, or place the caret immediately before/after it. The exact window, element, and resulting UTF-16 selection are verified. Use prefix/suffix to disambiguate repeated text.".into(),
        input_schema: serde_json::json!({
            "type": "object", "required": ["pid", "text"],
            "properties": {
                "session": {"type":"string"}, "pid": {"type":"integer"},
                "window_id": {"type":"integer"},
                "element_token": cua_driver_core::tool_schema::element_token_schema(),
                "text": {"type":"string"}, "prefix": {"type":"string"},
                "suffix": {"type":"string"},
                "selection_type": {"type":"string", "enum":["text","cursor_before","cursor_after"]}
            }, "additionalProperties": false
        }),
        read_only: false, destructive: false, idempotent: true, open_world: true,
    })
}

fn selection_range(
    value: &str,
    needle: &str,
    prefix: Option<&str>,
    suffix: Option<&str>,
    selection_type: &str,
) -> Result<AXTextRange, &'static str> {
    if needle.is_empty() {
        return Err("empty_selection_text");
    }
    let matches: Vec<_> = value
        .match_indices(needle)
        .filter(|(start, text)| {
            let end = start + text.len();
            prefix.is_none_or(|p| value[..*start].ends_with(p))
                && suffix.is_none_or(|s| value[end..].starts_with(s))
        })
        .collect();
    if matches.is_empty() {
        return Err("selection_text_not_found");
    }
    if matches.len() != 1 {
        return Err("ambiguous_selection_text");
    }
    let start = matches[0].0;
    let before = value[..start].encode_utf16().count() as isize;
    let length = needle.encode_utf16().count() as isize;
    Ok(match selection_type {
        "cursor_before" => AXTextRange {
            location: before,
            length: 0,
        },
        "cursor_after" => AXTextRange {
            location: before + length,
            length: 0,
        },
        _ => AXTextRange {
            location: before,
            length,
        },
    })
}

#[async_trait]
impl Tool for SelectTextTool {
    fn def(&self) -> &'static ToolDef {
        def()
    }

    async fn invoke(&self, args: Value) -> ToolResult {
        use cua_driver_core::tool_args::ArgsExt;
        let pid = match args.require_i32("pid") {
            Ok(v) => v,
            Err(e) => return e,
        };
        let text = match args.require_str("text") {
            Ok(v) => v,
            Err(e) => return e,
        };
        let kind = args
            .opt_str("selection_type")
            .unwrap_or_else(|| "text".into());
        if !matches!(kind.as_str(), "text" | "cursor_before" | "cursor_after") {
            return ToolResult::error("invalid_selection_type");
        }
        let (_element_index, window_id, guard) = match self.state.snapshots.resolve(pid, &args) {
            Ok(cua_driver_core::element_token::ResolvedElement::Element {
                element_index,
                window_id,
                element,
            }) => match u32::try_from(window_id) {
                Ok(window_id) => (element_index, window_id, element),
                Err(_) => return ToolResult::error("window_id is out of range for macOS."),
            },
            Ok(cua_driver_core::element_token::ResolvedElement::None) => {
                return ToolResult::error(
                    "select_text requires a fresh exact element_token from get_window_state.",
                )
            }
            Err(refusal) => return refusal,
        };
        let ptr = guard.as_ptr();
        let _lease = match super::gate_background_window_action(
            pid,
            window_id,
            Some(ptr),
            cua_driver_core::background_input::BackgroundAction::AxSemantic,
        )
        .await
        {
            Ok(lease) => lease,
            Err(refusal) => return refusal,
        };
        // A verified AXSelectedTextRange alone can belong to a background
        // editor while the next formatting shortcut goes to another restored
        // document. Selection must establish the exact first responder too.
        let activation = super::bring_to_front::BringToFrontTool
            .invoke(serde_json::json!({"pid": pid, "window_id": window_id}))
            .await;
        if activation.is_error == Some(true) {
            return activation;
        }
        let address = ptr as usize;
        let prefix = args.opt_str("prefix");
        let suffix = args.opt_str("suffix");
        match tokio::task::spawn_blocking(move || unsafe {
            let _retained = guard;
            let element = address as AXUIElementRef;
            crate::input::ax_actions::focus_element(address)
                .map_err(|error| format!("selection_focus_failed:{error}"))?;
            // AX focus can settle asynchronously after a successful request.
            // Observe the retained exact element without replaying input or
            // accepting a different first responder.
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
            while !crate::input::ax_actions::is_element_focused(pid, address)
                && std::time::Instant::now() < deadline
            {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            if !crate::input::ax_actions::is_element_focused(pid, address) {
                return Err("selection_focus_unverified".to_owned());
            }
            if !is_attribute_settable(element, "AXSelectedTextRange") {
                return Err("selection_range_not_settable".to_owned());
            }
            let value = copy_string_attr(element, "AXValue")
                .ok_or_else(|| "selection_text_unavailable".to_owned())?;
            let range = selection_range(&value, &text, prefix.as_deref(), suffix.as_deref(), &kind)
                .map_err(str::to_owned)?;
            let err = set_text_range_attr(element, range);
            if err != kAXErrorSuccess {
                return Err(format!("selection_range_set_failed:{err}"));
            }
            if copy_text_range_attr(element) != Some(range) {
                return Err("selection_range_unverified".to_owned());
            }
            Ok(range)
        })
        .await
        {
            Ok(Ok(range)) => ToolResult::text("Selected requested native text range")
                .with_structured(serde_json::json!({
                    "status":"completed", "verified":true,
                    "location":range.location, "length":range.length
                })),
            Ok(Err(error)) => ToolResult::error(error),
            Err(error) => ToolResult::error(format!("Selection task failed: {error}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf16_and_context_are_exact() {
        assert_eq!(
            selection_range(
                "😀 alpha / 😀 alpha",
                "alpha",
                Some("😀 "),
                Some(" /"),
                "text"
            ),
            Ok(AXTextRange {
                location: 3,
                length: 5
            })
        );
        assert_eq!(
            selection_range("😀 alpha / 😀 alpha", "alpha", None, None, "text"),
            Err("ambiguous_selection_text")
        );
        assert_eq!(
            selection_range("😀 alpha", "alpha", None, None, "cursor_after"),
            Ok(AXTextRange {
                location: 8,
                length: 0
            })
        );
    }
}
