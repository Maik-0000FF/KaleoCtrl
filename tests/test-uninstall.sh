#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────
# Sandbox test suite for uninstall.sh
#
# Runs uninstall.sh against a temporary $HOME and a fake project
# directory that mirrors what install.sh would have created.
# No real system state is touched.
#
# Usage:
#   bash tests/test-uninstall.sh
# ─────────────────────────────────────────────────────────────

set -euo pipefail

# Locate the repo root: the directory containing this script's parent
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
UNINSTALL_SH="$REPO_ROOT/uninstall.sh"

if [ ! -f "$UNINSTALL_SH" ]; then
    echo "ERROR: $UNINSTALL_SH not found" >&2
    exit 1
fi

PASS=0
FAIL=0

pass() { echo "  [PASS] $1"; PASS=$((PASS + 1)); }
fail() { echo "  [FAIL] $1"; FAIL=$((FAIL + 1)); }

setup_sandbox() {
    SANDBOX=$(mktemp -d -t kaleoctrl-uninstall-XXXXXX)
    FAKE_HOME="$SANDBOX/home"
    FAKE_PROJECT="$SANDBOX/project"

    mkdir -p \
        "$FAKE_HOME/.local/bin" \
        "$FAKE_HOME/.local/share/icons/hicolor/256x256/apps" \
        "$FAKE_HOME/.local/share/icons/hicolor/128x128/apps" \
        "$FAKE_HOME/.local/share/icons/hicolor/32x32/apps" \
        "$FAKE_HOME/.local/share/applications"

    printf 'bin\n'             > "$FAKE_HOME/.local/bin/kaleoctrl"
    chmod +x "$FAKE_HOME/.local/bin/kaleoctrl"
    printf 'PNG\n'             > "$FAKE_HOME/.local/share/icons/hicolor/256x256/apps/kaleoctrl.png"
    printf 'PNG\n'             > "$FAKE_HOME/.local/share/icons/hicolor/128x128/apps/kaleoctrl.png"
    printf 'PNG\n'             > "$FAKE_HOME/.local/share/icons/hicolor/32x32/apps/kaleoctrl.png"
    printf '[Desktop Entry]\n' > "$FAKE_HOME/.local/share/applications/kaleoctrl.desktop"

    mkdir -p \
        "$FAKE_PROJECT/models" \
        "$FAKE_PROJECT/src-tauri/target" \
        "$FAKE_PROJECT/node_modules" \
        "$FAKE_PROJECT/dist" \
        "$FAKE_PROJECT/config"
    printf '{}\n'           > "$FAKE_PROJECT/config/settings.json"
    printf 'fake-model\n'   > "$FAKE_PROJECT/models/ggml-test.bin"
    printf 'lock\n'         > "$FAKE_PROJECT/src-tauri/target/dummy"
    printf 'pkg\n'          > "$FAKE_PROJECT/node_modules/dummy"
    printf 'html\n'         > "$FAKE_PROJECT/dist/index.html"

    cp "$UNINSTALL_SH" "$FAKE_PROJECT/uninstall.sh"
    chmod +x "$FAKE_PROJECT/uninstall.sh"
}

teardown_sandbox() {
    rm -rf "$SANDBOX"
}

# Run uninstall.sh from inside the fake project, feeding the given stdin
run_uninstall() {
    local input="$1"
    ( cd "$FAKE_PROJECT" && HOME="$FAKE_HOME" bash uninstall.sh <<< "$input" )
}

assert_missing() {
    local path="$1" label="$2"
    if [ ! -e "$path" ]; then pass "$label removed"; else fail "$label still exists: $path"; fi
}

assert_present() {
    local path="$1" label="$2"
    if [ -e "$path" ]; then pass "$label kept"; else fail "$label unexpectedly removed: $path"; fi
}

# ─────────────────────────────────────────────────────────────
# Test 1 — Mode 1: standard uninstall removes desktop integration,
#                  project files untouched
# ─────────────────────────────────────────────────────────────
echo ""
echo "── Test 1: Mode 1 (Standard) with full install present"
setup_sandbox
run_uninstall "1" > /dev/null

