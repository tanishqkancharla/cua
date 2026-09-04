//! Cooperative, exact-window close for macOS.
//!
//! This deliberately has one route: map the requested `(pid, CGWindowID)` to
//! one fresh `AXWindow`, resolve its `AXCloseButton`, and perform `AXPress`.
//! It never activates the application and never falls back to menu labels,
//! keyboard shortcuts, coordinates, or process termination.

use async_trait::async_trait;
use core_foundation::base::{CFRelease, CFTypeRef};
use cua_driver_contract::{CloseWindowInput, CloseWindowStatus};
use cua_driver_core::{
    protocol::ToolResult,
    tool::{Tool, ToolDef},
    tool_args::parse_typed_input,
};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

use crate::{
    ax::{
        bindings::{
            ax_get_window_id, copy_action_names, copy_ax_windows, copy_bool_attr, copy_children,
            copy_element_attr, copy_string_attr, kAXErrorAPIDisabled, kAXErrorSuccess,
            perform_action, AXIsProcessTrusted, AXUIElementCreateApplication,
            AXUIElementSetMessagingTimeout,
        },
        exact_target::element_window_id,
    },
    windows::{resolve_window_owner, WindowOwner},
};

pub struct CloseWindowTool;

static DEF: std::sync::OnceLock<ToolDef> = std::sync::OnceLock::new();

fn def() -> &'static ToolDef {
    DEF.get_or_init(|| {
        let contract =
            cua_driver_contract::tool_contract("close_window").expect("close_window contract");
        ToolDef::from_contract(&contract)
    })
}

const AX_TIMEOUT_SECONDS: f32 = 2.0;
const CONFIRM_TIMEOUT: Duration = Duration::from_secs(2);
const CONFIRM_POLL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefusalStatus {
    Missing,
    OwnerMismatch,
    PermissionRequired,
    Unavailable,
    Ambiguous,
    Disabled,
    DeliveryFailed,
    ConfirmationRequired,
    Noop,
}

impl RefusalStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::OwnerMismatch => "owner_mismatch",
            Self::PermissionRequired => "permission_required",
            Self::Unavailable => "unavailable",
            Self::Ambiguous => "ambiguous",
            Self::Disabled => "disabled",
            Self::DeliveryFailed => "delivery_failed",
            Self::ConfirmationRequired => "confirmation_required",
            Self::Noop => "noop",
        }
    }
}

#[derive(Debug)]
struct CloseRefusal {
    status: RefusalStatus,
    code: &'static str,
    message: String,
}

fn refusal(status: RefusalStatus, code: &'static str, message: impl Into<String>) -> CloseRefusal {
    CloseRefusal {
        status,
        code,
        message: message.into(),
    }
}

fn refusal_result(pid: u32, window_id: u64, refusal: CloseRefusal) -> ToolResult {
    ToolResult::error(format!("close_window: {}", refusal.message)).with_structured(json!({
        "status": refusal.status.as_str(),
        "effect": "refused",
        "code": refusal.code,
        "pid": pid,
        "window_id": window_id,
        "message": refusal.message,
    }))
}

fn validate_window_server_owner(pid: i32, window_id: u32) -> Result<(), CloseRefusal> {
    match resolve_window_owner(pid, window_id) {
        WindowOwner::SamePid => Ok(()),
        WindowOwner::Unknown => Err(refusal(
            RefusalStatus::Missing,
            "window_not_found",
            format!("window_id {window_id} is closed, stale, or unknown to WindowServer"),
        )),
        WindowOwner::ForeignPid {
            owner_pid,
            owner_app_name,
        } => Err(refusal(
            RefusalStatus::OwnerMismatch,
            "window_owner_mismatch",
            format!(
                "window_id {window_id} belongs to pid {owner_pid} ({owner_app_name}), not pid {pid}"
            ),
        )),
    }
}

