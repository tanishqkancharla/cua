# OpenSky driver0.37 integration candidate

Merge canonical cua-driver-rs-v0.37.0 (`2d2f99228d4d2508dc6fcc84c176b296e146714a`), preserving upstream history and OpenSky's later fixes. Apply upstream `97991403b40d227c795ec4fd4e7b3ad7c873f21b` (injaneity) for validation/recovery of a screenshot prefetched before the AX walk.

Preserve OpenSky browser-key/clipboard routes alongside browser_select; bounded grant handling and uncertain-input semantics; Darwin buffered Unix reply polling; stronger exact native-window ancestry classification; native collection/selection observations; source-only installers and stable com.opensky.driver signing/TCC identity. Keep the later Rust TLS distrust lock0.103.16/1.0.10. An unused parallel ancestry helper was excluded in favor of the existing live classifier and its owners.

The merged driver compiles. Current CPU results:68contract,1154core,708Mac platform,353driver and3TLS owners pass;7platform and1core opt-in tests remain ignored.30installer/signing/TCC/uninstall owners pass. Original import-order failure and merge-owner failures are retained privately. MCP startup instructions now remain within the existing200word budget, retain conditional history/verification guidance and name OpenSky's own permission prompts.

Candidate only: generated contracts, stable-signed native controls and fresh exact-source Bookworm distribution must qualify before a supported SDK release. Windows native, Bookworm sandboxed Chrome, Mac19/66 and paid/source admission remain unresolved. No existing release asset, app recovery data, OS permission, VM or Chrome sandbox policy is changed by this source merge.
