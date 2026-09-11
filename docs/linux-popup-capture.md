# Linux dropdown capture

Status: reproduced observation gap; implementation and candidate acceptance pending.

This work is selected by the fork owner's Linux Computer Use parity goal. The fork
has issues disabled, so this document and the linked draft pull request retain the
problem, scope and evidence. The existing earlier PR stack is separate.

## User-visible problem

Opening Calc's Pass/Fail/Held validation dropdown produces a black rectangle in
OpenSky's exact-window screenshot. The actual desktop displays all three choices.
An agent therefore cannot read the menu from that screenshot, even though the
keyboard can select a valid choice.

The reproduction uses the public SDK and original OSWorld workbook, with driver
source `ed9fd15e39a1d2acd07c4f3078440c3ab2ad44f2`, certified binary SHA256
`8b50bfbdbd9c98aaa5d0ff9d4f2196b18115b96fe6c2d68847ddb16c21ce6692`.
Merged base `53b2a81deaa7119a31a0e68bbfc0962e1652bf90` has the same Git tree.

SDK diagnostic source:
[DROPDOWN-D01](https://github.com/tanishqkancharla/opensky/blob/33b4e9cf6d6329215206d5a78571198743216d3a/e2e/specs/linux-dropdown-popup.test.ts).
It brackets a read-only desktop capture with two public app observations, with no
intervening input. Both app PNGs are byte-identical and black in the popup region;
the desktop PNG visibly contains Pass, Fail and Held. Active PID/XID and owned
window geometry remain unchanged. The popup keeps its XID, location and width,
but its height settles from 71 to 61 pixels between metadata reads. These are
sequential observations, not an atomic or fully geometry-stable capture.

The disposable diagnostic image preserves all 402 original package versions and
base layers, adding ImageMagick and its seven required dependencies only. Image
ID: `sha256:2e3a8a80cf737938d0b550c7943f5a9604d088df917e536fe3ab552932c71331`.
Owned app/container cleanup and unchanged saved workbook were verified. Original
agent failures and scores remain unchanged.

## Implementation scope

Investigate composition of proven same-window popup surfaces into the existing
X11 screenshot canvas before resize. Preserve its coordinate origin and bounds.
Do not substitute a desktop crop or alter background-window capture behavior.

`list_windows` cannot be used as a popup inventory: the EWMH list omits unmanaged
windows, and existing discovery filters empty titles. A bounded root-child walk
can obtain geometry and properties, but same PID/class/client leader alone does
not associate a popup with a particular sibling document. Ownership metadata is
the next required discriminator before choosing an implementation.

Only include a popup after proving its relationship to the requested window,
viewability, stacking and stable identity/geometry. Preserve conservative behavior
for missing ownership, unsupported alpha/shape and window replacement. Clip any
owned popup pixels to the original canvas. This work targets X11; it does not
establish macOS, Windows or Wayland popup parity.

## Acceptance

- Public SDK screenshot exposes the actual three choices in this Calc workflow.
- Independent desktop observation agrees with the visible popup and its placement.
- Wrong-process and same-process sibling popups remain excluded.
- Missing/conflicting ownership, closing/replaced popup, changing geometry and
  popup bounds outside the target canvas have explicit checked outcomes.
- Existing background capture and action-coordinate behavior remain intact.
- Run focused tests while implementing; then the canonical Linux suites on the
  exact final candidate, followed by merge-tree and installer/start/cleanup smoke.

No production fix, candidate acceptance or new agent campaign score is claimed.
