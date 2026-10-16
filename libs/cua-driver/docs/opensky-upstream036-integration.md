# OpenSky driver0.36.0 integration

Tracking: https://github.com/tanishqkancharla/opensky/issues/71

The merge preserves canonical upstream history at `6d6c734c127025fb39ad5d585a10382808b39dc1` and the fork contracts at `4a0189e286cf48c6b05da5bdd76ffebce4accc84`. The installed, permissioned driver remains0.35.4 until qualification.

## Resolution

- Combine upstream app-menu ancestry, background key-window/content/drag and launch readiness with exact attached-sheet/inline-editor ancestry, window-only probes and viewport row collapsing.
- Keep foreground HID value focus, per-control Unicode packet guards, explicit launch-watchdog supersession, actual clipboard preservation and saved-text verification. The existing release installers still build this source distribution; no upstream download replaces it. Preserved Windows release installer follows canonical0.36.0.
- Keep trusted browser wheel delivery metadata, synthetic retained-node stale/refusal guards, debugger endpoint opt-in and exact session/target ownership while adding upstream isolated browser lifetime metadata.
- Adopt canonical X11 `TextClient` synchronization instead of maintaining two application-ping implementations. Retain the exact reply identity keeper in that seam and the actual delayed background-recipient/no-replay test. Preserve truthful unknown delivery on an unanswered final batch; upstream's original final-batch success is not retained. Remove the unused preflight helper and its implementation-only duplicate tests. Canonical real foreground synchronization controls remain the owners for foreground behavior.
- Preserve source-built executable permissions/PID-file CLI owners and fork namespace isolation. Add native public CLI/MCP dumps to existing build artifacts so generated contracts can be refreshed from actual platform builds.

## Validation status

Initial combined compilation exposed three merge-adaptation errors (a duplicated window serialization helper and two missing action borrows); corrected before acceptance. Existing common and Mac whole library tests pass:67 contract,1056 core (1 ignored),677 platform-macos (6 ignored). Ignored desktop cases have not run through these unit commands.

Candidate installation, signed native desktop qualification, native Linux typing and sandboxed browser contracts, generated snapshot refresh and paid regression pairs are pending. No new runtime or release qualification is claimed by this source merge.
