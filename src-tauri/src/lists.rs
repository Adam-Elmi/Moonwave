// ###################
// Your lists: a name and songs in your order. A song is in a list once (adding it again does
// nothing); reordering and removing renumber the positions. Deleting a list never touches songs
// ###################

use crate::browse::Song;
use crate::db::{now, Db};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

#[derive(Serialize, Debug, PartialEq)]
pub struct List {
	pub id: String,
	pub name: String,
	pub tracks: i64,
	pub duration_ms: i64,
	pub covers: Vec<String>,   // up to four different covers, for the list's picture
	pub updated: String,
}

fn clean(name: &str) -> Result<String, String> {
	let name = name.trim();
	if name.is_empty() {
		return Err("Give the list a name.".into());
	}
	Ok(name.chars().take(100).collect())
}

pub fn all(db: &Db) -> Result<Vec<List>, String> {
	let conn = db.conn.lock().map_err(|e| e.to_string())?;
	let mut stmt = conn
		.prepare(
			"SELECT l.id, l.name, COUNT(t.id), COALESCE(SUM(t.duration_ms), 0), l.updated
			 FROM lists l LEFT JOIN list_tracks lt ON lt.list_id = l.id LEFT JOIN tracks t ON t.id = lt.track_id
			 GROUP BY l.id ORDER BY l.name COLLATE NOCASE",
		)
		.map_err(|e| e.to_string())?;
	let mut lists = stmt
		.query_map([], |r| Ok(List { id: r.get(0)?, name: r.get(1)?, tracks: r.get(2)?, duration_ms: r.get(3)?, covers: Vec::new(), updated: r.get(4)? }))
		.map_err(|e| e.to_string())?
		.collect::<Result<Vec<_>, _>>()
		.map_err(|e| e.to_string())?;
	let mut covers = conn
		.prepare(
			"SELECT t.cover FROM list_tracks lt JOIN tracks t ON t.id = lt.track_id
			 WHERE lt.list_id = ?1 AND t.cover IS NOT NULL GROUP BY t.cover ORDER BY MIN(lt.position) LIMIT 4",
		)
		.map_err(|e| e.to_string())?;
	for list in &mut lists {
		list.covers = covers.query_map([&list.id], |r| r.get(0)).map_err(|e| e.to_string())?.collect::<Result<_, _>>().map_err(|e| e.to_string())?;
	}
	Ok(lists)
}

pub fn create(db: &Db, name: &str) -> Result<String, String> {
	let name = clean(name)?;
	let id = uuid::Uuid::new_v4().to_string();
	let at = now();
	let conn = db.conn.lock().map_err(|e| e.to_string())?;
	conn.execute("INSERT INTO lists (id, name, created, updated) VALUES (?1, ?2, ?3, ?3)", params![id, name, at]).map_err(|e| e.to_string())?;
	Ok(id)
}

pub fn rename(db: &Db, id: &str, name: &str) -> Result<(), String> {
	let name = clean(name)?;
	let conn = db.conn.lock().map_err(|e| e.to_string())?;
	let changed = conn.execute("UPDATE lists SET name = ?2, updated = ?3 WHERE id = ?1", params![id, name, now()]).map_err(|e| e.to_string())?;
	if changed == 0 { Err("That list doesn’t exist any more.".into()) } else { Ok(()) }
}