/// Resolve and press the close button from a fresh AXWindows snapshot.
///
/// The WindowServer owner is checked immediately before this call and the
/// close button's own ancestry is checked before `AXPress`, so a stale or
/// cross-window AX object never becomes a wildcard.
fn press_exact_close_button(pid: i32, window_id: u32) -> Result<(), CloseRefusal> {
    // SAFETY: every retained AX reference created/copied below is released on
    // every path before returning. AX calls are confined to this blocking
    // function and have bounded messaging timeouts.
    unsafe {
        if !AXIsProcessTrusted() {
            return Err(refusal(
                RefusalStatus::PermissionRequired,
                "accessibility_permission_required",
                "macOS Accessibility permission is not granted",
            ));
        }

        let app = AXUIElementCreateApplication(pid);
        if app.is_null() {
            return Err(refusal(
                RefusalStatus::Unavailable,
                "accessibility_application_unavailable",
                format!("could not create an accessibility element for pid {pid}"),
            ));
        }
        AXUIElementSetMessagingTimeout(app, AX_TIMEOUT_SECONDS);
        crate::ax::enablement::ensure_chromium_ax_enabled(pid, app);

        let windows = copy_ax_windows(app);
        let matches: Vec<_> = windows
            .iter()
            .copied()
            .filter(|window| ax_get_window_id(*window) == Some(window_id))
            .collect();

        let result = match matches.as_slice() {
            [] => Err(refusal(
                RefusalStatus::Unavailable,
                "accessibility_window_unmapped",
                format!(
                    "WindowServer confirms window_id {window_id} for pid {pid}, but no matching AXWindow is available"
                ),
            )),
            [target] => {
                AXUIElementSetMessagingTimeout(*target, AX_TIMEOUT_SECONDS);
                match copy_element_attr(*target, "AXCloseButton") {
                    None => Err(refusal(
                        RefusalStatus::Disabled,
                        "window_not_closable",
                        format!("window_id {window_id} exposes no AXCloseButton"),
                    )),
                    Some(button) => {
                        AXUIElementSetMessagingTimeout(button, AX_TIMEOUT_SECONDS);
                        let result = if element_window_id(button) != Some(window_id) {
                            Err(refusal(
                                RefusalStatus::Unavailable,
                                "close_button_target_unproven",
                                "the AXCloseButton could not be proven to belong to the requested window",
                            ))
                        } else if copy_bool_attr(button, "AXEnabled") == Some(false) {
                            Err(refusal(
                                RefusalStatus::Disabled,
                                "close_button_disabled",
                                format!("window_id {window_id}'s AXCloseButton is disabled"),
                            ))
                        } else if !copy_action_names(button).iter().any(|action| action == "AXPress")
                        {
                            Err(refusal(
                                RefusalStatus::Disabled,
                                "close_action_unavailable",
                                format!("window_id {window_id}'s AXCloseButton does not advertise AXPress"),
                            ))
                        } else {
                            // Final exact identity check at the mutation boundary.
                            validate_window_server_owner(pid, window_id).and_then(|()| {
                                if ax_get_window_id(*target) != Some(window_id)
                                    || element_window_id(button) != Some(window_id)
                                {
                                    return Err(refusal(
                                        RefusalStatus::Unavailable,
                                        "window_identity_changed",
                                        "the exact AX window identity changed before AXPress",
                                    ));
                                }
                                let error = perform_action(button, "AXPress");
                                if error == kAXErrorSuccess {
                                    Ok(())
                                } else if error == kAXErrorAPIDisabled {
                                    Err(refusal(
                                        RefusalStatus::PermissionRequired,
                                        "accessibility_permission_required",
                                        "macOS disabled the Accessibility API at AXPress",
                                    ))
                                } else {
                                    Err(refusal(
                                        RefusalStatus::DeliveryFailed,
                                        "close_action_failed",
                                        format!("AXPress on AXCloseButton failed with AXError {error}"),
                                    ))
                                }
                            })
                        };
                        CFRelease(button as CFTypeRef);
                        result
                    }
                }
            }
            _ => Err(refusal(
                RefusalStatus::Ambiguous,
                "ambiguous_accessibility_window",
                format!(
                    "more than one AXWindow mapped to pid {pid}, window_id {window_id}; refusing to choose"
                ),
            )),
        };

        for window in windows {
            CFRelease(window as CFTypeRef);
        }
        CFRelease(app as CFTypeRef);
        result
    }
}

/// Best-effort diagnostic after an accepted AXPress leaves the original
/// WindowServer identity present. This does not press or dismiss anything.
fn exact_window_has_confirmation_surface(pid: i32, window_id: u32) -> bool {
    unsafe {
        let app = AXUIElementCreateApplication(pid);
        if app.is_null() {
            return false;
        }
        AXUIElementSetMessagingTimeout(app, AX_TIMEOUT_SECONDS);
        let windows = copy_ax_windows(app);
        let target = windows
            .iter()
            .copied()
            .find(|window| ax_get_window_id(*window) == Some(window_id));
        let found = target.is_some_and(|target| {
            if copy_bool_attr(target, "AXModal") == Some(true) {
                return true;
            }
            let children = copy_children(target);
            let found = children.iter().copied().any(|child| {
                matches!(
                    copy_string_attr(child, "AXRole").as_deref(),
                    Some("AXSheet") | Some("AXDialog")
                ) || copy_bool_attr(child, "AXModal") == Some(true)
            });
            for child in children {
                CFRelease(child as CFTypeRef);
            }
            found
        });
        for window in windows {
            CFRelease(window as CFTypeRef);
        }
        CFRelease(app as CFTypeRef);
        found
    }
}

