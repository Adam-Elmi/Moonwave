// ###################
// The local library database: SQLite in the app's data folder (Moonwave.md §2)
// ###################

use rusqlite::Connection;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// bump when the tables change, and add the change to migrate()
const SCHEMA_VERSION: i32 = 1;

pub struct Db {
	pub conn: Mutex<Connection>,
	pub path: PathBuf,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Status {
	pub tracks: i64,
	pub folders: i64,
	pub lists: i64,
	pub database: String,
}

impl Db {
	pub fn open(folder: &Path) -> rusqlite::Result<Db> {
		std::fs::create_dir_all(folder).ok();
		let path = folder.join("library.db");
		let conn = Connection::open(&path)?;
		conn.pragma_update(None, "journal_mode", "WAL")?;
		conn.pragma_update(None, "foreign_keys", "ON")?;
		migrate(&conn)?;
		Ok(Db { conn: Mutex::new(conn), path })
	}

	pub fn status(&self) -> Result<Status, String> {
		let conn = self.conn.lock().map_err(|e| e.to_string())?;
		let count = |table: &str| conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).map_err(|e| e.to_string());
		Ok(Status { tracks: count("tracks")?, folders: count("folders")?, lists: count("lists")?, database: self.path.display().to_string() })
	}
}

// the time now as an ISO date (UTC), for added / updated columns; sorts as text
pub fn now() -> String {
	let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
	iso(secs)
}

// seconds since 1970 → "2026-10-24T09:30:00Z"
pub fn iso(secs: i64) -> String {
	let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
	// days since 1970-01-01 → year, month, day (Howard Hinnant's civil_from_days)
	let z = days + 719_468;
	let era = z.div_euclid(146_097);
	let doe = z - era * 146_097;
	let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
	let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
	let mp = (5 * doy + 2) / 153;
	let day = doy - (153 * mp + 2) / 5 + 1;
	let month = if mp < 10 { mp + 3 } else { mp - 9 };
	let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
	format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest % 3600 / 60, rest % 60)
}

// ###################
// Tables, created once; user_version remembers which version the file has
// ###################
fn migrate(conn: &Connection) -> rusqlite::Result<()> {
	let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
	if version < 1 {
		conn.execute_batch(
			"CREATE TABLE IF NOT EXISTS folders (
				id     TEXT PRIMARY KEY,
				path   TEXT NOT NULL UNIQUE,
				name   TEXT NOT NULL,
				added  TEXT NOT NULL
			);
			CREATE TABLE IF NOT EXISTS tracks (
				id            TEXT PRIMARY KEY,
				folder_id     TEXT REFERENCES folders (id) ON DELETE CASCADE,
				file          TEXT NOT NULL UNIQUE,            -- full path
				title         TEXT NOT NULL DEFAULT '',
				artist        TEXT NOT NULL DEFAULT '',
				album         TEXT NOT NULL DEFAULT '',
				album_artist  TEXT NOT NULL DEFAULT '',
				track_no      INTEGER,
				disc_no       INTEGER,
				year          INTEGER,
				duration_ms   INTEGER,
				cover         TEXT,                            -- small WebP in the data folder
				size          INTEGER,                         -- bytes
				modified      TEXT,                            -- the file's time, to skip unchanged files on rescan
				added         TEXT NOT NULL,
				plays         INTEGER NOT NULL DEFAULT 0,
				last_played   TEXT,
				source        TEXT NOT NULL DEFAULT 'file',    -- file / download
				source_url    TEXT
			);
			CREATE INDEX IF NOT EXISTS tracks_title ON tracks (title);
			CREATE INDEX IF NOT EXISTS tracks_artist ON tracks (artist);
			CREATE INDEX IF NOT EXISTS tracks_album ON tracks (album);
			CREATE INDEX IF NOT EXISTS tracks_added ON tracks (added);
			CREATE TABLE IF NOT EXISTS lists (
				id       TEXT PRIMARY KEY,
				name     TEXT NOT NULL,
				created  TEXT NOT NULL,
				updated  TEXT NOT NULL
			);
			CREATE TABLE IF NOT EXISTS list_tracks (
				list_id   TEXT NOT NULL REFERENCES lists (id) ON DELETE CASCADE,
				track_id  TEXT NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
				position  INTEGER NOT NULL,
				PRIMARY KEY (list_id, position)
			);",
		)?;
	}
	conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
	Ok(())
}

// ###################
// Tests: a new database has the tables, opening it again keeps the data, lists follow their tracks
// ###################
#[cfg(test)]
mod tests {
	use super::*;

	fn fresh(name: &str) -> (Db, PathBuf) {
		let folder = std::env::temp_dir().join(format!("moonwave-db-{name}-{}", std::process::id()));
		std::fs::remove_dir_all(&folder).ok();
		(Db::open(&folder).unwrap(), folder)
	}

	#[test]
	fn dates_are_iso() {
		assert_eq!(iso(0), "1970-01-01T00:00:00Z");
		assert_eq!(iso(1_792_835_400), "2026-10-24T09:50:00Z");
		assert_eq!(now().len(), 20);
	}

	#[test]
	fn a_new_library_is_empty() {
		let (db, folder) = fresh("empty");
		let s = db.status().unwrap();
		assert_eq!((s.tracks, s.folders, s.lists), (0, 0, 0));
		assert!(s.database.ends_with("library.db"));
		std::fs::remove_dir_all(&folder).ok();
	}

	#[test]
	fn reopening_keeps_the_data_and_the_version() {
		let (db, folder) = fresh("reopen");
		db.conn.lock().unwrap().execute("INSERT INTO folders (id, path, name, added) VALUES ('f', '/music', 'music', 'x')", []).unwrap();
		drop(db);
		let db = Db::open(&folder).unwrap();
		assert_eq!(db.status().unwrap().folders, 1);
		let version: i32 = db.conn.lock().unwrap().pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
		assert_eq!(version, SCHEMA_VERSION);
		std::fs::remove_dir_all(&folder).ok();
	}

	#[test]
	fn removing_a_track_takes_it_out_of_its_lists() {
		let (db, folder) = fresh("lists");
		{
			let conn = db.conn.lock().unwrap();
			conn.execute_batch(
				"INSERT INTO tracks (id, file, added) VALUES ('t1', '/a.m4a', 'x'), ('t2', '/b.m4a', 'x');
				 INSERT INTO lists (id, name, created, updated) VALUES ('l', 'Night', 'x', 'x');
				 INSERT INTO list_tracks VALUES ('l', 't1', 0), ('l', 't2', 1);
				 DELETE FROM tracks WHERE id = 't1';",
			)
			.unwrap();
			let left: i64 = conn.query_row("SELECT COUNT(*) FROM list_tracks", [], |r| r.get(0)).unwrap();
			assert_eq!(left, 1);
		}
		std::fs::remove_dir_all(&folder).ok();
	}
}
