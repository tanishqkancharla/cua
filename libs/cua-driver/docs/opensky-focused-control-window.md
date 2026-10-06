Refs https://github.com/tanishqkancharla/opensky/issues/17

## Problem and observed evidence

Maintainer-selected OpenSky parity loop (SET217). Native Terra1113 uses Chrome
Find Next/Previous successfully; fresh SDK1114 refuses Next before input and
recovers with shortcuts. Real Driver0.34 cold parent repro keeps domain / Result1of2
and repeats the same refusal. WindowServer main51917 versus Find auxiliary51930.
AXFocusedWindow maps to51917, while AXFocusedUIElement and all three Find buttons
map to51930. Same PID64024; AXParent chain ends at auxiliary AXWindow51930, not
main51917. Exact guard is correct; app-scope observations chose the wrong surface.
A first diagnostic fixture set AXValue without triggering actual Find matches;
that failed setup is kept separately from the successful repro.

## Selected scope

Expose additive read-only focused_element_window_id from the Mac list_windows
metadata, only when the actual focused element PID and exact visible/current
WindowServer owner agree. Keep focused_window_id unchanged. Native app-scope
SDK observations prefer that verified focused control surface and probe its exact
AXWindow before binding. Opaque document handles, browser bindings, tokens and
window/element ownership refusals remain exact. No same-PID ownership fallback.

## Acceptance plan

Whole affected unit crates and SDK focus/ownership tests; existing actual native
and browser regressions first. New real Find Next2of2 / Previous1of2 / Close
oracle on a cold controlled window, plus negative sibling-window binding test.
Fresh affected serial Terra comparisons after local gates. Preserve exact app,
helper and owned-session cleanup. No VM, canonical matrix remains draft-only.
