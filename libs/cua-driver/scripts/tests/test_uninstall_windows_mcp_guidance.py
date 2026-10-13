"""Regression coverage for ownership-safe Windows Claude MCP guidance."""

from pathlib import Path
import sys


REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / ".github/scripts"))
from validate_release_versions import driver_release_uninstaller_path

UNINSTALL_PS1 = driver_release_uninstaller_path(REPO_ROOT, "windows")


def test_windows_guidance_requires_command_ownership_verification() -> None:
    script = UNINSTALL_PS1.read_text(encoding="utf-8-sig")

    assert "Do NOT remove the shared" in script
    assert "verify that the exact user-scope entry's 'command' belongs to this" in script
    assert "Only if that command path is release-owned" in script
    assert "cua-driver-local" in script
    assert "claude mcp remove cua-computer-use -s user" in script
    assert "claude mcp remove cua-driver-rs -s user" in script

    assert "For the older cua-driver-rs name, verify the exact command ownership too" in script
