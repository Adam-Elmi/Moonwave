// ###################
// The live bars on Now Playing: a song's sound, analysed once. ffmpeg decodes it to mono at
// 22,050 Hz; every 1/30 s a Hann-windowed FFT of the last 2,048 samples is cut into 32 bands
// (log-spaced, 40 Hz–10 kHz: bass on the left, treble on the right), each as loudness in dB
// mapped to 0–255 for this song. The window plays these back in step with the song's position,
// so the bars move with the music without touching the sound. Kept in the data folder per song
// and file time, so a song is analysed once
// ###################

use rustfft::{num_complex::Complex, FftPlanner};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const RATE: u32 = 22_050;
pub const FPS: u32 = 30;
pub const BANDS: usize = 32;
const WINDOW: usize = 2048;

// the band edges in FFT bins: log-spaced from 40 Hz to 10 kHz, every band at least one bin wide
pub fn band_edges() -> Vec<usize> {
	let bin = |hz: f64| (hz / RATE as f64 * WINDOW as f64).round() as usize;
	let (lo, hi) = (40f64.ln(), 10_000f64.ln());
	let mut edges: Vec<usize> = (0..=BANDS).map(|i| bin((lo + (hi - lo) * i as f64 / BANDS as f64).exp())).collect();
	for i in 1..edges.len() {
		edges[i] = edges[i].max(edges[i - 1] + 1);
	}
	edges
}

// samples → frames of BANDS values 0–255 (FPS frames a second)
pub fn analyse(samples: &[f32]) -> Vec<u8> {
	let hop = (RATE / FPS) as usize;
	let frames = samples.len() / hop;
	if frames == 0 {
		return Vec::new();
	}
	let mut planner = FftPlanner::<f32>::new();
	let fft = planner.plan_fft_forward(WINDOW);
	let hann: Vec<f32> = (0..WINDOW).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (WINDOW - 1) as f32).cos()).collect();
	let edges = band_edges();
	let mut buf = vec![Complex::new(0.0f32, 0.0); WINDOW];
	let mut db = Vec::with_capacity(frames * BANDS);
	for f in 0..frames {
		let end = (f + 1) * hop;
		for (i, slot) in buf.iter_mut().enumerate() {
			// the WINDOW samples ending at this frame (silence before the start)
			let at = end as isize - WINDOW as isize + i as isize;
			let s = if at >= 0 { samples.get(at as usize).copied().unwrap_or(0.0) } else { 0.0 };
			*slot = Complex::new(s * hann[i], 0.0);
		}
		fft.process(&mut buf);
		for b in 0..BANDS {
			let power = buf[edges[b]..edges[b + 1]].iter().map(|c| c.norm_sqr()).fold(0.0f32, f32::max);
			db.push(10.0 * (power + 1e-9).log10());
		}
	}
	// this song's loud end at 255, 50 dB below it at 0 (quiet parts stay low, not empty)
	let top = db.iter().copied().fold(f32::MIN, f32::max);
	db.iter().map(|&v| (((v - (top - 50.0)) / 50.0).clamp(0.0, 1.0) * 255.0).round() as u8).collect()
}

pub fn cache_path(dir: &Path, id: &str, modified: &str) -> PathBuf {
	let safe: String = format!("{id}-{modified}").chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
	dir.join(format!("{safe}.bin"))
}

