# kdeconnectjb
KDE Connect implementation in Rust and application for jailbroken/TrollStore iOS

## Building
1. Install [Theos](https://theos.dev)
2. Set up building Rust for iOS; make sure `cargo build --target aarch64-apple-ios` works for a clean Rust project
3. Run `./make.sh do` in `kdeconnectjb` directory to build for iOS and deploy to your configured Theos device
