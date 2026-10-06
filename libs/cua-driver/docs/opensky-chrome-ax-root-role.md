# Chrome app-root accessibility enablement

Refs https://github.com/tanishqkancharla/opensky/issues/16

## Change and evidence

On macOS, query the application root AXRole before the existing bounded
Chromium opt-in/materialization path. Chrome enables native APIs in that
standard getter; window-only walks can otherwise expose toolbar/menu only.
https://chromium.googlesource.com/chromium/src/+/refs/heads/main/chrome/browser/chrome_browser_application_mac.mm

Independent cold-process witness: a read-only AXRole query (no setters, input,
or native plugin) changes SDK Count visibility from false to true. The matched
native observation also saw Count on the same uniquely owned URL/window before
SDK saw it. Its diagnostic HTTP teardown timed out; the observations and passed
outer cleanup are preserved separately, not presented as a passed test.

Runtime candidate d5a4330615be0b7568ffc1a8994c313f88b0381b:
- Whole platform-macos crate: 511 passed, 6 desktop tests ignored.
- Existing real PASTE-B01/TEXT-B01 and ACT-N01/GROUP-N01/KEY-N04 pass.
- Fresh STEP-N01 passes actual trusted Count 0→1→2→1 using three exact-window
  native increment/decrement calls; unadvertised action refuses before input.
- Clean SDK AX-N01 passes cold native page/control publication without acquiring
  a native-plugin handle or typed browser tab. Keeper ships in companion SDK PR15.
- Exact owned app/session/helper cleanup clear after each run; TCC grants retained.

An initial STEP run after this fix failed an outdated value-rendering assertion
(after the first actual increment). Only the fixture assertion was corrected to
accept both public value renderings; the fresh full trusted-event test passed.
Historical failures are retained.

Depends on Driver PR26 (including upgrade PR25). Process cache, opt-in retry
bounds, exact-window ownership, tokens and delivery semantics are unchanged.
No forced Chrome flags, broad EnhancedUI fallback, or replay of input.

Draft: canonical exact-candidate desktop matrix and fresh affected serial Terra
comparison remain pending. This Mac adapter repair makes no Linux/Windows
runtime or benchmark score claim. Later docs-only commits do not change the
installed runtime candidate.
