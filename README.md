<p align="center"><img src="moonwave-logo.svg" width="112" alt="Moonwave"></p>

<h1 align="center">Moonwave</h1>

<p align="center">A music player that lets you download your favorite songs from YouTube.</p>

<p align="center">
  <a href="https://github.com/Adam-Elmi/Moonwave/releases/latest"><img alt="Version 1.1.0" src="https://img.shields.io/badge/version-1.1.0-f06595"></a>
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-2f9e44"></a>
  <a href="https://github.com/Adam-Elmi/Moonwave/releases/latest"><img alt="Platform: Linux and Windows" src="https://img.shields.io/badge/platform-Linux%20%7C%20Windows-2f343e"></a>
  <a href="https://github.com/tauri-apps/tauri"><img alt="Tauri 2" src="https://img.shields.io/badge/Tauri-2-24c8db?logo=tauri&logoColor=white"></a>
  <a href="https://github.com/rust-lang/rust"><img alt="Rust" src="https://img.shields.io/badge/Rust-2024-b7410e?logo=rust&logoColor=white"></a>
  <a href="https://github.com/Adam-Elmi/ArcMoon"><img alt="ArcMoon" src="https://img.shields.io/badge/UI-ArcMoon-c2255c"></a>
  <a href="https://github.com/yt-dlp/yt-dlp"><img alt="yt-dlp" src="https://img.shields.io/badge/downloads-yt--dlp-ff0033?logo=youtube&logoColor=white"></a>
</p>

This is my personal music player. It plays my music folders, keeps my own lists, and lets me download my favorite songs from YouTube using [yt-dlp](https://github.com/yt-dlp/yt-dlp) and [ffmpeg](https://ffmpeg.org), straight into my library.

<p align="center"><img src="docs/library.jpg" width="100%" alt="The library: every song, with one playing in the bar at the bottom"></p>

<table>
  <tr>
    <td><img src="docs/now-playing.jpg" alt="Now Playing: the cover, live bars that move with the music, and Up Next"></td>
    <td><img src="docs/download.jpg" alt="Downloading a song: a looked-up video with its title, artist, folder and format"></td>
  </tr>
  <tr>
    <td align="center">Now Playing</td>
    <td align="center">Download</td>
  </tr>
  <tr>
    <td><img src="docs/list.jpg" alt="A list's page: its covers, Play and Shuffle, and its songs"></td>
    <td><img src="docs/artists.jpg" alt="Artists: round pictures with their song counts"></td>
  </tr>
  <tr>
    <td align="center">Your lists</td>
    <td align="center">Artists</td>
  </tr>
  <tr>
    <td colspan="2"><img src="docs/light.jpg" alt="The light theme"></td>
  </tr>
  <tr>
    <td colspan="2" align="center">Light theme</td>
  </tr>
</table>

## Install

Get it from [Releases](https://github.com/Adam-Elmi/Moonwave/releases):

- **Windows:** the setup `.exe` or the `.msi`. Windows may warn that the app is from an unknown publisher (it isn't signed): choose **More info → Run anyway**.
- **Linux:** the AppImage, `.deb` or `.rpm`. For MP3 and M4A install GStreamer's `gst-plugins-good` and `gst-libav`; downloads need Node.js.

## Build

Linux or Windows. You need Node.js and Rust; on Linux also GStreamer's `gst-plugins-good` and `gst-libav`, on Windows Git Bash (it comes with Git) to run the script.

```sh
npm install
./scripts/fetch-tools.sh   # yt-dlp and ffmpeg (and deno on Windows), checked against their checksums
npm run dev                # or: npm run app
```

## Note

The downloader is for your own personal listening. Only download songs you're allowed to: music you own, or music its owner lets people download (for example, free or Creative Commons songs). Downloading other people's music without permission can break copyright law and YouTube's terms.

## License

[MIT](LICENSE) © 2026 Adam Elmi
