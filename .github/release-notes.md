The first release of Moonwave: a music player that lets you download your favorite songs from YouTube.

## What's in it

- Your music folders as one library: songs, albums, artists, search
- Your own lists, plus Recently Added and Most Played
- Now Playing with live bars that move with the music; queue, shuffle, repeat, media keys
- Download a video's audio (M4A, Opus or MP3) into your library, with a connection check and Update downloader
- Light and dark themes, keyboard shortcuts (press `?`)

## Install (Linux)

- **AppImage** (most systems): `chmod +x Moonwave_1.0.0_amd64.AppImage`, then run it
- **Debian / Ubuntu:** `sudo apt install ./Moonwave_1.0.0_amd64.deb`
- **Fedora:** `sudo dnf install ./Moonwave-1.0.0-1.x86_64.rpm`

Moonwave plays audio through GStreamer: for MP3 and M4A install `gst-plugins-good` and `gst-libav` (on Ubuntu: `gstreamer1.0-plugins-good gstreamer1.0-libav`). Downloads need Node.js (yt-dlp uses it for YouTube).

## Note

Downloads are for your own listening. Only download what you have the right to keep.

The packages include [yt-dlp](https://github.com/yt-dlp/yt-dlp) (Unlicense) and a static build of [FFmpeg](https://ffmpeg.org) (GPL) from [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds), whose source code is available there.
