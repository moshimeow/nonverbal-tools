#!/bin/bash
# All paths are project-local; the system's `dx` may belong to Deno.
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
DX="$ROOT/.tools/dioxus/dx"
MODEL="$ROOT/ios/assets/kokoro-int8-multi-lang-v1_0"
export IPHONEOS_DEPLOYMENT_TARGET=18.0
case "${1:-help}" in
  setup)
    mkdir -p "$ROOT/.tools/dioxus" "$ROOT/.tools/downloads" "$ROOT/ios/assets"
    if [ ! -x "$DX" ]; then
      curl --fail --location --retry 3 https://github.com/DioxusLabs/dioxus/releases/download/v0.7.10/dx-aarch64-apple-darwin.tar.gz -o "$ROOT/.tools/downloads/dx.tar.gz"
      (cd "$ROOT/.tools/downloads"; echo '342c92bc33dca60dadd725ed5e249923f049ae2e1e50b7477f4968d56695b425  dx.tar.gz' | shasum -a 256 -c -)
      tar -xzf "$ROOT/.tools/downloads/dx.tar.gz" -C "$ROOT/.tools/dioxus"
    fi
    rustup target add aarch64-apple-ios aarch64-apple-ios-sim
    if [ ! -f "$MODEL/model.int8.onnx" ]; then
      curl --fail --location --retry 3 https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-int8-multi-lang-v1_0.tar.bz2 -o "$ROOT/.tools/downloads/kokoro.tar.bz2"
      (cd "$ROOT/.tools/downloads"; echo '4c3052abaa60943a341f193888cf6abd68787dae6ab8ae5c925a706caa247e4e  kokoro.tar.bz2' | shasum -a 256 -c -)
      tar -xjf "$ROOT/.tools/downloads/kokoro.tar.bz2" -C "$ROOT/ios/assets"
    fi
    cargo fetch --locked --manifest-path "$ROOT/ios/Cargo.toml"
    ;;
  check)
    shift
    cargo run --locked --release --manifest-path "$ROOT/ios/Cargo.toml" --bin speech-check -- "$@"
    ;;
  desktop)
    cd "$ROOT/ios"
    exec "$DX" serve --desktop --release --bin speak-pocket
    ;;
  build)
    cd "$ROOT/ios"
    "$DX" build --ios --target aarch64-apple-ios --release --bin speak-pocket --locked
    python3 scripts/package.py
    ;;
  deploy)
    : "${APPLE_TEAM_ID:?Set APPLE_TEAM_ID to your Xcode signing team ID}"
    : "${IOS_DEVICE_ID:?Set IOS_DEVICE_ID to the connected iPhone identifier}"
    DEVICE_INFO=$(mktemp)
    trap 'rm -f "$DEVICE_INFO"' EXIT
    xcrun devicectl device info details --device "$IOS_DEVICE_ID" --json-output "$DEVICE_INFO"
    DEVICE_UDID=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["result"]["hardwareProperties"]["udid"])' "$DEVICE_INFO")
    "$ROOT/ios.sh" build
    xcodebuild -project "$ROOT/ios/SpeakPocket.xcodeproj" -scheme SpeakPocket \
      -configuration Release -sdk iphoneos -destination "id=$DEVICE_UDID" \
      -derivedDataPath "$ROOT/ios/target/signing" \
      -allowProvisioningUpdates -allowProvisioningDeviceRegistration \
      "DEVELOPMENT_TEAM=$APPLE_TEAM_ID" clean build
    APP="$ROOT/ios/target/signing/Build/Products/Release-iphoneos/SpeakPocket.app"
    xcrun devicectl device install app --device "$IOS_DEVICE_ID" "$APP"
    xcrun devicectl device process launch --device "$IOS_DEVICE_ID" com.moshi.speakpocket
    ;;
  *)
    echo 'Usage: ./ios.sh setup | check [--play] | desktop | build | deploy'
    echo 'Deploy requires APPLE_TEAM_ID, IOS_DEVICE_ID, and Xcode account/signing setup.'
    ;;
esac
