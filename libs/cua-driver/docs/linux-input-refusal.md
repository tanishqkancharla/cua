# Preserve pixel-focus refusal details

Canonical Linux CI34558519742 at2e81b retains a failed background pixel typing case: the response says only `focus pixel-click at (0,0) failed`, losing the expected `background_unavailable` classification. The click implementation already returns the structured refusal before input; the shared Linux keyboard focus helper discards it. This prevents code and agent callers from distinguishing unsupported delivery from an uncertain failed input.

Scope: propagate the existing ToolResult unchanged when the prerequisite pixel focus action refuses. Keep input routes, fallback policy, focus, successful timing and public schemas unchanged. TypeText, PressKey and Hotkey share this helper. Stacked on the exact screen-origin candidate; unrelated hidden GTK fixture controls and scroll oracles stay separate. Fork issues are disabled, so the linked draft PR is the problem record.

Acceptance: unchanged canonical GTK3 background pixel typing must return background_unavailable with no leaked input and unchanged focus/z-order/cursor. Focused build/checks then the exact canonical row on a disposable remote Linux desktop; other affected keyboard callers must preserve the same response. Existing failing artifact remains retained. Complete certification remains required before readiness; no agent or cross-platform parity claim.
