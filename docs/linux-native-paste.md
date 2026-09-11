# Linux native paste

Work in progress. The current frozen OpenSky build refuses native `App.paste`.
This candidate adds a real clipboard paste into the exact native window, preserving
the prior clipboard and avoiding overwriting a later copy by another client.
The user selected Linux-first implementation on the OpenSky Driver fork.
Issues are disabled on this fork; the linked draft pull request is the scope
and implementation record. The focused plain-text acceptance below passes; broader native-paste parity remains incomplete.

## Contract

- Send one actual paste chord to the checked target. Do not substitute typing.
- Preserve supported original data formats, including property type, width and
  bytes. Refuse an unsupported snapshot before clipboard/input mutation.
- Associate the temporary selection with a transaction and restore only while
  it still owns the clipboard. An intervening owner wins, even with equal text.
- Use a persistent X11 owner to serve restored formats after the call returns.
- Distinguish refusal before input from uncertain delivery after input. Never
  replay automatically after a possibly submitted chord.
- Success reports a supported transfer completed to the target client; it does
  not claim that an arbitrary application accepted the edit or saved a file.
- Plain multiline text is the first implementation slice. HTML and Markdown,
  other native platforms and Wayland remain explicit outstanding acceptance.
  The final API must not silently reinterpret a requested format.

## Implementation direction

One X11 actor owns its connection, clipboard windows, event processing and data.
Use short server critical sections for ownership check/claim/conditional restore;
never hold the server while waiting for an application. Reuse exact-window input
guards and the driver clipboard-writer lock. XRes identifies the actual requestor
client, including hidden toolkit windows. TARGETS negotiation alone is not a text
transfer. A normal transfer's property deletion can establish the supported
completion boundary; larger transfers require the INCR protocol.

A disposable X11/LibreOffice feasibility probe observed XRes 1.2 map the visible
Writer window and hidden requestor to the same client and PID. After clipboard
format negotiation, one Ctrl+V produced a 36-byte UTF-8 text transfer followed by
property deletion. An earlier immediate-chord probe observed negotiation only.
These observations guide implementation; they are not driver acceptance or a
guarantee about every application.

## Required behavior evidence

- Public SDK: select a Unicode-containing Writer paragraph, paste two lines,
  save, and verify the new text plus unchanged surrounding paragraphs.
- Independent real clipboard client: seed text, HTML and opaque binary formats;
  after paste, read every supported format and compare its type/width/bytes.
- Concurrent external copy, including equal text with different binary data:
  the later owner and contents must survive.
- Delayed/large transfers, empty clipboard, wrong sibling/modal focus and
  uncertain input receipt: no early restoration or automatic duplicate paste.
- Actual HTML/Markdown saved outcomes before those formats are called supported.

Run focused primitive and SDK checks first. Before promotion, compile the exact
candidate and run the relevant canonical desktop regressions. Keep the existing
V3 popup evaluation source and results unchanged while this work is developed.

## Candidate validation and remaining limits

