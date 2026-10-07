# OpenSky source-installer metadata validation

Refs https://github.com/tanishqkancharla/opensky/issues/35. The driver fork has issues disabled; the linked SDK issue is the durable problem record.

The0.34.0 reconciliation retained source-built public OpenSky entry points, but inherited release CI read them as a Cua download installer. Its actual failure was a missing Windows baked-version sentinel. The preserved private Windows downloader was also still0.23.2, while the already merged Cua tag/state/Unix downloader carried0.33.4.

Internal `installer-distribution.json` declares the existing source-build distribution. CI verifies the exact Unix/PowerShell public delegation routes, argument forwarding, exit propagation and both source delegates. Unknown/missing/drifted declarations cannot relax canonical release checks. Without the declaration, Cua retains the public Windows release-installer path.

Then existing published-version/withdrawal/ahead-of-source validation runs against the preserved Cua release reference. `_upstream-install.ps1` is byte-identical to the already merged upstream0.34.0 tagb0968e1b12834e485dda68789541a3cc57664a9f; original upstream history and license remain. These Cua baked versions are not an OpenSky binary release. Public entry/delegate scripts and all Rust source stay byte-identical to basec114403c.

Existing release-version owner covers default Cua and source-build version drift/withdrawals, declaration/route/delegate refusals and argument/exit loss. Existing update/channel/integrity owners target the correct release reference and retain their original assertions. Command construction for the PowerShell integrity runner is checked even on a Mac; that check is not Windows execution.

Local evidence: direct `python .github/scripts/validate_release_versions.py --product all` passes. All four affected pytest owner modules:102passed/23platform-specific skips. Initial missing-version, old fixture5+3failures, a Windows command-construction typo and the missing default Python pytest package are retained in private parity artifacts. No GUI, installed app, signing, TCC, daemon or paid agent was involved. Hosted exact-head release validation remains pending; inherited contributor attribution/formatting failures and the canonical full platform matrix remain separate. Keep draft.

## Fast local owner check

For installer metadata or source/release routing changes, run these six complete existing owner modules before pushing. Use the checkout's Python test environment; no installer, daemon or desktop is started. Do not select individual test names in ordinary CI.

```sh
python -m pytest -q -p no:cacheprovider \
  .github/scripts/tests/test_validate_release_versions.py \
  .github/scripts/tests/test_update_cua_driver_installer_version.py \
  .github/scripts/tests/test_release_channel_installers.py \
  .github/scripts/tests/test_cua_driver_installer_integrity.py \
  .github/scripts/tests/test_cua_driver_default_distribution.py \
  .github/scripts/tests/test_cua_driver_release_wiring.py
```

The default-distribution and release-wiring owners use the same validated release reference. Release download, cursor floor, telemetry ordering/consent and autostart assertions remain intact. Local product paths, permission identity and post-install client guidance follow the explicitly declared source product. Default Cua keeps its original namespace, Muse configuration and capability flags. OpenSky source guidance must name its SDK, doctor, bundled skill and token-only permission commands instead.

The broader script run exposed ten source/release mismatches on both the original base and first metadata candidate. This focused command now passes all156cases/23platform skips in about14seconds locally. Six targeted mutations are rejected by the existing owners; all four default-Cua guidance/namespace branches also pass against original upstream source. That retrospective fixture check is not an installation or native Windows acceptance check. Remaining uninstall/attribution failures and the canonical matrix are tracked in the linked draft PR.
