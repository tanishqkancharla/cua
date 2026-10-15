//! `cua-driver permissions` reports usage errors with the documented exit code.

use std::process::{Command, Stdio};

#[test]
fn permissions_unknown_subcommand_is_a_usage_error() {
    let home = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cua-driver"))
        .args(["permissions", "bogus"])
        .stdin(Stdio::null())
        .env("HOME", home.path())
        .env("CUA_DRIVER_RS_HOME", home.path())
        .env("CUA_DRIVER_RS_TELEMETRY_ENABLED", "0")
        .output()
        .expect("run cua-driver permissions with an unknown subcommand");
    assert_eq!(output.status.code(), Some(64));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown permissions subcommand 'bogus'"),
        "stderr={stderr}"
    );
    assert!(stderr.contains("Valid: status, grant."), "stderr={stderr}");
}

/// Keep the selected endpoint at the public executable boundary. The daemon
/// uses a temporary home and standard authorization; no GUI input is sent.
#[test]
fn permission_status_honors_its_endpoint_and_platform_schema() {
    use cua_driver_testkit::{spawn_in_job, CliDriver};
    use std::path::Path;
    use std::time::{Duration, Instant};

    fn run(binary: &Path, home: &Path, args: &[&str]) -> std::process::Output {
        let mut child = spawn_in_job(
            Command::new(binary)
                .args(args)
                .env("HOME", home)
                .env("CUA_DRIVER_RS_HOME", home)
                .env("CUA_DRIVER_RS_TELEMETRY_ENABLED", "false")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped()),
        )
        .expect("start permission status client");
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if child.try_wait().unwrap().is_some() {
                return child.wait_with_output().unwrap();
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("permission status client exceeded its outer deadline");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    let artifact_home = tempfile::tempdir().unwrap();
    #[cfg(target_os = "macos")]
    let binary = {
        // Run the current Cargo artifact outside a protected Documents path.
        let copied = artifact_home.path().join("source-built-opensky-driver");
        std::fs::copy(env!("CARGO_BIN_EXE_cua-driver"), &copied).unwrap();
        copied
    };
    #[cfg(not(target_os = "macos"))]
    let binary = std::path::PathBuf::from(env!("CARGO_BIN_EXE_cua-driver"));
    let driver = CliDriver::with_binary(&binary);
    assert!(driver.available(), "current source daemon must start");
    let socket = driver.daemon_socket().unwrap();
    let home = driver.state_root().unwrap();
    let output = run(
        &binary,
        home,
        &["permissions", "status", "--json", "--socket", socket],
    );
    assert!(output.status.success(), "{output:?}");
    let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    #[cfg(target_os = "linux")]
    {
        assert!(status["x11"].is_boolean(), "{status}");
        assert!(status["atspi"].is_boolean(), "{status}");
        assert!(status["xsend_event"].is_boolean(), "{status}");
        assert!(status.get("accessibility").is_none(), "{status}");
    }
    #[cfg(target_os = "windows")]
    {
        assert_eq!(status["uia"], true, "{status}");
        assert_eq!(status["post_message"], true, "{status}");
        assert!(status["elevated"].is_boolean(), "{status}");
        assert!(status.get("accessibility").is_none(), "{status}");
    }
    #[cfg(target_os = "macos")]
    {
        if status.get("accessibility").is_some() {
            assert_eq!(status["source"]["attribution"], "driver-daemon", "{status}");
            assert!(status["source"]["pid"].as_u64().is_some_and(|pid| pid > 0));
        } else {
            assert_eq!(status["daemon_running"], true, "{status}");
            assert_eq!(status["status"], "unknown", "{status}");
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let text = run(
            &binary,
            home,
            &["permissions", "status", "--socket", socket],
        );
        assert!(text.status.success(), "{text:?}");
        let text = String::from_utf8(text.stdout).unwrap();
        assert!(!text.contains("Accessibility:"), "{text}");
        assert!(!text.contains("Screen Recording:"), "{text}");
        #[cfg(target_os = "linux")]
        assert!(text.contains("X11"), "{text}");
        #[cfg(target_os = "windows")]
        assert!(text.contains("Process integrity level:"), "{text}");
    }
    let absent = format!("{socket}.missing");
    let output = run(
        &binary,
        home,
        &["permissions", "status", "--socket", &absent, "--json"],
    );
    assert!(output.status.success(), "{output:?}");
    let absent_status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(absent_status["daemon_running"], false, "{absent_status}");
    assert_eq!(absent_status["status"], "unknown", "{absent_status}");
    assert!(
        absent_status.get("accessibility").is_none(),
        "{absent_status}"
    );
    assert!(absent_status.get("uia").is_none(), "{absent_status}");

    let grant = run(&binary, home, &["permissions", "grant", "--socket", socket]);
    assert_eq!(grant.status.code(), Some(64), "{grant:?}");
    assert!(String::from_utf8_lossy(&grant.stderr)
        .contains("permissions grant does not accept --socket"));
    drop(driver);
    drop(artifact_home);
}
