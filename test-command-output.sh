#!/usr/bin/env sh
set -euo pipefail

# homeos command output verification script
# Creates a temporary test environment and exercises all commands.
# Cleans up on exit (including on failure).

HOMEOS="cargo run --"
TEST_REPO="test-output-$$"
BASE_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/homeos"
REPO_DIR="$BASE_DIR/repos/$TEST_REPO"
PKG_DIR="$REPO_DIR/packages"
YML="$REPO_DIR/homeos.yml"
STATE="$REPO_DIR/state.yml"

verify() {
    echo "--- verify: homeos.yml ---"
    cat "$YML"
    if [ -f "$STATE" ]; then
        echo "--- verify: state.yml ---"
        cat "$STATE"
    fi
}

cleanup() {
    echo ""
    echo "=== Cleanup ==="
    $HOMEOS package uninstall --all --repo "$TEST_REPO" 2>/dev/null || true
    $HOMEOS repo remove "$TEST_REPO" 2>/dev/null || true
    echo "Done."
}
trap cleanup EXIT

echo "=== Setup ==="
$HOMEOS repo add "$TEST_REPO"

echo ""
echo "=== homeos init (already initialized) ==="
$HOMEOS init --repo "$TEST_REPO" 2>&1 || true

echo ""
echo "=== homeos package add ==="
$HOMEOS package add testpkg --repo "$TEST_REPO"
echo "--- verify: directory ---"
ls "$PKG_DIR/testpkg/"
verify

echo ""
echo "=== homeos package add (already exists) ==="
$HOMEOS package add testpkg --repo "$TEST_REPO" 2>&1 || true

echo ""
echo "=== homeos package list ==="
$HOMEOS package list --repo "$TEST_REPO"

echo ""
echo "=== homeos package info ==="
$HOMEOS package info testpkg --repo "$TEST_REPO"

echo ""
echo "=== homeos package cat ==="
$HOMEOS package cat testpkg --repo "$TEST_REPO"

echo ""
echo "=== homeos package disable ==="
$HOMEOS package disable testpkg --repo "$TEST_REPO"
verify

echo ""
echo "=== homeos package disable (already disabled) ==="
$HOMEOS package disable testpkg --repo "$TEST_REPO"

echo ""
echo "=== homeos package enable ==="
$HOMEOS package enable testpkg --repo "$TEST_REPO"
verify

echo ""
echo "=== homeos package enable (already enabled) ==="
$HOMEOS package enable testpkg --repo "$TEST_REPO"

echo ""
echo "=== homeos package add (with dependency) ==="
$HOMEOS package add deppkg --repo "$TEST_REPO"
$HOMEOS package add-dep testpkg deppkg --repo "$TEST_REPO"
verify

echo ""
echo "=== homeos package add-dep (already depends) ==="
$HOMEOS package add-dep testpkg deppkg --repo "$TEST_REPO"

echo ""
echo "=== homeos package add-dep (package not found) ==="
$HOMEOS package add-dep nonexistent deppkg --repo "$TEST_REPO" 2>&1 || true

echo ""
echo "=== homeos package info (with dependency) ==="
$HOMEOS package info testpkg --repo "$TEST_REPO"

echo ""
echo "=== homeos package list (with dependencies) ==="
$HOMEOS package list --repo "$TEST_REPO"

echo ""
echo "=== homeos package remove-dep ==="
$HOMEOS package remove-dep testpkg deppkg --repo "$TEST_REPO"
verify

echo ""
echo "=== homeos package remove-dep (not a dependency) ==="
$HOMEOS package remove-dep testpkg deppkg --repo "$TEST_REPO"

echo ""
echo "=== homeos package add-alias ==="
$HOMEOS package add-alias testpkg update=install --repo "$TEST_REPO"
verify

echo ""
echo "=== homeos package add-alias (already exists) ==="
$HOMEOS package add-alias testpkg update=install --repo "$TEST_REPO"

echo ""
echo "=== homeos package remove-alias ==="
$HOMEOS package remove-alias testpkg update --repo "$TEST_REPO"
verify

echo ""
echo "=== homeos package remove-alias (not found) ==="
$HOMEOS package remove-alias testpkg update --repo "$TEST_REPO"

echo ""
echo "=== homeos package remove (depended on) ==="
$HOMEOS package add-dep testpkg deppkg --repo "$TEST_REPO"
$HOMEOS package remove deppkg --repo "$TEST_REPO" 2>&1 || true
$HOMEOS package remove-dep testpkg deppkg --repo "$TEST_REPO"

