#/bin/bash
mv ../flutter ../flutter-3.24.5
mv ../flutter-3.22.3 ../flutter
flutter doctor -v
sed -i -e 's/extended_text: 14.0.0/extended_text: 13.0.0/g' flutter/pubspec.yaml
cd flutter
flutter pub get
cd ..
flutter_rust_bridge_codegen --rust-input ./src/flutter_ffi.rs --dart-output ./flutter/lib/generated_bridge.dart --c-output ./flutter/macos/Runner/bridge_generated.h
cp ./flutter/macos/Runner/bridge_generated.h ./flutter/ios/Runner/bridge_generated.h
mv ../flutter ../flutter-3.22.3
mv ../flutter-3.24.5 ../flutter
flutter doctor -v
