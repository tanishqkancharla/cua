#!/usr/bin/env bash
# Focused Linux/X11 diagnostic for snapshot-bound AT-SPI text selection.
# The test executable is built once and can be replayed against a separately
# supplied driver binary via CUA_TEST_DRIVER_BIN.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
DRIVER_ROOT="${REPO_ROOT}/libs/cua-driver"
RUST_ROOT="${DRIVER_ROOT}/rust"
ARTIFACT_DIR="${CUA_SELECTION_IDENTITY_ARTIFACT_DIR:-${REPO_ROOT}/artifacts/cua-driver/linux-selection-identity}"
TEST_NAME="harness_gtk3_select_text_binds_the_observed_identity_after_index_drift"

mkdir -p "${ARTIFACT_DIR}" "${ARTIFACT_DIR}/recordings" "${ARTIFACT_DIR}/compiled-test"
: > "${ARTIFACT_DIR}/cases.jsonl"
: > "${ARTIFACT_DIR}/environment.jsonl"
: > "${ARTIFACT_DIR}/results.jsonl"
export CUA_E2E_DECLARATIONS_FILE="${ARTIFACT_DIR}/cases.jsonl"
export CUA_E2E_ENVIRONMENT_FILE="${ARTIFACT_DIR}/environment.jsonl"
export CUA_E2E_RESULTS_FILE="${ARTIFACT_DIR}/results.jsonl"
export CUA_E2E_RECORDINGS_ROOT="${ARTIFACT_DIR}/recordings"
export CUA_TEST_WORKSPACE_ROOT="${RUST_ROOT}"
export CUA_TEST_APPS_ROOT="${RUST_ROOT}/test-apps"
export CUA_TEST_REQUIRE_FIXTURES=1
export CUA_TEST_DRIVER_STDERR=1
export CUA_E2E_FORBID_SKIPS=1
export CUA_E2E_UNRESTRICTED_GUI=1
export CUA_REQUIRE_GUI=1
export CUA_E2E_COMPOSITOR="${CUA_E2E_COMPOSITOR:-openbox-x11}"
export CUA_E2E_INPUT_BACKENDS="${CUA_E2E_INPUT_BACKENDS:-atspi,xsend-event,xtest}"
export CUA_DRIVER_SOURCE_SHA="${CUA_E2E_SOURCE_SHA:-$(git -C "${REPO_ROOT}" rev-parse HEAD)}"

candidate="${RUST_ROOT}/target/release/cua-driver"
(cd "${RUST_ROOT}" && cargo build --locked --release -p cua-driver) \
  2>&1 | tee "${ARTIFACT_DIR}/candidate-build.log"
cp "${candidate}" "${ARTIFACT_DIR}/compiled-test/cua-driver-candidate"
bash "${DRIVER_ROOT}/tests/fixtures/build/linux.sh" --only gtk3,electron \
  2>&1 | tee "${ARTIFACT_DIR}/fixture-build.log"
(cd "${RUST_ROOT}" && cargo test --locked --release -p cua-driver --test harness_gtk3_test \
  --no-run --message-format=json) \
  2>"${ARTIFACT_DIR}/test-build.stderr.log" | tee "${ARTIFACT_DIR}/test-build.jsonl"

test_bin="$(jq -r \
  'select(.reason == "compiler-artifact" and .target.name == "harness_gtk3_test") | .executable' \
  "${ARTIFACT_DIR}/test-build.jsonl" | tail -n 1)"
if [[ -z "${test_bin}" || ! -x "${test_bin}" ]]; then
  echo "focused GTK3 test executable was not built" >&2
  exit 1
fi
cp "${candidate}" "${ARTIFACT_DIR}/compiled-test/cua-driver-candidate"
cp "${test_bin}" "${ARTIFACT_DIR}/compiled-test/harness_gtk3_test"
cp -R "${RUST_ROOT}/test-apps/harness-gtk3" "${ARTIFACT_DIR}/fixtures-harness-gtk3"
cp -R "${RUST_ROOT}/test-apps/harness-electron" "${ARTIFACT_DIR}/fixtures-harness-electron"

run_case() {
  local label="$1"
  local driver_bin="$2"
  local log="${ARTIFACT_DIR}/${label}.log"
  set +e
  CUA_TEST_DRIVER_BIN="${driver_bin}" "${test_bin}" \
    --ignored --exact "${TEST_NAME}" --nocapture --test-threads=1 \
    2>&1 | tee "${log}"
  local status=${PIPESTATUS[0]}
  set -e
  printf '%s\n' "${status}" > "${ARTIFACT_DIR}/${label}.exit-code"
  return "${status}"
}

