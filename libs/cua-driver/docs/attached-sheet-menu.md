# Exact attached-sheet semantic menu dispatch

Selected by the OpenSky parity campaign. Refs https://github.com/tanishqkancharla/opensky/issues/12

On macOS an attached AppKit sheet can be AXFocusedWindow while its menu controls are observed through the bound parent document. Requiring that parent to become key before dispatching an advertised AX menu action rejects the action or dismisses its transient menu.

Planned scope: prove the retained menu item's owner and actual attached-sheet relationship to the exact requested AX document window, then deliver one advertised semantic action. Preserve existing foreground guards for keyboard/pointer/modifier delivery and unresolved controls. Do not infer scope from labels, shared PID, geometry or current focus alone.

Evidence: a matched native run opened TextEdit Move To / Where / Other; OpenSky refused before dispatch. One bounded reproduction captured distinct exact document and sheet CGWindowIDs, with the sheet focused before and after refusal. The source file stayed unchanged and owned app/helper cleanup passed.

Validation is pending: focused compilation/contracts, relevant existing real Mac GUI tests, new attached-sheet/sibling scope coverage, and affected unchanged paid task verification. No standalone candidate desktop matrix, Linux/Windows behavior or release acceptance is claimed. The change is scoped to macOS AppKit AX menu dispatch; other adapters retain their native semantic routes.
