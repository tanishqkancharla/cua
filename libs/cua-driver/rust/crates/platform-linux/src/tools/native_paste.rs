//! Exact-window native paste; transaction mechanics live in the X11 backend.
use async_trait::async_trait;
use cua_driver_core::{
    protocol::ToolResult,
    tool::{Tool, ToolDef},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::OnceLock;

pub(super) struct NativePasteTool;
static DEF: OnceLock<ToolDef> = OnceLock::new();

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    #[serde(rename = "session")]
    _session: Option<String>,
    pid: u32,
    window_id: u32,
    text: String,
    #[serde(default = "text_format")]
    format: String,
}
fn text_format() -> String {
    "text".into()
}

fn refused(message: impl Into<String>) -> ToolResult {
    ToolResult::error(message.into()).with_structured(json!({
        "status":"refused", "input_submitted":false,
        "clipboard_state":"unchanged", "retry_safe":true,
        "code":"native_paste_unavailable"
    }))
}

#[async_trait]
impl Tool for NativePasteTool {
    fn def(&self) -> &ToolDef {
        DEF.get_or_init(|| ToolDef {
            name:"native_paste".into(),
            description:"Paste plain multiline text once into the exact focused X11 native window. Preserve supported prior clipboard formats and restore only if no later owner took over. Transfer confirmation is not saved-document confirmation. Other native platforms and rich formats are unsupported. Never replay uncertain delivery.".into(),
            input_schema:json!({"type":"object","required":["pid","window_id","text"],"properties":{
                "session":cua_driver_core::tool_schema::session_schema(),
                "pid":{"type":"integer","minimum":1,"maximum":2147483647},
                "window_id":{"type":"integer","minimum":1,"maximum":4294967295u64},
                "text":{"type":"string"},"format":{"type":"string","enum":["text"],"default":"text"}
            },"additionalProperties":false}),
            read_only:false, destructive:true, idempotent:false, open_world:true,
        })
    }

    async fn invoke(&self, mut args: Value) -> ToolResult {
        cua_driver_core::tool_args::sanitize_reserved_args(&mut args);
        let input = match serde_json::from_value::<Input>(args) {
            Ok(input) => input,
            Err(error) => return refused(format!("Invalid native_paste parameters: {error}")),
        };
        if input.pid == 0 || input.pid > i32::MAX as u32 || input.window_id == 0 {
            return refused("native_paste requires a positive PID and exact window ID");
        }
        if input.format != "text" {
            return refused("native_paste currently supports plain text only");
        }
        let pid = input.pid;
        let request = crate::native_paste::PasteRequest {
            pid,
            window_id: input.window_id,
            text: input.text,
            format: input.format,
        };
        // Serialize with existing driver clipboard writers, never recursively
        // invoke clipboard_write inside this transaction.
        let _clipboard_writer = cua_driver_core::clipboard::CLIPBOARD_WRITE_LOCK
            .lock()
            .await;
        let scope = cua_driver_core::tool::current_dispatch_runtime_scope();
        let result = tokio::task::spawn_blocking(move || {
            let run = || {
                let result = crate::native_paste::paste(request);
                if result
                    .as_ref()
                    .map(|_| true)
                    .unwrap_or_else(|error| error.input_submitted)
                {
                    cua_driver_core::element_token::global().invalidate_pid_snapshots(pid as i32);
                }
                result
            };
            match scope {
                Some(scope) => cua_driver_core::tool::with_runtime_scope(scope, run),
                None => run(),
            }
        })
        .await;
        if result.is_err() {
            // A worker panic can follow input submission. The caller still
            // has its dispatch scope, so invalidate that runtime's snapshots.
            cua_driver_core::element_token::global().invalidate_pid_snapshots(pid as i32);
        }
        match result {
            Ok(Ok(outcome)) if outcome.transfer_verified => ToolResult::text("Native text transfer completed.")
                .with_structured(json!({"status":"completed","input_submitted":true,
                    "transfer_verified":true,"clipboard_restoration":outcome.clipboard_restoration,
                    "retry_safe":false})),
            Ok(Ok(outcome)) => ToolResult::error("Paste transfer is unverified; do not replay.")
                .with_structured(json!({"status":"uncertain","input_submitted":true,
                    "transfer_verified":false,"clipboard_restoration":outcome.clipboard_restoration,"retry_safe":false})),
            Ok(Err(error)) => ToolResult::error(error.message).with_structured(json!({
                "status":if error.input_submitted {"uncertain"} else {"refused"},
                "code":"native_paste_unavailable","input_submitted":error.input_submitted,
                "clipboard_state":error.clipboard_state,"retry_safe":false
            })),
            Err(error) => ToolResult::error(format!("Native paste worker failed; delivery is unknown and must not be replayed: {error}"))
                .with_structured(json!({"status":"uncertain","input_submitted":true,
                    "clipboard_state":"unknown","retry_safe":false})),
        }
    }
}
