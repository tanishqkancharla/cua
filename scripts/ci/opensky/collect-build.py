"""Retain a source-stamped OpenSky executable without starting its daemon."""
import json
import os
from pathlib import Path
import shutil
import subprocess

suffix = '.exe' if os.name == 'nt' else ''
binary = Path('libs/cua-driver/rust/target/debug') / f'cua-driver{suffix}'
evidence = Path('artifacts/opensky-build')
evidence.mkdir(parents=True, exist_ok=True)
version = subprocess.check_output([str(binary), '--version'], text=True, timeout=30).strip()
assert version.startswith('opensky-driver '), version
identity = json.loads(subprocess.check_output(
    [str(binary), '--opensky-driver-identity'], text=True, timeout=30))
assert identity['product'] == 'opensky-driver', identity
assert identity['protocolVersion'] == 1, identity
assert identity['source'] == os.environ['GITHUB_SHA'], identity
(evidence / 'provenance.json').write_text(json.dumps({
    'sourceSha': os.environ['GITHUB_SHA'],
    'runnerOS': os.environ['RUNNER_OS'],
    'runnerArch': os.environ['RUNNER_ARCH'],
    'version': version,
    'identity': identity,
    'configuration': 'debug; no debug symbols; unsigned development artifact',
    'guiAcceptance': False,
}, indent=2) + '\n')
shutil.copy2(binary, evidence / f'opensky-driver{suffix}')
print(json.dumps(identity))
