#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Keep this lane small enough to run after every edit. The existing M5 gate
# remains the broader regression check.
export CARGO_TARGET_DIR="${SIA32_ICMP_TARGET_DIR:-$ROOT/target/sia32-icmp}"
FEATURES="std,sia32,isle-errors"

run() {
    echo
    echo "==> $*"
    "$@"
}

# The inherited SIA32 tree is not repository-wide rustfmt-clean. Do not rewrite
# unrelated backend files from this focused compatibility branch.
run cargo check -p cranelift-codegen --no-default-features --features "$FEATURES"
run cargo test -p cranelift-codegen --test sia32_production \
    --no-default-features --features "$FEATURES" \
    sia32_integer_comparisons_compile_to_canonical_booleans -- --exact --nocapture

if [[ "${1:-}" == "full" ]]; then
    run bash scripts/check-sia32-m5.sh full
else
    echo
    echo "Focused icmp gate passed. Run '$0 full' before merge."
fi