Full Linux CI [34646289769](https://github.com/tanishqkancharla/cua/actions/runs/34646289769)
passes at source `445884192f616f4fa3be124a3d4b9148d18adc2c`, including Rust
tests, overlay and MIT-SHM regressions, uinput checks and portal compilation.
The initial CI vocabulary failure is retained: the test omitted existing shared
clipboard capability names. The correction changes the test vocabulary only;
actor/input code remains byte-identical to `9c7272351`.

The public SDK Writer case PASTE-L01 passes on SDK
`9cfbb709de8c87fa34f96f19ca6aadf322c83cc5`, using this full driver built by
[CI34646617341](https://github.com/tanishqkancharla/opensky/actions/runs/34646617341).
Driver binary SHA256:
`e9cfd5cd24d9fc2ae1df2bdc443924cbdb7174dd16c6a8dd0af41a91ebcbd770`.
The test selected a Unicode paragraph, pasted two lines, saved through normal
input and verified the exact saved ODT text plus unchanged surrounding paragraphs.
It completed in 17.64 seconds. Owned app, temporary files and container were
cleaned up. The fresh profile disables LibreOffice's ordinary Tip of the Day
preference; the preference file is retained with the evidence. Independent local
report: `work/native-paste-sdk-445-independent-check.json`.

A separate real actor/input check verified the same Writer saved outcome and
restoration of five original clipboard formats (UTF-8, HTML, opaque 8/16/32-bit),
including exact property type, width and bytes through an independent X11 client.
This supporting actor check is not full-SDK clipboard coverage.

Initial implementation supports X11 plain text up to 16 KiB, at most 32 saved
clipboard formats and 512 KiB total, with direct property transfers only. It
currently requires eager TARGETS negotiation before sending the chord; apps
that negotiate only after input are still unsupported. INCR, rich-text input,
clipboard managers, modal focus and delayed/large transfers still require
implementation or real desktop evidence. Restored data remains available only
while the driver process and its X11 connection remain alive. The exact-candidate
Linux canonical desktop gate passed as recorded below. Public-SDK clipboard
preservation and empty-clipboard checks also pass as recorded below; concurrent
ownership remains a separate acceptance gap.


### Focus and concurrent ownership controls

Public SDK PASTE-G01 (fixture `aad608b0fee3296749b2dababe50dca390ce5c57`)
passes on the same full driver in 49.78 seconds. Bringing a sibling document
forward causes paste to reject before input; saved files retain the intended
document unchanged and exactly the subsequent sibling edit. Owned app, temporary
files and container cleanup were verified. This does not cover modal focus.

Two separate actor/input controls use real X11 clients and saved Writer files.
Before input, a newer clipboard owner with equal text but different opaque data
survives; all five formats match and the document is unchanged. After input,
XRecord observes exactly one server Ctrl+V event before the second client claims
the clipboard. The claim is verified before the actor returns with
`inputSubmitted=true`, `skipped_owner_changed` and uncertain delivery. All five
newer formats survive. Writer consumes that client's distinct HTML once, with
surrounding paragraphs preserved. This is honest uncertainty coverage, not a
successful requested paste. XRecord observes server processing, not application
consumption. Both controls verify owned process/container cleanup; they do not
replace public-SDK clipboard-race acceptance.

The after-input control retains earlier strict document-oracle failures. Its
accepted oracle permits only unchanged text, the intended text once, or the
known newer HTML once when delivery is explicitly uncertain. It does not accept
arbitrary document changes or relabel the uncertain operation as success.

Canonical shared/native/capture and local installer checks passed at
`ae1577dd1faebd55385b16fe1a9c5453dceec4ee` in
[CI34650375969](https://github.com/tanishqkancharla/opensky/actions/runs/34650375969).
All 129 unchanged cases passed: 83 shared, 39 native and seven capture cases.
The local installer built the installed product, served `get_config`, and
cleaned its owned installation. This is not evidence of a published release.

The release binary from that run has SHA-256
`84e91eb18e208cf23a234ff685edd468e53aa585112937dcecfe898d9ba75d92`.
Both public-SDK cases were repeated against these exact bytes on the isolated
Linux worker, using SDK source `fca4efc369fada4d47d69d8be52459f5ac88be4f`:
PASTE-L01 saved the requested two lines with both surrounding paragraphs
unchanged (6.96 seconds); PASTE-G01 rejected the wrong sibling target and
preserved the intended document plus the single subsequent sibling edit
(16.72 seconds). Independent saved-ODT reads and owned app/container cleanup
passed. Local evidence is retained in
`work/native-paste-release-sdk-check.json` and the original worker artifacts.

Two additional public-SDK cases passed on those same release bytes with runtime
`50b9b22bdc720031364ad394ef6063b83baf745b` and fixture commit
`90614b7bbd8898de0ae9ea89291f9a6ea6548e9c` in the OpenSky repository:
PASTE-L02 preserved all five original clipboard formats, including exact
property types, 8/16/32-bit widths and bytes (9.38 seconds); PASTE-L03 restored
an initially empty clipboard (9.29 seconds). Both saved the requested two lines
with surrounding paragraphs unchanged. A separate real X11 client read the
clipboard after the public paste returned. Owned app, external clipboard owner,
temporary document and container cleanup were verified. The worker run is
`sdk-clipboard-preservation-v4-01`; independent evidence is retained in
`work/sdk-clipboard-v4-outcome-review.json`.

The original preservation attempt remains a failed test: text selection refused
before paste. The passing test refreshes the public observation after external
clipboard setup, then makes one selection attempt. This does not establish why
the original target became ineffective or resolve stale-element behavior.

These passes do not establish public-SDK concurrent ownership, modal behavior,
delayed/large transfers, clipboard-manager compatibility, or other platforms.
The pull request remains draft for those documented gaps. This documentation
update changes no executable input to the certified build or tests.