// the bytes the window gets: [FPS, BANDS, frame 0's bands, frame 1's, …]
pub fn spectrum(ffmpeg_dir: Option<&Path>, cache: &Path, id: &str, modified: &str, file: &Path) -> Result<Vec<u8>, String> {
	let kept = cache_path(cache, id, modified);
	if let Ok(bytes) = std::fs::read(&kept) {
		if bytes.len() >= 2 && bytes[0] as u32 == FPS && bytes[1] as usize == BANDS {
			return Ok(bytes);
		}
	}
	let ffmpeg = ffmpeg_dir.map(|d| d.join(crate::download::exe("ffmpeg"))).unwrap_or_else(|| PathBuf::from("ffmpeg"));
	let mut child = crate::download::quiet(&mut Command::new(ffmpeg))
		.args(["-v", "error", "-nostdin", "-i"])
		.arg(file)
		.args(["-vn", "-ac", "1", "-ar", &RATE.to_string(), "-f", "f32le", "-"])
		.stdout(Stdio::piped())
		.stderr(Stdio::null())
		.spawn()
		.map_err(|e| format!("Couldn’t start ffmpeg ({e})."))?;
	let mut raw = Vec::new();
	child.stdout.take().unwrap().read_to_end(&mut raw).map_err(|e| e.to_string())?;
	if !child.wait().map(|s| s.success()).unwrap_or(false) || raw.len() < 4 {
		return Err("The song couldn’t be read for its bars.".into());
	}
	let samples: Vec<f32> = raw.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
	let mut bytes = vec![FPS as u8, BANDS as u8];
	bytes.extend(analyse(&samples));
	std::fs::create_dir_all(cache).ok();
	let _ = std::fs::write(&kept, &bytes);
	Ok(bytes)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn tone(hz: f32, seconds: f32) -> Vec<f32> {
		(0..(RATE as f32 * seconds) as usize).map(|n| (std::f32::consts::TAU * hz * n as f32 / RATE as f32).sin() * 0.8).collect()
	}
	fn band_of(hz: f64) -> usize {
		let bin = (hz / RATE as f64 * WINDOW as f64).round() as usize;
		band_edges().windows(2).position(|w| bin >= w[0] && bin < w[1]).unwrap()
	}

	#[test]
	fn bands_cover_bass_to_treble() {
		let e = band_edges();
		assert_eq!(e.len(), BANDS + 1);
		assert!(e.windows(2).all(|w| w[1] > w[0]), "every band has a bin");
		assert!(e[BANDS] < WINDOW / 2);
	}

	#[test]
	fn a_low_tone_lights_the_left_a_high_tone_the_right() {
		for (hz, side) in [(80.0, "left"), (6000.0, "right")] {
			let frames = analyse(&tone(hz, 1.0));
			assert_eq!(frames.len() % BANDS, 0);
			let last = &frames[frames.len() - BANDS..];
			let loudest = last.iter().enumerate().max_by_key(|(_, v)| **v).unwrap().0;
			assert_eq!(loudest, band_of(hz as f64), "{hz} Hz is loudest in its own band ({side})");
		}
	}

	#[test]
	fn silence_then_sound() {
		let mut s = vec![0.0f32; RATE as usize];
		s.extend(tone(440.0, 1.0));
		let frames = analyse(&s);
		assert_eq!(frames.len() / BANDS, (s.len() / (RATE / FPS) as usize));
		let first: u32 = frames[..BANDS].iter().map(|&v| v as u32).sum();
		let late: u32 = frames[frames.len() - BANDS..].iter().map(|&v| v as u32).sum();
		assert_eq!(first, 0, "silence is flat");
		assert!(late > 255, "sound moves the bars");
		assert!(analyse(&[]).is_empty());
	}

	// a real file through ffmpeg (skipped when ffmpeg isn't on PATH)
	#[test]
	fn a_real_file() {
		if Command::new("ffmpeg").arg("-version").output().is_err() {
			return;
		}
		let dir = std::env::temp_dir().join(format!("moonwave-spectrum-{}", std::process::id()));
		std::fs::create_dir_all(&dir).unwrap();
		let file = dir.join("t.wav");
		let ok = Command::new("ffmpeg").args(["-v", "error", "-y", "-f", "lavfi", "-i", "sine=frequency=220:duration=2"]).arg(&file).status().unwrap();
		assert!(ok.success());
		let bytes = spectrum(None, &dir.join("bars"), "t1", "x", &file).unwrap();
		assert_eq!((bytes[0] as u32, bytes[1] as usize), (FPS, BANDS));
		assert!((bytes.len() - 2) / BANDS >= 55, "about 60 frames for 2 s");
		assert!(cache_path(&dir.join("bars"), "t1", "x").exists());
		std::fs::remove_dir_all(&dir).ok();
	}
}
