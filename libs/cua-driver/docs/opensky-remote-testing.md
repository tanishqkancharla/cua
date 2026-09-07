# Remote OpenSky Driver testing

Use the fork's `.github/workflows/opensky-driver.yml` on branch `opensky-driver`.
Every job runs on a disposable GitHub-hosted runner. No job uses the developer's
Mac, installs a local daemon, changes local permissions, or drives local apps.

## Build and unit checks

Relevant pushes build the current commit on Ubuntu 24.04, Windows 2025, and
macOS 26, run the driver binary and shared contract/core unit suites, then verify
OpenSky's offline version and identity. Each `opensky-build-*` artifact contains
logs, a development executable, and source/OS/architecture provenance. These are
unsigned debug builds, not installers or production releases. macOS build success
is not Accessibility, Screen Recording, Safari, clipboard, or native GUI evidence.

## Desktop checks

Desktop tests are opt-in. Dispatch `OpenSky Driver` on the fork branch with
`desktop=native`, `shared`, `capture`, or `all`:

```sh
gh workflow run opensky-driver.yml --repo tanishqkancharla/cua \
  --ref opensky-driver -f desktop=native
gh run list --repo tanishqkancharla/cua --workflow opensky-driver.yml
gh run view RUN_ID --repo tanishqkancharla/cua --log-failed
gh run download RUN_ID --repo tanishqkancharla/cua --dir work/driver-ci-RUN_ID
```

GitHub may require a workflow to exist on the default branch before accepting
manual dispatch. Until then, include `[desktop-e2e:native]` (or `shared`, `capture`,
`all`) in a relevant commit message and push `opensky-driver`. The push and all its
jobs use that exact commit. Routine pushes omit this marker and only build/test.

The jobs call the existing canonical Linux X11 and Windows harnesses. Linux uses
Xvfb, Openbox, D-Bus accessibility, and real fixture applications; Windows must
pass the interactive-desktop preflight with `-RequireGui`. The harnesses forbid
silent skips and retain assertions, preflight results, and trajectory recordings
in `opensky-desktop-*` artifacts for 14 days. Failed jobs remain failures; a
successful build never substitutes for a failed or unavailable desktop lane.

`native` is a diagnostic subset. Stable candidate certification uses `all`
(shared, native, and capture) plus the applicable standalone-browser and platform
lanes described in `test-harnesses-guide.md`. The SDK's consumer E2E drafts live
in the OpenSky repository and are separate from these Rust harnesses. There is
no Codex in-app-browser target.

## macOS GUI work

The existing macOS acceptance runner is
`libs/cua-driver/tests/runners/macos-lume/run-all.sh`. It requires a separate
Apple Silicon Mac running a disposable Lume guest with a logged-in Aqua session,
stable signing, and Accessibility/Screen Recording grants. Its upstream bundle
and install-path assumptions also need adapting to OpenSky before use.

No such remote Mac is configured here. GitHub's macOS build lane does not provision
that GUI environment. Hosted macOS arm64 runners do not support nested
virtualization, so running Lume inside that lane is not a supported solution.
exe.dev's Linux VMs can host persistent Linux testing, but cannot validate macOS
APIs or TCC behavior. Remote Mac GUI acceptance remains an explicit infrastructure
gap, including the macOS-specific items in `opensky-followups.md`.

References: [GitHub runner limitations](https://docs.github.com/en/actions/reference/runners/github-hosted-runners),
[exe.dev documentation](https://exe.dev/docs/all), and the repository's
[test harness guide](test-harnesses-guide.md).
