#!/usr/bin/env bash
# Run Playwright with its browser profiles and internal temp files on disk
# instead of a RAM-backed /tmp. See ethpayserver/e2e/scripts/run-tests.sh for
# the same fix on the server side; this repo's layout tests hit the same
# os.tmpdir() default for the chromium profile.
#
# The trap is the point: an interrupted run (Ctrl-C, a killed job) still
# leaves TMPDIR behind without it, and that is how these accumulate.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

mkdir -p .tmp
scratch="$(mktemp -d .tmp/run-XXXXXX)"
trap 'rm -rf "$scratch"' EXIT

TMPDIR="$(cd "$scratch" && pwd)" npx playwright test "$@"
