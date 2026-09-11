# Linux native paste

Work in progress. Native OpenSky `App.paste` currently refuses native targets.
This work adds a real clipboard paste into the exact native window, preserving
the prior clipboard and avoiding overwriting a later copy by another client.
The user selected Linux-first implementation on the OpenSky Driver fork.
Issues are disabled on this fork; the linked draft pull request is the scope
and implementation record. No native-paste acceptance is claimed yet.

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
