"""Synthetic result-oracle checks; importing the runner does not install anything."""

import importlib.util
import os
import re
import shutil
import subprocess
import tempfile
from unittest.mock import patch, Mock
from pathlib import Path
import unittest


spec = importlib.util.spec_from_file_location(
    "driver_first_run", Path(__file__).parents[1] / "verify-driver-first-run-linux.py"
)
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class DisplayResultTests(unittest.TestCase):
    def test_gtk_text_label(self):
        self.assertTrue(runner.displays_42({
            "elements": [{"role": "text", "label": "42", "value": None}]
        }))

    def test_accessible_text_value(self):
        self.assertTrue(runner.displays_42({
            "elements": [{"role": "entry", "label": "Result", "value": "42"}]
        }))

    def test_other_controls_numbers_and_snapshot_indices_do_not_pass(self):
        for element in (
            {"role": "toggle button", "label": "42"},
            {"role": "text", "label": "642"},
            {"role": "text", "label": "7", "element_index": 42},
        ):
            with self.subTest(element=element):
                self.assertFalse(runner.displays_42({
                    "elements": [element], "tree_markdown": '[42] text "7"'
                }))


class InstallationBoundaryReached(RuntimeError):
    pass


class ReleasePreflightTests(unittest.TestCase):
    """Exercise the real preflight, stopping before any installer executes."""

    def fixture(self, destination, *, source_build):
        root = runner.ROOT
        for relative in ("libs/cua-driver", "docs/content/docs/cua-driver/reference"):
            shutil.copytree(root / relative, destination / relative,
                            ignore=shutil.ignore_patterns("target", "node_modules", ".build", ".swiftpm"))
        for relative in (".release-please-manifest.json", "scripts/docs-generators/config.json"):
            path = destination / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(root / relative, path)
        shutil.copytree(root / ".github/release-state", destination / ".github/release-state")
        # Keep an explicit valid lag even after a future installer catches up.
        # These are copied metadata fixtures; no release download is attempted.
        base = destination / "libs/cua-driver"
        unix = base / "scripts/_install-rust.sh"
        unix.write_text(re.sub(r'^CUA_DRIVER_RS_BAKED_VERSION="[^"]+"',
                              'CUA_DRIVER_RS_BAKED_VERSION="0.0.0"', unix.read_text(),
                              count=1, flags=re.MULTILINE))
        windows = base / "scripts/_upstream-install.ps1"
        windows.write_text(re.sub(r'^\$Script:CuaDriverRsBakedVersion\s*=\s*"[^"]+"',
                                 '$Script:CuaDriverRsBakedVersion = "0.0.0"',
                                 windows.read_text(encoding="utf-8-sig"), count=1, flags=re.MULTILINE))
        (destination / ".github/release-state/cua-driver-rs-published-version").write_text("0.0.0\n")
        if not source_build:
            (base / "installer-distribution.json").unlink()
            shutil.copy(base / "scripts/_upstream-install.ps1", base / "scripts/install.ps1")
            shutil.copy(base / "scripts/_install-rust.sh", base / "scripts/install.sh")

    def boundary(self, root, temporary, expected_installer):
        evidence = temporary / "evidence"
        home = temporary / "home"
        home.mkdir()
        sha = "a" * 40
        calls = []

        def read_or_stop(args, **kwargs):
            calls.append(args)
            if args[:2] == ["git", "rev-parse"]:
                output = str(root) if args[-1] == "--show-toplevel" else sha
            elif args[:2] == ["git", "status"]:
                output = ""
            elif args[:2] == ["git", "symbolic-ref"]:
                output = "fixture"
            elif args[:2] == ["pgrep", "-x"]:
                return subprocess.CompletedProcess(args, 1, "", "")
            elif args[0] == "bash":
                self.assertEqual(args, ["bash", str(expected_installer), "--no-modify-path"])
                env = kwargs["env"]
                self.assertEqual(env["CUA_DRIVER_RS_INSTALL_DIR"].rsplit("/", 1)[1], "bin")
                self.assertNotIn("CUA_DRIVER_RS_VERSION", env)
                self.assertNotIn("CUA_DRIVER_VERSION", env)
                self.assertEqual(env.get("HOME"), os.environ.get("HOME"))
                raise InstallationBoundaryReached("installer admission verified; no execution")
            else:
                self.fail(f"Unexpected preflight call: {args}")
            return subprocess.CompletedProcess(args, 0, output, "")

        with patch.object(runner, "ROOT", root), patch.object(runner.sys, "platform", "linux"), \
                patch.object(runner.platform, "machine", return_value="x86_64"), \
                patch.object(runner.os, "geteuid", return_value=1000), \
                patch.object(runner.Path, "home", return_value=home), \
                patch.dict(os.environ, {"GITHUB_ACTIONS": "true", "RUNNER_ENVIRONMENT": "github-hosted",
                                        "RUNNER_TEMP": str(temporary), "CUA_DOCS_EVIDENCE": str(evidence),
                                        "CUA_DOCS_SOURCE_SHA": sha, "CUA_DRIVER_RS_VERSION": "9.9.9"}), \
                patch.object(runner, "run", side_effect=read_or_stop):
            runner.main()

    def test_lagging_published_release_uses_exact_validated_installer(self):
        for source_build in (True, False):
            with self.subTest(source_build=source_build), tempfile.TemporaryDirectory() as directory:
                temporary = Path(directory).resolve()
                root = temporary / "source"
                self.fixture(root, source_build=source_build)
                installer = root / "libs/cua-driver/scripts" / ("_install-rust.sh" if source_build else "install.sh")
                with self.assertRaisesRegex(InstallationBoundaryReached, "installer admission verified"):
                    self.boundary(root, temporary, installer)

    def test_drifted_source_route_is_refused_before_install(self):
        with tempfile.TemporaryDirectory() as directory:
            temporary = Path(directory).resolve()
            root = temporary / "source"
            self.fixture(root, source_build=True)
            (root / "libs/cua-driver/scripts/install.sh").write_text("exit 0\n")
            with self.assertRaisesRegex(RuntimeError, "source-build installer route drift"):
                self.boundary(root, temporary, root / "libs/cua-driver/scripts/_install-rust.sh")


