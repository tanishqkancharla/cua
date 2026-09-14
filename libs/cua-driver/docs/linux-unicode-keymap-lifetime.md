# X11 Unicode keymap lifetime

A public SDK typing call can return success while an application that briefly
stalls loses a Unicode character. The XTEST route borrows an unused keycode,
installs the character's keysym, sends the key, waits 200 ms, then restores the
mapping. A server round trip proves server processing, not client translation.

## Reproduced failure

The unchanged SDK typed `A café near Dublin Zoo — 中文 😀.` into a fresh Writer
document and saved it. An external diagnostic paused only the exact owned app
process for 654.64 ms after observing its first remapped é keypress. XRecord
captured the em-dash mapping, keypress and restoration while that process was
stopped. After resumption, both application text-event insertions and the actual
saved file lacked the em dash, although the public typing call returned success.

- SDK: `c69d3b8533ffc860759707737e45dc5578466b23`
- Driver: `bd7c5a52253b20a8169d0a9a10437b8a14c68f6a`
- Driver SHA-256: `ba8b7c854f05f15be028ceee7ea0fbdf3266bd6f616309d5e78f0014c00c6e7d`
- Saved DOCX SHA-256: `5935df4377d0cce61e21820aa08734441aed37cb5a344a96069c2f2da86af482`
- Diagnostic: `v14-sdk-app-stall-01`, local receipt `work/v14-app-stall/acceptance.json`

The diagnostic used pidfd/start-time identity checks, observed stopped/running
states, a bounded resume watchdog and independent parent cleanup. The keymap was
restored; all observers and the app exited and the container was removed. It is
not an agent benchmark score or a comparison against native under the same stall.
It does not establish the cause of an earlier, differently corrupted prefix.

## Intended correction and acceptance

Keep keyboard typing semantics, exact focus guards, and no replay after partial
input. Replace elapsed-time-only temporary-map release with a supported client
processing barrier. Do not merely lengthen the fixed sleep or substitute paste,
AX text replacement, app-specific input recipes, or success-shaped acknowledgments.

The candidate uses EWMH `_NET_WM_PING` only when the exact client advertises it
in `WM_PROTOCOLS`. Its reply establishes client event processing, not document
consumption. GTK 3.24.43 queues/translates a preceding key event and emits that
event before pulling the following ping from X11; this provides the relevant
translation boundary on that event loop. Other toolkits are not certified by
that source inspection. The real saved document remains the acceptance oracle.

Clients without the protocol retain the existing 200 ms fallback; that path
still has an unverified mapping-lifetime limitation. A participating client
which does not reply within the bounded wait must produce a partial-delivery
error, restore the keymap, and never trigger replay. Synthetic typing and named
key injection are outside this focused XTEST text correction.

Sources: [EWMH ping protocol](https://specifications.freedesktop.org/wm/latest-single/)
and [GTK 3.24.43 event source](https://github.com/GNOME/gtk/blob/3.24.43/gdk/x11/gdkeventsource.c)
(`_gdk_x11_display_queue_events`, `gdk_event_source_dispatch`).

First acceptance is the same controlled stall through the public SDK, with the
complete saved sentence and restored keymap, followed by ordinary Unicode typing
and a meaningful shortcut control. Retain original failures. Broader driver CI
runs once the implementation is stable; no frozen agent scores change implicitly.
