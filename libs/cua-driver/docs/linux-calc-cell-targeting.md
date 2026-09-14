# Preserve Calc cell targets across edits and window movement

A frozen Linux evaluation filled 46 requested blanks but also overwrote four existing names. The unchanged public-SDK FILL-L01 reproduces the error in two edits: click C10, type, Enter, click the retained C11 index, type, Enter, save. The certified driver403 writes C10/C12 instead of C10/C11. A name-box control writes only the intended cells.

The geometry candidate d3005c0c3604c989e9f53e996e3e56f46173f9e3 passes FILL-L01–L05 on the original desktop image: consecutive edits, name-box control, focus transfer from the name box, replacing a marked range, and merged-cell editing with all merges preserved. Build34574603781 passed78 focused checks; binary SHA256 b1ca737e5fa6c20991efe95dd77e5845ec6aaf93ba827c4c737b03c59bb6aca0. FILL-L06 still fails after moving/restoring Calc: it writes C9/C11 instead of C10/C11. The placement tool reports(201,118), but the independently inspected X11 client origin is(200,100); these coordinate sources must not be conflated.

The identical moved-window diagnostic on driver403 also fails, proving that the moved-window problem predates d300. Screenshots show C9 selected immediately after the first indexed C10 click, before typing. Cell identity and Window coordinates remain unchanged; Screen Y shifts24 after pointer input. No fixed24px adjustment is justified.

## Source mechanism and discriminating experiment

LibreOffice24.2.7.2 creates a menu row above its GTK3 event box. Configure events store top-level coordinates, while the first pointer motion/button event stores the event-box origin. The cell's Screen geometry uses that cached origin; ATK Window geometry is accumulated independently through accessible parents. The retained source analysis is in the evaluation workspace's `work/lo-calc-coordinate-mechanism.md`.

Primary sources: [GTK frame event handling](https://github.com/LibreOffice/core/blob/libreoffice-24.2.7.2/vcl/unx/gtk3/gtkframe.cxx), [menu insertion](https://github.com/LibreOffice/core/blob/libreoffice-24.2.7.2/vcl/unx/gtk3/gtksalmenu.cxx), [ATK coordinate conversion](https://github.com/LibreOffice/core/blob/libreoffice-24.2.7.2/vcl/unx/gtk3/a11y/atkcomponent.cxx), [Calc cell geometry](https://github.com/LibreOffice/core/blob/libreoffice-24.2.7.2/sc/source/ui/Accessibility/AccessibleCell.cxx).

`calc-motion-diagnostic-after-01` proves motion alone updates C10 Screen Y425→449 before any button/key event. The original subsequent click/edit sequence then saves only C10/C11. This is a diagnostic setup, not acceptance of a product fix. X11 exposes only an unrelated1×1 child, so its tree does not provide the drawing-area inset for a pure geometric correction. All completed diagnostics retain whole saved files, identities, screenshots and verified owned app/container cleanup.

## Current candidate and acceptance

The candidate retains d300's narrowly scoped Calc coordinate normalization and adds pointer preparation for plain single-left indexed Calc cell clicks. It checks exact active client and visible input surface, moves the pointer, then refreshes the same retained accessible's bounds before issuing one button gesture. Snapshot token, object name/role/liveness, window ownership, focus and input surface are checked again. It never resolves another ordinal after movement. Errors after preparation cannot become a fallback or automatic replay. Foreground activation uses the same guarded route; other providers and modified/multiple clicks retain existing behavior.

A server round-trip and50ms native UI settling interval precede the refreshed accessibility read. This is not an application event acknowledgment or atomic input lease; asynchronous focus/layout changes remain a limitation. No guessed pixel inset, semantic selection, cell GrabFocus, or edit is added.

Before promotion, the unchanged six public-SDK saved-workbook contracts must pass on the original desktop image, followed by displaced Code/font and relevant indexed-click controls, affected canonical Linux certification and postmerge smoke. The frozen paid campaign remains unchanged. No macOS/Windows/Wayland verification is implied. Issues are disabled in this fork; draft PR8 is the scoped problem record.

Rejected experiments remain in history:30189c6 SelectChild marked cells without moving typing from A1;15adb GrabFocus passed the first two cases but failed name-box focus transfer and existing range selection. They are completely removed from the current product diff.