pub fn delete(db: &Db, id: &str) -> Result<(), String> {
	let conn = db.conn.lock().map_err(|e| e.to_string())?;
	conn.execute("DELETE FROM lists WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
	Ok(())
}

// the list's song ids, in order
fn order(conn: &rusqlite::Connection, id: &str) -> Result<Vec<String>, String> {
	let mut stmt = conn.prepare("SELECT track_id FROM list_tracks WHERE list_id = ?1 ORDER BY position").map_err(|e| e.to_string())?;
	let ids = stmt.query_map([id], |r| r.get(0)).map_err(|e| e.to_string())?.collect::<Result<Vec<String>, _>>().map_err(|e| e.to_string())?;
	Ok(ids)
}

// write a new order (positions 0, 1, 2, …) in one transaction
fn write(conn: &mut rusqlite::Connection, id: &str, ids: &[String]) -> Result<(), String> {
	let tx = conn.transaction().map_err(|e| e.to_string())?;
	tx.execute("DELETE FROM list_tracks WHERE list_id = ?1", [id]).map_err(|e| e.to_string())?;
	for (position, track) in ids.iter().enumerate() {
		tx.execute("INSERT INTO list_tracks (list_id, track_id, position) VALUES (?1, ?2, ?3)", params![id, track, position as i64]).map_err(|e| e.to_string())?;
	}
	tx.execute("UPDATE lists SET updated = ?2 WHERE id = ?1", params![id, now()]).map_err(|e| e.to_string())?;
	tx.commit().map_err(|e| e.to_string())
}

fn exists(conn: &rusqlite::Connection, id: &str) -> Result<(), String> {
	conn.query_row("SELECT 1 FROM lists WHERE id = ?1", [id], |_| Ok(()))
		.optional()
		.map_err(|e| e.to_string())?
		.ok_or_else(|| "That list doesn’t exist any more.".into())
}

// adds songs to the end (those already in the list are skipped); returns how many were added
pub fn add(db: &Db, id: &str, tracks: &[String]) -> Result<usize, String> {
	let mut conn = db.conn.lock().map_err(|e| e.to_string())?;
	exists(&conn, id)?;
	let mut ids = order(&conn, id)?;
	let before = ids.len();
	for track in tracks {
		if !ids.contains(track) {
			ids.push(track.clone());
		}
	}
	let added = ids.len() - before;
	if added > 0 {
		write(&mut conn, id, &ids)?;
	}
	Ok(added)
}

pub fn remove_at(db: &Db, id: &str, position: usize) -> Result<(), String> {
	let mut conn = db.conn.lock().map_err(|e| e.to_string())?;
	let mut ids = order(&conn, id)?;
	if position < ids.len() {
		ids.remove(position);
		write(&mut conn, id, &ids)?;
	}
	Ok(())
}

// moves the song at `from` to `to` (both positions in the list as it is now)
pub fn reorder(db: &Db, id: &str, from: usize, to: usize) -> Result<(), String> {
	let mut conn = db.conn.lock().map_err(|e| e.to_string())?;
	let mut ids = order(&conn, id)?;
	if from >= ids.len() || from == to {
		return Ok(());
	}
	let moved = ids.remove(from);
	ids.insert(to.min(ids.len()), moved);
	write(&mut conn, id, &ids)
}

pub fn songs(db: &Db, id: &str) -> Result<Vec<Song>, String> {
	let conn = db.conn.lock().map_err(|e| e.to_string())?;
	let mut stmt = conn
		.prepare(
			"SELECT t.id, t.title, t.artist, t.album, COALESCE(NULLIF(t.album_artist, ''), t.artist), lt.position + 1, t.duration_ms, t.cover, t.file
			 FROM list_tracks lt JOIN tracks t ON t.id = lt.track_id WHERE lt.list_id = ?1 ORDER BY lt.position",
		)
		.map_err(|e| e.to_string())?;
	let songs = stmt
		.query_map([id], |r| {
			Ok(Song { id: r.get(0)?, title: r.get(1)?, artist: r.get(2)?, album: r.get(3)?, album_artist: r.get(4)?, track_no: r.get(5)?, duration_ms: r.get(6)?, cover: r.get(7)?, file: r.get(8)? })
		})
		.map_err(|e| e.to_string())?
		.collect::<Result<Vec<_>, _>>()
		.map_err(|e| e.to_string())?;
	Ok(songs)
}

// ###################
// Tests
// ###################
#[cfg(test)]
mod tests {
	use super::*;
	use std::path::PathBuf;

	fn library(name: &str) -> (Db, PathBuf) {
		let folder = std::env::temp_dir().join(format!("moonwave-lists-{name}-{}", std::process::id()));
		std::fs::remove_dir_all(&folder).ok();
		let db = Db::open(&folder).unwrap();
		for (id, title, cover) in [("a", "Alpha", Some("c1")), ("b", "Bravo", Some("c1")), ("c", "Charlie", None), ("d", "Delta", Some("c2"))] {
			db.conn.lock().unwrap().execute("INSERT INTO tracks (id, file, title, cover, duration_ms, added) VALUES (?1, ?2, ?3, ?4, 1000, 'x')", params![id, format!("/{id}"), title, cover]).unwrap();
		}
		(db, folder)
	}
	fn titles(db: &Db, id: &str) -> Vec<String> {
		songs(db, id).unwrap().into_iter().map(|s| s.title).collect()
	}
	fn ids(v: &[&str]) -> Vec<String> {
		v.iter().map(|s| s.to_string()).collect()
	}

	#[test]
	fn create_add_reorder_remove() {
		let (db, folder) = library("basic");
		let night = create(&db, "  Night drive  ").unwrap();
		assert_eq!(add(&db, &night, &ids(&["c", "a", "d"])).unwrap(), 3);
		assert_eq!(add(&db, &night, &ids(&["a", "b"])).unwrap(), 1, "a is already in");
		assert_eq!(titles(&db, &night), vec!["Charlie", "Alpha", "Delta", "Bravo"]);
		assert_eq!(songs(&db, &night).unwrap()[0].track_no, Some(1), "positions from 1");
		reorder(&db, &night, 3, 0).unwrap();
		assert_eq!(titles(&db, &night), vec!["Bravo", "Charlie", "Alpha", "Delta"]);
		reorder(&db, &night, 0, 9).unwrap();
		assert_eq!(titles(&db, &night), vec!["Charlie", "Alpha", "Delta", "Bravo"], "past the end goes last");
		remove_at(&db, &night, 1).unwrap();
		assert_eq!(titles(&db, &night), vec!["Charlie", "Delta", "Bravo"]);
		let listed = all(&db).unwrap();
		assert_eq!((listed[0].name.as_str(), listed[0].tracks, listed[0].duration_ms), ("Night drive", 3, 3000));
		assert_eq!(listed[0].covers, vec!["c2", "c1"], "different covers, in list order");
		std::fs::remove_dir_all(&folder).ok();
	}

	#[test]
	fn names_rename_delete_and_missing_songs() {
		let (db, folder) = library("names");
		assert!(create(&db, "   ").is_err());
		let id = create(&db, "Old").unwrap();
		rename(&db, &id, "New").unwrap();
		assert!(rename(&db, &id, "").is_err());
		assert!(rename(&db, "nope", "X").is_err());
		assert!(add(&db, "nope", &ids(&["a"])).is_err());
		add(&db, &id, &ids(&["a", "b"])).unwrap();
		// a song leaving the library leaves the list
		db.conn.lock().unwrap().execute("DELETE FROM tracks WHERE id = 'a'", []).unwrap();
		assert_eq!(titles(&db, &id), vec!["Bravo"]);
		delete(&db, &id).unwrap();
		assert!(all(&db).unwrap().is_empty());
		assert_eq!(db.status().unwrap().tracks, 3, "songs stay");
		std::fs::remove_dir_all(&folder).ok();
	}
}
