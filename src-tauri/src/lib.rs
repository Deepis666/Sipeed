use rusqlite::{params, Connection};
use scandir::Walk;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::{atomic::AtomicBool, Mutex},
    thread,
    time::{Duration, Instant},
};
use tauri::Emitter;
use tauri::Manager;

include!(concat!(env!("OUT_DIR"), "/cooked_keys.rs"));

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS categories (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS games (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  exe_path TEXT NOT NULL UNIQUE,
  icon_path TEXT,
  sgdb_cover_path TEXT,
  developer TEXT,
  category_id INTEGER,
  created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
  FOREIGN KEY(category_id) REFERENCES categories(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS game_stats (
  game_id INTEGER PRIMARY KEY,
  total_playtime INTEGER NOT NULL DEFAULT 0,
  last_played INTEGER,
  status INTEGER NOT NULL DEFAULT 0,
  rating REAL,
  short_description TEXT,
  genres TEXT,
  themes TEXT,
  play_modes TEXT,
  tags TEXT,
  steam_app_id INTEGER,
  steam_description TEXT,
  steam_image_url TEXT,
  FOREIGN KEY(game_id) REFERENCES games(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS scan_folders (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  path TEXT NOT NULL UNIQUE,
  enabled INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);

CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
"#;

/// Priority: 1) User setting in DB  2) Compile-time env var  3) Empty
fn get_api_key(state: &DbState, setting_key: &str, compile_key: &str) -> Option<String> {
    if let Ok(conn) = state.0.lock() {
        if let Ok(Some(val)) = conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            rusqlite::params![setting_key],
            |row| row.get::<_, Option<String>>(0),
        ) {
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    if !compile_key.is_empty() {
        return Some(compile_key.to_string());
    }
    None
}

fn get_covers_dir() -> Result<PathBuf, String> {
    if cfg!(debug_assertions) {
        Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or("无法获取项目目录")?
            .join("covers"))
    } else {
        Ok(std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .ok_or("无法获取程序目录")?
            .join("covers"))
    }
}

fn migrate_db(conn: &Connection) {
    // v0.1.1: add steam metadata columns to game_stats
    let _ = conn.execute("ALTER TABLE game_stats ADD COLUMN steam_description TEXT", []);
    let _ = conn.execute("ALTER TABLE game_stats ADD COLUMN steam_image_url TEXT", []);
    let _ = conn.execute("ALTER TABLE game_stats ADD COLUMN steam_app_id INTEGER", []);
    // v0.1.2: add play status column
    let _ = conn.execute("ALTER TABLE game_stats ADD COLUMN status INTEGER NOT NULL DEFAULT 0", []);
    // v0.1.3: add developer and rating columns
    let _ = conn.execute("ALTER TABLE games ADD COLUMN developer TEXT", []);
    let _ = conn.execute("ALTER TABLE game_stats ADD COLUMN rating REAL", []);
    // v0.1.4: add GameBrain metadata columns
    let _ = conn.execute("ALTER TABLE game_stats ADD COLUMN short_description TEXT", []);
    let _ = conn.execute("ALTER TABLE game_stats ADD COLUMN genres TEXT", []);
    let _ = conn.execute("ALTER TABLE game_stats ADD COLUMN themes TEXT", []);
    let _ = conn.execute("ALTER TABLE game_stats ADD COLUMN play_modes TEXT", []);
    let _ = conn.execute("ALTER TABLE game_stats ADD COLUMN tags TEXT", []);
    // v0.1.5: add sgdb_cover_path for cover priority
    let _ = conn.execute("ALTER TABLE games ADD COLUMN sgdb_cover_path TEXT", []);
    // One-time migration: move old icon_path to sgdb_cover_path (SGDB predates GB)
    let _ = conn.execute(
        "UPDATE games SET sgdb_cover_path = icon_path, icon_path = NULL \
         WHERE sgdb_cover_path IS NULL AND icon_path IS NOT NULL AND icon_path != '' AND icon_path NOT LIKE '%_gb.%'",
        [],
    );
    // Also scan covers dir for orphaned SGDB files (name doesn't contain _gb)
    if let Ok(current_dir) = get_covers_dir() {
        let search_dirs = vec![
            current_dir.clone(),
            std::env::current_dir().map(|d| d.join("covers")).unwrap_or_default(),
            std::env::current_exe().map(|e| e.parent().unwrap().join("covers")).unwrap_or_default(),
        ];
        for dir in search_dirs {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_file() { continue; }
                    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    if stem.ends_with("_gb") { continue; }
                    if let Ok(gid) = stem.parse::<i64>() {
                        let existing: Option<String> = conn.query_row(
                            "SELECT sgdb_cover_path FROM games WHERE id = ?1",
                            params![gid],
                            |row| row.get(0),
                        ).ok().flatten();
                        if existing.is_none() || existing.as_deref() == Some("") {
                            // Copy to current covers dir and update DB
                            let dest = current_dir.join(path.file_name().unwrap());
                            let _ = std::fs::copy(&path, &dest);
                            let _ = conn.execute(
                                "UPDATE games SET sgdb_cover_path = ?1 WHERE id = ?2",
                                params![dest.to_string_lossy().to_string(), gid],
                            );
                            println!("[RUST-DEBUG] migrate: 发现孤儿SGDB封面 {} -> {}, game={}", path.display(), dest.display(), gid);
                        }
                    }
                }
            }
        }
    }
    // Fix cover paths: try current covers dir, then old src-tauri/covers
    if let Ok(current_dir) = get_covers_dir() {
        // Also check old locations for legacy files
        let old_dirs: Vec<PathBuf> = vec![
            std::env::current_dir().ok().map(|d| d.join("covers")),
            std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("covers"))),
        ].into_iter().flatten().collect();

        if let Ok(mut stmt) = conn.prepare("SELECT id, icon_path, sgdb_cover_path FROM games") {
            if let Ok(rows) = stmt.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, Option<String>>(2)?))
            }) {
                let dirs: Vec<&PathBuf> = std::iter::once(&current_dir).chain(old_dirs.iter()).collect();
                for row in rows.flatten() {
                    let (gid, icon, sgdb) = row;
                    for (is_sgdb, path_opt) in [(true, &sgdb), (false, &icon)] {
                        if let Some(ref p) = path_opt {
                            let name = Path::new(p).file_name().and_then(|n| n.to_str());
                            if let Some(name) = name {
                                let dest = current_dir.join(name);
                                let col = if is_sgdb { "sgdb_cover_path" } else { "icon_path" };
                                // If file exists at old path, copy to current dir
                                if Path::new(p).exists() && p != &dest.to_string_lossy() {
                                    let _ = fs::copy(p, &dest);
                                    let _ = conn.execute(
                                        &format!("UPDATE games SET {} = ?1 WHERE id = ?2", col),
                                        rusqlite::params![dest.to_string_lossy().to_string(), gid],
                                    );
                                } else if !Path::new(p).exists() {
                                    // File missing, search all known dirs
                                    for d in &dirs {
                                        let candidate = d.join(name);
                                        if candidate.exists() {
                                            let _ = fs::copy(&candidate, &dest);
                                            let _ = conn.execute(
                                                &format!("UPDATE games SET {} = ?1 WHERE id = ?2", col),
                                                rusqlite::params![dest.to_string_lossy().to_string(), gid],
                                            );
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

struct DbState(Mutex<Connection>);

#[derive(Serialize)]
struct ScanFolder {
    id: i64,
    path: String,
    enabled: bool,
}

#[derive(Serialize, Clone)]
struct ScanProgress {
    scanned_dirs: usize,
    found_games: usize,
    inserted_games: usize,
    current_dir: String,
}

#[derive(Serialize)]
struct ScanSummary {
    found_games: usize,
    inserted_games: usize,
    duration_ms: u128,
}

#[allow(dead_code)]
struct RunningGame {
    child: Child,
    game_id: i64,
    start_time: Instant,
    canceled: AtomicBool,
}

#[derive(Serialize)]
struct GameStats {
    total_playtime: i64,
    last_played: Option<i64>,
}

#[derive(Serialize)]
struct Category {
    id: i64,
    name: String,
}

#[derive(Serialize)]
struct CategoryWithCount {
    id: i64,
    name: String,
    count: i64,
}

#[derive(Serialize, Clone)]
struct SteamMetadata {
    name: String,
    description: String,
    header_image: Option<String>,
}

#[derive(Serialize)]
struct GameWithStats {
    id: i64,
    name: String,
    exe_path: String,
    icon_path: Option<String>,
    developer: Option<String>,
    category_id: Option<i64>,
    category_name: Option<String>,
    total_playtime: i64,
    last_played: Option<i64>,
    status: i64,
    rating: Option<f64>,
    short_description: Option<String>,
    genres: Option<String>,
    themes: Option<String>,
    play_modes: Option<String>,
    tags: Option<String>,
    steam_description: Option<String>,
    steam_image_url: Option<String>,
}

fn init_db(app: &tauri::App) -> Result<DbState, Box<dyn std::error::Error>> {
    let app_data_dir = app.path().app_data_dir()?;
    fs::create_dir_all(&app_data_dir)?;
    let db_path = app_data_dir.join("sipeed.db");
    let conn = Connection::open(db_path)?;
    conn.execute_batch(SCHEMA)?;
    migrate_db(&conn);
    Ok(DbState(Mutex::new(conn)))
}

fn db_path_from_handle(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|err| err.to_string())?;
    fs::create_dir_all(&app_data_dir).map_err(|err| err.to_string())?;
    Ok(app_data_dir.join("sipeed.db"))
}

fn file_name_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string()
}

fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case(ext))
        .unwrap_or(false)
}

#[cfg(unix)]
fn is_unix_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = fs::metadata(path) {
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    } else {
        false
    }
}

#[cfg(not(unix))]
fn is_unix_executable(_path: &Path) -> bool {
    false
}

fn collect_game_candidates(base: &Path, relative_root: &str, toc: &scandir::Toc) -> Vec<(String, String)> {
    let dir_path = if relative_root.is_empty() {
        base.to_path_buf()
    } else {
        base.join(relative_root)
    };
    let mut entries = Vec::new();
    for file in &toc.files {
        let file_path = dir_path.join(file);
        let is_exec = if cfg!(target_os = "windows") || cfg!(target_os = "macos") {
            has_extension(&file_path, "exe")
        } else {
            is_unix_executable(&file_path)
        };
        if is_exec {
            entries.push((
                file_name_from_path(&file_path),
                file_path.to_string_lossy().to_string(),
            ));
        }
    }

    #[cfg(target_os = "macos")]
    {
        for dir in &toc.dirs {
            if dir.to_lowercase().ends_with(".app") {
                let dir_path = root_path.join(dir);
                entries.push((
                    file_name_from_path(&dir_path),
                    dir_path.to_string_lossy().to_string(),
                ));
            }
        }
    }

    entries
}

fn insert_games_batch(
    conn: &mut Connection,
    games: Vec<(String, String)>,
    category_id: Option<i64>,
) -> Result<usize, String> {
    if games.is_empty() {
        return Ok(0);
    }
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    let mut inserted = 0usize;
    for (name, exe_path) in games {
        let rows = tx
            .execute(
                "INSERT OR IGNORE INTO games (name, exe_path, icon_path) VALUES (?1, ?2, NULL)",
                params![name, exe_path],
            )
            .map_err(|err| err.to_string())?;
        if rows > 0 {
            let game_id = tx.last_insert_rowid();
            tx.execute(
                "INSERT OR IGNORE INTO game_stats (game_id) VALUES (?1)",
                params![game_id],
            )
            .map_err(|err| err.to_string())?;
            if let Some(cid) = category_id {
                tx.execute("UPDATE games SET category_id = ?1 WHERE id = ?2", params![cid, game_id])
                    .map_err(|err| err.to_string())?;
            }
            inserted += 1;
        }
    }
    tx.commit().map_err(|err| err.to_string())?;
    Ok(inserted)
}

#[tauri::command]
fn insert_game(
    state: tauri::State<DbState>,
    name: String,
    exe_path: String,
    icon_path: Option<String>,
) -> Result<i64, String> {
    let conn = state
        .0
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    conn.execute(
        "INSERT INTO games (name, exe_path, icon_path) VALUES (?1, ?2, ?3)",
        params![name, exe_path, icon_path],
    )
    .map_err(|err| err.to_string())?;
    let game_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT OR IGNORE INTO game_stats (game_id) VALUES (?1)",
        params![game_id],
    )
    .map_err(|err| err.to_string())?;
    Ok(game_id)
}

#[tauri::command]
fn list_games(state: tauri::State<DbState>) -> Result<Vec<GameWithStats>, String> {
    let conn = state
        .0
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT g.id, g.name, g.exe_path, g.icon_path, g.developer, \
             g.category_id, c.name, \
             ifnull(s.total_playtime, 0), s.last_played, s.status, s.rating, \
             s.short_description, s.steam_description, s.steam_image_url \
             FROM games g \
             LEFT JOIN game_stats s ON g.id = s.game_id \
             LEFT JOIN categories c ON g.category_id = c.id \
             ORDER BY g.name COLLATE NOCASE",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(GameWithStats {
                id: row.get(0)?,
                name: row.get(1)?,
                exe_path: row.get(2)?,
                icon_path: row.get(3)?,
                developer: row.get(4)?,
                category_id: row.get(5)?,
                category_name: row.get(6)?,
                total_playtime: row.get(7)?,
                last_played: row.get(8)?,
                status: row.get(9)?,
                rating: row.get(10)?,
                short_description: row.get(11)?,
                genres: None,
                themes: None,
                play_modes: None,
                tags: None,
                steam_description: row.get(12)?,
                steam_image_url: row.get(13)?,
            })
        })
        .map_err(|err| err.to_string())?;
    let mut games = Vec::new();
    for row in rows {
        games.push(row.map_err(|err| err.to_string())?);
    }
    Ok(games)
}

