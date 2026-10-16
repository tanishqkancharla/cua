# Native plaintext clipboard paste on X11

Verified native-only implementation for [OpenSky issue69](https://github.com/tanishqkancharla/opensky/issues/69); SDKd14db52/Driverc36e63e actual bounded Bookworm/TigerVNC run38875586155 now passes exact ASCII/Unicode saved replacements, payload retention, pre-input permission refusals, partial application effects and host Stop/drain. All seven owned supervisors exit without force. Sandboxed Chrome remains not_run/unaccepted. Raw ordinary-user Bookworm/TigerVNC controls38868718952 and38869048206 show that GTK AT-SPI PasteText acknowledges without changing text, while one real foreground Ctrl+V inserts exact ASCII/Unicode and replaces a selection. Those raw controls are not driver acceptance.

The existing native_paste tool is registered on X11, not Wayland. It requires exact positive pid/window_id, text of1..16384 UTF-8 bytes, format text, clipboard_policy leave and delivery_mode foreground. Existing desktop target and clipboard authorization both apply. Missing foreground, rich formats and restore policy refuse before clipboard mutation or input; no background/AT-SPI/typing fallback exists.

The existing X11 foreground transaction activates and confirms the exact window. A complete window-scoped AT-SPI walk must find one enabled, focused native editable. Password/web-content targets, ambiguous/partial observations and unprovable windows refuse. The bounded readback supports at most262144 characters. Clipboard payload and selection/value/focus are checked again before exactly one Ctrl+V. The same retained bus/object identity is read after input and must equal the expected character-range insertion. No transfer provenance is claimed from a value readback.

Clipboard is deliberately left with the payload; an observed newer external writer is never overwritten. No snapshot/restore policy is supplied on Linux. Clipboard write failures may already have changed the board. Structured errors distinguish input_dispatched and clipboard_may_have_changed. Any failure after the first XTEST event is effect unknown, never permission to replay. A post-input read timeout does not undo application input. Client cancellation does not cancel a dispatched daemon action; hosts retain/await its tracked action before conflicting work.

As with ordinary desktop input, another desktop/clipboard client can race after the final checks. No atomic global desktop/clipboard transaction or rollback is promised. Those buffer/saved-byte, bounded-grant, Stop/drain and cleanup checks passed in run38875586155; the private GTK partial-acceptance handler saves only par and the SDK returns native_paste_unverified/effect unknown with one dispatch and no replay. Value verification is not transfer provenance. Mac candidate qualification remains separate. No Chrome sandbox or Wayland acceptance follows from this X11 implementation.

## Explicit clipboard grants

The first bounded SDK run38871519468 refuses before dispatch: tool/desktop grants cannot name the already-enforced clipboard resource. Complete that existing adapter's manifest migration with version3 clipboard declarations. This is an explicit resource ceiling; tool allowlists and desktop/application/window scopes remain separate. Undeclared clipboard resources still deny. Legacy version1/2 manifests do not gain clipboard permission.

For native plaintext paste, add this to a version3 manifest (version3 has no `mode` field):

```yaml
version: 3
expires_after: 1h
idle_timeout: 30m
resources:
  clipboard:
    write: [text]
  desktop:
    windows:
      - pid: 1234
        window_id: 5678
allow:
  tools: [start_session, end_session, native_paste]
```

Select `--permission-mode bounded` and approve that exact manifest at trusted daemon startup. Include the observation/discovery tools and their exact desktop scopes when the SDK needs to bind an app. The real Halo owner uses that private editor's PID/window and explicit display/discovery scopes.

`clipboard.write` accepts only text, image, file_url, with no wildcard. Text denotes textual clipboard data, including representations produced by an otherwise-granted text-paste tool; it does not constrain each tool's supported formats. The X11 native tool itself accepts only plaintext. `clipboard.read` separately accepts types or text; text includes the types returned alongside content. A text-write grant never permits exposing existing clipboard text. Native paste reads its own payload internally to check delivery, without exposing prior contents. Direct clipboard-read, file-transfer, and desktop-input operations keep their independent tool/resource requirements.

The actual SDK owner must prove that a same-target daemon with no clipboard resource refuses before input; granting text-write still refuses clipboard-read; missing foreground refuses; and exact saved text, partial effects and host completion remain truthful. A fresh editor-state sequence prevents refusal checks from accidentally accepting an old unchanged sample. The blocking native paste worker retains the clipboard lease even if its async waiter is dropped.


The canonical pinned MCP client job38875328364 independently discovers native_paste on Linux and fails its unchanged pre-feature exact tool roster. Add this one X11 registry entry to the Linux compatibility manifest, retaining exact equality and output-schema counts. No production Rust source or test assertion is relaxed.
