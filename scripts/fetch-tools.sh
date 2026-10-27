#!/usr/bin/env bash
# ###################
# The tools Moonwave bundles for downloading (Tauri sidecars in src-tauri/binaries, named with
# the target triple): yt-dlp's standalone build, and static ffmpeg + ffprobe (BtbN builds); on
# Windows also deno (yt-dlp's JavaScript runtime there: most Windows computers have none).
# Linux or Windows (Git Bash, as on GitHub's Windows runners), picked from rustc's host.
# Each download is checked against the checksums its release publishes. Run once before
# building; run again for newer tools. The binaries are not kept in git
# ###################
set -euo pipefail

TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
OUT="$(cd "$(dirname "$0")/.." && pwd)/src-tauri/binaries"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$OUT"
cd "$WORK"

case "$TRIPLE" in
	x86_64-unknown-linux-gnu) YT_ASSET=yt-dlp_linux; FF_NAME=ffmpeg-master-latest-linux64-gpl; EXT="" ;;
	x86_64-pc-windows-msvc) YT_ASSET=yt-dlp.exe; FF_NAME=ffmpeg-master-latest-win64-gpl; EXT=".exe" ;;
	*) echo "No tools for $TRIPLE (Linux and Windows x86_64 only)" >&2; exit 1 ;;
esac

unpack() {   # a .zip or .tar.xz, into the current folder
	case "$1" in
		*.zip) if command -v unzip >/dev/null; then unzip -q "$1"; else 7z x -y "$1" >/dev/null; fi ;;
		*) tar -xJf "$1" --wildcards '*/bin/ff*' ;;
	esac
}

echo "yt-dlp…"
YT="https://github.com/yt-dlp/yt-dlp/releases/latest/download"
curl -fsSL -o "$YT_ASSET" "$YT/$YT_ASSET"
curl -fsSL -o SHA2-256SUMS "$YT/SHA2-256SUMS"
grep " $YT_ASSET\$" SHA2-256SUMS | sha256sum -c -

echo "ffmpeg + ffprobe…"
FF="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest"
FF_FILE="$FF_NAME.$([ -n "$EXT" ] && echo zip || echo tar.xz)"
curl -fsSL -o "$FF_FILE" "$FF/$FF_FILE"
curl -fsSL -o checksums.sha256 "$FF/checksums.sha256"
grep " $FF_FILE\$" checksums.sha256 | sha256sum -c -
unpack "$FF_FILE"

install -m 755 "$YT_ASSET" "$OUT/yt-dlp-$TRIPLE$EXT"
install -m 755 "$FF_NAME/bin/ffmpeg$EXT" "$OUT/ffmpeg-$TRIPLE$EXT"
install -m 755 "$FF_NAME/bin/ffprobe$EXT" "$OUT/ffprobe-$TRIPLE$EXT"

if [ -n "$EXT" ]; then
	echo "deno…"
	DENO="https://github.com/denoland/deno/releases/latest/download"
	DENO_FILE="deno-$TRIPLE.zip"
	curl -fsSL -o "$DENO_FILE" "$DENO/$DENO_FILE"
	curl -fsSL -o deno.sha256sum "$DENO/$DENO_FILE.sha256sum"
	# the checksum file is plain or PowerShell's table: take its one 64-digit hash
	want="$(grep -oiE '[0-9a-f]{64}' deno.sha256sum | head -1 | tr 'A-F' 'a-f')"
	got="$(sha256sum "$DENO_FILE" | cut -d' ' -f1)"
	[ -n "$want" ] && [ "$want" = "$got" ] || { echo "deno's checksum doesn't match" >&2; exit 1; }
	unpack "$DENO_FILE"
	install -m 755 "deno.exe" "$OUT/deno-$TRIPLE.exe"
fi

echo "done:"
ls -lh "$OUT"
"$OUT/yt-dlp-$TRIPLE$EXT" --version
