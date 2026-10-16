//! Read-only Activity Monitor viewport owner on an externally owned cold app.
//! The caller supplies exact PID/window and owns lifecycle/cleanup. No app is
//! launched or closed here, and no keyboard, pointer or clipboard input is sent.
#![cfg(target_os = "macos")]
use cua_driver_testkit::{Driver, McpDriver};
use serde_json::json;

#[test]
#[ignore]
fn native_process_viewport_keeps_toolbar_and_explicit_all_recovers_unread_rows() {
    let pid: u32 = std::env::var("CUA_E2E_ACTIVITY_MONITOR_PID")
        .expect("owned Activity Monitor PID required")
        .parse()
        .unwrap();
    let window: u64 = std::env::var("CUA_E2E_ACTIVITY_MONITOR_WINDOW")
        .expect("owned Activity Monitor window required")
        .parse()
        .unwrap();
    let mut driver = McpDriver::spawn_macos_daemon_proxy_named("macos-native-row-viewport")
        .expect("installed signed daemon proxy required");
    let windows = driver.call("list_windows", json!({"pid":pid}));
    assert!(!windows.is_error(), "{}", windows.text());
    assert!(
        windows.structured()["windows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["window_id"].as_u64() == Some(window)
                && w["pid"].as_u64() == Some(u64::from(pid))
                && w["app_name"] == "Activity Monitor"),
        "supplied exact owned Activity Monitor window absent"
    );
    let read = |driver: &mut McpDriver, scope, cap| {
        let response = driver.call("get_window_state",json!({"pid":pid,"window_id":window,"native_row_scope":scope,"full_output":true,"max_elements":cap,"timeout_ms":5000,"include_screenshot":false}));
        assert!(!response.is_error(), "{}", response.text());
        response.structured().clone()
    };
    let visible = read(&mut driver, "visible", 2000);
    assert_eq!(
        visible["truncated"], false,
        "viewport must reach controls under the normal node budget"
    );
    assert!(
        visible["collapsed_rows"].as_u64().unwrap_or(0) > 0,
        "requires a real offscreen process list"
    );
    assert_eq!(
        visible["elements_complete"], false,
        "intentional unread rows never promise complete absence"
    );
    let elements = visible["elements"].as_array().unwrap();
    assert!(elements.iter().any(|e| e["role"] == "AXToolbar"));
    assert!(elements
        .iter()
        .any(|e| e["role"] == "AXRadioButton" && e["label"] == "CPU"));
    let search_groups: Vec<_> = elements
        .iter()
        .filter(|e| {
            e["role"] == "AXGroup"
                && e["help"]
                    .as_str()
                    .is_some_and(|v| v.contains("Show only processes"))
        })
        .filter_map(|e| e["element_index"].as_u64())
        .collect();
    assert_eq!(search_groups.len(), 1, "real process search group missing");
    assert!(
        elements
            .iter()
            .any(|e| e["role"] == "AXTextField"
                && e["parent_index"].as_u64() == Some(search_groups[0])),
        "actual search control missing"
    );
    let all = read(&mut driver, "all", 10000);
    assert_eq!(
        all["truncated"], false,
        "explicit bounded all read must finish on this prepared desktop"
    );
    assert_eq!(all.get("collapsed_rows"), None);
    assert!(
        all["elements"].as_array().unwrap().len() > elements.len() * 3,
        "full path must recover actual unread rows"
    );
    // Driver refusal is before native ownership/walking. Exact message, not any failure.
    let refused=driver.call("get_window_state",json!({"pid":pid,"window_id":window,"native_row_scope":"visible","full_output":false,"include_screenshot":false}));
    assert!(refused.is_error());
    assert!(refused
        .text()
        .contains("native_row_scope requires full_output:true"));
    // Retain only aggregate evidence here; raw private process rows stay in the recording.
    let result = json!({"pid":pid,"window":window,"visibleElements":elements.len(),"collapsedRows":visible["collapsed_rows"],"allElements":all["elements"].as_array().unwrap().len(),"viewportHasToolbarAndCpuAndSearch":true,"noInputSent":true});
    if let Ok(path) = std::env::var("CUA_E2E_NATIVE_ROW_RECEIPT") {
        std::fs::write(path, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    }
    println!("{}", result);
}
