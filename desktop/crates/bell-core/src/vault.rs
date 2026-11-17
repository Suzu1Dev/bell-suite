use crate::edition::{is_valid_target, AppId};
use chrono::{SecondsFormat, Utc};
use rusqlite::{backup::Backup, params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use thiserror::Error;

pub const DEFAULT_VAULT_QUOTA_BYTES: u64 = 5 * 1024 * 1024 * 1024; // 5 GiB

const PR_JSON_STORE_FILES: &[&str] = &[
    "state.json",
    "app-settings.json",
    "discovery.json",
    "coordination.json",
    "collab.json",
];

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("invalid preference: {0}")]
    InvalidPref(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopPrefs {
    pub theme: String,
    pub lang: String,
    pub active_target: String,
    pub auto_start: bool,
}

impl Default for DesktopPrefs {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            lang: "zh-tw".to_string(),
            active_target: "hub".to_string(),
            auto_start: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefsPatch {
    pub theme: Option<String>,
    pub lang: Option<String>,
    pub active_target: Option<String>,
    pub auto_start: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultDatabase {
    pub app_id: AppId,
    pub path: String,
    pub bytes: u64,
    pub healthy: bool,
    pub tables: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub root_dir: String,
    pub total_bytes: u64,
    pub quota_bytes: u64,
    pub backups_count: usize,
    pub last_backup_at: Option<String>,
    pub databases: Vec<VaultDatabase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultBackupResult {
    pub backup_dir: String,
    pub files: Vec<String>,
    pub total_bytes: u64,
    pub created_at: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupMeta {
    created_at: String,
}

pub fn default_vault_root() -> PathBuf {
    if let Ok(raw) = std::env::var("BELL_DESKTOP_VAULT_DIR") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("BELL");
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(local).join("BELL");
        }
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("BELL");
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            if !xdg.trim().is_empty() {
                return PathBuf::from(xdg).join("bell");
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".local").join("share").join("bell");
        }
    }
    std::env::temp_dir().join("bell-vault")
}

pub struct Vault {
    root_dir: PathBuf,
    prefs_db_path: PathBuf,
    prefs_lock: Mutex<()>,
}

impl Vault {
    pub fn open(root_dir: PathBuf) -> Result<Self, VaultError> {
        fs::create_dir_all(&root_dir)?;
        for app in AppId::ALL {
            fs::create_dir_all(root_dir.join(app.as_str()))?;
        }
        let prefs_db_path = root_dir.join("desktop-prefs.sqlite");
        let vault = Self {
            root_dir,
            prefs_db_path,
            prefs_lock: Mutex::new(()),
        };
        vault.init_prefs_db()?;
        Ok(vault)
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    /// Reads `<vault_root>/pr/api-token`, returning it only if it is a valid 64-char ASCII hex secret.
    pub fn read_pr_api_token(&self) -> Option<String> {
        read_pr_api_token_at_root(&self.root_dir)
    }

    /// Ensures `<vault_root>/pr/api-token` exists with a 64-char CSPRNG lowercase hex secret (`0600` on Unix).
    pub fn ensure_pr_api_token(&self) -> Result<String, VaultError> {
        Ok(ensure_pr_api_token_at_root(&self.root_dir)?)
    }

    /// Ensures `<vault_root>/pr/state.json` exists on fresh startup unless prior `state.json.bak.*` files exist.
    pub fn ensure_pr_initial_store(&self) -> Result<(), VaultError> {
        let pr_dir = self.root_dir.join(AppId::Pr.as_str());
        fs::create_dir_all(&pr_dir)?;
        let state_path = pr_dir.join("state.json");
        if state_path.exists() {
            return Ok(());
        }
        let has_bak = fs::read_dir(&pr_dir)
            .ok()
            .map(|entries| {
                entries.flatten().any(|e| {
                    e.file_name()
                        .to_str()
                        .is_some_and(|n| n.starts_with("state.json.bak."))
                })
            })
            .unwrap_or(false);
        if has_bak {
            return Ok(());
        }
        atomic_write_bytes(&state_path, b"{\"version\":1,\"repos\":[],\"runs\":[]}\n")?;
        Ok(())
    }

    fn open_prefs_conn(&self) -> Result<Connection, VaultError> {
        let conn = Connection::open(&self.prefs_db_path)?;
        conn.busy_timeout(Duration::from_secs(3))?;
        Ok(conn)
    }

    fn init_prefs_db(&self) -> Result<(), VaultError> {
        let _guard = self.prefs_lock.lock().unwrap_or_else(|e| e.into_inner());
        let conn = self.open_prefs_conn()?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS desktop_prefs (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 theme TEXT NOT NULL,
                 lang TEXT NOT NULL,
                 active_target TEXT NOT NULL,
                 auto_start INTEGER NOT NULL
             );",
        )?;
        let defaults = DesktopPrefs::default();
        conn.execute(
            "INSERT OR IGNORE INTO desktop_prefs (id, theme, lang, active_target, auto_start)
             VALUES (1, ?1, ?2, ?3, ?4)",
            params![
                defaults.theme,
                defaults.lang,
                defaults.active_target,
                i32::from(defaults.auto_start)
            ],
        )?;
        Ok(())
    }

    pub fn load_prefs(&self) -> Result<DesktopPrefs, VaultError> {
        let _guard = self.prefs_lock.lock().unwrap_or_else(|e| e.into_inner());
        let conn = self.open_prefs_conn()?;
        let prefs = conn.query_row(
            "SELECT theme, lang, active_target, auto_start FROM desktop_prefs WHERE id = 1",
            [],
            |row| {
                let auto_int: i32 = row.get(3)?;
                Ok(DesktopPrefs {
                    theme: row.get(0)?,
                    lang: row.get(1)?,
                    active_target: row.get(2)?,
                    auto_start: auto_int != 0,
                })
            },
        )?;
        Ok(prefs)
    }

    pub fn update_prefs(&self, patch: PrefsPatch) -> Result<DesktopPrefs, VaultError> {
        if let Some(ref theme) = patch.theme {
            if !matches!(theme.as_str(), "system" | "light" | "dark") {
                return Err(VaultError::InvalidPref(format!("invalid theme '{theme}'")));
            }
        }
        if let Some(ref lang) = patch.lang {
            if !matches!(lang.as_str(), "zh-tw" | "en") {
                return Err(VaultError::InvalidPref(format!("invalid lang '{lang}'")));
            }
        }
        if let Some(ref target) = patch.active_target {
            if !is_valid_target(target) {
                return Err(VaultError::InvalidPref(format!(
                    "invalid activeTarget '{target}'"
                )));
            }
        }

        let _guard = self.prefs_lock.lock().unwrap_or_else(|e| e.into_inner());
        let conn = self.open_prefs_conn()?;
        let mut current = conn.query_row(
            "SELECT theme, lang, active_target, auto_start FROM desktop_prefs WHERE id = 1",
            [],
            |row| {
                let auto_int: i32 = row.get(3)?;
                Ok(DesktopPrefs {
                    theme: row.get(0)?,
                    lang: row.get(1)?,
                    active_target: row.get(2)?,
                    auto_start: auto_int != 0,
                })
            },
        )?;

        if let Some(theme) = patch.theme {
            current.theme = theme;
        }
        if let Some(lang) = patch.lang {
            current.lang = lang;
        }
        if let Some(target) = patch.active_target {
            current.active_target = target;
        }
        if let Some(auto_start) = patch.auto_start {
            current.auto_start = auto_start;
        }

        conn.execute(
            "UPDATE desktop_prefs
             SET theme = ?1, lang = ?2, active_target = ?3, auto_start = ?4
             WHERE id = 1",
            params![
                current.theme,
                current.lang,
                current.active_target,
                i32::from(current.auto_start)
            ],
        )?;
        Ok(current)
    }

    /// Fast O(files) metadata-only scan without opening any SQLite connection or running `PRAGMA quick_check`.
    pub fn scan_fast(&self) -> VaultStatus {
        let mut databases = Vec::new();
        for app in AppId::ALL {
            let app_dir = self.root_dir.join(app.as_str());
            let mut found_paths = Vec::new();
            collect_sqlite_files(&app_dir, 1, 3, &mut found_paths);
            found_paths.sort();
            for path in found_paths {
                let bytes = sqlite_total_bytes(&path);
                databases.push(VaultDatabase {
                    app_id: app,
                    path: path.to_string_lossy().into_owned(),
                    bytes,
                    healthy: true,
                    tables: 0,
                });
            }
            if app == AppId::Pr {
                for json_path in collect_pr_json_stores(&app_dir) {
                    let bytes = fs::metadata(&json_path).map(|m| m.len()).unwrap_or(0);
                    databases.push(VaultDatabase {
                        app_id: AppId::Pr,
                        path: json_path.to_string_lossy().into_owned(),
                        bytes,
                        healthy: bytes > 0,
                        tables: usize::from(bytes > 0),
                    });
                }
            }
        }

        let total_bytes = databases.iter().map(|d| d.bytes).sum();
        let (backups_count, last_backup_at) = self.scan_backups();

        VaultStatus {
            root_dir: self.root_dir.to_string_lossy().into_owned(),
            total_bytes,
            quota_bytes: DEFAULT_VAULT_QUOTA_BYTES,
            backups_count,
            last_backup_at,
            databases,
        }
    }

    pub fn scan(&self) -> VaultStatus {
        let mut databases = Vec::new();
        for app in AppId::ALL {
            let app_dir = self.root_dir.join(app.as_str());
            let mut found_paths = Vec::new();
            collect_sqlite_files(&app_dir, 1, 3, &mut found_paths);
            found_paths.sort();
            for path in found_paths {
                let bytes = sqlite_total_bytes(&path);
                let (healthy, tables) = inspect_sqlite_db(&path);
                databases.push(VaultDatabase {
                    app_id: app,
                    path: path.to_string_lossy().into_owned(),
                    bytes,
                    healthy,
                    tables,
                });
            }
            if app == AppId::Pr {
                for json_path in collect_pr_json_stores(&app_dir) {
                    let bytes = fs::metadata(&json_path).map(|m| m.len()).unwrap_or(0);
                    let (healthy, tables) = inspect_json_store(&json_path);
                    databases.push(VaultDatabase {
                        app_id: AppId::Pr,
                        path: json_path.to_string_lossy().into_owned(),
                        bytes,
                        healthy,
                        tables,
                    });
                }
            }
        }

        let total_bytes = databases.iter().map(|d| d.bytes).sum();
        let (backups_count, last_backup_at) = self.scan_backups();

        VaultStatus {
            root_dir: self.root_dir.to_string_lossy().into_owned(),
            total_bytes,
            quota_bytes: DEFAULT_VAULT_QUOTA_BYTES,
            backups_count,
            last_backup_at,
            databases,
        }
    }

    fn scan_backups(&self) -> (usize, Option<String>) {
        let backups_root = self.root_dir.join("backups");
        let Ok(entries) = fs::read_dir(&backups_root) else {
            return (0, None);
        };
        let mut dirs: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        let count = dirs.len();
        let last_backup_at = dirs.last().and_then(|dir| {
            let meta_path = dir.join(".backup-meta.json");
            if let Ok(raw) = fs::read_to_string(&meta_path) {
                if let Ok(meta) = serde_json::from_str::<BackupMeta>(&raw) {
                    return Some(meta.created_at);
                }
            }
            dir.file_name()
                .and_then(|n| n.to_str())
                .and_then(parse_backup_dirname_to_iso)
        });
        (count, last_backup_at)
    }

    pub fn backup_vault(&self) -> Result<VaultBackupResult, VaultError> {
        let now = Utc::now();
        let created_at = now.to_rfc3339_opts(SecondsFormat::Secs, true);
        let stamp = now.format("%Y%m%d-%H%M%S").to_string();
        let backups_root = self.root_dir.join("backups");
        fs::create_dir_all(&backups_root)?;

        let mut backup_dir = backups_root.join(&stamp);
        let mut suffix = 2u32;
        while backup_dir.exists() {
            backup_dir = backups_root.join(format!("{stamp}-{suffix}"));
            suffix += 1;
        }
        fs::create_dir_all(&backup_dir)?;

        let mut files = Vec::new();
        let mut total_bytes = 0u64;

        for app in AppId::ALL {
            let app_dir = self.root_dir.join(app.as_str());
            let mut found_paths = Vec::new();
            collect_sqlite_files(&app_dir, 1, 3, &mut found_paths);
            found_paths.sort();

            for src_path in found_paths {
                let rel_in_app = src_path.strip_prefix(&app_dir).unwrap_or(&src_path);
                let rel_in_backup = Path::new(app.as_str()).join(rel_in_app);
                let dst_path = backup_dir.join(&rel_in_backup);
                if let Some(parent) = dst_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                hot_backup_sqlite(&src_path, &dst_path)?;
                let snap_bytes = fs::metadata(&dst_path).map(|m| m.len()).unwrap_or(0);
                total_bytes += snap_bytes;
                files.push(rel_in_backup.to_string_lossy().replace('\\', "/"));
            }

            if app == AppId::Pr {
                for src_json in collect_pr_json_stores(&app_dir) {
                    let Some(name) = src_json.file_name() else {
                        continue;
                    };
                    let rel_in_backup = Path::new(AppId::Pr.as_str()).join(name);
                    let dst_path = backup_dir.join(&rel_in_backup);
                    if let Some(parent) = dst_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    let raw_bytes = fs::read(&src_json)?;
                    atomic_write_bytes(&dst_path, &raw_bytes)?;
                    total_bytes += raw_bytes.len() as u64;
                    files.push(rel_in_backup.to_string_lossy().replace('\\', "/"));
                }
            }
        }

        let meta_json = serde_json::to_string(&BackupMeta {
            created_at: created_at.clone(),
        })
        .unwrap_or_default();
        fs::write(backup_dir.join(".backup-meta.json"), meta_json)?;

        Ok(VaultBackupResult {
            backup_dir: backup_dir.to_string_lossy().into_owned(),
            files,
            total_bytes,
            created_at,
        })
    }
}

fn hot_backup_sqlite(src_path: &Path, dst_path: &Path) -> Result<(), VaultError> {
    let src = Connection::open_with_flags(
        src_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    src.busy_timeout(Duration::from_secs(5))?;
    let mut dst = Connection::open(dst_path)?;
    dst.busy_timeout(Duration::from_secs(5))?;
    {
        let backup = Backup::new(&src, &mut dst)?;
        backup.step(-1)?;
    }
    Ok(())
}

fn collect_sqlite_files(dir: &Path, depth: usize, max_depth: usize, out: &mut Vec<PathBuf>) {
    if depth > max_depth || !dir.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_dir() {
            collect_sqlite_files(&path, depth + 1, max_depth, out);
        } else if ft.is_file() && is_sqlite_extension(&path) {
            out.push(path);
        }
    }
}

fn is_sqlite_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "db" | "sqlite" | "sqlite3"
            )
        })
        .unwrap_or(false)
}

