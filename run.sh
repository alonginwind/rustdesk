#!/bin/bash
if [ "$1" == "android" ]; then
    flutter/ndk_arm64.sh
    cp target/aarch64-linux-android/release/liblibrustdesk.so flutter/android/app/src/main/jniLibs/arm64-v8a/librustdesk.so
    cd flutter
    flutter build apk --release --target-platform android-arm64 --split-per-abi  --obfuscate --split-debug-info ./split-debug-info
elif [ "$1" == "linux" ]; then
    cargo build --locked --lib --features hwcodec,flutter,unix-file-copy-paste --release
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
    cargo rustc --locked --features flutter --lib --release --target=x86_64-pc-windows-gnu -- -L native=$(pwd)/$extra/lib
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
