#!/usr/bin/env bash
# ###################
# The tools Moonwave bundles for downloading (Tauri sidecars in src-tauri/binaries, named with
# the target triple): yt-dlp's standalone Linux build, and static ffmpeg + ffprobe (BtbN builds).
# Each download is checked against the checksums its release publishes. Run once before
# building; run again for newer tools. The binaries are not kept in git
# ###################
set -euo pipefail

TRIPLE="x86_64-unknown-linux-gnu"
OUT="$(cd "$(dirname "$0")/.." && pwd)/src-tauri/binaries"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$OUT"
cd "$WORK"

echo "yt-dlp…"
YT="https://github.com/yt-dlp/yt-dlp/releases/latest/download"
curl -fsSL -o yt-dlp_linux "$YT/yt-dlp_linux"
curl -fsSL -o SHA2-256SUMS "$YT/SHA2-256SUMS"
grep ' yt-dlp_linux$' SHA2-256SUMS | sha256sum -c -

echo "ffmpeg + ffprobe…"
FF="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest"
NAME="ffmpeg-master-latest-linux64-gpl"
curl -fsSL -o "$NAME.tar.xz" "$FF/$NAME.tar.xz"
curl -fsSL -o checksums.sha256 "$FF/checksums.sha256"
grep " $NAME.tar.xz$" checksums.sha256 | sha256sum -c -
tar -xJf "$NAME.tar.xz" "$NAME/bin/ffmpeg" "$NAME/bin/ffprobe"

install -m 755 yt-dlp_linux "$OUT/yt-dlp-$TRIPLE"
install -m 755 "$NAME/bin/ffmpeg" "$OUT/ffmpeg-$TRIPLE"
install -m 755 "$NAME/bin/ffprobe" "$OUT/ffprobe-$TRIPLE"
echo "done:"
ls -lh "$OUT"
"$OUT/yt-dlp-$TRIPLE" --version