#[tauri::command]
fn get_game(state: tauri::State<DbState>, id: i64) -> Result<GameWithStats, String> {
    let conn = state
        .0
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    conn.query_row(
        "SELECT g.id, g.name, g.exe_path, g.icon_path, g.developer, g.category_id, c.name, \
         ifnull(s.total_playtime, 0), s.last_played, s.status, s.rating, \
         s.short_description, s.genres, s.themes, s.play_modes, s.tags, \
         s.steam_description, s.steam_image_url \
         FROM games g \
         LEFT JOIN game_stats s ON g.id = s.game_id \
         LEFT JOIN categories c ON g.category_id = c.id \
         WHERE g.id = ?1",
        params![id],
        |row| {
            Ok(GameWithStats {
                id: row.get(0)?,
                name: row.get(1)?,
                exe_path: row.get(2)?,
                icon_path: row.get(3)?,
                developer: row.get(4)?,
                category_id: row.get(5)?,
                category_name: row.get(6)?,
                total_playtime: row.get(7)?,
                last_played: row.get(8)?,
                status: row.get(9)?,
                rating: row.get(10)?,
                short_description: row.get(11)?,
                genres: row.get(12)?,
                themes: row.get(13)?,
                play_modes: row.get(14)?,
                tags: row.get(15)?,
                steam_description: row.get(16)?,
                steam_image_url: row.get(17)?,
            })
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
fn get_recent_games(
    state: tauri::State<DbState>,
    limit: i64,
) -> Result<Vec<GameWithStats>, String> {
    let conn = state
        .0
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT g.id, g.name, g.exe_path, g.icon_path, g.developer, g.category_id, c.name, \
             ifnull(s.total_playtime, 0), s.last_played, s.status, s.rating, \
             s.short_description, s.steam_description, s.steam_image_url \
             FROM games g \
             LEFT JOIN game_stats s ON g.id = s.game_id \
             LEFT JOIN categories c ON g.category_id = c.id \
             ORDER BY s.last_played DESC NULLS LAST \
             LIMIT ?1",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(GameWithStats {
                id: row.get(0)?,
                name: row.get(1)?,
                exe_path: row.get(2)?,
                icon_path: row.get(3)?,
                developer: row.get(4)?,
                category_id: row.get(5)?,
                category_name: row.get(6)?,
                total_playtime: row.get(7)?,
                last_played: row.get(8)?,
                status: row.get(9)?,
                rating: row.get(10)?,
                short_description: row.get(11)?,
                genres: None,
                themes: None,
                play_modes: None,
                tags: None,
                steam_description: row.get(12)?,
                steam_image_url: row.get(13)?,
            })
        })
        .map_err(|err| err.to_string())?;
    let mut games = Vec::new();
    for row in rows {
        games.push(row.map_err(|err| err.to_string())?);
    }
    Ok(games)
}

