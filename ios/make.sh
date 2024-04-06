#!/usr/bin/env bash
set -euo pipefail

cd ../kdeconnectjb
if [[ $# > 0 ]] && [[ "${1}x" == "cleanx" ]]; then
  cargo clean
fi 

if [[ "$*" == *"FINALPACKAGE=1"* ]]; then
  cargo b -r --target aarch64-apple-ios
else
  cargo b --target aarch64-apple-ios
fi
cargo r --bin generate-headers --features headers -- ../target/kdeconnectjb.h
cd ../ios

make "$@"
