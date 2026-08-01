#/bin/bash
if [ "$1" == "android" ]; then
    flutter/ndk_arm64.sh
    cp target/aarch64-linux-android/release/liblibrustdesk.so flutter/android/app/src/main/jniLibs/arm64-v8a/librustdesk.so
    cd flutter
    flutter build apk --release --target-platform android-arm64 --split-per-abi  --obfuscate --split-debug-info ./split-debug-info
elif [ "$1" == "linux" ]; then
    cargo build --locked --lib --features hwcodec,flutter,unix-file-copy-paste --release
    python3 ./build.py --flutter --skip-cargo
fi