#[tauri::command]
fn remove_scan_folder(state: tauri::State<DbState>, id: i64) -> Result<(), String> {
    let conn = state
        .0
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    conn.execute("DELETE FROM scan_folders WHERE id = ?1", params![id])
        .map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_game_folder(exe_path: String) -> Result<(), String> {
    let path = Path::new(&exe_path);

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg("/select,")
            .arg(path)
            .spawn()
            .map_err(|err| format!("Failed to open folder: {}", err))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
            .map_err(|err| format!("Failed to open folder: {}", err))?;
    }

    #[cfg(target_os = "linux")]
    {
        let folder = path.parent().unwrap_or(path);
        Command::new("xdg-open")
            .arg(folder)
            .spawn()
            .map_err(|err| format!("Failed to open folder: {}", err))?;
    }

    Ok(())
}

fn do_scan(app_handle: &tauri::AppHandle, path: &str, category_id: Option<i64>) -> Result<ScanSummary, String> {
    let input_path = PathBuf::from(path);
    if !input_path.exists() {
        return Err(format!("scan path not found: {}", path));
    }

    let db_path = db_path_from_handle(app_handle)?;
    let mut conn = Connection::open(db_path).map_err(|err| err.to_string())?;
    conn.execute_batch(SCHEMA).map_err(|err| err.to_string())?;
    migrate_db(&conn);
    // Clean up old entries with relative paths (before the absolute-path fix)
    let _ = conn.execute(
        "DELETE FROM games WHERE exe_path NOT LIKE '%\\%' AND exe_path NOT LIKE '%/%'",
        [],
    );

    // If the user selected a single executable file, import only that file
    if input_path.is_file() {
        let is_exec = if cfg!(target_os = "windows") || cfg!(target_os = "macos") {
            has_extension(&input_path, "exe")
        } else {
            is_unix_executable(&input_path)
        };
        if !is_exec {
            return Err(format!("不是可执行文件? {}", path));
        }
        let name = file_name_from_path(&input_path);
        let exe_path = input_path.to_string_lossy().to_string();
        let found = 1usize;
        let inserted = insert_games_batch(&mut conn, vec![(name, exe_path)], category_id)?;
        return Ok(ScanSummary {
            found_games: found,
            inserted_games: inserted,
            duration_ms: 0,
        });
    }

    let root_path = input_path;
    let mut walk = Walk::new(&root_path, None).map_err(|err| err.to_string())?;
    walk = walk.skip_hidden(true).sorted(false);

    #[cfg(target_os = "windows")]
    {
        walk = walk.file_include(Some(vec!["*.exe".to_string()]));
    }

    #[cfg(target_os = "macos")]
    {
        walk = walk.file_include(Some(vec!["*.exe".to_string()]));
    }

    let start = Instant::now();
    let mut found_games = 0usize;
    let mut inserted_games = 0usize;
    let mut scanned_dirs = 0usize;

    walk.start().map_err(|err| err.to_string())?;
    loop {
        let results = walk.results(true);
        if !results.is_empty() {
            scanned_dirs += results.len();
            let mut current_dir = String::new();
            let mut batch = Vec::new();
            for (root, toc) in results {
                current_dir = root.clone();
                let mut candidates = collect_game_candidates(&root_path, &root, &toc);
                found_games += candidates.len();
                batch.append(&mut candidates);
            }
            let inserted = insert_games_batch(&mut conn, batch, category_id)?;
            inserted_games += inserted;
            app_handle
                .emit(
                    "scan-progress",
                    ScanProgress {
                        scanned_dirs,
                        found_games,
                        inserted_games,
                        current_dir,
                    },
                )
                .map_err(|err| err.to_string())?;
        }
        if !walk.busy() {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    let remaining = walk.results(true);
    if !remaining.is_empty() {
        scanned_dirs += remaining.len();
        let mut current_dir = String::new();
        let mut batch = Vec::new();
        for (root, toc) in remaining {
            current_dir = root.clone();
            let mut candidates = collect_game_candidates(&root_path, &root, &toc);
            found_games += candidates.len();
            batch.append(&mut candidates);
        }
        let inserted = insert_games_batch(&mut conn, batch, category_id)?;
        inserted_games += inserted;
        app_handle
            .emit(
                "scan-progress",
                ScanProgress {
                    scanned_dirs,
                    found_games,
                    inserted_games,
                    current_dir,
                },
            )
            .map_err(|err| err.to_string())?;
    }

    walk.collect().map_err(|err| err.to_string())?;

    Ok(ScanSummary {
        found_games,
        inserted_games,
        duration_ms: start.elapsed().as_millis(),
    })
}

#[tauri::command]
async fn rescan_folders(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, DbState>,
) -> Result<Vec<ScanSummary>, String> {
    let folders: Vec<String> = {
        let conn = state
            .0
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        let mut stmt = conn
            .prepare("SELECT path FROM scan_folders WHERE enabled != 0")
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|err| err.to_string())?;
        let mut paths = Vec::new();
        for row in rows {
            paths.push(row.map_err(|err| err.to_string())?);
        }
        paths
    };

    let handle = app_handle.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut summaries = Vec::new();
        for folder_path in folders {
            let summary = do_scan(&handle, &folder_path, None)?;
            summaries.push(summary);
        }
        Ok(summaries)
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
fn add_scan_folder(state: tauri::State<DbState>, path: String) -> Result<i64, String> {
    let conn = state
        .0
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO scan_folders (path) VALUES (?1)",
        params![path],
    )
    .map_err(|err| err.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id FROM scan_folders WHERE path = ?1")
        .map_err(|err| err.to_string())?;
    let id = stmt
        .query_row([path], |row| row.get::<_, i64>(0))
        .map_err(|err| err.to_string())?;
    Ok(id)
}

#[tauri::command]
fn list_scan_folders(state: tauri::State<DbState>) -> Result<Vec<ScanFolder>, String> {
    let conn = state
        .0
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, path, enabled FROM scan_folders ORDER BY id DESC")
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let enabled: i64 = row.get(2)?;
            Ok(ScanFolder {
                id: row.get(0)?,
                path: row.get(1)?,
                enabled: enabled != 0,
            })
        })
        .map_err(|err| err.to_string())?;
    let mut folders = Vec::new();
    for row in rows {
        folders.push(row.map_err(|err| err.to_string())?);
    }
    Ok(folders)
}

