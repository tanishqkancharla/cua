# X11 key-hold timing candidate

The public OpenSky REPL can lose an initial text prefix after Select All, while the matched native program saves the complete text. Diagnostic CI [34705099922](https://github.com/tanishqkancharla/opensky/actions/runs/34705099922) retains both documents, driver results and passive X11 device/focus traces. It observed a 0 ms OpenSky Select All key hold versus a 33 ms native hold. The missing initial character was itself recorded at the X server; this does not establish application consumption or the root cause.

`send_key_xtest` currently queues key-down, waits its existing KEY_DELAY_MS, then queues release and flushes. Both events therefore reach the server together. The proposed correction flushes key-down before that existing wait, preserving its duration and avoiding a new app-specific delay. Character typing and clipboard behavior remain unchanged. This is an X11 adapter change; Windows, macOS and Wayland are outside the changed code.

Acceptance: build the exact candidate, rerun the unchanged public native/OpenSky document program with device recording, verify an actual key hold and the saved document, then repeat the affected workflow and its nearest shortcut control. Preserve first failures and unrun states. A single successful document does not establish repeatability or prove this timing defect caused prior text loss. Follow canonical driver gates after the candidate is stable; the public REPL reproduction is supporting diagnostic evidence.

This fork has GitHub issues disabled. The linked draft PR serves as the visible work record for this user-authorized investigation, stacked on the existing native-paste branch. No other input or paste implementation is being replaced.
