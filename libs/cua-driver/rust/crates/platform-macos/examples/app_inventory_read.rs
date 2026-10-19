#[cfg(target_os = "macos")]
use std::time::Instant;
#[cfg(target_os = "macos")]
fn main() {
    // Public read-only process/bundle enumeration. No driver or GUI input.
    let mode = std::env::args().nth(1).expect("running or all");
    for iteration in 0..7 {
        let start = Instant::now();
        let apps = match mode.as_str() {
            "running" => platform_macos::apps::list_running_apps(),
            "all" => platform_macos::apps::list_all_apps(),
            _ => panic!("unknown mode"),
        };
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        println!(
            "{}",
            serde_json::json!({"iteration":iteration,"mode":mode,"elapsedMs":elapsed,"apps":apps,"readOnly":true})
        );
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This read-only macOS app inventory example is unsupported on this platform.");
    std::process::exit(2);
}