fn close_and_verify(input: CloseWindowInput) -> Result<Value, CloseRefusal> {
    let pid = i32::try_from(input.pid).map_err(|_| {
        refusal(
            RefusalStatus::Unavailable,
            "invalid_pid",
            format!("pid {} is out of range on macOS", input.pid),
        )
    })?;
    let window_id = u32::try_from(input.window_id).map_err(|_| {
        refusal(
            RefusalStatus::Unavailable,
            "invalid_window_id",
            format!("window_id {} is out of range on macOS", input.window_id),
        )
    })?;

    // Initial check gives a precise missing/owner-mismatch error. The helper
    // repeats it immediately before AXPress to close the TOCTOU window.
    validate_window_server_owner(pid, window_id)?;
    press_exact_close_button(pid, window_id)?;

    let deadline = Instant::now() + CONFIRM_TIMEOUT;
    loop {
        match resolve_window_owner(pid, window_id) {
            WindowOwner::Unknown => {
                return Ok(json!({
                    "status": CloseWindowStatus::Closed,
                    "pid": input.pid,
                    "window_id": input.window_id,
                }))
            }
            WindowOwner::ForeignPid { .. } => {
                return Err(refusal(
                    RefusalStatus::OwnerMismatch,
                    "window_identity_reused",
                    "the original WindowServer id disappeared and was reused by another process before verification completed",
                ))
            }
            WindowOwner::SamePid if Instant::now() < deadline => {
                std::thread::sleep(CONFIRM_POLL);
            }
            WindowOwner::SamePid => break,
        }
    }

    if exact_window_has_confirmation_surface(pid, window_id) {
        Err(refusal(
            RefusalStatus::ConfirmationRequired,
            "close_confirmation_required",
            "AXPress was accepted, but the original window remains and exposes a confirmation surface; resolve it explicitly before retrying",
        ))
    } else {
        Err(refusal(
            RefusalStatus::Noop,
            "close_unconfirmed",
            "AXPress was accepted, but the original WindowServer window_id did not disappear before the verification deadline",
        ))
    }
}

#[async_trait]
impl Tool for CloseWindowTool {
    fn def(&self) -> &ToolDef {
        def()
    }

    async fn invoke(&self, args: Value) -> ToolResult {
        let input: CloseWindowInput = match parse_typed_input("close_window", args) {
            Ok(input) => input,
            Err(error) => return error,
        };
        let pid = input.pid;
        let window_id = input.window_id;
        match tokio::task::spawn_blocking(move || close_and_verify(input)).await {
            Ok(Ok(output)) => ToolResult::text(format!(
                "Closed window_id {window_id} for pid {pid}; WindowServer confirmed the original id disappeared."
            ))
            .with_structured(output),
            Ok(Err(error)) => refusal_result(pid, window_id, error),
            Err(error) => refusal_result(
                pid,
                window_id,
                refusal(
                    RefusalStatus::DeliveryFailed,
                    "close_worker_failed",
                    format!("blocking close worker failed: {error}"),
                ),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusal_statuses_are_stable_machine_values() {
        assert_eq!(RefusalStatus::Missing.as_str(), "missing");
        assert_eq!(RefusalStatus::Disabled.as_str(), "disabled");
        assert_eq!(RefusalStatus::Ambiguous.as_str(), "ambiguous");
        assert_eq!(RefusalStatus::Noop.as_str(), "noop");
        assert_eq!(
            RefusalStatus::ConfirmationRequired.as_str(),
            "confirmation_required"
        );
    }

    #[test]
    fn refusal_payload_keeps_exact_target_and_recovery_code() {
        let result = refusal_result(
            42,
            7,
            refusal(RefusalStatus::Disabled, "close_button_disabled", "disabled"),
        );
        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.expect("structured refusal");
        assert_eq!(structured["status"], "disabled");
        assert_eq!(structured["code"], "close_button_disabled");
        assert_eq!(structured["pid"], 42);
        assert_eq!(structured["window_id"], 7);
    }
}
