// ###################
// Media keys and the system media panel (MPRIS on Linux, through souvlaki): the page tells Rust
// what's playing; the keys and the panel's buttons come back to the page as "media-control".
// If the system has no media service, Moonwave just plays without it
// ###################

use serde::{Deserialize, Serialize};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig, SeekDirection};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub struct Media(pub Mutex<Option<MediaControls>>);

// what the page sends: the song, and whether it plays
#[derive(Deserialize, Debug)]
pub struct NowPlaying {
	pub title: String,
	pub artist: String,
	pub album: String,
	pub cover: Option<String>,   // a full path
	pub duration_ms: Option<u64>,
	pub position_ms: u64,
	pub playing: bool,
	pub stopped: bool,
}

// what goes to the page: play / pause / toggle / next / previous / stop / seek (by ms) / position (ms)
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Control {
	pub action: String,
	pub ms: Option<i64>,
}

pub fn to_control(event: MediaControlEvent) -> Option<Control> {
	let simple = |a: &str| Some(Control { action: a.into(), ms: None });
	match event {
		MediaControlEvent::Play => simple("play"),
		MediaControlEvent::Pause => simple("pause"),
		MediaControlEvent::Toggle => simple("toggle"),
		MediaControlEvent::Next => simple("next"),
		MediaControlEvent::Previous => simple("previous"),
		MediaControlEvent::Stop => simple("stop"),
		MediaControlEvent::Seek(d) => Some(Control { action: "seek".into(), ms: Some(if d == SeekDirection::Forward { 10_000 } else { -10_000 }) }),
		MediaControlEvent::SeekBy(d, by) => {
			let ms = by.as_millis() as i64;
			Some(Control { action: "seek".into(), ms: Some(if d == SeekDirection::Forward { ms } else { -ms }) })
		}
		MediaControlEvent::SetPosition(MediaPosition(at)) => Some(Control { action: "position".into(), ms: Some(at.as_millis() as i64) }),
		_ => None,
	}
}

pub fn start(app: &AppHandle) -> Option<MediaControls> {
	let config = PlatformConfig { display_name: "Moonwave", dbus_name: "moonwave", hwnd: None };
	let mut controls = MediaControls::new(config).ok()?;
	let app = app.clone();
	controls
		.attach(move |event| {
			if let Some(control) = to_control(event) {
				let _ = app.emit("media-control", control);
			}
		})
		.ok()?;
	Some(controls)
}

pub fn update(media: &Media, now: &NowPlaying) -> Result<(), String> {
	let mut guard = media.0.lock().map_err(|e| e.to_string())?;
	let Some(controls) = guard.as_mut() else { return Ok(()) };
	let cover = now.cover.as_ref().map(|path| format!("file://{path}"));
	controls
		.set_metadata(MediaMetadata {
			title: Some(&now.title),
			artist: (!now.artist.is_empty()).then_some(now.artist.as_str()),
			album: (!now.album.is_empty()).then_some(now.album.as_str()),
			cover_url: cover.as_deref(),
			duration: now.duration_ms.map(Duration::from_millis),
		})
		.map_err(|e| format!("{e:?}"))?;
	let progress = Some(MediaPosition(Duration::from_millis(now.position_ms)));
	let playback = if now.stopped { MediaPlayback::Stopped } else if now.playing { MediaPlayback::Playing { progress } } else { MediaPlayback::Paused { progress } };
	controls.set_playback(playback).map_err(|e| format!("{e:?}"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn keys_become_page_actions() {
		let action = |e| to_control(e).map(|c| (c.action, c.ms));
		assert_eq!(action(MediaControlEvent::Toggle), Some(("toggle".into(), None)));
		assert_eq!(action(MediaControlEvent::Next), Some(("next".into(), None)));
		assert_eq!(action(MediaControlEvent::Seek(SeekDirection::Backward)), Some(("seek".into(), Some(-10_000))));
		assert_eq!(action(MediaControlEvent::SeekBy(SeekDirection::Forward, Duration::from_secs(5))), Some(("seek".into(), Some(5_000))));
		assert_eq!(action(MediaControlEvent::SetPosition(MediaPosition(Duration::from_millis(61_500)))), Some(("position".into(), Some(61_500))));
		assert_eq!(action(MediaControlEvent::Raise), None);
	}
}
