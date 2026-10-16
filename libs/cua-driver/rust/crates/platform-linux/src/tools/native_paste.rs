//! One verified X11 clipboard paste; never a typing or AT-SPI PasteText fallback.
use async_trait::async_trait;
use cua_driver_core::{
    clipboard::{ClipboardBackend, CLIPBOARD_WRITE_LOCK},
    protocol::ToolResult,
    tool::{Tool, ToolDef},
};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
use x11rb::{connection::Connection as _, protocol::xproto::ConnectionExt as _};

pub(super) struct NativePasteTool {
    clipboard: Arc<dyn ClipboardBackend>,
}
impl NativePasteTool {
    pub(super) fn new(clipboard: Arc<dyn ClipboardBackend>) -> Self {
        Self { clipboard }
    }
}
static DEF: std::sync::OnceLock<ToolDef> = std::sync::OnceLock::new();

fn failure(code: &str, message: &str, dispatched: bool, clipboard_changed: bool) -> ToolResult {
    ToolResult::error(format!(
        "{code}: {message}; do not replay an uncertain paste"
    ))
    .with_structured(json!({"status":if dispatched {"unknown"} else {"refused"},
            "code":code,"effect":if dispatched {"unknown"} else {"none"},
            "input_dispatched":dispatched,"target_value_verified":false,"transfer_verified":false,
            "clipboard_policy":"leave","clipboard_may_have_changed":clipboard_changed,
            "clipboard_restored":false}))
}

struct Input {
    pid: u32,
    xid: u64,
    text: String,
}
fn input(args: &Value) -> Result<Input, ToolResult> {
    let pid = args
        .get("pid")
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n > 0);
    let xid = args
        .get("window_id")
        .and_then(Value::as_u64)
        .filter(|n| *n > 0 && *n <= u32::MAX as u64);
    let text = args.get("text").and_then(Value::as_str);
    let (Some(pid), Some(xid), Some(text)) = (pid, xid, text) else {
        return Err(failure(
            "invalid_params",
            "exact positive pid/window_id and text are required",
            false,
            false,
        ));
    };
    if text.is_empty() || text.len() > 16 * 1024 {
        return Err(failure(
            "invalid_params",
            "plaintext payload must contain1..16384 UTF-8 bytes",
            false,
            false,
        ));
    }
    if args
        .get("format")
        .is_some_and(|value| value.as_str() != Some("text"))
    {
        return Err(failure(
            "unsupported_native_paste_format",
            "X11 native paste supports plaintext only",
            false,
            false,
        ));
    }
    if args.get("clipboard_policy").and_then(Value::as_str) != Some("leave") {
        return Err(failure(
            "unsupported_clipboard_policy",
            "X11 native paste requires explicit clipboard_policy leave",
            false,
            false,
        ));
    }
    if args.get("delivery_mode").and_then(Value::as_str) != Some("foreground") {
        return Err(failure(
            "foreground_required",
            "X11 paste requires foreground delivery chosen before dispatch",
            false,
            false,
        ));
    }
    Ok(Input {
        pid,
        xid,
        text: text.into(),
    })
}

fn expected(value: &str, start: i32, end: i32, payload: &str) -> Option<String> {
    let (start, end) = (usize::try_from(start).ok()?, usize::try_from(end).ok()?);
    let count = value.chars().count();
    if end < start || end > count {
        return None;
    }
    let prefix: String = value.chars().take(start).collect();
    let suffix: String = value.chars().skip(end).collect();
    Some(prefix + payload + &suffix)
}
fn clipboard_owner() -> anyhow::Result<u32> {
    let (conn, _) = x11rb::connect(None)?;
    let atom = conn.intern_atom(false, b"CLIPBOARD")?.reply()?.atom;
    let owner = conn.get_selection_owner(atom)?.reply()?.owner;
    Ok(owner)
}

