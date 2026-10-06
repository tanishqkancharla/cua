# Exact attached-sheet semantic menu dispatch

Selected by the OpenSky parity campaign. Refs https://github.com/tanishqkancharla/opensky/issues/12

## Problem and implementation

An attached AppKit sheet can be the focused window while its menu controls are observed through the bound parent document. Re-keying the document before a semantic menu action rejects the action or dismisses the popup.

The macOS route now proves that the retained, enabled menu item advertises the requested action, belongs to the requested process, and has an actual AXSheet ancestor. The exact WindowServer document must belong to that process, and its fresh AXChildren must contain the identical sheet object (CFEqual). The proof is bounded; unknown ancestry keeps the existing route. Deliver one advertised AX action without re-keying the document. No title, geometry, current focus or shared PID alone authorizes dispatch.

Keyboard, pointer and modifier delivery retain the existing foreground guards. A failed semantic menu action cannot escalate into collection-selection/focus repair. Other platform adapters are unchanged.

## Evidence and validation

- Read-only native AX capture proves the menu-item/menu/popup/sheet/document ancestry and that the sheet is a direct document AXChildren object. AXSheet is a role, not a document attribute.
- Final isolated module: cargo check -p platform-macos passed. OpenSky E2E TypeScript check passed.
- Integrated campaign runtime SHA256 418a39166bb1271589dc0bc453ec4382f46df688476150aa320d2bd739a740f4: existing real MENU-N01, SELECTION-OBS-N01/N03 and KEYS-N01 passed with exact cleanup. This integrated runtime contains earlier campaign changes; these are supporting Mac diagnostics, not standalone candidate matrix acceptance.
- Fresh unchanged gpt-5.6-terra CLI pair: native1081 and OpenSky1082 both opened the actual Other picker and canceled to the unchanged original RTF. Before the fix OpenSky1079 refused Other before dispatch. Afterward one Other action exposed indexed Cancel/Move and succeeded without that refusal. Both source identity/hash and owned app/helper cleanup passed.
- Whole-task comparison: native224.670s/649663 aggregate input tokens; OpenSky282.865s/823083. Opening/menu recovery means this run does not establish an overall resource gain.

## Remaining gates

The draft remains incomplete: no passing complete new MENU-N05 gate or sibling-scope GUI gate, and no canonical cross-platform desktop matrix at this branch SHA. The optional new gate exposed a large system-temp chooser tree exhausting the AX budget and a separate cold project-file launch timeout; those are retained unresolved findings, not passes. The paid task uses standard File Open and validates the original reported sheet-menu failure.

Linux/Windows behavior and release acceptance are not claimed. The user has explicitly excluded VMs, so the canonical macOS Lume wrapper has not run.
