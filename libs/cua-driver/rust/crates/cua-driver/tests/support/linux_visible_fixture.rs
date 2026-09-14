//! Ordinary X11 window setup for fixture controls that begin below the viewport.

use std::time::{Duration, Instant};

use cua_driver_testkit::e2e::DisplayServer;
use cua_driver_testkit::{Driver, McpDriver};

pub fn prepare_visible_controls(
    driver: &mut McpDriver,
    pid: u32,
    window_id: u64,
    required: &[&str],
) {
    // Native Wayland does not expose the public geometry mutation used here.
    // Preserve those lanes' existing setup rather than falling back to input.
    if DisplayServer::current() != DisplayServer::X11 || required.is_empty() {
        return;
    }
    let first = driver.call(
        "get_window_state",
        serde_json::json!({"pid": pid, "window_id": window_id}),
    );
    assert!(
        !first.is_error(),
        "visible fixture setup read failed: {}",
        first.text()
    );
    if required
        .iter()
        .all(|marker| first.tree_text().contains(*marker))
    {
        return;
    }

    let screen = driver.call("get_screen_size", serde_json::json!({}));
    assert!(
        !screen.is_error(),
        "fixture screen size unavailable: {}",
        screen.text()
    );
    let screen_width = screen.structured()["width"].as_f64().expect("screen width");
    let screen_height = screen.structured()["height"]
        .as_f64()
        .expect("screen height");
    assert!(
        screen_width.is_finite()
            && screen_height.is_finite()
            && screen_width > 40.0
            && screen_height > 80.0,
        "fixture desktop has unusable dimensions: {}",
        screen.text()
    );
    let (x, y) = (20.0, 20.0);
    let width = 800.0_f64.min(screen_width - 40.0);
    let height = screen_height - 80.0;
    let resized = driver.call(
        "set_window_frame",
        serde_json::json!({"pid": pid, "window_id": window_id,
            "x": x, "y": y, "width": width, "height": height}),
    );
    assert!(
        !resized.is_error(),
        "visible fixture resize failed: {}",
        resized.text()
    );
    // A non-error action can still report an unconfirmed geometry change.
    // Read independently; never replay the mutation while waiting for the WM.
    let geometry_deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let listed = driver.call("list_windows", serde_json::json!({"pid": pid}));
        assert!(
            !listed.is_error(),
            "fixture geometry read failed: {}",
            listed.text()
        );
        let actual = listed.structured()["windows"]
            .as_array()
            .and_then(|windows| {
                windows.iter().find(|window| {
                    window["window_id"].as_u64() == Some(window_id)
                        && window["pid"].as_u64() == Some(u64::from(pid))
                })
            });
        let matched = actual.is_some_and(|window| {
            [("x", x), ("y", y), ("width", width), ("height", height)]
                .iter()
                .all(|(key, expected)| {
                    window[*key]
                        .as_f64()
                        .is_some_and(|value| value.is_finite() && (value - *expected).abs() <= 2.0)
                })
        });
        if matched {
            break;
        }
        assert!(Instant::now() < geometry_deadline,
            "visible fixture geometry did not match ({x},{y},{width},{height}); action={}; observed={actual:?}",
            resized.text());
        std::thread::sleep(Duration::from_millis(100));
    }

    let visibility_deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let state = driver.call(
            "get_window_state",
            serde_json::json!({"pid": pid, "window_id": window_id}),
        );
        assert!(
            !state.is_error(),
            "visible fixture setup read failed: {}",
            state.text()
        );
        let missing: Vec<_> = required
            .iter()
            .filter(|marker| !state.tree_text().contains(**marker))
            .collect();
        if missing.is_empty() {
            break;
        }
        assert!(
            Instant::now() < visibility_deadline,
            "fixture controls/oracles remain hidden after verified resize: {missing:?}; tree:\n{}",
            state.tree_text()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}
