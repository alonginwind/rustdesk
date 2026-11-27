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

# windows-core 0.54/0.61 statically imports COM/WinRT entry points that Windows
# 7 does not export. Under MinGW the unused raw-dylib stubs still land in the
# import table, so the DLL fails to load. Resolve them at run time instead;
# MSVC keeps the static import.
patch_windows_core() {
    local file="$1"
    if grep -q 'win7-gnu-resolve' "$file"; then
        echo "ok       $file"
        return
    fi
    if python3 - "$file" <<'PY'
import sys

path = sys.argv[1]
src = open(path).read()

helper = '''// win7-gnu-resolve: Windows 7 lacks these exports, so MinGW builds resolve
// them lazily instead of writing a static import the loader rejects.
#[cfg(target_env = "gnu")]
unsafe fn win7_gnu_resolve(library: &[u8], function: &[u8]) -> FARPROC {
    let module: HMODULE = LoadLibraryExA(library.as_ptr(), core::mem::zeroed(), 0);
    if module == core::mem::zeroed() {
        return None;
    }
    GetProcAddress(module, function.as_ptr())
}
'''

gnu_mta = '''#[cfg(target_env = "gnu")]
pub unsafe fn CoIncrementMTAUsage(pcookie : *mut CO_MTA_USAGE_COOKIE) -> HRESULT {
    let f = core::mem::transmute::<FARPROC, Option<unsafe extern "system" fn(*mut CO_MTA_USAGE_COOKIE) -> HRESULT>>(
        win7_gnu_resolve(b"ole32.dll\\0", b"CoIncrementMTAUsage\\0"),
    );
    match f {
        Some(f) => f(pcookie),
        None => 0x80004005u32 as HRESULT,
    }
}'''

gnu_ro_hstring = '''#[cfg(target_env = "gnu")]
pub unsafe fn RoGetActivationFactory(activatableclassid : HSTRING, iid : *const GUID, factory : *mut *mut core::ffi::c_void) -> HRESULT {
    let f = core::mem::transmute::<FARPROC, Option<unsafe extern "system" fn(HSTRING, *const GUID, *mut *mut core::ffi::c_void) -> HRESULT>>(
        win7_gnu_resolve(b"api-ms-win-core-winrt-l1-1-0.dll\\0", b"RoGetActivationFactory\\0"),
    );
    match f {
        Some(f) => f(activatableclassid, iid, factory),
        None => 0x80004005u32 as HRESULT,
    }
}'''

gnu_ro_ptr = '''#[cfg(target_env = "gnu")]
pub unsafe fn RoGetActivationFactory(activatableclassid : * mut::core::ffi::c_void, iid : *const GUID, factory : *mut *mut ::core::ffi::c_void) -> HRESULT {
    let f = core::mem::transmute::<FARPROC, Option<unsafe extern "system" fn(*mut ::core::ffi::c_void, *const GUID, *mut *mut ::core::ffi::c_void) -> HRESULT>>(
        win7_gnu_resolve(b"api-ms-win-core-winrt-l1-1-0.dll\\0", b"RoGetActivationFactory\\0"),
    );
    match f {
        Some(f) => f(activatableclassid, iid, factory),
        None => 0x80004005u32 as HRESULT,
    }
}'''

targets = [
    ('CoIncrementMTAUsage(pcookie : *mut CO_MTA_USAGE_COOKIE) -> HRESULT);', gnu_mta),
    ('RoGetActivationFactory(activatableclassid : HSTRING, iid : *const GUID, factory : *mut *mut core::ffi::c_void) -> HRESULT);', gnu_ro_hstring),
    ('RoGetActivationFactory(activatableclassid : * mut::core::ffi::c_void, iid : *const GUID, factory : *mut *mut ::core::ffi::c_void) -> HRESULT);', gnu_ro_ptr),
]

changed = 0
for sig, gnu in targets:
    if sig not in src:
        continue
    idx = src.index(sig)
    start = src.rfind('\n', 0, idx) + 1
    end = src.index('\n', idx) + 1
    src = src[:start] + '#[cfg(not(target_env = "gnu"))]\n' + src[start:end] + gnu + '\n' + src[end:]
    changed += 1

if changed == 0:
    print('skip     ' + path + ' (no Win8+ entry points)')
    sys.exit(2)

src = src.rstrip() + '\n\n' + helper + '\n'
open(path, 'w').write(src)
print('patched ' + path)
PY
    then
        PATCHED=1
    fi
}

