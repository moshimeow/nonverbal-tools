#!/bin/bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PROJECT="$ROOT/keyboard/TwoABC.xcodeproj"
BUILD="$ROOT/keyboard/build"
case "${1:-help}" in
  check)
    mkdir -p "$BUILD"
    xcrun swiftc -module-cache-path "$BUILD/ModuleCache" \
      "$ROOT/keyboard/Sources/KeyMap.swift" "$ROOT/keyboard/Tests/main.swift" -o "$BUILD/keymap-check"
    "$BUILD/keymap-check"
    ;;
  build)
    xcodebuild -project "$PROJECT" -scheme TwoABC -configuration Debug \
      -sdk iphoneos -destination 'generic/platform=iOS' \
      -derivedDataPath "$BUILD" CODE_SIGNING_ALLOWED=NO build
    ;;
  simulator)
    xcodebuild -project "$PROJECT" -scheme TwoABC -configuration Debug \
      -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' \
      -derivedDataPath "$BUILD" CODE_SIGNING_ALLOWED=NO build
    ;;
  test)
    shift
    : "${IOS_SIMULATOR_ID:?Set IOS_SIMULATOR_ID to a simulator UDID from xcrun simctl list devices}"
    xcodebuild -project "$PROJECT" -scheme TwoABC -configuration Debug \
      -destination "platform=iOS Simulator,id=$IOS_SIMULATOR_ID" \
      -derivedDataPath "$BUILD" -parallel-testing-enabled NO CODE_SIGNING_ALLOWED=NO test "$@"
    ;;
  deploy)
    : "${APPLE_TEAM_ID:?Set APPLE_TEAM_ID to your Xcode development team ID}"
    : "${IOS_DEVICE_ID:?Set IOS_DEVICE_ID to your connected iPhone identifier}"
    PHONE_INFO=$(mktemp)
    trap 'rm -f "$PHONE_INFO"' EXIT
    xcrun devicectl device info details --device "$IOS_DEVICE_ID" --json-output "$PHONE_INFO"
    PHONE_UDID=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["result"]["hardwareProperties"]["udid"])' "$PHONE_INFO")
    xcodebuild -project "$PROJECT" -scheme TwoABC -configuration Release \
      -destination "id=$PHONE_UDID" -derivedDataPath "$BUILD/device" \
      -allowProvisioningUpdates -allowProvisioningDeviceRegistration \
      "DEVELOPMENT_TEAM=$APPLE_TEAM_ID" build
    APP="$BUILD/device/Build/Products/Release-iphoneos/TwoABC.app"
    xcrun devicectl device install app --device "$IOS_DEVICE_ID" "$APP"
    xcrun devicectl device process launch --device "$IOS_DEVICE_ID" com.moshi.twoabc
    ;;
  *) echo 'Usage: ./keyboard.sh check | build | simulator | test | deploy' ;;
esac
