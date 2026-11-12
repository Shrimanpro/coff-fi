#!/bin/sh
# Builds Coff-fi for iOS and packages an UNSIGNED .ipa for sideloading tools
# (AltStore, SideStore, Sideloadly), which sign it with your Apple ID when installing.
#
#   npm run ipa:unsigned
#
# Output: src-tauri/gen/apple/build/arm64/Coff-fi-unsigned.ipa
#
# The app is taken from the Xcode archive, not the exported .ipa: with a free Apple ID the
# export step can fail depending on which device Xcode has selected, but the archive is fine.
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD="$ROOT/src-tauri/gen/apple/build"
ARCHIVE_APP="$BUILD/coff-fi_iOS.xcarchive/Products/Applications/Coff-fi.app"
OUT="$BUILD/arm64/Coff-fi-unsigned.ipa"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
touch "$TMP/.start"

cd "$ROOT"
echo "==> Building iOS release…"
if ! npx tauri ios build --export-method debugging; then
  echo "    (export step failed — fine for an unsigned .ipa, using the archive)"
fi

# Make sure we package what was just built, never a stale archive.
if [ ! -d "$ARCHIVE_APP" ] || [ ! "$ARCHIVE_APP/Info.plist" -nt "$TMP/.start" ]; then
  echo "error: the build did not produce a fresh archive; see the errors above" >&2
  exit 1
fi

echo "==> Removing code signature…"
mkdir -p "$TMP/Payload"
cp -R "$ARCHIVE_APP" "$TMP/Payload/"
APP="$TMP/Payload/Coff-fi.app"
find "$APP" \( -name "*.dylib" -o -name "*.framework" -o -name "*.appex" \) -print | while read -r f; do
  codesign --remove-signature "$f" 2>/dev/null || true
done
codesign --remove-signature "$APP/Coff-fi"
rm -rf "$APP/_CodeSignature" "$APP/embedded.mobileprovision"
# Extensions (the lyrics Live Activity widget) carry their own signature and profile.
for X in "$APP"/PlugIns/*.appex; do
  [ -d "$X" ] || continue
  codesign --remove-signature "$X/$(basename "$X" .appex)" 2>/dev/null || true
  rm -rf "$X/_CodeSignature" "$X/embedded.mobileprovision"
done

mkdir -p "$(dirname "$OUT")"
rm -f "$OUT"
(cd "$TMP" && zip -qry "$OUT" Payload)

echo "==> Done: $OUT"
