#!/usr/bin/env bash
# Patch third-party build scripts in the cargo cache for Linux -> Windows
# cross compilation. Run after a fresh `cargo fetch`, after `cargo update`, or
# whenever a new cargo home is used. Safe to re-run.
#
#   ./patch-deps.sh            # patch only
#   ./patch-deps.sh --clean    # patch and drop the affected build caches

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
PATCHED=0

patch_sodium() {
    local file="$1"
    if grep -q 'CARGO_CFG_TARGET_OS' "$file"; then
        echo "ok       $file"
        return
    fi
    python3 - "$file" <<'PY'
import sys

path = sys.argv[1]
src = open(path).read()
old = '    let lib_dir_isset = env::var("SODIUM_LIB_DIR").is_ok();'
new = (
    '    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();\n'
    '    let lib_dir_isset = target_os == "windows" && env::var("SODIUM_LIB_DIR").is_ok();'
)
if old not in src:
    sys.exit("cannot patch (unexpected content): " + path)
open(path, "w").write(src.replace(old, new, 1))
print("patched " + path)
PY
    PATCHED=1
}

patch_machine_uid() {
    local file="$1"
    if grep -q 'CARGO_CFG_TARGET_OS' "$file"; then
        echo "ok       $file"
        return
    fi
    python3 - "$file" <<'PY'
import sys

path = sys.argv[1]
src = open(path).read()
old = '''    #[cfg(target_os = "windows")]
    {
        println!("cargo:rustc-link-lib=Kernel32");
        Build::new().file("src/win.cpp").compile("machine-uid");
    }'''
new = '''    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-lib=kernel32");
        Build::new().file("src/win.cpp").compile("machine-uid");
    }'''
if old not in src:
    sys.exit("cannot patch (unexpected content): " + path)
open(path, "w").write(src.replace(old, new, 1))
print("patched " + path)
PY
    PATCHED=1
}

while IFS= read -r file; do
    patch_sodium "$file"
done < <(find "$CARGO_HOME/registry/src" -maxdepth 3 -type f \
    -path '*libsodium-sys-0.2.7/build.rs' 2>/dev/null)

while IFS= read -r file; do
    patch_machine_uid "$file"
done < <(find "$CARGO_HOME/git/checkouts" -maxdepth 3 -type f \
    -path '*machine-uid-*/*/build.rs' 2>/dev/null)

if [ "${1:-}" = "--clean" ]; then
    for profile in debug release; do
        rm -rf "$ROOT"/target/x86_64-pc-windows-gnu/$profile/build/machine-uid-* \
               "$ROOT"/target/x86_64-pc-windows-gnu/$profile/deps/*machine_uid* \
               "$ROOT"/target/$profile/build/machine-uid-* \
               "$ROOT"/target/$profile/deps/*machine_uid*
    done
    # libsodium-sys is a registry dependency; drop host and cross outputs for
    # both profiles so a debug build also reruns the patched build script.
    (cd "$ROOT" && for profile in dev release; do
        cargo clean -p libsodium-sys --profile "$profile" 2>/dev/null || true
        cargo clean -p libsodium-sys --profile "$profile" --target x86_64-pc-windows-gnu 2>/dev/null || true
    done)
    echo "cleaned cached builds for machine-uid and libsodium-sys"
fi

if [ "$PATCHED" = 0 ]; then
    echo "nothing to patch"
else
    echo "patched; run with --clean to force the build scripts to run again"
fi
