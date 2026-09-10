from __future__ import annotations

import json
import os
import platform
import unittest
from pathlib import Path


def _library_name() -> str:
    if os.name == "nt":
        return "cua_driver_sdk.dll"
    if platform.system() == "Darwin":
        return "libcua_driver_sdk.dylib"
    return "libcua_driver_sdk.so"


LIBRARY = Path(__file__).parents[1] / "src" / "cua_driver" / _library_name()

if LIBRARY.exists():
    from cua_driver import (
        CaptureScope, CuaDriver, DriverExecutionMode, EmbeddedCuaDriverHost,
        EmbeddedDriverHostOptions, EndSessionInput, GetSessionStateInput,
        StartSessionInput,
    )


@unittest.skipUnless(LIBRARY.exists(), "host-native UniFFI library is not staged")
class GeneratedOptionsTests(unittest.TestCase):
    def test_embedded_overlay_option_defaults_false_and_accepts_true(self) -> None:
        required = {
            "binary_path": "/example/cua-driver",
            "host_bundle_id": "com.example.host",
            "socket_path": None,
            "startup_timeout_ms": None,
            "shutdown_timeout_ms": None,
            "permission_mode": None,
            "session_policy_path": None,
            "approve_session_policy": False,
            "dangerously_bypass_approvals": False,
            "environment": [],
            "inherit_stderr": False,
        }

        self.assertFalse(EmbeddedDriverHostOptions(**required).no_overlay)
        self.assertTrue(
            EmbeddedDriverHostOptions(**required, no_overlay=True).no_overlay
        )


@unittest.skipUnless(LIBRARY.exists(), "host-native UniFFI library is not staged")
@unittest.skipIf(os.name == "nt", "Unix socket lifecycle assertions; Windows has a native harness")
class SdkProcessLoaderTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self) -> None:
        default_binary = Path(__file__).parents[2] / "rust" / "target" / "debug" / "cua-driver"
        binary = Path(os.environ.get("CUA_TEST_DRIVER_BIN", str(default_binary))).resolve()
        if not binary.is_file():
            raise RuntimeError(f"Build the real driver or set CUA_TEST_DRIVER_BIN: {binary}")
        self.host = EmbeddedCuaDriverHost.with_options(EmbeddedDriverHostOptions(
            binary_path=str(binary), host_bundle_id="com.example.python-embedded",
            socket_path=None, startup_timeout_ms=None, shutdown_timeout_ms=None,
            permission_mode=None, session_policy_path=None, approve_session_policy=False,
            dangerously_bypass_approvals=False, environment=[], inherit_stderr=True,
            no_overlay=True,
        ))
        self.connection = None
        self.addAsyncCleanup(self.close_host)
        self.connection = await self.host.start()
        self.driver = CuaDriver.connect(self.connection.socket_path)

    async def close_host(self) -> None:
        await self.host.stop()
        if self.connection is not None:
            self.assertFalse(Path(self.connection.socket_path).exists())

    async def test_connect_reports_the_real_owned_daemon_and_tools(self) -> None:
        self.assertTrue((await self.driver.metadata()).embedded)
        self.assertEqual((await self.driver.metadata()).pid, self.connection.pid)
        self.assertEqual((await self.driver.metadata()).host_bundle_id, "com.example.python-embedded")
        self.assertIn("close_window", {tool["name"] for tool in json.loads(await self.driver.list_tools_json())["tools"]})

    async def test_typed_session_lifecycle_uses_the_real_daemon(self) -> None:
        self.assertTrue((await self.driver.start_session(StartSessionInput(session="python-loader", capture_scope=CaptureScope.WINDOW, cursor_theme=None))).active)
        self.assertEqual((await self.driver.get_session_state(GetSessionStateInput(session="python-loader"))).session, "python-loader")
        self.assertFalse((await self.driver.end_session(EndSessionInput(session="python-loader"))).active)

    async def test_stopping_the_host_disconnects_its_sdk_client(self) -> None:
        self.assertTrue(self.driver.is_available())
        await self.host.stop()
        self.assertFalse(self.driver.is_available())


@unittest.skipUnless(LIBRARY.exists(), "host-native UniFFI library is not staged")
class InProcessLoaderTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self) -> None:
        self.driver = CuaDriver.create()
        self.addAsyncCleanup(self.driver.shutdown)

    async def test_generated_python_sdk_can_own_the_runtime_in_process(self) -> None:
        self.assertEqual(self.driver.execution_mode(), DriverExecutionMode.EMBEDDED)
        self.assertEqual(self.driver.socket_path(), "")
        self.assertTrue(self.driver.is_available())
        self.assertTrue((await self.driver.metadata()).embedded)
        self.assertEqual((await self.driver.metadata()).pid, os.getpid())
        await self.driver.shutdown()
        self.assertFalse(self.driver.is_available())


if os.environ.get("CUA_DRIVER_REQUIRE_UNIFFI") == "1" and not LIBRARY.exists():
    raise RuntimeError(f"required staged UniFFI library is missing: {LIBRARY}")


if __name__ == "__main__":
    unittest.main()
