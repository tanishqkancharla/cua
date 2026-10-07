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

## Guarded uninstall fixtures

The selector now verifies all four public source installation/removal routes and their delegates. `--print-driver-release-uninstaller unix|windows` prints the exact preserved Cua reference for a declared OpenSky checkout, or the canonical public uninstaller for default Cua. It executes neither script and does not certify installed behavior. Missing references and changed delegation/arguments/exit codes are errors.

The private Unix and PowerShell uninstallers are byte-identical to already merged upstream b0968e1b. Release ownership, survivor notices and refusal tests use those references. The source-removal owner now runs the public OpenSky wrapper, retaining all original local removal/preservation assertions. No public entry/delegate or Rust changed.

CI passes the selected reference explicitly to existing Unix history and Windows PowerShell5.1 fixtures. Their guarded process/discovery/removal seams remain. The Unix history fixture refuses an unavailable source or a product without the source-only release seam before sourcing, and uses an explicit temporary template under its private root with EXIT cleanup. This avoids system Bash falsely returning success for missing `source` and system `mktemp` ignoring the intended temporary-root oracle. Success, missing-source and wrong-product runs prove actual teardown; deleting the trap is detected and only that owned mutation residue is removed by the parent.

Local affected owners182pass/12Windows skips; complete Python scripts664pass/32platform skips. The final history fixture and its refusal/cleanup paths pass after the additional fixture corrections. These are disposable mocked installs, not user uninstall/desktop acceptance. Windows execution, remaining source installer/Lume/attribution failures and the canonical matrix remain hosted-validation questions. Current mutable outcomes belong in the linked draft PR rather than status-only source commits.


## Source identity feedback

Source installer owners check the actual OpenSky app, CLI, state, autostart and permission namespace. Release autostart owners read the validated preserved Cua downloader. Existing custom-target fixture still checks actual staged driver, cursor compiler, GNOME helper upgrade and absence of a Cua release alias; its output now checks OpenSky SDK/doctor/identity/skill guidance. The obsolete Windows migration duplicate is removed because it required release download advice forbidden by the existing fork migration owner. Four keeper mutations reject that advice or loss of public source delegation.

Windows failed-registration recovery now names opensky-driver-serve and opensky-driver retry commands, matching its successful branch. Its existing owner checks exact task naming and executes only the real extracted closing output branch when PowerShell is available. Mac runner initializes the original ScreenCaptureApprovals file and com.opensky.driver client. Existing sourced identity owner compares that client to independent runtime bundle identity and detects removing the initialization. There is no new client environment override or shipped test hook. No VM, actual host installation/signing or TCC action occurs in these fixtures.

Run complete source owners locally before pushing:

```sh
python -m pytest -q -p no:cacheprovider \
  libs/cua-driver/scripts/tests/test_install_autostart_summary.py \
  libs/cua-driver/scripts/tests/test_install_isolated_autostart.py \
  libs/cua-driver/scripts/tests/test_install_local.py \
  libs/cua-driver/scripts/tests/test_install_local_signing.py \
  libs/cua-driver/scripts/tests/test_install_local_migration.py \
  libs/cua-driver/scripts/tests/test_macos_lume_runner.py
```

The broader whole installer directory and existing encoding owner pass313cases/26platform skips and3subtests in about34seconds locally. CI runs the entire installer directory before the longer root Python suite, retaining every later check. This detects installer failures sooner without waiting for the unrelated suite. Skipped PowerShell cases and mocked/sourced Mac tests are not native Windows or canonical desktop acceptance. Mutable exact-head hosted outcomes remain in the linked draft PR.
