# Permission status and exact daemon endpoints

Fixes [OpenSky61](https://github.com/tanishqkancharla/opensky/issues/61). Upstream1ff756bd79856ef4c34b180ea1a6fb36ca368d73 is adopted with injaneity and Ritikag33 attribution preserved. Only the two added optional prompt properties are applied to the fork's native schema snapshots; unrelated fork contracts remain.

`permissions status [--json] --socket PATH` queries only PATH. If absent, JSON reports `daemon_running:false,status:"unknown"` and no grant booleans. Without `--socket`, existing default endpoint/TCC attribution checks remain. Linux/Windows accept the existing optional boolean `prompt` property; the native implementations do not prompt. CLI human output uses their native status text rather than Mac TCC labels. Shared authorization still refuses agent `prompt:true`.

`permissions grant --socket PATH` exits64 before requesting UI: grant configures the installed Mac app rather than an arbitrary daemon. Embedded applications own their permission setup. This does not grant additional action capabilities or establish successful application input/capture.

Before: SDK0.2.15 with driver1007552 reported the default signed daemon's granted Mac permissions even when explicitly selecting a nonexistent socket. Actual SDK/CLI responses retained privately; no input dispatched.

Local verification:327 whole driver units, two real CLI permission tests and the existing protected-prompt refusal owner pass. The CLI test starts its own source-built daemon with an isolated home and standard authorization, checks native status, checks an absent endpoint and refuses endpoint-specific grant. Removing only selected-endpoint dispatch makes this owner fail at the selected-daemon assertion; restoration passes. Five docs generator tests and actionlint pass.

A source artifact under Documents initially stalled before its socket existed. The original failure is retained; the existing testkit override uses a byte-identical current Cargo artifact outside Documents, never an older installed binary. The new CLI owner copies its own current Cargo artifact into its temporary home on Mac.

The all3OS workflow selects both existing complete permission binaries. Its Linux lane also removes the optional prompt schema, requires the same CLI owner to fail with invalid_arguments, restores source in finally, and requires the whole owner binary to pass again. Native Linux/Windows results are pending at commit time. No paid benchmark, GUI, TigerVNC/Chrome, Wayland or native desktop certification is claimed here.
