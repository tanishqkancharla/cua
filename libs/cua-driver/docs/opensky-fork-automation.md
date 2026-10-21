# OpenSky fork automation scope

OpenSky SDK releases are distributed from `tanishqkancharla/opensky` with exact fork driver source and independently qualified immutable assets. Canonical upstream release tags, registry doctor ledgers, image builders and VM orchestration are separate products/infrastructure. They must not be represented as OpenSky acceptance.

After integrating canonical driver0.37, inherited schedules started in this fork. On2026-10-21 the following workflows were disabled through this fork's GitHub Actions workflow settings, preserving the user instruction against VMs and unbudgeted model runs. Source/unit/contract CI and the fork's source-installer monitor remain active. Disabling an unrelated upstream schedule never marks its historical failures passed or changes a parity/source/cleanup gate.

- `cd-image-linux.yml`
- `cd-image-macos.yml`
- `cd-image-omarchy.yml`
- `ci-check-docs-image-refs.yml`
- `ci-test-models.yml`
- `docs-nightly.yml`
- `e2e-cua-driver-release-gate.yml`
- `e2e-cua-sdk.yml`
- `nightly-contrib-doctor.yml`
- `nightly-cua-driver.yml`
- `nightly-hyprland-plugin-requalify.yml`
- `nightly-lume.yml`

These are repository workflow states, not modifications to upstream source. Keep them disabled unless there is an explicit, scoped authorization and the required infrastructure. The original fork failures remain recorded. Driver nightly/release-gate planning expected canonical release tags absent in the fork; docs-image checks expected canonical registry doctor ledgers; VM/image/provider/model jobs exceed this source-only/native qualification scope.

Separate verified integration fix: Linux and Nix Rust owners failed six skill-placement assertions because they expected upstream `cua-driver` directories, while the established fork owns `opensky-driver`. The same public CLI tests now verify exact links, bundled content, platform filtering, idempotence, and preservation of user directories/developer symlinks at the fork's actual paths:10pass, versus4pass/6fail before. No product Rust behavior changed. Include the complete integration-test binary in the OpenSky build workflow so this mismatch is caught before publication.

The upstream `jev-use` native AppKit job used `/Applications/CuaDriver.app` from the upstream released installer and failed before input with missing capture_id. Its result remains failed; it is not new evidence for the separately source-pinned OpenSky driver, whose prefetch recovery and actual capture_id checks are documented in [0.37qualification](opensky-upgrade037.md). No full Mac/Windows/Bookworm browser acceptance is inferred. Mac issues19/66 and paid-source admission remain unresolved.

## macOS source CLI tests under Documents

A local macOS 27.0 (26A428) run of the source-built CLI initially failed seven integration-test targets: its isolated daemons did not publish their private sockets within 10 seconds. A sample of the exact test-owned disclaimed child shows dyld waiting in `__open` before Rust startup. No SDK/app action was dispatched in that failing control.

The same compiled executable, SHA-256 `d7af3ce89141fd4b95ba2c2761314dbefa4952aa376972fb1a8294c37a87498e`, succeeds when staged in a job-owned private temporary directory outside the Documents checkout. Use the testkit's existing relocation contract:

```sh
# Build first; stage and checksum the exact executable in a private job directory.
CUA_TEST_DRIVER_BIN=/private/tmp/your-owned-job/cua-driver \
CUA_TEST_REQUIRE_DRIVER_BIN=1 \
cargo test --locked -p cua-driver --test compatibility_contract_test
```

The one-test path control fails from the original checkout and passes from the temporary path. Rechecking the seven original targets through this route passes 27 checks, with one pre-existing ignored test, in 56 seconds. The runner verifies source and executable identity, keeps isolated homes, removes desktop test opt-ins, and removes the temporary binary after every child exits. No permission setting, signing identity, installed driver, or test assertion is changed. The precise OS policy responsible for the path dependence is not established; a protected-folder/TCC explanation is an inference.

This is headless protocol/filesystem coverage. GUI tests continue to use the stable signed installed OpenSkyDriver.app. This result does not close Mac cold app/document issues 19/66, grant new GUI capabilities, or admit paid comparisons. At executable source `3a832bb677280d6d4bd7497de7c80c94937f9509`, [OpenSky Linux CPU](https://github.com/tanishqkancharla/cua/actions/runs/39563385415), [the full Linux Rust job](https://github.com/tanishqkancharla/cua/actions/runs/39563386970), and [Nix source checks](https://github.com/tanishqkancharla/cua/actions/runs/39563387022) pass.
