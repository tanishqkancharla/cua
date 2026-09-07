//! Real Chromium paste. Clipboard writes are serialized with the driver's other
//! writers; no clipboard restoration can overwrite a later user copy.
use super::cdp_ws::CdpConnection;
use crate::clipboard::{ClipboardBackend, CLIPBOARD_WRITE_LOCK};
use serde_json::json;
use std::sync::Arc;

const READY: &str = "function() { \
    if (!this.isConnected || this.ownerDocument !== document) return false; \
    const root=this.getRootNode(); \
    if (root.activeElement !== this) return false; \
    return this.isContentEditable || ((!this.disabled && !this.readOnly) && \
        (this.tagName==='TEXTAREA' || (this.tagName==='INPUT' && \
        !['button','checkbox','color','file','hidden','image','radio','range','reset','submit'].includes(this.type)))); \
}";

async fn ready(conn: &CdpConnection, cdp: &str, object: &str) -> Result<(), String> {
    let result = conn
        .call(
            Some(cdp),
            "Runtime.callFunctionOn",
            json!({
                "objectId":object,"functionDeclaration":READY,"returnByValue":true
            }),
        )
        .await
        .map_err(|e| e.to_string())?;
    if result["result"]["value"].as_bool() == Some(true) && result.get("exceptionDetails").is_none()
    {
        Ok(())
    } else {
        Err(
            "Paste target is no longer the connected, focused editor; no paste was dispatched."
                .into(),
        )
    }
}

pub(super) async fn deliver(
    conn: &CdpConnection,
    cdp: &str,
    object: &str,
    clipboard: Arc<dyn ClipboardBackend>,
    text: &str,
    format: &str,
) -> Result<(), String> {
    let _writer = CLIPBOARD_WRITE_LOCK.lock().await;
    ready(conn, cdp, object).await?;
    let (plain, html) = if format == "html" {
        // Template content is inert: this derives the text/plain fallback
        // without inserting nodes or executing the supplied HTML in the page.
        let result=conn.call(Some(cdp), "Runtime.callFunctionOn",json!({
            "objectId":object,"functionDeclaration":"function(html) { const t=this.ownerDocument.createElement('template'); t.innerHTML=html; return t.content.textContent || ''; }",
            "arguments":[{"value":text}],"returnByValue":true
        })).await.map_err(|e|e.to_string())?;
        let plain = result["result"]["value"]
            .as_str()
            .filter(|_| result.get("exceptionDetails").is_none())
            .ok_or("Could not prepare HTML paste formats; clipboard was not touched.")?;
        (plain.to_owned(), Some(text.to_owned()))
    } else {
        (text.to_owned(), None)
    };
    let expected = plain.clone();
    let writer = clipboard.clone();
    tokio::task::spawn_blocking(move || writer.write_paste(plain, html))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Clipboard preparation failed; paste was not dispatched: {e}"))?;
    ready(conn, cdp, object).await?;
    let reader = clipboard.clone();
    let current = tokio::task::spawn_blocking(move || reader.read_text())
        .await
        .map_err(|e| e.to_string())??;
    if current.as_deref() != Some(expected.as_str()) {
        return Err("Clipboard changed before paste; no paste was dispatched and the newer content was preserved.".into());
    }
    // Chromium's editing command invokes its actual paste path (including
    // trusted paste/beforeinput/input events and normal editor cancellation).
    conn.call(Some(cdp),"Input.dispatchKeyEvent",json!({
        "type":"rawKeyDown","key":"v","code":"KeyV","commands":["paste"]
    })).await.map_err(|e|format!("Paste delivery is unknown: {e}. Observe before deciding what to do next; do not replay automatically."))?;
    conn.call(
        Some(cdp),
        "Input.dispatchKeyEvent",
        json!({"type":"keyUp","key":"v","code":"KeyV"}),
    )
    .await
    .map_err(|e| {
        format!("Paste was dispatched but key release failed: {e}. Do not replay automatically.")
    })?;
    Ok(())
}
