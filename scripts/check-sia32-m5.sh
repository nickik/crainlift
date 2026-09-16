#!/usr/bin/env bash
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MODE="fast"
PROBE=""
TRACE=0
RUN_INTEGRATION=0

usage() {
    cat <<'EOF'
Usage: scripts/check-sia32-m5.sh [fast|smoke|full|probe <filter>|trace <filter>]

fast            SIA32 build/ISLE gate plus focused SIA32 lowering tests when present.
smoke           fast + registration/lib/encoder smoke tests.
full            smoke + retained ARM64/RISC-V and shared Cranelift checks.
probe <filter>  run only one SIA32 lowering probe after the build/ISLE gate.
trace <filter>  same as probe, with Cranelift/ISLE trace logging enabled.

The command stops at the first failure and keeps generated ISLE Rust and logs
under target/sia32-m5/ for inspection.
EOF
}

if [[ $# -gt 0 ]]; then
    MODE="$1"
    shift
fi

case "$MODE" in
    fast) ;;
    smoke)
        RUN_INTEGRATION=1
        ;;
    full)
        RUN_INTEGRATION=1
        ;;
    probe)
        PROBE="${1:-}"
        if [[ -z "$PROBE" ]]; then
            echo "error: probe mode requires a test filter" >&2
            usage >&2
            exit 2
        fi
        ;;
    trace)
        TRACE=1
        PROBE="${1:-}"
        if [[ -z "$PROBE" ]]; then
            echo "error: trace mode requires a test filter" >&2
            usage >&2
            exit 2
        fi
        ;;
    -h|--help|help)
        usage
        exit 0
        ;;
    *)
        echo "error: unknown mode '$MODE'" >&2
        usage >&2
        exit 2
        ;;
esac

STATE_DIR="${SIA32_M5_STATE_DIR:-$ROOT/target/sia32-m5}"
ISLE_DIR="$STATE_DIR/isle"
LOG_DIR="$STATE_DIR/logs"
TARGET_DIR="${SIA32_M5_TARGET_DIR:-$STATE_DIR/cargo-target}"
mkdir -p "$ISLE_DIR" "$LOG_DIR" "$TARGET_DIR"

FEATURES="std,sia32,isle-errors"
if [[ "$TRACE" == 1 ]]; then
    FEATURES="$FEATURES,trace-log"
    export RUST_LOG="${RUST_LOG:-cranelift_codegen=trace}"
fi

START_SECONDS=$SECONDS
PASSED=()

elapsed() {
    printf '%ss' "$((SECONDS - START_SECONDS))"
}

print_generated_isle_context() {
    local log="$1"
    local generated="$ISLE_DIR/isle_sia32.rs"
    [[ -f "$generated" ]] || return 0

    echo "---- generated SIA32 ISLE signatures ----"
    grep -n -E 'trait Context|fn constructor_(lower|lower_branch)|fn output_reg|fn isle_output' "$generated" | head -n 80 || true

    local location
    location="$(grep -o -m1 -E 'isle_sia32\.rs:[0-9]+' "$log" || true)"
    if [[ -n "$location" ]]; then
        local line
        line="${location##*:}"
        local first=$(( line > 12 ? line - 12 : 1 ))
        local last=$(( line + 12 ))
        echo "---- generated SIA32 ISLE around first compiler error (line $line) ----"
        nl -ba "$generated" | sed -n "${first},${last}p" || true
    fi
}

print_failure_excerpt() {
    local log="$1"
    echo
    echo "---- first useful diagnostic ----"
    grep -n -E 'Error building ISLE files|\.isle:[0-9]+|error(\[|:)|panicked at|FAILED|failures:' "$log" | head -n 40 || true
    print_generated_isle_context "$log"
    echo "---- tail ----"
    tail -n 100 "$log" || true
}

run_stage() {
    local stage="$1"
    shift
    local slug
    slug="$(printf '%s' "$stage" | tr '[:upper:] /' '[:lower:]--' | tr -cd '[:alnum:]_-')"
    local log="$LOG_DIR/${slug}.log"

    echo
    echo "== SIA32 M5: $stage =="
    echo "log: $log"

    set +e
    "$@" 2>&1 | tee "$log"
    local status=${PIPESTATUS[0]}
    set -e

    if [[ $status -ne 0 ]]; then
        echo
        echo "SIA32 M5 FAILURE"
        echo "stage: $stage"
        echo "elapsed: $(elapsed)"
        echo "log: $log"
        if grep -q 'Error building ISLE files' "$log"; then
            echo "classification: ISLE"
        elif grep -Eq 'error(\[|:)' "$log"; then
            echo "classification: RUST/BUILD"
        else
            echo "classification: TEST/EXECUTION"
        fi
        print_failure_excerpt "$log"
        exit "$status"
    fi

    PASSED+=("$stage")
    echo "PASS: $stage ($(elapsed))"
}