if ! run_case fixed "${candidate}"; then
  echo "fixed selection-identity diagnostic failed" >&2
  exit 1
fi

baseline_bin="${CUA_SELECTION_IDENTITY_BASELINE_BIN:-}"
baseline_label="${CUA_SELECTION_IDENTITY_BASELINE_LABEL:-not-provided}"
baseline_sha256="${CUA_SELECTION_IDENTITY_BASELINE_SHA256:-}"
baseline_status="not-run"
baseline_uncertain=0
if [[ -n "${baseline_bin}" ]]; then
  if [[ ! -x "${baseline_bin}" ]]; then
    echo "baseline driver is not executable: ${baseline_bin}" >&2
    exit 1
  fi
  actual_baseline_sha256="$(sha256sum "${baseline_bin}" | awk '{print $1}')"
  if [[ -n "${baseline_sha256}" && "${actual_baseline_sha256}" != "${baseline_sha256}" ]]; then
    echo "baseline driver SHA256 mismatch: expected ${baseline_sha256}, got ${actual_baseline_sha256}" >&2
    exit 1
  fi
  cp "${baseline_bin}" "${ARTIFACT_DIR}/compiled-test/cua-driver-baseline-override"
  if run_case baseline-override "${baseline_bin}"; then
    baseline_status="unexpected-pass"
    baseline_uncertain=1
  elif grep -Fq "selection_target=selection-drift-insert" "${ARTIFACT_DIR}/baseline-override.log"; then
    baseline_status="expected-wrong-target"
  else
    baseline_status="uncertain-failure"
    baseline_uncertain=1
  fi
else
  actual_baseline_sha256=""
fi

candidate_sha256="$(sha256sum "${candidate}" | awk '{print $1}')"
test_sha256="$(sha256sum "${test_bin}" | awk '{print $1}')"
fixture_sha256="$(sha256sum "${RUST_ROOT}/test-apps/harness-gtk3/main.py" | awk '{print $1}')"
sentinel_sha256="$(sha256sum "${RUST_ROOT}/test-apps/harness-electron/CuaTestHarness.Electron" | awk '{print $1}')"
sentinel_fixture_sha256="$(find "${RUST_ROOT}/test-apps/harness-electron" -type f -print0 | sort -z | xargs -0 sha256sum | sha256sum | awk '{print $1}')"
jq -n \
  --arg schema "cua-selection-identity-diagnostic-v1" \
  --arg test_source_sha "${CUA_E2E_SOURCE_SHA:-$(git -C "${REPO_ROOT}" rev-parse HEAD)}" \
  --arg candidate_sha256 "${candidate_sha256}" \
  --arg test_sha256 "${test_sha256}" \
  --arg fixture_sha256 "${fixture_sha256}" \
  --arg sentinel_sha256 "${sentinel_sha256}" \
  --arg sentinel_fixture_sha256 "${sentinel_fixture_sha256}" \
  --arg baseline_label "${baseline_label}" \
  --arg baseline_expected_sha256 "${baseline_sha256}" \
  --arg baseline_actual_sha256 "${actual_baseline_sha256}" \
  --arg baseline_status "${baseline_status}" \
  --arg test_name "${TEST_NAME}" \
  '{schema:$schema,test_source_sha:$test_source_sha,candidate_driver_sha256:$candidate_sha256,test_executable_sha256:$test_sha256,gtk3_fixture_sha256:$fixture_sha256,electron_sentinel_launcher_sha256:$sentinel_sha256,electron_sentinel_fixture_sha256:$sentinel_fixture_sha256,baseline:{label:$baseline_label,expected_driver_sha256:$baseline_expected_sha256,actual_driver_sha256:$actual_baseline_sha256,status:$baseline_status},test_name:$test_name}' \
  > "${ARTIFACT_DIR}/identity-receipt.json"
{
  sha256sum \
    "${ARTIFACT_DIR}/compiled-test/cua-driver-candidate" \
    "${ARTIFACT_DIR}/compiled-test/harness_gtk3_test"
  find "${ARTIFACT_DIR}/fixtures-harness-gtk3" \
    "${ARTIFACT_DIR}/fixtures-harness-electron" \
    -type f -print0 | sort -z | xargs -0 sha256sum
} > "${ARTIFACT_DIR}/files.sha256"
cat "${ARTIFACT_DIR}/identity-receipt.json"
if [[ "${baseline_uncertain}" == 1 ]]; then
  echo "baseline replay did not provide the expected wrong-target evidence (${baseline_status})" >&2
  exit 1
fi