fn sqlite_total_bytes(path: &Path) -> u64 {
    let base = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let path_str = path.to_string_lossy();
    let wal = fs::metadata(format!("{path_str}-wal"))
        .map(|m| m.len())
        .unwrap_or(0);
    let shm = fs::metadata(format!("{path_str}-shm"))
        .map(|m| m.len())
        .unwrap_or(0);
    base + wal + shm
}

fn inspect_sqlite_db(path: &Path) -> (bool, usize) {
    let Ok(conn) = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return (false, 0);
    };
    let _ = conn.busy_timeout(Duration::from_millis(1500));

    let healthy = conn
        .query_row("PRAGMA quick_check(1)", [], |r| r.get::<_, String>(0))
        .map(|s| s.eq_ignore_ascii_case("ok"))
        .unwrap_or(false);

    if !healthy {
        return (false, 0);
    }

    let tables = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n.max(0) as usize)
        .unwrap_or(0);

    (healthy, tables)
}

pub fn read_pr_api_token_at_root(vault_root: &Path) -> Option<String> {
    let token_path = vault_root.join(AppId::Pr.as_str()).join("api-token");
    let raw = fs::read_to_string(token_path).ok()?;
    let trimmed = raw.trim();
    if trimmed.len() == 64 && trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(trimmed.to_ascii_lowercase())
    } else {
        None
    }
}

