from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]


def test_installers_never_suggest_or_alias_upstream_releases():
    for name in ("_install-local-rust.sh", "install-local.ps1"):
        text = (SCRIPTS / name).read_text()
        assert "opensky-driver" in text
        assert "https://cua.ai/driver/install" not in text
        assert "RELEASE_BIN=" not in text
        assert "$releaseBinary =" not in text


def test_entry_points_build_from_the_selected_checkout():
    for suffix in ("sh", "ps1"):
        text = (SCRIPTS / f"install.{suffix}").read_text()
        assert f"install-local.{suffix}" in text
        assert "cua.ai" not in text
        assert "releases/download" not in text
