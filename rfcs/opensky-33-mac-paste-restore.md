---
title: Restore Mac clipboard after a verified native paste
authors:
  - tanishqkancharla
created: 2026-10-07
last_updated: 2026-10-07
status: review
discussion: https://github.com/tanishqkancharla/opensky/issues/33
implementation: []
---

# Mac paste clipboard preservation

## Scope and ownership

OpenSky's maintainer requests native Codex computer-use parity and authorizes ongoing repairs. The actual public TextEdit comparison in the discussion proves an insertion-independent gap: both interfaces save the same exact multiline payload, but only native Codex preserves the seeded clipboard marker. This fork has issues disabled, so the linked OpenSky issue holds the discussion. This proposal and its draft candidate remain subject to review; no upstream RFC acceptance, merge or release is claimed.

Extend the existing native_paste owner adapted from Tanishq Kancharla's contributions #14 and #19. Keep their authorship and exact target/window/value/UTF-16 guards. Add an explicit restore policy; existing leave requests retain their behavior. OpenSky's Mac app paste will request restore only with a supporting driver, rather than silently falling back to leave.

## Proposed contract

Snapshot all advertised pasteboard item representations privately before mutation. A stable snapshot is required; unreadable representations and a clipboard changed during capture refuse before input. Bound retained data and item/type counts, with explicit refusal before clipboard mutation when limits are exceeded. The driver-local clipboard lock serializes its own operations, not unrelated apps.

After one verified insertion, restore the prior items/formats only while the board still has the operation's own change token and exact payload. Materialize local replacement items before the final ownership check; publish them together, verify exact item/type/byte readback, and report restoration separately from insertion. Do not replay an uncertain input. If dispatch is unknown, leave the payload and report restoration skipped; a late app could still consume it. An observed newer writer is preserved and reported. A restoration failure cannot turn a proved paste into a reusable/refused input result.

NSPasteboard has no atomic compare-and-swap. A writer may race between the final check and write; report this OS limitation and never promise atomic preservation. This is additive and Mac-specific; existing Linux restoration/transfer semantics and Windows capabilities are unchanged.

## Acceptance and sequencing

The existing strongest plain/rich public SDK paste owners prove explicit saved-file body/style plus supported prior clipboard restoration. Extend the keeper rather than add redundant insertion tests. Hermetic whole-platform crate tests cover snapshot representation/comparison/limits and receipt decisions without real clipboard access. Real clipboard tests require explicit opt-in and privately restore the user's original board in teardown. Retain the actual red comparison and malformed starter fixture refusal.

Run existing affected native paste/selection/rich style/focus gates on the exact stable candidate before matched independent Terra verification. Preserve stable signing/TCC identity. Canonical desktop platform matrix remains required before ready/merge; no VM is requested or used.

## Decision status

Review proposal and local draft candidate only. Upstream acceptance and release remain pending. The maintainer's active parity goal authorizes preparation and validation of this candidate; it does not grant approval to merge or publish a release.