/// Ensures `<vault_root>/pr/api-token` exists using OS CSPRNG without opening SQLite.
pub fn ensure_pr_api_token_at_root(vault_root: &Path) -> std::io::Result<String> {
    if let Some(existing) = read_pr_api_token_at_root(vault_root) {
        return Ok(existing);
    }
    let pr_dir = vault_root.join(AppId::Pr.as_str());
    fs::create_dir_all(&pr_dir)?;
    let token_path = pr_dir.join("api-token");

    let mut random_bytes = [0u8; 32];
    getrandom::getrandom(&mut random_bytes).map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut tok = String::with_capacity(64);
    for b in random_bytes {
        let _ = write!(tok, "{b:02x}");
    }

    let tmp_path = pr_dir.join(format!("api-token.tmp-{}", std::process::id()));
    fs::write(&tmp_path, format!("{tok}\n"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&tmp_path, &token_path)?;
    Ok(tok)
}

const B64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn encode_base64_32(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(44);
    let mut i = 0;
    while i < 30 {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8) | (bytes[i + 2] as u32);
        out.push(B64_ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(B64_ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        out.push(B64_ALPHABET[((n >> 6) & 0x3f) as usize] as char);
        out.push(B64_ALPHABET[(n & 0x3f) as usize] as char);
        i += 3;
    }
    let b30 = bytes[30] as u32;
    let b31 = bytes[31] as u32;
    out.push(B64_ALPHABET[(b30 >> 2) as usize] as char);
    out.push(B64_ALPHABET[(((b30 & 0x03) << 4) | (b31 >> 4)) as usize] as char);
    out.push(B64_ALPHABET[((b31 & 0x0f) << 2) as usize] as char);
    out.push('=');
    out
}

fn is_valid_base64_32(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 44 || b[43] != b'=' {
        return false;
    }
    let is_b64_char = |c: u8| c.is_ascii_alphanumeric() || c == b'+' || c == b'/';
    if !b[..42].iter().copied().all(is_b64_char) {
        return false;
    }
    // Canonical 32-byte Base64 requires the two padding bits in char 42 to be zero.
    matches!(
        b[42],
        b'A' | b'E'
            | b'I'
            | b'M'
            | b'Q'
            | b'U'
            | b'Y'
            | b'c'
            | b'g'
            | b'k'
            | b'o'
            | b's'
            | b'w'
            | b'0'
            | b'4'
            | b'8'
    )
}

pub fn read_personal_master_key_at_root(vault_root: &Path) -> Option<String> {
    let key_path = vault_root.join("personal-secrets").join("master.key");
    let raw = fs::read_to_string(key_path).ok()?;
    let trimmed = raw.trim();
    if is_valid_base64_32(trimmed) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Ensures `<vault_root>/personal-secrets/master.key` (32-byte canonical Base64) exists
/// outside `<vault_root>/personal` so SQLite backups never carry credential secrets.
pub fn ensure_personal_master_key_at_root(vault_root: &Path) -> std::io::Result<String> {
    if let Some(existing) = read_personal_master_key_at_root(vault_root) {
        return Ok(existing);
    }
    let secrets_dir = vault_root.join("personal-secrets");
    fs::create_dir_all(&secrets_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&secrets_dir, fs::Permissions::from_mode(0o700));
    }
    let key_path = secrets_dir.join("master.key");

    let mut random_bytes = [0u8; 32];
    getrandom::getrandom(&mut random_bytes).map_err(|e| std::io::Error::other(e.to_string()))?;
    let encoded = encode_base64_32(&random_bytes);

    let tmp_path = secrets_dir.join(format!("master.key.tmp-{}", std::process::id()));
    fs::write(&tmp_path, format!("{encoded}\n"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&tmp_path, &key_path)?;
    Ok(encoded)
}

fn collect_pr_json_stores(pr_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for &file_name in PR_JSON_STORE_FILES {
        let candidate = pr_dir.join(file_name);
        if candidate.is_file() {
            out.push(candidate);
        }
    }
    out.sort();
    out
}

fn inspect_json_store(path: &Path) -> (bool, usize) {
    let Ok(bytes) = fs::read(path) else {
        return (false, 0);
    };
    if serde_json::from_slice::<serde_json::Value>(&bytes).is_ok() {
        (true, 1)
    } else {
        (false, 0)
    }
}

fn atomic_write_bytes(dst_path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let tmp_path = dst_path.with_extension("json.tmp-backup");
    fs::write(&tmp_path, bytes)?;
    fs::rename(&tmp_path, dst_path)?;
    Ok(())
}

fn parse_backup_dirname_to_iso(name: &str) -> Option<String> {
    if name.len() < 15 {
        return None;
    }
    let s = &name[..15];
    let b = s.as_bytes();
    if b[8] != b'-'
        || !b[..8].iter().all(|c| c.is_ascii_digit())
        || !b[9..15].iter().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    Some(format!(
        "{}-{}-{}T{}:{}:{}Z",
        &s[0..4],
        &s[4..6],
        &s[6..8],
        &s[9..11],
        &s[11..13],
        &s[13..15]
    ))
}