echo ""
echo "=== homeos package remove (not found) ==="
$HOMEOS package remove nonexistent --repo "$TEST_REPO" 2>&1 || true

echo ""
echo "=== homeos package remove ==="
$HOMEOS package remove deppkg --purge --repo "$TEST_REPO" <<< "y"
verify
echo "--- verify: directory removed ---"
ls "$PKG_DIR/deppkg/" 2>&1 || echo "(directory does not exist)"

echo ""
echo "=== homeos package remove (abort) ==="
$HOMEOS package add deppkg --repo "$TEST_REPO"
$HOMEOS package remove deppkg --repo "$TEST_REPO" <<< "n"
echo "--- verify: still in homeos.yml after abort ---"
verify
$HOMEOS package remove deppkg --purge --repo "$TEST_REPO" <<< "y"

echo ""
echo "=== homeos plugin list ==="
$HOMEOS plugin list --repo "$TEST_REPO"

echo ""
echo "=== homeos plugin add (local) ==="
$HOMEOS plugin add testplugin --local --repo "$TEST_REPO"
echo "--- verify: plugin directory ---"
ls "$REPO_DIR/plugins/testplugin/"
verify

echo ""
echo "=== homeos plugin list (after add) ==="
$HOMEOS plugin list --repo "$TEST_REPO"

echo ""
echo "=== homeos plugin cat ==="
$HOMEOS plugin cat testplugin --repo "$TEST_REPO"

echo ""
echo "=== homeos plugin remove ==="
$HOMEOS plugin remove testplugin --repo "$TEST_REPO" <<< "y"
verify
echo "--- verify: plugin directory still exists (no --purge) ---"
ls "$REPO_DIR/plugins/testplugin/" 2>&1 || echo "(directory does not exist)"

echo ""
echo "=== Setup: write dummy scripts ==="
cat > "$PKG_DIR/testpkg/install.sh" << 'SCRIPT'
#!/usr/bin/env sh
echo "testpkg install script executed successfully"
SCRIPT

cat > "$PKG_DIR/testpkg/update.sh" << 'SCRIPT'
#!/usr/bin/env sh
echo "testpkg update script executed successfully"
SCRIPT

cat > "$PKG_DIR/testpkg/uninstall.sh" << 'SCRIPT'
#!/usr/bin/env sh
echo "testpkg uninstall script executed successfully"
SCRIPT

echo ""
echo "=== homeos package install ==="
$HOMEOS package install testpkg --repo "$TEST_REPO" <<< "y"
verify

echo ""
echo "=== homeos package install (already installed) ==="
$HOMEOS package install testpkg --repo "$TEST_REPO" <<< "y"

echo ""
echo "=== homeos package update ==="
$HOMEOS package update testpkg --repo "$TEST_REPO" <<< "y"

echo ""
echo "=== homeos package list (after install) ==="
$HOMEOS package list --repo "$TEST_REPO"

echo ""
echo "=== homeos package remove (installed, should fail) ==="
$HOMEOS package remove testpkg --repo "$TEST_REPO" 2>&1 || true

echo ""
echo "=== homeos package uninstall ==="
$HOMEOS package uninstall testpkg --repo "$TEST_REPO" <<< "y"
verify

echo ""
echo "=== homeos package uninstall (not installed) ==="
$HOMEOS package uninstall testpkg --repo "$TEST_REPO" <<< "y"

echo ""
echo "=== Setup: write failing script ==="
$HOMEOS package enable testpkg --repo "$TEST_REPO"
cat > "$PKG_DIR/testpkg/install.sh" << 'SCRIPT'
#!/usr/bin/env sh
echo "this script will fail"
exit 1
SCRIPT

echo ""
echo "=== homeos package install (script fails) ==="
$HOMEOS package install testpkg --repo "$TEST_REPO" <<< "y"
echo "--- verify: not recorded in state.yml after failure ---"
verify

echo ""
echo "=== homeos package install (abort) ==="
$HOMEOS package enable testpkg --repo "$TEST_REPO"
$HOMEOS package install testpkg --repo "$TEST_REPO" <<< "n"

echo ""
echo "=== homeos repo list ==="
$HOMEOS repo list

echo ""
echo "=== homeos repo add (already exists) ==="
$HOMEOS repo add "$TEST_REPO" 2>&1 || true

echo ""
echo "=== homeos repo remove (default) ==="
$HOMEOS repo remove default 2>&1 || true

echo ""
echo "=== All tests completed ==="