patch_windows_result() {
    local file="$1"
    if grep -q 'win7-gnu-resolve' "$file"; then
        echo "ok       $file"
        return
    fi
    if ! grep -q 'windows_link::link!("kernel32.dll" "system" fn LoadLibraryExA' "$file"; then
        echo "skip     $file (unsupported layout)"
        return
    fi
    python3 - "$file" <<'PY'
import sys

path = sys.argv[1]
src = open(path).read()

helper = '''// win7-gnu-resolve: Windows 7 lacks this export, so MinGW builds resolve it
// lazily instead of writing a static import the loader rejects.
#[cfg(target_env = "gnu")]
pub type FarProc = Option<unsafe extern "system" fn() -> isize>;
#[cfg(target_env = "gnu")]
windows_link::link!("kernel32.dll" "system" fn GetProcAddress(hmodule : HMODULE, lpprocname : PCSTR) -> FarProc);
#[cfg(target_env = "gnu")]
unsafe fn win7_gnu_resolve(library: &[u8], function: &[u8]) -> FarProc {
    let module = LoadLibraryExA(library.as_ptr(), core::ptr::null_mut(), 0);
    if module.is_null() {
        return None;
    }
    GetProcAddress(module, function.as_ptr())
}
'''

old = 'windows_link::link!("api-ms-win-core-winrt-error-l1-1-0.dll" "system" fn RoOriginateErrorW(error : HRESULT, cchmax : u32, message : PCWSTR) -> BOOL);'
new = '''#[cfg(not(target_env = "gnu"))]
windows_link::link!("api-ms-win-core-winrt-error-l1-1-0.dll" "system" fn RoOriginateErrorW(error : HRESULT, cchmax : u32, message : PCWSTR) -> BOOL);
#[cfg(target_env = "gnu")]
pub unsafe fn RoOriginateErrorW(error : HRESULT, cchmax : u32, message : PCWSTR) -> BOOL {
    let f = core::mem::transmute::<FarProc, Option<unsafe extern "system" fn(HRESULT, u32, PCWSTR) -> BOOL>>(
        win7_gnu_resolve(b"api-ms-win-core-winrt-error-l1-1-0.dll\\0", b"RoOriginateErrorW\\0"),
    );
    match f {
        Some(f) => f(error, cchmax, message),
        None => 0,
    }
}'''

if old not in src:
    sys.exit("cannot patch (unexpected content): " + path)
src = src.replace(old, new, 1)

marker = ')]\n'
idx = src.index(marker) + len(marker)
src = src[:idx] + helper + src[idx:]
open(path, "w").write(src)
print("patched " + path)
PY
    PATCHED=1
}

while IFS= read -r file; do
    patch_windows_core "$file"
done < <(find "$CARGO_HOME/registry/src" -maxdepth 6 -type f \
    -path '*windows-core-*/src/imp/bindings.rs' 2>/dev/null)

while IFS= read -r file; do
    patch_windows_result "$file"
done < <(find "$CARGO_HOME/registry/src" -maxdepth 6 -type f \
    -path '*windows-result-*/src/bindings.rs' 2>/dev/null)