fn paste(input: Input, clipboard: &dyn ClipboardBackend) -> ToolResult {
    if crate::wayland::is_wayland() {
        return failure(
            "route_unavailable",
            "native clipboard paste is currently X11 only",
            false,
            false,
        );
    }
    if crate::x11::window_pid(input.xid) != Some(input.pid) {
        return failure(
            "native_target_unproven",
            "exact window does not belong to requested process",
            false,
            false,
        );
    }
    let mut dispatched = false;
    let mut clipboard_changed = false;
    let mut verified = false;
    let mut clipboard_status = "untouched";
    let mut code = "native_target_unproven";
    let mut message = String::new();
    let result = crate::input::with_x11_foreground_opts(
        input.xid,
        crate::input::ForegroundOptions::keyboard(),
        || {
            let before = crate::atspi::native::prepare_native_paste(input.pid, input.xid)?;
            let wanted = expected(&before.value, before.start, before.end, &input.text)
                .ok_or_else(|| anyhow::anyhow!("invalid native selection"))?;
            code = "clipboard_write_failed";
            clipboard_changed = true; // A backend error may follow a partial clipboard mutation.
            clipboard
                .write_text(input.text.clone())
                .map_err(anyhow::Error::msg)?;
            clipboard_status = "unknown";
            let owner = clipboard_owner()?;
            code = "clipboard_changed";
            if owner == 0
                || clipboard
                    .read_text()
                    .map_err(anyhow::Error::msg)?
                    .as_deref()
                    != Some(input.text.as_str())
            {
                anyhow::bail!("clipboard payload was not independently read back before input");
            }
            clipboard_status = "payload_left";
            code = "native_target_changed";
            let current = crate::atspi::native::read_native_paste_target(&before.object_ref)?;
            if current != before
                || !current.focused
                || crate::x11::active_window() != Some(input.xid)
                || crate::x11::focused_window_among(&[input.xid]) != Some(input.xid)
            {
                anyhow::bail!("exact focus, value or selection changed before paste");
            }
            code = "clipboard_changed";
            if clipboard_owner()? != owner {
                anyhow::bail!("new clipboard owner observed before input");
            }
            code = "native_paste_unverified";
            dispatched = true; // XTEST can fail after some events; never retry that chord.
            crate::input::send_key_xtest("v", &["ctrl"])?;
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                if let Ok(after) =
                    crate::atspi::native::read_native_paste_target(&before.object_ref)
                {
                    if after.value == wanted {
                        verified = true;
                        break;
                    }
                }
                if Instant::now() >= deadline {
                    anyhow::bail!("exact native value did not confirm the selected insertion");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            clipboard_status = match clipboard_owner() {
                Ok(now) if now == owner => "payload_left",
                Ok(_) => "newer_writer_observed",
                Err(_) => "unknown",
            };
            Ok(())
        },
    );
    match result {
        Ok(((),report)) if verified=>ToolResult::text("Native plaintext paste confirmed in the exact X11 editable; clipboard leave policy applies.")
            .with_structured(json!({"status":"completed","effect":"confirmed","pid":input.pid,
                "window_id":input.xid,"path":"x11_foreground_clipboard","input_dispatched":true,
                "target_value_verified":true,"transfer_verified":false,"clipboard_policy":"leave",
                "clipboard_status":clipboard_status,"clipboard_restored":false,"focus":report.to_json()})),
        Err(error)=>{
            message.push_str(&error.to_string());
            if !clipboard_changed && !dispatched {
                if let Some(foreground)=crate::input::foreground::error_code(&error) {code=foreground;}
            }
            failure(code,&message,dispatched,clipboard_changed)
        },
        _=>failure("native_paste_unverified","paste returned without a verified value",dispatched,clipboard_changed),
    }
}

#[async_trait]
impl Tool for NativePasteTool {
    fn def(&self) -> &ToolDef {
        DEF.get_or_init(||ToolDef {name:"native_paste".into(),
            description:"Paste plaintext into one exact X11 native editable with one foreground Ctrl+V. Requires exact PID/window, explicit foreground delivery and clipboard_policy leave. Proves focused AT-SPI identity, current value/selection and clipboard payload before input, then exact value after input. Leaves the payload clipboard; observed external writers are not overwritten. AT-SPI PasteText and replacement typing are never fallbacks. Rich formats and Wayland refuse before mutation. Unknown input must not be replayed; observe actual application state and use the host completion/drain contract after client timeout.".into(),
            input_schema:json!({"type":"object","required":["pid","window_id","text","clipboard_policy"],
                "properties":{"session":cua_driver_core::tool_schema::session_schema(),
                    "pid":{"type":"integer","minimum":1,"description":"Exact target process ID; must own the observed X11 window."},"window_id":{"type":"integer","minimum":1,"description":"Exact observed X11 window ID belonging to pid; no first-window fallback."},
                    "text":{"type":"string","description":"Plaintext clipboard payload,1..16384 UTF-8 bytes."},"format":{"type":"string","enum":["text"],"default":"text","description":"Plaintext only; HTML and Markdown refuse before mutation."},
                    "clipboard_policy":{"type":"string","enum":["leave"],"description":"Required leave policy: retain the payload clipboard; observed newer writers are not overwritten. Linux does not restore prior formats."},
                    "delivery_mode":cua_driver_core::tool_schema::delivery_mode_schema_with("Choose foreground before dispatch: activates and confirms this exact X11 window, sends one Ctrl+V and leaves the target active. Background/default refuses before clipboard mutation or input; no implicit escalation or replay.")},"additionalProperties":false}),
            read_only:false,destructive:true,idempotent:false,open_world:true})
    }
    async fn invoke(&self, args: Value) -> ToolResult {
        let input = match input(&args) {
            Ok(v) => v,
            Err(result) => return result,
        };
        let _lock = CLIPBOARD_WRITE_LOCK.lock().await;
        let clipboard = self.clipboard.clone();
        match tokio::task::spawn_blocking(move || paste(input, clipboard.as_ref())).await {
            Ok(result) => result,
            Err(_) => failure(
                "native_paste_worker_failed",
                "delivery and clipboard state are unknown",
                true,
                true,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_paste_validates_unicode_character_ranges() {
        assert_eq!(
            expected("éΩ中🙂tail", 1, 4, "café東京"),
            Some("écafé東京tail".into())
        );
        assert_eq!(expected("abc", 1, 1, "X"), Some("aXbc".into()));
        for (start, end) in [(-1, 0), (2, 1), (0, 4)] {
            assert!(expected("abc", start, end, "X").is_none());
        }
    }
    #[test]
    fn native_paste_admission_refuses_before_desktop_or_clipboard_access() {
        let base = json!({"pid":42,"window_id":7,"text":"éΩ中🙂","clipboard_policy":"leave","delivery_mode":"foreground"});
        assert!(input(&base).is_ok());
        for (key, value, code) in [
            ("delivery_mode", json!("background"), "foreground_required"),
            (
                "clipboard_policy",
                json!("restore"),
                "unsupported_clipboard_policy",
            ),
            ("format", json!("html"), "unsupported_native_paste_format"),
            ("pid", json!(0), "invalid_params"),
            ("window_id", json!(4294967296u64), "invalid_params"),
            ("text", json!(""), "invalid_params"),
            ("text", json!("x".repeat(16385)), "invalid_params"),
        ] {
            let mut args = base.clone();
            args[key] = value;
            let result = input(&args).err().expect("must refuse");
            let s = result.structured_content.unwrap();
            assert_eq!(s["code"], code);
            assert_eq!(s["input_dispatched"], false);
            assert_eq!(s["clipboard_may_have_changed"], false);
        }
        let mut missing = base;
        missing.as_object_mut().unwrap().remove("delivery_mode");
        assert_eq!(
            input(&missing).err().unwrap().structured_content.unwrap()["code"],
            "foreground_required"
        );
    }
}
