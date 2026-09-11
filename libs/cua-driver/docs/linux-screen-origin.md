# X11 screen-origin correction

A maximized LibreOffice Impress window exposes its decorated frame at screen (0,0), while the X11 client starts at (0,17). The font field reports Screen (1197,173,59,34) and Window (1197,156,59,34). Its Screen coordinates are already translated. Driver 2ff75ef21 treats the near-zero frame as a renderer-local Screen provider and adds another 17 pixels.

The public SDK font controls preserve real input and independently read the saved PPTX. In font-field-bounds-before-02, clicking the visible field saves the unchanged text at60pt; indexed clicking saves70pt. Both controls share observations and input sequence. Source, application exit, temporary removal and container termination are verified. Two further pixel controls discriminate the computed rebased center from the actual Screen center; their results are pending.

Scope: correct Screen-coordinate inference in the Linux X11 adapter while preserving genuinely renderer-local Chromium coordinates. Do not change GTK/Wayland reconstruction, public API schemas, or the frozen paid campaign. This work is stacked on the visibility correction and does not replace its scope. Issues are disabled in this fork, so the draft PR is the problem record.

Acceptance: unchanged indexed font editing must save60pt with text preserved; visible-coordinate controls and displaced Chromium/Code indexed targeting must remain correct. Focused Rust checks plus exact-source real SDK outcomes are required. Canonical Linux certification remains pending before readiness; targeted controls are supporting evidence only.
