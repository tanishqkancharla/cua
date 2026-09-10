# OpenSky Driver compatibility fixtures

These fixtures lock selected stable public contract fields from the
`cua-driver-rs-v0.12.6` release tag at commit
`9eb1f481b8a12cd6ffda2ad5af21653a9e5aa9e5`.

This fork intentionally changes the CLI help command and MCP server identity
to `opensky-driver`, as requested for OpenSky's own driver. Only those two
identity fields in the upstream baseline are adapted; the protocol envelope,
required tools, argument shapes, package signatures and compatibility apps
remain locked. This is a named fork contract, not a claim that the original
Cua Driver identity is unchanged. The pinned-client discovery manifest also
includes the fork's additive `browser_key` and `close_window` tools.

The snapshots were derived from the release-tagged package sources, generated
bindings, `cua-driver --help`, `cua-driver manifest`, and MCP JSON-RPC responses.
The corresponding tests compare semantic fields instead of whole-process
output. This intentionally excludes executable paths, socket paths, PIDs,
session identifiers, platform-specific prose, and other volatile values.
Release version fields are checked for their documented shape rather than
frozen to `0.12.6`, because a compatible later release must change them.

- `python-package.json` locks package-root exports and callable signatures.
- `typescript-package.json` locks package subpath exports, declaration exports,
  and the generated `CuaDriver` declaration methods.
- `cli.json` locks the CLI help header/catalog and selected manifest fields.
- `mcp.json` locks initialize, tools-list envelope/tool fields, and the
  method-not-found error category.
- `apps/` contains unchanged Rust, Python, and TypeScript applications written
  against that baseline. CI compiles or executes them against candidate
  packages.

Additive manifest fields, tools, and package metadata are permitted. Removing
or changing an item recorded here requires an explicit compatibility decision
and an intentional fixture update.

RFC 2549 independently accepts one additive CLI change: `cua-driver mcp
--direct`. Bare MCP behavior remains platform-defined (direct on Windows and
Linux, signed app service on macOS), while `--socket` continues to select an
explicit service. The semantic compatibility test permits that additive flag
without rewriting the frozen `cli.json` baseline.
