# macOS typed-browser Select All

Refs https://github.com/tanishqkancharla/opensky/issues/11

Selected by maintainer request to address GitHub issues and reconcile0.34.
Depends on upgrade PR25; upstream native modifier fix is a separate route.
Actual owned Chrome on0.34 receives trusted Meta+A but leaves seed selection
11..11 instead of0..11 before typing. Investigate CDP editing command on the
same base event; preserve exact tab/ref ownership and no-replay failure behavior.

Draft before runtime edits. Acceptance requires actual Unicode selection and
replacement, relevant existing browser E2Es and serialized-CDP contract tests.
Mac-specific mapping; no Linux/Windows acceptance claimed. No VM.

Implementation: on macOS only, exact Meta+A with no other modifier attaches
Chromium selectAll to the existing base key-down. Other chords/platforms stay
unchanged; delivery uncertainty and releases retain their existing behavior.
Core905 whole-crate tests pass, including serialized event assertions for Meta+A,
Ctrl+A, Meta+Shift+A and Meta+B. Removing the editing command fails the keeper.
Actual post-fix owned browser gate pending.

Acceptance: current public clean SDK PR15 passes actual KEY-B01 on signed
Driver d0b065bf12bf81cab99c7566a43a466277a3c722, along with existing PASTE-B01 and
TEXT-B01. Before typing the page's observer proves trusted Meta+A and range0..11
for “Old café 😀”; after fresh semantic focus observation, exact “New 東京 😀”
replacement and trusted input/current owned title pass. Core905 tests and the
keeper mutation check pass. No Linux/Windows or canonical full matrix claim.
Current final diff after installed SHA consists only of reconciliation docs.
