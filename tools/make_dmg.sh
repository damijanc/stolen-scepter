#!/usr/bin/env bash
# Build the macOS app bundle and DMG for The Quest for a Stolen Scepter.
#
#   tools/make_dmg.sh [VERSION] [CHANNEL] [MODE]
#
# VERSION defaults to the shell crate's version; CHANNEL is "preview" (the
# default) or "stable". MODE is --all (the default), --app-only or --dmg-only:
#
#   --all        assemble and sign the app, then pack the DMG (a local build)
#   --app-only   build, assemble and sign the app, then stop; the release
#                workflow notarizes the app before a DMG exists
#   --dmg-only   pack an existing dist/Stolen Scepter.app; no rebuild and no
#                re-sign, so the app's signature and ticket stay intact
#
# CODESIGN_IDENTITY picks the signature. Unset or "-" signs ad hoc, which is
# all a preview gets: it runs on the Mac that built it, and elsewhere only
# after the user clears the quarantine. Anything else is a Developer ID, and
# then the inner binary and the bundle are signed with the hardened runtime
# and a timestamp, without --deep.
#
# Writes everything under dist/:
#
#   dist/Stolen Scepter.app      the bundle, pack in Contents/Resources/packs/
#   dist/Stolen-Scepter-<VERSION>-<CHANNEL>-macos-arm64.dmg   (preview)
#   dist/Stolen-Scepter-<VERSION>-macos-arm64.dmg             (stable)
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "${root}"

version="${1:-$(sed -n 's/^version = "\(.*\)"/\1/p' shell/Cargo.toml | head -1)}"
channel="${2:-preview}"
mode="${3:---all}"
app_name="Stolen Scepter"
title="The Quest for a Stolen Scepter"
bundle_id="si.cavar.stolen-scepter"
exe="stolen-scepter"
arch="$(uname -m)"
identity="${CODESIGN_IDENTITY:--}"

case "${channel}" in
  preview|stable) ;;
  *)
    echo "Unknown channel: ${channel} (expected preview or stable)" >&2
    exit 1
    ;;
esac
case "${mode}" in
  --all|--app-only|--dmg-only) ;;
  *)
    echo "Unknown mode: ${mode} (expected --all, --app-only or --dmg-only)" >&2
    exit 1
    ;;
esac

dist="${root}/dist"
app="${dist}/${app_name}.app"
# A stable download is the only one, so it needs no channel in its name.
if [[ "${channel}" == "stable" ]]; then
  dmg="${dist}/Stolen-Scepter-${version}-macos-${arch}.dmg"
else
  dmg="${dist}/Stolen-Scepter-${version}-${channel}-macos-${arch}.dmg"
fi
work="$(mktemp -d "${TMPDIR:-/tmp}/stolen-scepter.XXXXXX")"
trap 'rm -rf "${work}"' EXIT

# Inner code first, then the bundle: --deep would seal whatever it happened
# to find rather than prove the one nested executable is Developer ID signed.
sign_app() {
  if [[ "${identity}" == "-" ]]; then
    codesign --force --deep --sign - "${app}"
  else
    codesign --force --options runtime --timestamp --sign "${identity}" \
      "${app}/Contents/MacOS/${exe}"
    codesign --force --options runtime --timestamp --sign "${identity}" "${app}"
  fi
  codesign --verify --deep --strict "${app}"
}

if [[ "${mode}" != "--dmg-only" ]]; then
  echo "Building ${title} ${version} (${channel}, ${arch})"
  cargo build --release -p openbounty-shell

  rm -rf "${app}" "${dmg}"
  mkdir -p "${app}/Contents/MacOS" "${app}/Contents/Resources/packs"
  cp target/release/openbounty-shell "${app}/Contents/MacOS/${exe}"

  # The pack as the game reads it: game.json, maps, art, audio, palette, and
  # the provenance record. data/ is the builder's source, not needed to play.
  pack_src="packs/stolen-scepter"
  pack_dst="${app}/Contents/Resources/packs/stolen-scepter"
  mkdir -p "${pack_dst}"
  for item in game.json provenance.json maps art audio palettes; do
    cp -R "${pack_src}/${item}" "${pack_dst}/"
  done

  # Icon: drawn straight into an iconset, then packed by iconutil.
  iconset="${work}/AppIcon.iconset"
  python3 tools/gen_icon.py "${iconset}"
  iconutil -c icns "${iconset}" -o "${app}/Contents/Resources/AppIcon.icns"

  cat > "${app}/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>${app_name}</string>
  <key>CFBundleDisplayName</key><string>${title}</string>
  <key>CFBundleIdentifier</key><string>${bundle_id}</string>
  <key>CFBundleExecutable</key><string>${exe}</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.strategy-games</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>MIT License. Music CC0 (see Credits).</string>
</dict>
</plist>
PLIST
  plutil -lint "${app}/Contents/Info.plist" >/dev/null

  sign_app
fi

if [[ "${mode}" == "--app-only" ]]; then
  echo "Wrote ${app#"${root}"/}"
  exit 0
fi

if [[ ! -d "${app}" ]]; then
  echo "Missing ${app#"${root}"/}; run --app-only first." >&2
  exit 1
fi

echo "Packing ${app#"${root}"/} (${channel}, ${arch})"

# The DMG: the app, a link to /Applications to drag it onto, and the licence.
stage="${work}/dmg"
mkdir -p "${stage}"
cp -R "${app}" "${stage}/"
ln -s /Applications "${stage}/Applications"
cp LICENSE "${stage}/LICENSE.txt"
rm -rf "${dmg}"
hdiutil create -volname "${app_name}" -srcfolder "${stage}" -ov -format UDZO "${dmg}" >/dev/null
hdiutil verify "${dmg}" >/dev/null

echo "Wrote ${dmg#"${root}"/}"
