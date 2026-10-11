"""Retain fresh crash reports from this job's platform-macos test binary."""
import json
import os
from pathlib import Path
import shutil
import time


def collect_reports(report_root, marker, destination, wait_seconds=10):
    since = marker.stat().st_mtime
    deadline = time.monotonic() + wait_seconds
    destination.mkdir(parents=True, exist_ok=True)
    retained = []
    while True:
        for report in sorted(report_root.glob("platform_macos-*")):
            if report.is_symlink() or not report.is_file():
                continue
            if report.suffix not in (".ips", ".crash") or report.stat().st_mtime < since:
                continue
            shutil.copyfile(report, destination / report.name)
            retained.append(report.name)
        if retained or time.monotonic() >= deadline:
            break
        time.sleep(0.5)
    result = {"reports": retained, "sinceMarkerMtime": since,
              "scope": "fresh platform_macos test-binary reports only",
              "acceptance": False}
    (destination / "collection.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))
    return result


if __name__ == "__main__":
    temporary = Path(os.environ["RUNNER_TEMP"])
    collect_reports(Path.home() / "Library/Logs/DiagnosticReports",
                    temporary / "platform-macos-unit-start",
                    temporary / "macos-unit-diagnostics")
