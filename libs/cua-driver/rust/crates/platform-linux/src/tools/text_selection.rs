//! Exact native Text-interface selection. No pointer/keyboard fallback.
use crate::{
    atspi::ElementCache,
    text_selection::{SelectionKind, SelectionRequest},
};
use async_trait::async_trait;
use cua_driver_core::{
    element_token::{resolve_element_args, ResolvedElement},
    protocol::ToolResult,
    tool::{Tool, ToolDef},
    tool_args::ArgsExt,
};
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock};

pub(super) struct SelectTextTool {
    pub element_cache: Arc<ElementCache>,
}
static DEF: OnceLock<ToolDef> = OnceLock::new();

fn refused(message: impl Into<String>) -> ToolResult {
    let message = message.into();
    ToolResult::error(message.clone()).with_structured(json!({
        "status":"refused", "mutation_submitted":false,
        "refusal":{"code":"selection_unavailable","message":message}
    }))
}

#[async_trait]
impl Tool for SelectTextTool {
    fn def(&self) -> &ToolDef {
        DEF.get_or_init(|| ToolDef {
            name: "select_text".into(),
            description: "Select an exact unique live text match or place a caret in a snapshot-bound native element. Linux X11 AT-SPI Text only; no activation, content replacement, or keyboard fallback. Prefix and suffix disambiguate duplicate matches. A post-mutation error must not be replayed.".into(),
            input_schema: json!({"type":"object","required":["pid","text"],"properties":{
                "session":cua_driver_core::tool_schema::session_schema(),
                "pid":{"type":"integer","minimum":1},
                "window_id":{"type":"integer","minimum":1},
                "element_index":cua_driver_core::tool_schema::element_index_schema(),
                "element_token":cua_driver_core::tool_schema::element_token_schema(),
                "snapshot_id":cua_driver_core::tool_schema::snapshot_id_schema(),
                "text":{"type":"string","minLength":1},
                "prefix":{"type":"string"},"suffix":{"type":"string"},
                "selection_type":{"type":"string","enum":["text","cursor_before","cursor_after"],"default":"text"}
            },"additionalProperties":false}),
            read_only:false, destructive:true, idempotent:false, open_world:true,
        })
    }

    async fn invoke(&self, mut args: Value) -> ToolResult {
        // The registry has already admitted and namespaced trusted metadata.
        // This tool consumes only its public target parameters.
        cua_driver_core::tool_args::sanitize_reserved_args(&mut args);
        let Some(object) = args.as_object() else {
            return refused("select_text expects an object");
        };
        for key in object.keys() {
            if ![
                "session",
                "pid",
                "window_id",
                "element_index",
                "element_token",
                "snapshot_id",
                "text",
                "prefix",
                "suffix",
                "selection_type",
            ]
            .contains(&key.as_str())
            {
                return refused(format!("select_text does not support parameter {key}"));
            }
        }
        for key in [
            "session",
            "element_token",
            "snapshot_id",
            "prefix",
            "suffix",
            "selection_type",
        ] {
            if args.get(key).is_some_and(|value| !value.is_string()) {
                return refused(format!("{key} must be a string"));
            }
        }
        let pid = match args.require_u32("pid") {
            Ok(pid) if pid > 0 && pid <= i32::MAX as u32 => pid,
            _ => return refused("pid must be a positive signed 32-bit process id"),
        };
        let text = match args.require_str("text") {
            Ok(text) if !text.is_empty() => text,
            _ => return refused("text must be a nonempty string"),
        };
        let kind = match SelectionKind::parse(&args.str_or("selection_type", "text")) {
            Ok(kind) => kind,
            Err(error) => return refused(error),
        };
        let window = match args.get("window_id") {
            None => None,
            Some(value) => match value
                .as_u64()
                .and_then(|v| u32::try_from(v).ok())
                .filter(|v| *v > 0)
            {
                Some(value) => Some(value),
                None => return refused("window_id must be a positive X11 window id"),
            },
        };
        let index = match args.get("element_index") {
            None => None,
            Some(value) => match value.as_u64().and_then(|v| usize::try_from(v).ok()) {
                Some(value) => Some(value),
                None => return refused("element_index must be a nonnegative integer"),
            },
        };
        let token = args.opt_str("element_token");
        let snapshot = args.opt_str("snapshot_id");
        let (index, window, resolved_token) = match resolve_element_args(
            pid as i32,
            index,
            token.as_deref(),
            snapshot.as_deref(),
            window,
            "select_text",
        ) {
            Ok(ResolvedElement::Element {
                element_index,
                window_id: Some(window_id),
                element_token,
                ..
            }) => (element_index, window_id, element_token),
            Ok(_) => return refused("select_text requires an exact snapshot-bound element"),
            Err(error) => return error,
        };
        let observed_identity =
            match self
                .element_cache
                .observed_identity(pid, window as u64, index, &resolved_token)
            {
                Ok(identity) => identity,
                Err(error) => return refused(error),
            };
        let request = SelectionRequest {
            pid,
            window_id: window as u64,
            element_index: index,
            element_token: token,
            snapshot_id: snapshot,
            observed_identity,
            text,
            prefix: args.opt_str("prefix"),
            suffix: args.opt_str("suffix"),
            kind,
        };
        let runtime_scope = cua_driver_core::tool::current_dispatch_runtime_scope();
        match tokio::task::spawn_blocking(move || {
            match runtime_scope {
                Some(scope) => cua_driver_core::tool::with_runtime_scope(scope, || crate::atspi::select_text(&request)),
                None => crate::atspi::select_text(&request),
            }
        }).await {
            Ok(Ok(selection)) => ToolResult::text("Selected the requested live text range and verified read-back.")
                .with_structured(json!({"status":"completed","verified":true,"path":"atspi_text",
                    "range":{"start":selection.range.start,"end":selection.range.end,"unit":selection.unit.name()}})),
            Ok(Err(error)) if !error.mutation_submitted => refused(error.message),
            Ok(Err(error)) => ToolResult::error(error.message).with_structured(json!({
                "status":"partial","verified":false,"mutation_submitted":true,
                "code":"selection_unverified","retry_safe":false
            })),
            Err(error) => {
                // Worker failure cannot prove a pre-input refusal. The native
                // guard handles unwind after submission; this also covers an
                // unknown worker boundary before a normal result was returned.
                cua_driver_core::element_token::global().invalidate_pid_snapshots(pid as i32);
                ToolResult::error(format!("select_text worker failed; mutation outcome unknown: {error}. Observe; do not replay."))
                    .with_structured(json!({"status":"partial","verified":false,"code":"selection_unverified","retry_safe":false}))
            },
        }
    }
}
