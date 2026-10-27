The first release of Moonwave, now for Linux and Windows: a music player that lets you download your favorite songs from YouTube.

## What's in it

- Your music folders as one library: songs, albums, artists, search
- Your own lists, plus Recently Added and Most Played
- Now Playing with live bars that move with the music; queue, shuffle, repeat, media keys
- Download a video's audio (M4A, Opus or MP3) into your library, with a connection check and Update downloader
- A dark theme in near-black with rose and plum (or a light one), keyboard shortcuts (press `?`)
- Fits any window size, from small (360 × 420) to full screen

## Install (Windows)

- **Setup** (`Moonwave_1.1.0_x64-setup.exe`) or **MSI** (`Moonwave_1.1.0_x64_en-US.msi`). Moonwave isn't signed yet, so Windows may say it's from an unknown publisher: choose **More info → Run anyway**. Everything it needs comes with it.

## Install (Linux)

- **AppImage** (most systems): `chmod +x Moonwave_1.1.0_amd64.AppImage`, then run it
- **Debian / Ubuntu:** `sudo apt install ./Moonwave_1.1.0_amd64.deb`
- **Fedora:** `sudo dnf install ./Moonwave-1.1.0-1.x86_64.rpm`

Moonwave plays audio through GStreamer: for MP3 and M4A install `gst-plugins-good` and `gst-libav` (on Ubuntu: `gstreamer1.0-plugins-good gstreamer1.0-libav`). Downloads need Node.js (yt-dlp uses it for YouTube).

## Note

The downloader is for your own personal listening. Only download songs you're allowed to: music you own, or music its owner lets people download (for example, free or Creative Commons songs). Downloading other people's music without permission can break copyright law and YouTube's terms.

The packages include [yt-dlp](https://github.com/yt-dlp/yt-dlp) (Unlicense) and a static build of [FFmpeg](https://ffmpeg.org) (GPL) from [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds), whose source code is available there; the Windows packages also include [deno](https://github.com/denoland/deno) (MIT), which yt-dlp uses for YouTube.
