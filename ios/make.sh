#!/usr/bin/env bash
set -euo pipefail

cd ../kdeconnectjb
if [[ $# > 0 ]] && [[ "${1}x" == "cleanx" ]]; then
  cargo clean
elif [[ "$*" == *"FINALPACKAGE=1"* ]]; then
  cargo b -r --target aarch64-apple-ios
  cargo r -r --bin generate-headers --features headers -- ../target/kdeconnectjb.h
else
  cargo b --target aarch64-apple-ios
  cargo r --bin generate-headers --features headers -- ../target/kdeconnectjb.h
fi
cd ../ios

make "$@"
