# Verified Mac focused-element observation (draft)

Decision record: https://github.com/tanishqkancharla/opensky/issues/4.

The proposed optional `elements[].focused: true` identifies the actual focused
object only when native equality uniquely matches an already indexed object in
the requested live PID/window snapshot. It is observation metadata, not input
authority. OpenSky already renders this field. No focus is inferred from input
history, an empty value, a label, or text-selection role alone.

The reader must retain native references across bounded work, validate the
focused window/element owner and associated window, exclude secure controls,
and revalidate unchanged focus and window. A known direct window mismatch must
never be replaced by an associated-window fallback. Unknown or ambiguous focus
is omitted. Capture-only, unresolved and passive/probe paths omit this metadata;
query projection must not reveal or mint a hidden index.

Implementation and acceptance are pending. Relevant existing Mac GUI tests must
precede a real new Reminders continuation: create a disposable list, commit the
first item once, observe the focused blank next field, write the second item to
that observed index, and verify actual two-item content and exact reversible
artifact/app cleanup. Then re-run the unchanged affected native/OpenSky agent
task and report all three resource ratios with attribution limits.

Native macOS `getApp` requires an app name, path or bundle ID; a window-ID target
is unsupported. Separate-window controlled observations and raw SDK exact-window
snapshots establish the current missing metadata. They are not a successful
same-window native binding or an isolated whole-agent speed measurement.

This is a Mac adapter change toward the existing optional cross-platform focus
contract. No Linux/Windows behavior, input delivery, signer, permission or VM
change is proposed. Broader platform/release certification remains pending.