class OwnedCleanupTests(unittest.TestCase):
    def test_launch_binding_refuses_adoption_or_identity_change_without_signalling(self):
        identity = {"pid": 42, "start_time": "100", "executable": "/usr/bin/galculator"}
        with patch.object(runner, "process_identity", return_value=identity), \
                patch.object(runner.os, "pidfd_open", return_value=99, create=True) as opened, \
                patch.object(runner.os, "close") as closed:
            for launch in ({"pid": True, "launcher_pid": True}, {"pid": 42, "launcher_pid": 7}):
                with self.subTest(launch=launch), self.assertRaisesRegex(RuntimeError, "exact newly launched process"):
                    runner.bind_launched_process(launch)
            opened.assert_not_called()
            with patch.object(runner, "process_identity", side_effect=[identity, {**identity, "start_time": "101"}]):
                with self.assertRaisesRegex(RuntimeError, "identity changed"):
                    runner.bind_launched_process({"pid": 42, "launcher_pid": 42})
            closed.assert_called_once_with(99)

    def test_bound_exit_uses_only_the_descriptor_and_observes_exit(self):
        ready = ([99], [], [])
        pending = ([], [], [])
        for observations, expected_signals, failure in (
            ([ready], [], False),
            ([pending, ready], [runner.signal.SIGTERM], False),
            ([pending, pending, ready], [runner.signal.SIGTERM, runner.signal.SIGKILL], False),
            ([pending, pending, pending], [runner.signal.SIGTERM, runner.signal.SIGKILL], True),
        ):
            with self.subTest(observations=observations), \
                    patch.object(runner.select, "select", side_effect=observations), \
                    patch.object(runner.signal, "pidfd_send_signal", create=True) as send, \
                    patch.object(runner.os, "close") as close, \
                    patch.object(runner.os, "kill") as numeric_kill:
                if failure:
                    with self.assertRaisesRegex(RuntimeError, "did not exit after owned cleanup"):
                        runner.stop_bound_process(99)
                else:
                    self.assertTrue(runner.stop_bound_process(99)["exited"])
                self.assertEqual([x.args for x in send.call_args_list], [(99, sig) for sig in expected_signals])
                numeric_kill.assert_not_called()
                close.assert_called_once_with(99)

    def test_unknown_launch_is_not_successful_cleanup(self):
        with tempfile.TemporaryDirectory() as directory:
            receipt = runner.cleanup_fixture(Path(directory), None, [], None, launch_attempted=True)
            self.assertFalse(receipt["passed"])
            self.assertEqual(receipt["errors"], ["Attempted Calculator launch lacks exact cleanup ownership"])

    def test_cleanup_continues_after_error_and_records_all_owned_handles(self):
        process1, process2 = Mock(pid=1), Mock(pid=2)
        process1.poll.return_value = 0
        process2.poll.return_value = None
        mcp = Mock()
        mcp.close.side_effect = RuntimeError("MCP stop refused")
        with tempfile.TemporaryDirectory() as directory, \
                patch.object(runner, "stop", side_effect=[RuntimeError("process stop refused"), None]) as stop, \
                patch.object(runner, "stop_bound_process", return_value={"exited": True}) as bound:
            receipt = runner.cleanup_fixture(Path(directory), mcp, [process1, process2], (99, {"pid": 42}))
            self.assertFalse(receipt["passed"])
            self.assertEqual(len(receipt["errors"]), 2)
            self.assertEqual([x.args for x in stop.call_args_list], [(process2,), (process1,)])
            self.assertEqual(receipt["processes"], [{"pid": 1, "exit_code": 0}])
            bound.assert_called_once_with(99)
            self.assertTrue((Path(directory) / "cleanup.json").is_file())


if __name__ == "__main__":
    unittest.main()
