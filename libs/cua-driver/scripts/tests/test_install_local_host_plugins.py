from pathlib import Path
import os
import subprocess
import pytest

INSTALLER = Path(__file__).resolve().parents[1] / "_install-local-rust.sh"

@pytest.mark.parametrize("platform,mode", [("Darwin", "release"), ("Darwin", "debug"), ("Linux", "release"), ("Linux", "debug")])
def test_actual_local_build_arguments(platform, mode, tmp_path):
    source = INSTALLER.read_text()
    start = source.index("BUILD_TARGET_DIR=")
    end = source.index("\nBUILT_BINARY=", start)
    env = os.environ.copy()
    env.update(OS=platform, BUILD_CONFIG=mode, REPO_ROOT=str(tmp_path), CARGO_TARGET_DIR=str(tmp_path / "target"), BOLD="", NORMAL="")
    program = 'set -euo pipefail; cargo() { printf "ARG=%s\\n" "$@"; };' + "\n" + source[start:end]
    completed = subprocess.run(["/bin/bash", "-c", program], env=env, capture_output=True, text=True)
    assert completed.returncode == 0, completed.stderr
    actual = [line[4:] for line in completed.stdout.splitlines() if line.startswith("ARG=")]
    expected = ["build"] + (["--release"] if mode == "release" else []) + ["-p", "cua-driver", "-p", "cursor-theme-cli"]
    if platform == "Darwin" and mode == "release":
        expected = ["--config", 'profile.release.build-override.strip="none"'] + expected
    assert actual == expected
