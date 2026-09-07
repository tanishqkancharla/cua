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

## Initial hosted evidence — 2026-09-07

The initial diagnostic run is
[34149118620](https://github.com/tanishqkancharla/cua/actions/runs/34149118620),
source `407c83ae8276e5e7df134858e9ad3b9a1506a856`. Linux native E2E passed:
32 delivered actions, seven expected refusals, zero failures, and zero skips.
The environment preflight and video validation passed. The
[Linux native artifact](https://github.com/tanishqkancharla/cua/actions/runs/34149118620/artifacts/10028976101)
contains the generated report, structured results, and recordings. Expected
refusals verify the declared limitations; they do not prove those operations
are supported.

The first build run successfully compiled all three platforms and passed their
driver binary unit suites, then exposed a stale upstream contract-version
assertion. The next run exposed four Windows-only core failures: two Unix-path
fixtures and two startup-instruction word-budget checks. The fixes preserve the
fork contract assertion, use platform-appropriate synthetic paths in both
manifests and attestations, and shorten the Windows documentation pointer.
CI now retains build artifacts even if tests fail and checks the executable's
embedded source SHA. It does not suppress these tests or convert failures into
allowed failures.

The final build/test candidate is `637723da86b3ea42aadf9e12047258a4499d361c`:
[34150504250](https://github.com/tanishqkancharla/cua/actions/runs/34150504250).
The diagnostic GUI evidence above is from the initial SHA, not this later SHA.
Changes afterward are CI/evidence handling, a contract test assertion,
platform-aware test fixtures (including Windows canonical verbatim paths), and
a shorter Windows instruction pointer. No
native input implementation changed, and no full parity certification is claimed.

Windows native E2E also passed at the initial source SHA: 39 delivered actions,
four expected refusals, zero failures, and zero skips. The interactive-session,
UIA/capture preflight and video checks passed. WPF, WinUI3, WebView2, Electron,
launch, and cursor evidence is in the
[Windows native artifact](https://github.com/tanishqkancharla/cua/actions/runs/34149118620/artifacts/10029180653).
The initial workflow run is red because of its stale unit assertion; both
independent desktop jobs are green. Do not summarize the entire initial run as
passing.

Final build/unit result: **all three jobs passed**, with zero test failures or
ignored tests in the selected suites:

| Hosted platform | Driver binary unit | Contract unit | Core unit | Offline source identity |
| --- | ---: | ---: | ---: | --- |
| Ubuntu 24.04 | 188 | 33 | 609 | Exact candidate verified |
| Windows 2025 | 180 | 33 | 604 | Exact candidate verified |
| macOS 26 | 192 | 33 | 608 | Exact candidate verified |

The final run retains one development binary and logs per platform. Desktop
jobs in that build-only run are intentionally unselected; use the two independent
native jobs in the initial diagnostic run for GUI evidence. This record is a
documentation-only addition after the tested commit; it does not recertify other
source revisions or the deferred SDK/driver capability work.