#[tauri::command]
async fn scan_folder(app_handle: tauri::AppHandle, path: String, category_id: Option<i64>) -> Result<ScanSummary, String> {
    let app_handle = app_handle.clone();
    tauri::async_runtime::spawn_blocking(move || do_scan(&app_handle, &path, category_id))
        .await
        .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn launch_game(app_handle: tauri::AppHandle, state: tauri::State<'_, DbState>, game_id: i64) -> Result<(), String> {
    let exe_path = {
        let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
        conn.query_row(
            "SELECT exe_path FROM games WHERE id = ?1",
            params![game_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|err| err.to_string())?
    };

    let path = std::path::Path::new(&exe_path);
    let dir = path.parent().unwrap_or(path);

    let mut child = Command::new(&exe_path)
        .current_dir(dir)
        .spawn()
        .map_err(|err| format!("启动失败: {}", err))?;

    let start = Instant::now();
    let db_path = db_path_from_handle(&app_handle)?;
    let app_handle = app_handle.clone();

    tauri::async_runtime::spawn(async move {
        let result = tauri::async_runtime::spawn_blocking(move || {
            child.wait().map_err(|err| err.to_string())?;

            let conn = Connection::open(&db_path).map_err(|err| err.to_string())?;
            let duration = start.elapsed().as_secs() as i64;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            conn.execute(
                "UPDATE game_stats SET total_playtime = total_playtime + ?1, last_played = ?2 WHERE game_id = ?3",
                params![duration, now, game_id],
            )
            .map_err(|err| err.to_string())?;

            let _ = app_handle.emit("game-stopped", game_id);
            Ok::<_, String>(())
        })
        .await;

        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => eprintln!("launch_game error: {}", e),
            Err(e) => eprintln!("launch_game join error: {}", e),
        }
    });

    Ok(())
}

#[tauri::command]
fn get_game_stats(state: tauri::State<'_, DbState>, game_id: i64) -> Result<GameStats, String> {
    let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    conn.query_row(
        "SELECT total_playtime, last_played FROM game_stats WHERE game_id = ?1",
        params![game_id],
        |row| {
            Ok(GameStats {
                total_playtime: row.get(0)?,
                last_played: row.get(1)?,
            })
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
fn set_game_status(state: tauri::State<'_, DbState>, game_id: i64, status: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    conn.execute(
        "UPDATE game_stats SET status = ?1 WHERE game_id = ?2",
        params![status, game_id],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
fn delete_game(state: tauri::State<'_, DbState>, game_id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    conn.execute("DELETE FROM games WHERE id = ?1", params![game_id])
        .map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
fn update_game(
    state: tauri::State<'_, DbState>,
    game_id: i64,
    name: String,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    conn.execute(
        "UPDATE games SET name = ?1 WHERE id = ?2",
        params![name, game_id],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
fn list_categories(state: tauri::State<'_, DbState>) -> Result<Vec<Category>, String> {
    let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name FROM categories ORDER BY name COLLATE NOCASE")
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Category {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })
        .map_err(|err| err.to_string())?;
    let mut categories = Vec::new();
    for row in rows {
        categories.push(row.map_err(|err| err.to_string())?);
    }
    Ok(categories)
}

#[tauri::command]
fn list_categories_with_counts(state: tauri::State<'_, DbState>) -> Result<Vec<CategoryWithCount>, String> {
    let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.name, COUNT(g.id) as cnt \
             FROM categories c LEFT JOIN games g ON g.category_id = c.id \
             GROUP BY c.id ORDER BY c.name COLLATE NOCASE",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(CategoryWithCount {
                id: row.get(0)?,
                name: row.get(1)?,
                count: row.get(2)?,
            })
        })
        .map_err(|err| err.to_string())?;
    let mut cats = Vec::new();
    for row in rows {
        cats.push(row.map_err(|err| err.to_string())?);
    }
    Ok(cats)
}

#[tauri::command]
fn add_category(state: tauri::State<'_, DbState>, name: String) -> Result<i64, String> {
    let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    conn.execute(
        "INSERT INTO categories (name) VALUES (?1)",
        params![name],
    )
    .map_err(|err| err.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
fn delete_category(state: tauri::State<'_, DbState>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    conn.execute("UPDATE games SET category_id = NULL WHERE category_id = ?1", params![id])
        .map_err(|err| err.to_string())?;
    conn.execute("DELETE FROM categories WHERE id = ?1", params![id])
        .map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
fn rename_category(state: tauri::State<'_, DbState>, id: i64, name: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    conn.execute("UPDATE categories SET name = ?1 WHERE id = ?2", params![name, id])
        .map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
fn delete_category_with_transfer(state: tauri::State<'_, DbState>, id: i64, transfer_to: Option<i64>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    // Move games to target category (or NULL)
    conn.execute("UPDATE games SET category_id = ?1 WHERE category_id = ?2", params![transfer_to, id])
        .map_err(|err| err.to_string())?;
    conn.execute("DELETE FROM categories WHERE id = ?1", params![id])
        .map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
fn batch_move_games(state: tauri::State<'_, DbState>, game_ids: Vec<i64>, category_id: Option<i64>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    for gid in &game_ids {
        conn.execute("UPDATE games SET category_id = ?1 WHERE id = ?2", params![category_id, gid])
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn assign_game_category(state: tauri::State<'_, DbState>, game_id: i64, category_id: Option<i64>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    conn.execute(
        "UPDATE games SET category_id = ?1 WHERE id = ?2",
        params![category_id, game_id],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
async fn fetch_steam_metadata(
    state: tauri::State<'_, DbState>,
    game_id: i64,
    app_id: u32,
) -> Result<SteamMetadata, String> {
    use reqwest::Client;

    let url = format!("https://store.steampowered.com/api/appdetails?appids={}", app_id);
    let client = Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let json: serde_json::Value = resp.json().await.map_err(|err| err.to_string())?;

    let app_data = &json[app_id.to_string()]["data"];

    let name = app_data["name"].as_str().unwrap_or("").to_string();
    let description = app_data["short_description"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let header_image = app_data["header_image"].as_str().map(|s| s.to_string());

    let metadata = SteamMetadata {
        name,
        description,
        header_image,
    };

    let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    conn.execute(
        "UPDATE games SET name = ?1 WHERE id = ?2",
        params![metadata.name, game_id],
    )
    .map_err(|err| err.to_string())?;

    conn.execute(
        "UPDATE game_stats SET steam_app_id = ?1, steam_description = ?2, steam_image_url = ?3 WHERE game_id = ?4",
        params![app_id as i64, metadata.description, metadata.header_image, game_id],
    )
    .map_err(|err| err.to_string())?;

    Ok(metadata)
}

#[tauri::command]
async fn fetch_grid_cover(
    _app_handle: tauri::AppHandle,
    state: tauri::State<'_, DbState>,
    game_id: i64,
) -> Result<String, String> {
    use reqwest::Client;

    let (game_name, mut api_key) = {
        let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
        let name: String = conn
            .query_row("SELECT name FROM games WHERE id = ?1", params![game_id], |row| {
                row.get(0)
            })
            .map_err(|err| err.to_string())?;
        let key = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'sgdb_api_key'",
                [],
                |row| row.get::<_, String>(0),
            )
            .map_err(|_| "请先在设置中配置 SteamGridDB API Key".to_string())?;
        (name, key)
    };

    // Fallback to compile-time key if user hasn't set one
    if api_key.is_empty() && !STEAMGRID_KEY.is_empty() {
        api_key = STEAMGRID_KEY.to_string();
    }
    if api_key.is_empty() {
        return Err("请先在设置中配置 SteamGridDB API Key".to_string());
    }

    let client = Client::new();

    // Step 1: Search for game
    let search_url = format!(
        "https://www.steamgriddb.com/api/v2/search/autocomplete/{}",
        urlencode(&game_name)
    );
    let resp = client
        .get(&search_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|err| format!("搜索失败: {}", err))?;
    let json: serde_json::Value = resp.json().await.map_err(|err| err.to_string())?;
    let data = json["data"].as_array().ok_or("未找到匹配的游戏")?;
    if data.is_empty() {
        return Err("未找到匹配的游戏".to_string());
    }
    let sgdb_id = data[0]["id"]
        .as_i64()
        .ok_or("无法获取游戏ID")?;

    // Step 2: Get grids
    let grids_url = format!(
        "https://www.steamgriddb.com/api/v2/grids/game/{}?dimensions=600x900&limit=1",
        sgdb_id
    );
    let resp = client
        .get(&grids_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|err| format!("获取封面失败: {}", err))?;
    let json: serde_json::Value = resp.json().await.map_err(|err| err.to_string())?;
    let grids = json["data"].as_array().ok_or("无可用封面")?;
    if grids.is_empty() {
        return Err("该游戏暂无封面".to_string());
    }
    let image_url = grids[0]["url"]
        .as_str()
        .ok_or("无法获取封面URL")?;

    // Step 3: Download image
    let image_bytes = client
        .get(image_url)
        .send()
        .await
        .map_err(|err| format!("下载失败: {}", err))?
        .bytes()
        .await
        .map_err(|err| err.to_string())?;

    // Step 4: Save to covers/
    let covers_dir = get_covers_dir()?;
    fs::create_dir_all(&covers_dir).map_err(|err| err.to_string())?;
    let ext = image_url.rsplit('.').next().unwrap_or("png");
    let file_name = format!("{}.{}", game_id, ext);
    let file_path = covers_dir.join(&file_name);
    fs::write(&file_path, &image_bytes).map_err(|err| err.to_string())?;
    println!("[RUST-DEBUG] fetch_grid_cover: 已保�?SGDB 封面�?{}", file_path.display());

    // Step 5: Update DB
    let local_path = file_path.to_string_lossy().to_string();
    {
        let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
        conn.execute(
            "UPDATE games SET sgdb_cover_path = ?1 WHERE id = ?2",
            params![local_path, game_id],
        )
        .map_err(|err| err.to_string())?;
    }

    Ok(local_path)
}

#[tauri::command]
async fn fetch_gamebrain_metadata(
    state: tauri::State<'_, DbState>,
    game_id: i64,
) -> Result<String, String> {
    use reqwest::Client;

    let (game_name, mut api_key) = {
        let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
        let name: String = conn
            .query_row("SELECT name FROM games WHERE id = ?1", params![game_id], |row| {
                row.get(0)
            })
            .map_err(|err| err.to_string())?;
        let key = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'gb_api_key'",
                [],
                |row| row.get::<_, String>(0),
            )
            .map_err(|_| "请先在设置中配置 GameBrain API Key".to_string())?;
        (name, key)
    };

    // Fallback to compile-time key
    if api_key.is_empty() && !GAMEBRAIN_KEY.is_empty() {
        api_key = GAMEBRAIN_KEY.to_string();
    }
    if api_key.is_empty() {
        return Err("请先在设置中配置 GameBrain API Key".to_string());
    }

    let client = Client::new();

    // Step 0: Try SteamGridDB cover first (if API key is configured)
    let mut sgdb_msg = String::new();
    let sgdb_key: String = {
        let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
        let raw: String = conn.query_row(
            "SELECT value FROM settings WHERE key = 'sgdb_api_key'", [],
            |row| row.get(0)).unwrap_or_default();
        let user_key = if raw.is_empty() {
            if STEAMGRID_KEY.is_empty() { String::new() } else { STEAMGRID_KEY.to_string() }
        } else {
            raw
        };
        user_key
    };
    if !sgdb_key.is_empty() {
            let sgdb_search = format!(
                "https://www.steamgriddb.com/api/v2/search/autocomplete/{}",
                urlencode(&game_name)
            );
            if let Ok(resp) = client.get(&sgdb_search)
                .header("Authorization", format!("Bearer {}", sgdb_key))
                .send().await
            {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(data) = json["data"].as_array() {
                        if let Some(first) = data.first() {
                            if let Some(sgdb_id) = first["id"].as_i64() {
                                let grids_url = format!(
                                    "https://www.steamgriddb.com/api/v2/grids/game/{}?dimensions=600x900&limit=1",
                                    sgdb_id
                                );
                                if let Ok(resp) = client.get(&grids_url)
                                    .header("Authorization", format!("Bearer {}", sgdb_key))
                                    .send().await
                                {
                                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                                        if let Some(grids) = json["data"].as_array() {
                                            if let Some(grid) = grids.first() {
                                                if let Some(img_url) = grid["url"].as_str() {
                                                    if let Ok(resp) = client.get(img_url).send().await {
                                                        if let Ok(bytes) = resp.bytes().await {
                                                            if let Ok(covers_dir) = get_covers_dir() {
                                                                let _ = fs::create_dir_all(&covers_dir);
                                                                let ext = img_url.rsplit('.').next().unwrap_or("png");
                                                                let file_path = covers_dir.join(&format!("{}.{}", game_id, ext));
                                                                let _ = fs::write(&file_path, &bytes);
                                                                {
                                                                    let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
                                                                    let _ = conn.execute(
                                                                        "UPDATE games SET sgdb_cover_path = ?1 WHERE id = ?2",
                                                                        params![file_path.to_string_lossy().to_string(), game_id],
                                                                    );
                                                                }
                                                                sgdb_msg = format!("SGDB封面已保�? {}", file_path.display());
                                                                println!("[RUST-DEBUG] fetch_gamebrain_metadata: {}", sgdb_msg);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    // Log directory
    let log_dir = if cfg!(debug_assertions) {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("log")
    } else {
        std::env::current_exe().map_err(|e| e.to_string())?.parent().unwrap().join("log")
    };
    let _ = fs::create_dir_all(&log_dir);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let search_url = format!(
        "https://api.gamebrain.co/v1/games?query={}&offset=0&limit=3&filters=[]&sort=computed_rating&sort_order=desc&generate_filter_options=false&api-key={}",
        urlencode(&game_name), api_key
    );
    let resp = client.get(&search_url).header("Accept", "application/json")
        .send().await.map_err(|e| format!("搜索失败: {}", e))?;
    let search_status = resp.status().as_u16();
    let json_text = resp.text().await.map_err(|e| e.to_string())?;

    // Log search response
    let search_log = serde_json::json!({
        "timestamp": ts,
        "game_id": game_id,
        "query": game_name,
        "url": search_url,
        "status": search_status,
        "response": serde_json::from_str::<serde_json::Value>(&json_text).unwrap_or(serde_json::Value::String(json_text.clone())),
    });
    let search_log_path = log_dir.join(format!("gb_search_{}_{}.json", game_id, ts));
    let _ = fs::write(&search_log_path, serde_json::to_string_pretty(&search_log).unwrap_or_default());

    let json: serde_json::Value = serde_json::from_str(&json_text).map_err(|e| format!("JSON解析失败: {}", e))?;
    let results = json["results"].as_array().ok_or("未找到匹配的游戏")?;
    if results.is_empty() {
        return Err(format!("未找到匹配的游戏 (日志已保�? {})", search_log_path.display()));
    }
    let gb_id = results[0]["id"].as_i64().ok_or("无法获取游戏ID")?;

    let detail_url = format!("https://api.gamebrain.co/v1/games/{}?api-key={}", gb_id, api_key);
    let resp = client.get(&detail_url).header("Accept", "application/json")
        .send().await.map_err(|e| format!("获取详情失败: {}", e))?;
    let detail_status = resp.status().as_u16();
    let detail_text = resp.text().await.map_err(|e| e.to_string())?;

    // Log detail response
    let detail_log = serde_json::json!({
        "timestamp": ts,
        "game_id": game_id,
        "gb_game_id": gb_id,
        "url": detail_url,
        "status": detail_status,
        "response": serde_json::from_str::<serde_json::Value>(&detail_text).unwrap_or(serde_json::Value::String(detail_text.clone())),
    });
    let detail_log_path = log_dir.join(format!("gb_detail_{}_{}.json", game_id, ts));
    let _ = fs::write(&detail_log_path, serde_json::to_string_pretty(&detail_log).unwrap_or_default());

    let game: serde_json::Value = serde_json::from_str(&detail_text).map_err(|e| format!("详情JSON解析失败: {}", e))?;

    let name = game["name"].as_str().unwrap_or(&game_name).to_string();
    let description = game["description"].as_str();
    let short_description = game["short_description"].as_str();
    let developer = game["developer"].as_str();
    let rating = game["rating"]["mean"].as_f64();
    let image_url = game["image"].as_str();

    let genres = json_arr_str(&game["genres"], "name");
    let themes = json_arr_str(&game["themes"], "name");
    let play_modes = json_arr_str(&game["play_modes"], "name");
    let tags = json_arr_to_val(&game["tags"]);

    let mut cover_path = String::new();
    if let Some(url) = image_url {
        if let Ok(resp) = client.get(url).send().await {
            if let Ok(bytes) = resp.bytes().await {
                if let Ok(covers_dir) = get_covers_dir() {
                    let _ = fs::create_dir_all(&covers_dir);
                    let file_path = covers_dir.join(&format!("{}_gb.jpg", game_id));
                    let _ = fs::write(&file_path, &bytes);
                    println!("[RUST-DEBUG] fetch_gamebrain_metadata: 已保�?GB 封面�?{}", file_path.display());
                    cover_path = file_path.to_string_lossy().to_string();
                }
            }
        }
    }

    {
        let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
        conn.execute("UPDATE games SET name = ?1, developer = ?2 WHERE id = ?3", params![name, developer, game_id])
            .map_err(|e| e.to_string())?;
        if !cover_path.is_empty() {
            conn.execute("UPDATE games SET icon_path = ?1 WHERE id = ?2", params![cover_path, game_id])
                .map_err(|e| e.to_string())?;
        }
        conn.execute("UPDATE game_stats SET \
            steam_description = ?1, rating = ?2, \
            short_description = ?3, genres = ?4, themes = ?5, \
            play_modes = ?6, tags = ?7 \
            WHERE game_id = ?8",
            params![description, rating, short_description, genres, themes, play_modes, tags, game_id])
            .map_err(|e| e.to_string())?;
    }

    let mut msg = format!("已获�? {}", name);
    if !sgdb_msg.is_empty() { msg.push_str(&format!("\n{}", sgdb_msg)); }
    if let Some(d) = developer { msg.push_str(&format!(" | 开发商: {}", d)); }
    if let Some(r) = rating { msg.push_str(&format!(" | 评分: {:.1}/10", r * 10.0)); }
    Ok(msg)
}

#[tauri::command]
async fn extract_icon(exe_path: String) -> Result<Option<String>, String> {
    #[cfg(not(windows))]
    {
        let _ = exe_path;
        return Ok(None);
    }

    #[cfg(windows)]
    {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        use pelite::pe64::Pe;

        let path = std::path::Path::new(&exe_path);
        if !path.exists() || !path.is_file() {
            return Ok(None);
        }

        let result: Option<String> = tauri::async_runtime::spawn_blocking(move || {
            let data = match std::fs::read(&exe_path) {
                Ok(d) => d,
                Err(_) => return None,
            };

            let pe = match pelite::pe64::PeFile::from_bytes(&data) {
                Ok(p) => p,
                Err(_) => return None,
            };

            let resources = match pe.resources() {
                Ok(r) => r,
                Err(_) => return None,
            };

            // Use pelite's built-in icons iterator
            let mut best_width: u32 = 0;
            let mut best_data: Vec<u8> = Vec::new();

            for icon_result in resources.icons() {
                let (_name, group) = match icon_result {
                    Ok(g) => g,
                    Err(_) => continue,
                };
                for entry in group.entries() {
                    let w = entry.bWidth as u32;
                    let h = entry.bHeight as u32;
                    if w * h > best_width {
                        if let Ok(raw) = group.image(entry.nId) {
                            best_width = w * h;
                            best_data = raw.to_vec();
                        }
                    }
                }
            }

            if best_data.is_empty() {
                return None;
            }

            // Parse extracted icon with ico crate
            let icon_dir = match ico::IconDir::read(std::io::Cursor::new(&best_data)) {
                Ok(dir) => dir,
                Err(_) => return None,
            };

            let entries = icon_dir.entries();
            let best = entries.iter().max_by_key(|e| e.width() as u32 * e.height() as u32)?;

            let image = match best.decode() {
                Ok(img) => img,
                Err(_) => return None,
            };

            let rgba = image.rgba_data();
            let mut png = Vec::new();
            let mut encoder = png::Encoder::new(std::io::Cursor::new(&mut png), image.width(), image.height());
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = match encoder.write_header() {
                Ok(w) => w,
                Err(_) => return None,
            };
            if writer.write_image_data(rgba).is_err() {
                return None;
            }
            drop(writer);

            let b64 = STANDARD.encode(&png);
            Some(format!("data:image/png;base64,{}", b64))
        })
        .await
        .unwrap_or(None);

        Ok(result)
    }
}

#[tauri::command]
fn diagnose_cover_migration(state: tauri::State<'_, DbState>) -> Result<String, String> {
    let mut report = String::from("=== 封面迁移诊断报告 ===\n\n");

    // Scan all known covers directories
    let search_dirs: Vec<PathBuf> = {
        let mut dirs = Vec::new();
        if let Ok(d) = get_covers_dir() { dirs.push(d); }
        if let Ok(cwd) = std::env::current_dir() { dirs.push(cwd.join("covers")); }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(p) = exe.parent() { dirs.push(p.join("covers")); }
        }
        dirs.sort();
        dirs.dedup();
        dirs
    };

    let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    report.push_str(&format!("搜索目录 ({} �?:\n", search_dirs.len()));
    for d in &search_dirs {
        report.push_str(&format!("  - {}\n", d.display()));
    }
    report.push('\n');

    report.push_str("| 文件�?| _gb? | 游戏ID | DB存在? | sgdb已设? | 在covers/ ? |\n");
    report.push_str("|--------|------|--------|----------|-----------|-------------|\n");

    let current_covers = get_covers_dir().unwrap_or_default();

    for dir in &search_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() { continue; }
                let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                let has_gb = fname.contains("_gb");
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

                let gid: Option<i64> = if !has_gb {
                    stem.split('_').next().and_then(|s| s.parse().ok())
                } else {
                    None
                };

                let db_exists = if let Some(id) = gid {
                    conn.query_row("SELECT COUNT(*) FROM games WHERE id = ?1", params![id], |row| row.get::<_, i64>(0))
                        .map(|c| c > 0).unwrap_or(false)
                } else { false };

                let sgdb_set = if let Some(id) = gid {
                    conn.query_row("SELECT sgdb_cover_path FROM games WHERE id = ?1", params![id], |row| {
                        let p: Option<String> = row.get(0)?;
                        Ok(p.filter(|s| !s.is_empty()).is_some())
                    }).unwrap_or(false)
                } else { false };

                let in_covers = gid.is_some() && current_covers.join(fname).exists();

                report.push_str(&format!("| {} | {} | {} | {} | {} | {} |\n",
                    fname,
                    if has_gb { "yes" } else { "no" },
                    gid.map(|i| i.to_string()).unwrap_or_else(|| "-".into()),
                    if db_exists { "yes" } else { "no" },
                    if sgdb_set { "yes" } else { "no" },
                    if in_covers { "yes" } else { "no" },
                ));
            }
        }
    }

    Ok(report)
}

#[tauri::command]
fn read_cover_base64(state: tauri::State<'_, DbState>, game_id: i64) -> Result<Option<String>, String> {
    let (sgdb_path, gb_path): (Option<String>, Option<String>) = {
        let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
        conn.query_row(
            "SELECT sgdb_cover_path, icon_path FROM games WHERE id = ?1",
            params![game_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|err| err.to_string())?
    };

    // Determine which path to use based on priority
    let chosen = sgdb_path.as_ref().filter(|s| !s.is_empty())
        .or(gb_path.as_ref().filter(|s| !s.is_empty()));

    println!("[RUST-DEBUG] read_cover_base64 game_id={} sgdb_path={:?} gb_path={:?} chosen={:?}",
        game_id, sgdb_path, gb_path, chosen);

    let path = chosen.ok_or_else(|| "no cover available".to_string())?;
    match fs::read(&path) {
        Ok(bytes) => {
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            println!("[RUST-DEBUG] read_cover_base64: 成功读取文件 {} ({} bytes)", path, bytes.len());
            let ext = std::path::Path::new(&path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("png");
            let mime = match ext.to_lowercase().as_str() {
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                "webp" => "image/webp",
                _ => "image/png",
            };
            let b64 = STANDARD.encode(&bytes);
            Ok(Some(format!("data:{};base64,{}", mime, b64)))
        }
        Err(e) => {
            println!("[RUST-DEBUG] read_cover_base64: 读取文件失败 {}: {}", path, e);
            Err(format!("读取封面失败: {}", e))
        }
    }
}

fn json_arr_str(arr: &serde_json::Value, key: &str) -> Option<String> {
    let items: Vec<String> = arr.as_array()?.iter()
        .filter_map(|v| v[key].as_str().map(|s| s.to_string()))
        .collect();
    if items.is_empty() { None } else { Some(serde_json::to_string(&items).unwrap_or_default()) }
}

fn json_arr_to_val(arr: &serde_json::Value) -> Option<String> {
    let items: Vec<String> = arr.as_array()?.iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    if items.is_empty() { None } else { Some(serde_json::to_string(&items).unwrap_or_default()) }
}

fn urlencode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut result = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => result.push(b as char),
            b' ' => result.push_str("%20"),
            _ => result.push_str(&format!("%{:02X}", b)),
        }
    }
    result
}

#[tauri::command]
fn set_setting(state: tauri::State<'_, DbState>, key: String, value: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_setting(state: tauri::State<'_, DbState>, key: String) -> Result<Option<String>, String> {
    let conn = state.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    let result = conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    );
    match result {
        Ok(val) => Ok(Some(val)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let db_state = init_db(app)?;
            app.manage(db_state);
            Ok(())
        })
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            insert_game,
            list_games,
            get_game,
            get_recent_games,
            add_scan_folder,
            list_scan_folders,
            remove_scan_folder,
            scan_folder,
            rescan_folders,
            launch_game,
            get_game_stats,
            set_game_status,
            delete_game,
            update_game,
            open_game_folder,
            list_categories,
            list_categories_with_counts,
            add_category,
            delete_category,
            rename_category,
            delete_category_with_transfer,
            batch_move_games,
            assign_game_category,
            fetch_steam_metadata,
            fetch_grid_cover,
            read_cover_base64,
            fetch_gamebrain_metadata,
            extract_icon,
            diagnose_cover_migration,
            set_setting,
            get_setting
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::path::Path;

    // --- Pure function tests ---

    #[test]
    fn test_file_name_from_path_basic() {
        let name = file_name_from_path(Path::new("C:\\Games\\portal2.exe"));
        assert_eq!(name, "portal2");
    }

    #[test]
    fn test_file_name_from_path_no_extension() {
        let name = file_name_from_path(Path::new("/usr/bin/bash"));
        assert_eq!(name, "bash");
    }

    #[test]
    fn test_file_name_from_path_empty() {
        let name = file_name_from_path(Path::new(""));
        assert_eq!(name, "");
    }

    #[test]
    fn test_has_extension_exact() {
        assert!(has_extension(Path::new("game.exe"), "exe"));
    }

    #[test]
    fn test_has_extension_case_insensitive() {
        assert!(has_extension(Path::new("GAME.EXE"), "exe"));
        assert!(has_extension(Path::new("Game.Exe"), "exe"));
    }

    #[test]
    fn test_has_extension_no_extension() {
        assert!(!has_extension(Path::new("game"), "exe"));
    }

    #[test]
    fn test_has_extension_wrong_extension() {
        assert!(!has_extension(Path::new("game.txt"), "exe"));
    }

    // --- Database tests ---

    fn create_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("failed to create in-memory DB");
        conn.execute_batch(SCHEMA).expect("failed to create schema");
        conn
    }

    #[test]
    fn test_insert_games_batch_empty() {
        let mut conn = create_test_db();
        let result = insert_games_batch(&mut conn, vec![], None).unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_insert_games_batch_single() {
        let mut conn = create_test_db();
        let games = vec![("Portal 2".to_string(), "C:\\Games\\portal2.exe".to_string())];
        let result = insert_games_batch(&mut conn, games, None).unwrap();
        assert_eq!(result, 1);
    }

    #[test]
    fn test_insert_games_batch_multiple() {
        let mut conn = create_test_db();
        let games = vec![
            ("GameA".to_string(), "/games/a.exe".to_string()),
            ("GameB".to_string(), "/games/b.exe".to_string()),
            ("GameC".to_string(), "/games/c.exe".to_string()),
        ];
        let result = insert_games_batch(&mut conn, games, None).unwrap();
        assert_eq!(result, 3);
    }

    #[test]
    fn test_insert_games_batch_deduplicate() {
        let mut conn = create_test_db();
        let games = vec![
            ("GameA".to_string(), "/games/a.exe".to_string()),
            ("GameB".to_string(), "/games/b.exe".to_string()),
            ("GameA Again".to_string(), "/games/a.exe".to_string()), // same exe_path
        ];
        let result = insert_games_batch(&mut conn, games, None).unwrap();
        assert_eq!(result, 2); // only 2 unique inserted
    }

    #[test]
    fn test_insert_games_batch_creates_game_stats() {
        let mut conn = create_test_db();
        let games = vec![("Portal 2".to_string(), "C:\\Games\\portal2.exe".to_string())];
        insert_games_batch(&mut conn, games, None).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM game_stats", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    // --- SQL operations tested directly (bypass Tauri State) ---

    #[test]
    fn test_insert_game_sql() {
        let conn = create_test_db();
        let state = DbState(Mutex::new(conn));
        let guard = state.0.lock().unwrap();

        guard
            .execute(
                "INSERT INTO games (name, exe_path) VALUES (?1, ?2)",
                params!["Portal 2", "C:\\portal2.exe"],
            )
            .unwrap();
        let game_id = guard.last_insert_rowid();
        guard
            .execute(
                "INSERT OR IGNORE INTO game_stats (game_id) VALUES (?1)",
                params![game_id],
            )
            .unwrap();

        let name: String = guard
            .query_row(
                "SELECT name FROM games WHERE id = ?1",
                params![game_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(name, "Portal 2");
    }

    #[test]
    fn test_list_games_sql_empty() {
        let conn = create_test_db();
        let state = DbState(Mutex::new(conn));
        let guard = state.0.lock().unwrap();
        let count: i64 = guard
            .query_row("SELECT COUNT(*) FROM games", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_scan_folder_sql_insert_and_ignore() {
        let conn = create_test_db();
        let state = DbState(Mutex::new(conn));
        let guard = state.0.lock().unwrap();

        guard
            .execute(
                "INSERT OR IGNORE INTO scan_folders (path) VALUES (?1)",
                params!["D:\\Games"],
            )
            .unwrap();
        let id1 = guard.last_insert_rowid();

        guard
            .execute(
                "INSERT OR IGNORE INTO scan_folders (path) VALUES (?1)",
                params!["D:\\Games"],
            )
            .unwrap();
        let id2 = guard.last_insert_rowid();

        // Duplicate INSERT OR IGNORE: last_insert_rowid stays same
        assert_eq!(id1, id2);

        let count: i64 = guard
            .query_row("SELECT COUNT(*) FROM scan_folders", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    // --- collect_game_candidates test ---

    #[test]
    fn test_collect_game_candidates_no_matches() {
        let toc = scandir::Toc {
            files: vec!["readme.txt".to_string(), "config.ini".to_string()],
            dirs: vec![],
            errors: vec![],
            other: vec![],
            symlinks: vec![],
        };
        let candidates = collect_game_candidates(Path::new("/fake/root"), "", &toc);
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_collect_game_candidates_finds_exe_on_windows() {
        let toc = scandir::Toc {
            files: vec!["portal2.exe".to_string(), "readme.txt".to_string()],
            dirs: vec![],
            errors: vec![],
            other: vec![],
            symlinks: vec![],
        };
        let candidates = collect_game_candidates(Path::new("/fake/root"), "", &toc);
        if cfg!(target_os = "windows") || cfg!(target_os = "macos") {
            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0].0, "portal2");
        }
    }
}

