// ###################
// Browsing: songs one page at a time (search, sort, or one album / artist), albums and artists.
// An album is its name plus its album artist (or the track artist when there's none), so two
// albums called "Greatest Hits" stay apart. Songs with no album / artist aren't listed there
// ###################

use crate::db::Db;
use rusqlite::{params_from_iter, types::Value};
use serde::{Deserialize, Serialize};

// the artist an album belongs to
const ALBUM_ARTIST: &str = "COALESCE(NULLIF(album_artist, ''), artist)";

#[derive(Deserialize, Debug, Default)]
#[serde(default)]
pub struct SongQuery {
	pub search: String,
	pub sort: String,             // title / artist / album / recent / length / played (only played songs, most first)
	pub album: Option<String>,    // one album: its name …
	pub album_artist: Option<String>, // … and its artist
	pub artist: Option<String>,   // one artist's songs
	pub folder: Option<String>,   // one music folder's songs (its id)
	pub offset: i64,
	pub limit: i64,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Song {
	pub id: String,
	pub title: String,
	pub artist: String,
	pub album: String,
	pub album_artist: String,     // the album's artist (the track artist when there's none)
	pub track_no: Option<i64>,
	pub duration_ms: Option<i64>,
	pub cover: Option<String>,
	pub file: String,
}

#[derive(Serialize, Debug)]
pub struct SongPage {
	pub total: i64,
	pub songs: Vec<Song>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Album {
	pub album: String,
	pub artist: String,
	pub year: Option<i64>,
	pub tracks: i64,
	pub duration_ms: i64,
	pub cover: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Artist {
	pub artist: String,
	pub tracks: i64,
	pub albums: i64,
	pub cover: Option<String>,
}

// title, artist or album contains the search; % and _ are plain characters
fn search_part(search: &str, parts: &mut Vec<String>, values: &mut Vec<Value>) {
	let search = search.trim();
	if search.is_empty() {
		return;
	}
	let like = format!("%{}%", search.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
	parts.push("(title LIKE ? ESCAPE '\\' OR artist LIKE ? ESCAPE '\\' OR album LIKE ? ESCAPE '\\')".into());
	values.extend([Value::from(like.clone()), Value::from(like.clone()), Value::from(like)]);
}

fn where_clause(parts: &[String]) -> String {
	if parts.is_empty() { String::new() } else { format!("WHERE {}", parts.join(" AND ")) }
}

// ###################
// Songs
// ###################
pub fn songs(db: &Db, q: &SongQuery) -> Result<SongPage, String> {
	let mut parts = Vec::new();
	let mut values: Vec<Value> = Vec::new();
	search_part(&q.search, &mut parts, &mut values);
	if let Some(album) = &q.album {
		parts.push(format!("album = ? AND {ALBUM_ARTIST} = ?"));
		values.extend([Value::from(album.clone()), Value::from(q.album_artist.clone().unwrap_or_default())]);
	}
	if let Some(artist) = &q.artist {
		parts.push("artist = ?".into());
		values.push(Value::from(artist.clone()));
	}
	if let Some(folder) = &q.folder {
		parts.push("folder_id = ?".into());
		values.push(Value::from(folder.clone()));
	}
	if q.sort == "played" && q.album.is_none() {
		parts.push("plays > 0".into());
	}
	let filter = where_clause(&parts);
	// an album plays in its own order
	let order = if q.album.is_some() {
		"disc_no IS NULL, disc_no, track_no IS NULL, track_no, title COLLATE NOCASE"
	} else {
		match q.sort.as_str() {
			"artist" => "artist = '', artist COLLATE NOCASE, album COLLATE NOCASE, disc_no, track_no, title COLLATE NOCASE",
			"album" => "album = '', album COLLATE NOCASE, disc_no, track_no, title COLLATE NOCASE",
			"recent" => "added DESC, title COLLATE NOCASE",
			"length" => "duration_ms DESC, title COLLATE NOCASE",
			"played" => "plays DESC, last_played DESC, title COLLATE NOCASE",
			_ => "title COLLATE NOCASE, artist COLLATE NOCASE",
		}
	};
	let conn = db.conn.lock().map_err(|e| e.to_string())?;
	let total: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM tracks {filter}"), params_from_iter(values.iter()), |r| r.get(0)).map_err(|e| e.to_string())?;
	// up to 100,000: the player asks for a whole view at once (its queue)
	let limit = if q.limit > 0 { q.limit.min(100_000) } else { 200 };
	let mut paged = values.clone();
	paged.extend([Value::from(limit), Value::from(q.offset.max(0))]);
	let mut stmt = conn
		.prepare(&format!("SELECT id, title, artist, album, {ALBUM_ARTIST}, track_no, duration_ms, cover, file FROM tracks {filter} ORDER BY {order} LIMIT ? OFFSET ?"))
		.map_err(|e| e.to_string())?;
	let songs = stmt
		.query_map(params_from_iter(paged.iter()), |r| {
			Ok(Song { id: r.get(0)?, title: r.get(1)?, artist: r.get(2)?, album: r.get(3)?, album_artist: r.get(4)?, track_no: r.get(5)?, duration_ms: r.get(6)?, cover: r.get(7)?, file: r.get(8)? })
		})
		.map_err(|e| e.to_string())?
		.collect::<Result<Vec<_>, _>>()
		.map_err(|e| e.to_string())?;
	Ok(SongPage { total, songs })
}

// a song played long enough to count (the page decides when): one more play, and when
pub fn record_play(db: &Db, id: &str) -> Result<(), String> {
	let conn = db.conn.lock().map_err(|e| e.to_string())?;
	conn.execute("UPDATE tracks SET plays = plays + 1, last_played = ?2 WHERE id = ?1", rusqlite::params![id, crate::db::now()]).map_err(|e| e.to_string())?;
	Ok(())
}

// ###################
// Albums: A–Z, or only one artist's (the albums they appear on)
// ###################
pub fn albums(db: &Db, search: &str, artist: Option<&str>) -> Result<Vec<Album>, String> {
	let mut parts = vec!["album <> ''".to_string()];
	let mut values: Vec<Value> = Vec::new();
	search_part(search, &mut parts, &mut values);
	if let Some(artist) = artist {
		parts.push(format!("(artist = ? OR {ALBUM_ARTIST} = ?)"));
		values.extend([Value::from(artist.to_string()), Value::from(artist.to_string())]);
	}
	let conn = db.conn.lock().map_err(|e| e.to_string())?;
	let mut stmt = conn
		.prepare(&format!(
			"SELECT album, {ALBUM_ARTIST} AS who, MAX(year), COUNT(*), COALESCE(SUM(duration_ms), 0), MAX(cover)
			 FROM tracks {} GROUP BY album, who ORDER BY {} album COLLATE NOCASE, who COLLATE NOCASE",
			where_clause(&parts),
			if artist.is_some() { "MAX(year) IS NULL, MAX(year) DESC," } else { "" }
		))
		.map_err(|e| e.to_string())?;
	let albums = stmt
		.query_map(params_from_iter(values.iter()), |r| {
			Ok(Album { album: r.get(0)?, artist: r.get(1)?, year: r.get(2)?, tracks: r.get(3)?, duration_ms: r.get(4)?, cover: r.get(5)? })
		})
		.map_err(|e| e.to_string())?
		.collect::<Result<Vec<_>, _>>()
		.map_err(|e| e.to_string())?;
	Ok(albums)
}

// ###################
// Artists: A–Z, with how many songs and albums, and a cover from one of their albums
// ###################
pub fn artists(db: &Db, search: &str) -> Result<Vec<Artist>, String> {
	let mut parts = vec!["artist <> ''".to_string()];
	let mut values: Vec<Value> = Vec::new();
	search_part(search, &mut parts, &mut values);
	let conn = db.conn.lock().map_err(|e| e.to_string())?;
	let mut stmt = conn
		.prepare(&format!(
			"SELECT artist, COUNT(*), COUNT(DISTINCT NULLIF(album, '')), MAX(cover)
			 FROM tracks {} GROUP BY artist ORDER BY artist COLLATE NOCASE",
			where_clause(&parts)
		))
		.map_err(|e| e.to_string())?;
	let artists = stmt
		.query_map(params_from_iter(values.iter()), |r| Ok(Artist { artist: r.get(0)?, tracks: r.get(1)?, albums: r.get(2)?, cover: r.get(3)? }))
		.map_err(|e| e.to_string())?
		.collect::<Result<Vec<_>, _>>()
		.map_err(|e| e.to_string())?;
	Ok(artists)
}

// ###################
// Tests: a small library with two albums of the same name, a compilation, and loose songs
// ###################
#[cfg(test)]
mod tests {
	use super::*;
	use std::path::PathBuf;

	fn library(name: &str) -> (Db, PathBuf) {
		let folder = std::env::temp_dir().join(format!("moonwave-browse-{name}-{}", std::process::id()));
		std::fs::remove_dir_all(&folder).ok();
		let db = Db::open(&folder).unwrap();
		{
			let conn = db.conn.lock().unwrap();
			// id, title, artist, album, album_artist, disc, track, year, ms, cover, added
			let rows: [(&str, &str, &str, &str, &str, Option<i64>, Option<i64>, Option<i64>, i64, Option<&str>, &str); 8] = [
				("1", "Harbour Lights", "Ayan Noor", "Coastline", "", Some(1), Some(2), Some(2019), 215_000, Some("c1.webp"), "2026-01-01"),
				("2", "Slow River", "Ayan Noor", "Coastline", "", Some(1), Some(1), Some(2019), 242_000, Some("c1.webp"), "2026-01-02"),
				("3", "Night Market", "The Lanterns", "Greatest Hits", "", None, Some(1), Some(2021), 188_000, None, "2026-01-03"),
				("4", "Sea Glass", "Ayan Noor", "Greatest Hits", "", None, Some(1), Some(2024), 200_000, Some("c2.webp"), "2026-01-04"),
				("5", "Mix One", "Lio Grant", "Summer Mix", "Various Artists", None, Some(1), None, 100_000, None, "2026-01-05"),
				("6", "Mix Two", "Ayan Noor", "Summer Mix", "Various Artists", None, Some(2), None, 110_000, None, "2026-01-06"),
				("7", "Demo 100%", "", "", "", None, None, None, 96_000, None, "2026-01-07"),
				("8", "under_score", "Hodan", "", "", None, None, None, 305_000, None, "2026-01-08"),
			];
			for (id, title, artist, album, aa, disc, track, year, ms, cover, added) in rows {
				conn.execute(
					"INSERT INTO tracks (id, file, title, artist, album, album_artist, disc_no, track_no, year, duration_ms, cover, added)
					 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
					rusqlite::params![id, format!("/m/{id}.m4a"), title, artist, album, aa, disc, track, year, ms, cover, added],
				)
				.unwrap();
			}
		}
		(db, folder)
	}

	fn ids(page: &SongPage) -> Vec<&str> {
		page.songs.iter().map(|s| s.id.as_str()).collect()
	}

	#[test]
	fn songs_sort_page_and_search() {
		let (db, folder) = library("songs");
		let q = |sort: &str| SongQuery { sort: sort.into(), ..Default::default() };
		let page = songs(&db, &q("title")).unwrap();
		assert_eq!(page.total, 8);
		assert_eq!(ids(&page), vec!["7", "1", "5", "6", "3", "4", "2", "8"]);
		assert_eq!(ids(&songs(&db, &q("recent")).unwrap())[0], "8");
		assert_eq!(ids(&songs(&db, &q("length")).unwrap())[0], "8");
		assert_eq!(ids(&songs(&db, &q("artist")).unwrap()).last(), Some(&"7"), "no artist goes last");
		let second = songs(&db, &SongQuery { limit: 3, offset: 3, ..Default::default() }).unwrap();
		assert_eq!((second.total, ids(&second)), (8, vec!["6", "3", "4"]));
		let found = songs(&db, &SongQuery { search: "noor".into(), ..Default::default() }).unwrap();
		assert_eq!(found.total, 4, "artist matches");
		assert_eq!(ids(&songs(&db, &SongQuery { search: "100%".into(), ..Default::default() }).unwrap()), vec!["7"], "% is plain text");
		assert_eq!(ids(&songs(&db, &SongQuery { search: "r_s".into(), ..Default::default() }).unwrap()), vec!["8"], "_ matches itself");
		assert_eq!(ids(&songs(&db, &SongQuery { search: "de_".into(), ..Default::default() }).unwrap()), Vec::<&str>::new(), "_ isn't a wildcard (it would match \"der\", \"Dem\")");
		std::fs::remove_dir_all(&folder).ok();
	}

	#[test]
	fn an_album_in_track_order_and_an_artist() {
		let (db, folder) = library("album");
		let album = |name: &str, who: &str| songs(&db, &SongQuery { album: Some(name.into()), album_artist: Some(who.into()), sort: "title".into(), ..Default::default() }).unwrap();
		assert_eq!(ids(&album("Coastline", "Ayan Noor")), vec!["2", "1"], "track order, not titles");
		assert_eq!(ids(&album("Greatest Hits", "The Lanterns")), vec!["3"], "same name, other artist");
		assert_eq!(ids(&album("Summer Mix", "Various Artists")), vec!["5", "6"]);
		assert_eq!(album("Summer Mix", "Various Artists").songs[1].album_artist, "Various Artists", "songs know their album's artist");
		assert_eq!(album("Coastline", "Ayan Noor").songs[0].album_artist, "Ayan Noor", "the track artist when there's none");
		let theirs = songs(&db, &SongQuery { artist: Some("Ayan Noor".into()), ..Default::default() }).unwrap();
		assert_eq!(theirs.total, 4);
		db.conn.lock().unwrap().execute_batch("INSERT INTO folders (id, path, name, added) VALUES ('f', '/f', 'f', 'x'); UPDATE tracks SET folder_id = 'f' WHERE id IN ('2', '7')").unwrap();
		let in_folder = songs(&db, &SongQuery { folder: Some("f".into()), ..Default::default() }).unwrap();
		assert_eq!(ids(&in_folder), vec!["7", "2"], "one folder's songs");
		std::fs::remove_dir_all(&folder).ok();
	}

	#[test]
	fn plays_are_counted() {
		let (db, folder) = library("plays");
		record_play(&db, "1").unwrap();
		record_play(&db, "1").unwrap();
		let (plays, last): (i64, Option<String>) = db.conn.lock().unwrap().query_row("SELECT plays, last_played FROM tracks WHERE id = '1'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
		assert_eq!(plays, 2);
		record_play(&db, "3").unwrap();
		let most = songs(&db, &SongQuery { sort: "played".into(), ..Default::default() }).unwrap();
		assert_eq!(ids(&most), vec!["1", "3"], "only played songs, most first");
		assert_eq!(last.map(|l| l.len()), Some(20));
		std::fs::remove_dir_all(&folder).ok();
	}

	#[test]
	fn albums_and_artists() {
		let (db, folder) = library("groups");
		let all = albums(&db, "", None).unwrap();
		let names: Vec<(&str, &str)> = all.iter().map(|a| (a.album.as_str(), a.artist.as_str())).collect();
		assert_eq!(names, vec![("Coastline", "Ayan Noor"), ("Greatest Hits", "Ayan Noor"), ("Greatest Hits", "The Lanterns"), ("Summer Mix", "Various Artists")]);
		assert_eq!((all[0].tracks, all[0].duration_ms, all[0].year, all[0].cover.as_deref()), (2, 457_000, Some(2019), Some("c1.webp")));
		// an artist's albums: their own (newest first) and the compilation they're on
		let hers = albums(&db, "", Some("Ayan Noor")).unwrap();
		assert_eq!(hers.iter().map(|a| a.album.as_str()).collect::<Vec<_>>(), vec!["Greatest Hits", "Coastline", "Summer Mix"]);
		assert_eq!(albums(&db, "coast", None).unwrap().len(), 1);

		let people = artists(&db, "").unwrap();
		let summary: Vec<(&str, i64, i64)> = people.iter().map(|a| (a.artist.as_str(), a.tracks, a.albums)).collect();
		assert_eq!(summary, vec![("Ayan Noor", 4, 3), ("Hodan", 1, 0), ("Lio Grant", 1, 1), ("The Lanterns", 1, 1)]);
		assert_eq!(artists(&db, "lan").unwrap().len(), 1);
		std::fs::remove_dir_all(&folder).ok();
	}
}
