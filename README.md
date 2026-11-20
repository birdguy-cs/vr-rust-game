# vr-rust-game
A simple vr game made in the rust programming language.

Code is partially based on the indite example.

# Building

To build for PC you can just do
```bash
cargo build --release
```

Before running you have to make sure that your OpenXR runtime is active, ie SteamVR, Oculus, Monado, etc.

To build for oculus / OpenXR you gotta have Android SDK and NDK 30 from my testing, w/ ANDROID_HOME and ANDROID_NDK_ROOT set up so the exports script will work.

You also will need to install prerequisites for Android builds which you can set up through:
```bash
curl -L -o openxr_loader.aar \
  https://repo1.maven.org/maven2/org/khronos/openxr/openxr_loader_for_android/1.1.57/openxr_loader_for_android-1.1.57.aar
mkdir -p android/libs/arm64-v8a
unzip -j openxr_loader.aar "jni/arm64-v8a/libopenxr_loader.so" -d android/libs/arm64-v8a/
rm openxr_loader.aar

rustup target add aarch64-linux-android
cargo install cargo-apk
```

The keystore in Cargo.toml pulls from ~/.android/debug.keystore you should probably use your own.

After confirming you have all requisites you can build via:
```bash
source exports
cargo apk build --release --lib
```

After building you can install the apk however you usually will either through adb or sidequest
