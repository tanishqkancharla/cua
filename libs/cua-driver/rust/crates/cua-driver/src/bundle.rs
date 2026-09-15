//! OpenSky Driver identity, shared by every build and transport.
//! Crate names stay compatible with upstream; product identity does not depend
//! on the executable filename (including direct Cargo builds).

/// All builds of this fork are source-managed. Never invoke upstream updates.
pub fn is_local_installation() -> bool {
    true
}
pub fn cli_name() -> &'static str {
    "opensky-driver"
}
pub fn state_namespace() -> &'static str {
    "opensky-driver"
}
pub fn user_home_subdirectory() -> &'static str {
    ".opensky-driver"
}
pub fn app_name() -> &'static str {
    "OpenSkyDriver"
}
pub fn app_bundle_path() -> String {
    format!("/Applications/{}.app", app_name())
}
pub fn bundle_id() -> &'static str {
    "com.opensky.driver"
}

#[cfg(target_os = "windows")]
pub fn uia_executable_name() -> &'static str {
    "opensky-driver-uia.exe"
}
#[cfg(target_os = "windows")]
pub fn autostart_task_name() -> &'static str {
    "opensky-driver-serve"
}

/// A bundled daemon already has its stable TCC responsibility identity and
/// must not disclaim it during startup.
#[cfg(target_os = "macos")]
pub fn is_executable_inside_cuadriver_app() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|path| std::fs::canonicalize(path).ok())
        .is_some_and(|path| {
            path.to_str()
                .is_some_and(|path| path.contains("/OpenSkyDriver.app/Contents/MacOS/"))
        })
}

/// Returns `true` when the env var is one of `1|true|yes|on`
/// (case-insensitive). Anything else, including unset, is falsy.
#[cfg(target_os = "windows")]
pub fn is_env_truthy(name: &str) -> bool {
    match std::env::var(name) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opensky_runtime_never_uses_upstream_identity() {
        assert!(is_local_installation());
        assert_eq!(cli_name(), "opensky-driver");
        assert_eq!(state_namespace(), "opensky-driver");
        assert_eq!(user_home_subdirectory(), ".opensky-driver");
        assert_eq!(app_bundle_path(), "/Applications/OpenSkyDriver.app");
        assert_eq!(bundle_id(), "com.opensky.driver");
        assert_ne!(cua_driver_contract::CONTRACT_VERSION, "0.7.0");
        let update = crate::version_check::check_update_state(true);
        assert_eq!(update.source, "opensky_source");
        assert!(!update.update_available);
        assert!(update.install_command.is_none());
        assert!(!crate::telemetry::is_enabled());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn env_truthiness_is_strict() {
        let name = "CUA_DRIVER_RS_TEST_TRUTHY";
        for value in ["1", "true", "TRUE", "Yes", "on", " 1 "] {
            std::env::set_var(name, value);
            assert!(is_env_truthy(name), "expected truthy for {value:?}");
        }
        for value in ["0", "false", "no", "off", ""] {
            std::env::set_var(name, value);
            assert!(!is_env_truthy(name), "expected falsy for {value:?}");
        }
        std::env::remove_var(name);
    }
}