assert_missing "$FAKE_HOME/.local/bin/kaleoctrl"                                          "binary"
assert_missing "$FAKE_HOME/.local/share/icons/hicolor/256x256/apps/kaleoctrl.png"         "icon 256"
assert_missing "$FAKE_HOME/.local/share/icons/hicolor/128x128/apps/kaleoctrl.png"         "icon 128"
assert_missing "$FAKE_HOME/.local/share/icons/hicolor/32x32/apps/kaleoctrl.png"           "icon 32"
assert_missing "$FAKE_HOME/.local/share/applications/kaleoctrl.desktop"                   "desktop entry"
assert_present "$FAKE_PROJECT/models"                                                     "models/"
assert_present "$FAKE_PROJECT/src-tauri/target"                                           "src-tauri/target/"
assert_present "$FAKE_PROJECT/node_modules"                                               "node_modules/"
assert_present "$FAKE_PROJECT/dist"                                                       "dist/"
assert_present "$FAKE_PROJECT/config"                                                     "config/"
teardown_sandbox

# ─────────────────────────────────────────────────────────────
# Test 2 — Mode 1 again on empty system: idempotency, no error
# ─────────────────────────────────────────────────────────────
echo ""
echo "── Test 2: Mode 1 on empty system (idempotency)"
setup_sandbox
# wipe artifacts to simulate "already uninstalled"
rm -rf "$FAKE_HOME/.local/bin/kaleoctrl" \
       "$FAKE_HOME/.local/share/icons/hicolor/"*"/apps/kaleoctrl.png" \
       "$FAKE_HOME/.local/share/applications/kaleoctrl.desktop"

if run_uninstall "1" > /dev/null; then
    pass "exits 0 with nothing to remove"
else
    fail "non-zero exit on idempotent run"
fi
teardown_sandbox

# ─────────────────────────────────────────────────────────────
# Test 3 — Mode 2: y/n answers map exactly to delete/keep
# ─────────────────────────────────────────────────────────────
echo ""
echo "── Test 3: Mode 2 (Deep clean) with mixed y/n answers"
setup_sandbox
# answers: models=y, target=n, node_modules=y, dist=n
run_uninstall "$(printf '2\ny\nn\ny\nn\n')" > /dev/null

assert_missing "$FAKE_HOME/.local/bin/kaleoctrl"   "binary"
assert_missing "$FAKE_PROJECT/models"              "models/"
assert_present "$FAKE_PROJECT/src-tauri/target"    "src-tauri/target/"
assert_missing "$FAKE_PROJECT/node_modules"        "node_modules/"
assert_present "$FAKE_PROJECT/dist"                "dist/"
assert_present "$FAKE_PROJECT/config"              "config/"
teardown_sandbox

# ─────────────────────────────────────────────────────────────
# Test 4 — Mode 3 with "no" to every optional prompt:
#          desktop integration is removed, nothing destructive runs,
#          no sudo invocation is attempted (we'd see permission errors)
# ─────────────────────────────────────────────────────────────
echo ""
echo "── Test 4: Mode 3 (Full wipe) with 'n' to every optional prompt"
setup_sandbox
# answers: models=n, target=n, node_modules=n, dist=n, config=n, system_pkgs=n
run_uninstall "$(printf '3\nn\nn\nn\nn\nn\nn\n')" > /dev/null

assert_missing "$FAKE_HOME/.local/bin/kaleoctrl"   "binary"
assert_present "$FAKE_PROJECT/models"              "models/"
assert_present "$FAKE_PROJECT/config"              "config/"
teardown_sandbox

# ─────────────────────────────────────────────────────────────
# Test 5 — Invalid mode selection must exit non-zero
# ─────────────────────────────────────────────────────────────
echo ""
echo "── Test 5: Invalid mode selection"
setup_sandbox
if run_uninstall "9" > /dev/null 2>&1; then
    fail "expected non-zero exit on invalid selection"
else
    pass "non-zero exit on invalid selection"
fi
teardown_sandbox

# ─────────────────────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────────────────────
echo ""
echo "═══════════════════════════════════════════"
echo "  Results: $PASS passed, $FAIL failed"
echo "═══════════════════════════════════════════"

[ "$FAIL" -eq 0 ]