# Stage 1 deliberately uses the normal Cranelift build script. This gives us
# the exact production ISLE compilation inputs while isle-errors makes type and
# rule diagnostics useful. ISLE_SOURCE_DIR keeps generated Rust instead of
# hiding it in a transient Cargo OUT_DIR.
run_stage "ISLE + SIA32 build" \
    env \
        ISLE_SOURCE_DIR="$ISLE_DIR" \
        CARGO_TARGET_DIR="$TARGET_DIR" \
        cargo check -p cranelift-codegen --no-default-features --features "$FEATURES"

if [[ -f "$ISLE_DIR/isle_sia32.rs" ]]; then
    echo "generated SIA32 ISLE: $ISLE_DIR/isle_sia32.rs"
elif [[ -f "$ROOT/cranelift/codegen/src/isa/sia32/lower.isle" ]]; then
    echo "warning: lower.isle exists but isle_sia32.rs was not generated; check SIA32 ISLE registration" >&2
fi

LOWERING_TEST="$ROOT/cranelift/codegen/tests/sia32_production.rs"
LOWERING_TEST_NAME="sia32_production"

if [[ -n "$PROBE" ]]; then
    if [[ ! -f "$LOWERING_TEST" ]]; then
        echo
        echo "SIA32 M5 FAILURE"
        echo "stage: LOWERING PROBE"
        echo "probe: $PROBE"
        echo "classification: HARNESS"
        echo "reason: cranelift/codegen/tests/sia32_lowering.rs does not exist yet"
        exit 3
    fi
    run_stage "lowering probe: $PROBE" \
        env CARGO_TARGET_DIR="$TARGET_DIR" RUST_LOG="${RUST_LOG:-}" \
        cargo test -p cranelift-codegen --test "$LOWERING_TEST_NAME" --no-default-features \
            --features "$FEATURES" "$PROBE" -- --exact --nocapture
else
    if [[ -f "$LOWERING_TEST" ]]; then
        run_stage "focused lowering probes" \
            env CARGO_TARGET_DIR="$TARGET_DIR" \
            cargo test -p cranelift-codegen --test "$LOWERING_TEST_NAME" --no-default-features \
                --features "$FEATURES"
    else
        echo "PENDING: focused production lowering probes (sia32_production.rs not added yet)"
    fi
fi

if [[ "$RUN_INTEGRATION" == 1 ]]; then
    run_stage "SIA32 registration" \
        env CARGO_TARGET_DIR="$TARGET_DIR" \
        cargo test -p cranelift-codegen --test sia32_integration --no-default-features \
            --features "$FEATURES"

    run_stage "SIA32 library tests" \
        env CARGO_TARGET_DIR="$TARGET_DIR" \
        cargo test -p cranelift-codegen --lib isa::sia32 --no-default-features \
            --features "$FEATURES"

    run_stage "SIA32 encoding" \
        env CARGO_TARGET_DIR="$TARGET_DIR" \
        cargo test -p cranelift-codegen --test sia32_encode --no-default-features \
            --features "$FEATURES"
fi

if [[ "$MODE" == "full" ]]; then
    run_stage "retained ARM64/RISC-V" \
        env CARGO_TARGET_DIR="$TARGET_DIR" \
        cargo check -p cranelift-codegen --no-default-features --features std,arm64,riscv64

    run_stage "Cranelift frontend" env CARGO_TARGET_DIR="$TARGET_DIR" cargo check -p cranelift-frontend
    run_stage "Cranelift module" env CARGO_TARGET_DIR="$TARGET_DIR" cargo check -p cranelift-module
    run_stage "Cranelift native" env CARGO_TARGET_DIR="$TARGET_DIR" cargo check -p cranelift-native
fi

echo
echo "SIA32 M5 SUMMARY"
echo "-----------------"
for stage in "${PASSED[@]}"; do
    printf 'PASS  %s\n' "$stage"
done
if [[ ! -f "$LOWERING_TEST" ]]; then
    echo "PEND  focused production lowering probes"
fi
echo "elapsed: $(elapsed)"
echo "state: $STATE_DIR"
