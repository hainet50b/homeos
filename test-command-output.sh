#!/usr/bin/env sh
set -euo pipefail

# homeos command output verification script
# Creates a temporary test environment and exercises all commands.
# Cleans up on exit (including on failure).

export HOMEOS_DATA_DIR="$(mktemp -d)"
HOMEOS="cargo run --"
PKG_DIR="$HOMEOS_DATA_DIR/packages"
YML="$HOMEOS_DATA_DIR/homeos.yml"
STATE="$HOMEOS_DATA_DIR/state.yml"

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
    rm -rf "$HOMEOS_DATA_DIR"
    echo "Done."
}
trap cleanup EXIT

echo "=== homeos init ==="
$HOMEOS init

echo ""
echo "=== homeos init (already initialized) ==="
$HOMEOS init 2>&1 || true

echo ""
echo "=== homeos package add ==="
$HOMEOS package add testpkg
echo "--- verify: directory ---"
ls "$PKG_DIR/testpkg/"
verify

echo ""
echo "=== homeos package add (already exists) ==="
$HOMEOS package add testpkg 2>&1 || true

echo ""
echo "=== homeos package list ==="
$HOMEOS package list

echo ""
echo "=== homeos package info ==="
$HOMEOS package info testpkg

echo ""
echo "=== homeos package cat ==="
$HOMEOS package cat testpkg

echo ""
echo "=== homeos package disable ==="
$HOMEOS package disable testpkg
verify

echo ""
echo "=== homeos package disable (already disabled) ==="
$HOMEOS package disable testpkg

echo ""
echo "=== homeos package enable ==="
$HOMEOS package enable testpkg
verify

echo ""
echo "=== homeos package enable (already enabled) ==="
$HOMEOS package enable testpkg

echo ""
echo "=== homeos package add (with dependency) ==="
$HOMEOS package add deppkg
$HOMEOS package add-dep testpkg deppkg
verify

echo ""
echo "=== homeos package add-dep (already depends) ==="
$HOMEOS package add-dep testpkg deppkg

echo ""
echo "=== homeos package add-dep (dependency not found in homeos.yml) ==="
$HOMEOS package add-dep testpkg nonexistent 2>&1 || true

echo ""
echo "=== homeos package add-dep (circular dependency) ==="
$HOMEOS package add-dep deppkg testpkg 2>&1 || true

echo ""
echo "=== homeos package add-dep (package not found) ==="
$HOMEOS package add-dep nonexistent deppkg 2>&1 || true

echo ""
echo "=== homeos package info (with dependency) ==="
$HOMEOS package info testpkg

echo ""
echo "=== homeos package list (with dependencies) ==="
$HOMEOS package list

echo ""
echo "=== homeos package remove-dep ==="
$HOMEOS package remove-dep testpkg deppkg
verify

echo ""
echo "=== homeos package remove-dep (not a dependency) ==="
$HOMEOS package remove-dep testpkg deppkg

echo ""
echo "=== homeos package add-alias ==="
$HOMEOS package add-alias testpkg update=install
verify

echo ""
echo "=== homeos package add-alias (already exists) ==="
$HOMEOS package add-alias testpkg update=install

echo ""
echo "=== homeos package remove-alias ==="
$HOMEOS package remove-alias testpkg update
verify

echo ""
echo "=== homeos package remove-alias (not found) ==="
$HOMEOS package remove-alias testpkg update

echo ""
echo "=== homeos package remove (depended on) ==="
$HOMEOS package add-dep testpkg deppkg
$HOMEOS package remove deppkg 2>&1 || true
$HOMEOS package remove-dep testpkg deppkg

echo ""
echo "=== homeos package remove (not found) ==="
$HOMEOS package remove nonexistent 2>&1 || true

echo ""
echo "=== homeos package remove ==="
$HOMEOS package remove deppkg --purge <<< "y"
verify
echo "--- verify: directory removed ---"
ls "$PKG_DIR/deppkg/" 2>&1 || echo "(directory does not exist)"

echo ""
echo "=== homeos package remove (abort) ==="
$HOMEOS package add deppkg
$HOMEOS package remove deppkg <<< "n"
echo "--- verify: still in homeos.yml after abort ---"
verify
$HOMEOS package remove deppkg --purge <<< "y"

echo ""
echo "=== homeos plugin list ==="
$HOMEOS plugin list

echo ""
echo "=== homeos plugin add (local) ==="
$HOMEOS plugin add testplugin --local
echo "--- verify: plugin directory ---"
ls "$HOMEOS_DATA_DIR/plugins/testplugin/"
verify

echo ""
echo "=== homeos plugin list (after add) ==="
$HOMEOS plugin list

echo ""
echo "=== homeos plugin cat ==="
$HOMEOS plugin cat testplugin

echo ""
echo "=== homeos plugin remove ==="
$HOMEOS plugin remove testplugin <<< "y"
verify
echo "--- verify: plugin directory still exists (no --purge) ---"
ls "$HOMEOS_DATA_DIR/plugins/testplugin/" 2>&1 || echo "(directory does not exist)"

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
$HOMEOS package install testpkg <<< "y"
verify

echo ""
echo "=== homeos package install (already installed) ==="
$HOMEOS package install testpkg <<< "y"

echo ""
echo "=== homeos package update ==="
$HOMEOS package update testpkg <<< "y"

echo ""
echo "=== homeos package list (after install) ==="
$HOMEOS package list

echo ""
echo "=== homeos package remove (installed, should fail) ==="
$HOMEOS package remove testpkg 2>&1 || true

echo ""
echo "=== homeos package uninstall ==="
$HOMEOS package uninstall testpkg <<< "y"
verify

echo ""
echo "=== homeos package uninstall (not installed) ==="
$HOMEOS package uninstall testpkg <<< "y"

echo ""
echo "=== Setup: write failing script ==="
$HOMEOS package enable testpkg
cat > "$PKG_DIR/testpkg/install.sh" << 'SCRIPT'
#!/usr/bin/env sh
echo "this script will fail"
exit 1
SCRIPT

echo ""
echo "=== homeos package install (script fails) ==="
$HOMEOS package install testpkg <<< "y"
echo "--- verify: not recorded in state.yml after failure ---"
verify

echo ""
echo "=== homeos package install (abort) ==="
$HOMEOS package enable testpkg
$HOMEOS package install testpkg <<< "n"

echo ""
echo "=== Setup: create circular dependency ==="
$HOMEOS package enable testpkg
$HOMEOS package add circpkg
$HOMEOS package add-dep testpkg circpkg
$HOMEOS package add-dep circpkg testpkg
verify

cat > "$PKG_DIR/circpkg/install.sh" << 'SCRIPT'
#!/usr/bin/env sh
echo "circpkg install script executed successfully"
SCRIPT

cat > "$PKG_DIR/testpkg/install.sh" << 'SCRIPT'
#!/usr/bin/env sh
echo "testpkg install script executed successfully"
SCRIPT

echo ""
echo "=== homeos package install (circular dependency) ==="
$HOMEOS package install testpkg circpkg <<< "y"
verify

echo ""
echo "=== Setup: cleanup circular dependency ==="
$HOMEOS package remove-dep testpkg circpkg
$HOMEOS package remove-dep circpkg testpkg
$HOMEOS package remove circpkg --purge <<< "y"

echo ""
echo "=== All tests completed ==="
