<p align="center"><img src="moonwave-logo.svg" width="112" alt="Moonwave"></p>

<h1 align="center">Moonwave</h1>

<p align="center">A music player that lets you download your favorite songs from YouTube.</p>

<p align="center">
  <a href="https://github.com/Adam-Elmi/Moonwave/releases"><img alt="Version 1.0.0" src="https://img.shields.io/badge/version-1.0.0-f06595"></a>
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-2f9e44"></a>
  <a href="https://github.com/torvalds/linux"><img alt="Platform: Linux" src="https://img.shields.io/badge/platform-Linux-2f343e?logo=linux&logoColor=white"></a>
  <a href="https://github.com/tauri-apps/tauri"><img alt="Tauri 2" src="https://img.shields.io/badge/Tauri-2-24c8db?logo=tauri&logoColor=white"></a>
  <a href="https://github.com/rust-lang/rust"><img alt="Rust" src="https://img.shields.io/badge/Rust-2024-b7410e?logo=rust&logoColor=white"></a>
  <a href="https://github.com/Adam-Elmi/ArcMoon"><img alt="ArcMoon" src="https://img.shields.io/badge/UI-ArcMoon-c2255c"></a>
  <a href="https://github.com/yt-dlp/yt-dlp"><img alt="yt-dlp" src="https://img.shields.io/badge/downloads-yt--dlp-ff0033?logo=youtube&logoColor=white"></a>
</p>

This is my personal music player. It plays my music folders, keeps my own lists, and lets me download my favorite songs from YouTube using [yt-dlp](https://github.com/yt-dlp/yt-dlp) and [ffmpeg](https://ffmpeg.org), straight into my library.

<p align="center"><img src="docs/library.gif" width="100%" alt="Browsing: songs, albums, an album, artists and a list"></p>
<p align="center"><img src="docs/now-playing.gif" width="100%" alt="Now Playing with live bars that move with the music"></p>
<p align="center"><img src="docs/download.gif" width="100%" alt="Downloading a song: paste a link, check it, download"></p>
<p align="center"><img src="docs/themes.gif" width="100%" alt="Dark and light themes"></p>

<sub>Shown with a made-up sample library.</sub>

## Build

Linux only for now. You need Node.js, Rust, and GStreamer's `gst-plugins-good` and `gst-libav` (for MP3 and M4A).

```sh
npm install
./scripts/fetch-tools.sh   # yt-dlp and ffmpeg, checked against their checksums
npm run dev                # or: npm run app
```

## Note

Downloads are for your own listening. Only download what you have the right to keep.

## License

[MIT](LICENSE) © 2026 Adam Elmi
