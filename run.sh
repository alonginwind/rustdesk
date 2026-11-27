#!/bin/bash
if [ "$1" == "android" ]; then
    flutter/ndk_arm64.sh
    cp target/aarch64-linux-android/release/liblibrustdesk.so flutter/android/app/src/main/jniLibs/arm64-v8a/librustdesk.so
    cd flutter
    flutter build apk --release --target-platform android-arm64 --split-per-abi  --obfuscate --split-debug-info ./split-debug-info
elif [ "$1" == "linux" ]; then
    # Apply the VAAPI CQP fallback to the hwcodec checkout pinned by Cargo.lock,
    # so CQP-only drivers (e.g. Gen9.x iHD) can use h264_vaapi. Idempotent; the
    # retry only triggers when the driver rejects bitrate rate control.
    patch_file="$PWD/.github/patches/hwcodec_vaapi_cqp.diff"
    cargo fetch --locked || exit 1
    for d in "${CARGO_HOME:-$HOME/.cargo}"/git/checkouts/hwcodec-*/*/; do
        if [ -f "$d/cpp/ffmpeg_ram/ffmpeg_ram_encode.cpp" ] && \
           git -C "$d" rev-parse -q --verify HEAD 2>/dev/null | grep -q '^ec38ebaf2454269d4c5668a57e3813c6e5541be7$' && \
           ! grep -q 'cqp_retry_' "$d/cpp/ffmpeg_ram/ffmpeg_ram_encode.cpp" 2>/dev/null; then
            git -C "$d" apply "$patch_file" || exit 1
            echo "patched hwcodec for VAAPI CQP-only drivers"
        fi
    done
    cargo build --locked --lib --features hwcodec,flutter,unix-file-copy-paste,drm,drm-wake --release
    python3 ./build.py --flutter --skip-cargo
elif [ "$1" == "windows" ]; then
    if [ ! -f "target/libsodium/build/lib/libsodium.a" ]; then
        ./patch-deps.sh --clean
        mkdir -p target
        ./sodium.sh
    fi
    # sas.dll/newdev.dll import libs are missing from mingw-w64
    extra=target/mingw-extra
    mkdir -p "$extra/lib"
    if [ ! -f "target/mingw-extra/lib/libsas.a" ]; then
        printf 'LIBRARY sas.dll\nEXPORTS\nSendSAS\n' > "$extra/sas.def"
        x86_64-w64-mingw32-dlltool -d "$extra/sas.def" -l "$extra/lib/libsas.a" -k
    fi
    if [ ! -f "target/mingw-extra/lib/libNewdev.a" ]; then
        printf 'LIBRARY newdev.dll\nEXPORTS\nUpdateDriverForPlugAndPlayDevicesA\nUpdateDriverForPlugAndPlayDevicesW\nInstallUpdate\n' > "$extra/newdev.def"
        x86_64-w64-mingw32-dlltool -d "$extra/newdev.def" -l "$extra/lib/libNewdev.a" -k
    fi
    export SODIUM_LIB_DIR=$(pwd)/target/libsodium/build/lib
    # Resolve Win8+ COM/WinRT entry points at run time under gnu so the DLL
    # still loads on Windows 7. Idempotent; drops affected cross builds once.
    ./patch-deps.sh
    cargo rustc --locked --features flutter --lib --release --target=x86_64-pc-windows-gnu -- \
        -L native=$(pwd)/$extra/lib \
        -C link-arg=-static

    # CI links the MSVC CRT statically; report any import that Windows 7 still
    # lacks so a stale build is not mistaken for a Win7-capable one.
    dll=target/x86_64-pc-windows-gnu/release/librustdesk.dll
    imports=$(x86_64-w64-mingw32-objdump -p "$dll")
    for sym in CoIncrementMTAUsage RoGetActivationFactory RoOriginateErrorW; do
        if echo "$imports" | grep -q " $sym$"; then
            echo "warning: $dll imports $sym, which Windows 7 does not provide" >&2
        fi
    done
elif [ "$1" == "aarch64" ]; then
    export PKG_CONFIG_SYSROOT_DIR=$HOME/Documents/sysroot
    cargo build --locked --lib --features hwcodec,flutter,unix-file-copy-paste --release --target=aarch64-unknown-linux-gnu
elif [ "$1" == "web" ]; then
    rm -rf flutter/build/web/*
    pushd libs/web_bridge
    wasm-pack build --release --target web --out-dir ../../flutter/web/pkg
    popd
    python3 ./extract_translations.py
    pushd flutter
    rm web/pkg/.gitignore
    rm web/pkg/package.json
    rm web/pkg/web_bridge.d.ts
    rm web/pkg/web_bridge_bg.wasm.d.ts
    flutter build web --release
    rm build/web/load_bridge.tpl.js
    rm -rf web/pkg/
    rm web/load_bridge.js
    sed -i 's/\r$//' build/web/assets/NOTICES
    popd
fi