# The windows crate re-declares the same Win8+ entry points inside generated
# functions. Nothing here calls them, but MinGW still emits the import stub, so
# gate the bodies behind cfg and keep the gnu version free of any import.
patch_windows_crate() {
    local file="$1"
    if grep -q 'win7-gnu-resolve' "$file"; then
        echo "ok       $file"
        return
    fi
    python3 - "$file" <<'PY'
import sys

path = sys.argv[1]
src = open(path).read()

com = '''pub unsafe fn CoIncrementMTAUsage() -> windows_core::Result<CO_MTA_USAGE_COOKIE> {
    windows_link::link!("ole32.dll" "system" fn CoIncrementMTAUsage(pcookie : *mut CO_MTA_USAGE_COOKIE) -> windows_core::HRESULT);
    unsafe {
        let mut result__ = core::mem::zeroed();
        CoIncrementMTAUsage(&mut result__).map(|| result__)
    }
}'''

com_gnu = '''// win7-gnu-resolve: Windows 7 does not export this entry point; it is never
// called here, so drop the static import and keep the API shape.
#[cfg(target_env = "gnu")]
pub unsafe fn CoIncrementMTAUsage() -> windows_core::Result<CO_MTA_USAGE_COOKIE> {
    Err(windows_core::Error::from_hresult(windows_core::HRESULT(0x80004005u32 as i32)))
}'''

winrt = '''pub unsafe fn RoGetActivationFactory<T>(activatableclassid: &windows_core::HSTRING) -> windows_core::Result<T>
where
    T: windows_core::Interface,
{
    windows_link::link!("api-ms-win-core-winrt-l1-1-0.dll" "system" fn RoGetActivationFactory(activatableclassid : * mut core::ffi::c_void, iid : *const windows_core::GUID, factory : *mut *mut core::ffi::c_void) -> windows_core::HRESULT);
    let mut result__ = core::ptr::null_mut();
    unsafe { RoGetActivationFactory(core::mem::transmute_copy(activatableclassid), &T::IID, &mut result__).and_then(|| windows_core::Type::from_abi(result__)) }
}'''

winrt_gnu = '''// win7-gnu-resolve: Windows 7 does not export this entry point; it is never
// called here, so drop the static import and keep the API shape.
#[cfg(target_env = "gnu")]
pub unsafe fn RoGetActivationFactory<T>(_activatableclassid: &windows_core::HSTRING) -> windows_core::Result<T>
where
    T: windows_core::Interface,
{
    Err(windows_core::Error::from_hresult(windows_core::HRESULT(0x80004005u32 as i32)))
}'''

if path.endswith('/Com/mod.rs'):
    old, new = com, com_gnu
elif path.endswith('/WinRT/mod.rs'):
    old, new = winrt, winrt_gnu
else:
    sys.exit("unexpected file: " + path)

if old not in src:
    sys.exit("cannot patch (unexpected content): " + path)
src = src.replace(old, '#[cfg(not(target_env = "gnu"))]\n' + old + '\n' + new, 1)
open(path, "w").write(src)
print("patched " + path)
PY
    PATCHED=1
}

while IFS= read -r file; do
    patch_windows_crate "$file"
done < <(find "$CARGO_HOME/registry/src" -maxdepth 12 -type f \
    \( -path '*windows-0.61.1/src/Windows/Win32/System/Com/mod.rs' \
       -o -path '*windows-0.61.1/src/Windows/Win32/System/WinRT/mod.rs' \) 2>/dev/null)

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

# windows-core / windows-result / windows sources are patched in place; cargo
# fingerprints registry crates by package id, so the cross outputs must be
# dropped once after a patch. The stamp remembers that the drop already ran.
STAMP="$ROOT/target/.patch-deps-win7"
if [ "$PATCHED" = 1 ] || [ ! -f "$STAMP" ]; then
    (cd "$ROOT" && cargo clean -p windows-core -p windows-result -p windows \
        --target x86_64-pc-windows-gnu --release || true)
    mkdir -p "$ROOT/target" && touch "$STAMP"
fi

if [ "$PATCHED" = 0 ]; then
    echo "nothing to patch"
else
    echo "patched; run with --clean to force the build scripts to run again"
fi
