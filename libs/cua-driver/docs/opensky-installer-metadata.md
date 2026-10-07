# OpenSky source-installer metadata validation

Refs https://github.com/tanishqkancharla/opensky/issues/35. The driver fork has issues disabled; the linked SDK issue is the durable problem record.

The0.34.0 reconciliation retained source-built public OpenSky entry points, but inherited release CI read them as a Cua download installer. Its actual failure was a missing Windows baked-version sentinel. The preserved private Windows downloader was also still0.23.2, while the already merged Cua tag/state/Unix downloader carried0.33.4.

Internal `installer-distribution.json` declares the existing source-build distribution. CI verifies the exact Unix/PowerShell public delegation routes, argument forwarding, exit propagation and both source delegates. Unknown/missing/drifted declarations cannot relax canonical release checks. Without the declaration, Cua retains the public Windows release-installer path.

Then existing published-version/withdrawal/ahead-of-source validation runs against the preserved Cua release reference. `_upstream-install.ps1` is byte-identical to the already merged upstream0.34.0 tagb0968e1b12834e485dda68789541a3cc57664a9f; original upstream history and license remain. These Cua baked versions are not an OpenSky binary release. Public entry/delegate scripts and all Rust source stay byte-identical to basec114403c.

Existing release-version owner covers default Cua and source-build version drift/withdrawals, declaration/route/delegate refusals and argument/exit loss. Existing update/channel/integrity owners target the correct release reference and retain their original assertions. Command construction for the PowerShell integrity runner is checked even on a Mac; that check is not Windows execution.

Local evidence: direct `python .github/scripts/validate_release_versions.py --product all` passes. All four affected pytest owner modules:102passed/23platform-specific skips. Initial missing-version, old fixture5+3failures, a Windows command-construction typo and the missing default Python pytest package are retained in private parity artifacts. No GUI, installed app, signing, TCC, daemon or paid agent was involved. Hosted exact-head release validation remains pending; inherited contributor attribution/formatting failures and the canonical full platform matrix remain separate. Keep draft.
