use crate::contract::{
    aggregate_today_with_tokens, build_suite_env, read_bounded_bytes, validate_manifest,
    TodayOverview, MAX_FETCH_BYTES,
};
use crate::edition::{is_valid_target, AppId, Edition};
use crate::vault::{
    default_vault_root, DesktopPrefs, PrefsPatch, Vault, VaultBackupResult, VaultError,
    VaultStatus,
};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::net::TcpListener;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tar::Archive;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader as TokioBufReader};
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, watch, Mutex as AsyncMutex};

pub const LOG_RING_CAPACITY: usize = 200;
const VAULT_CACHE_TTL: Duration = Duration::from_secs(8);

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("vault error: {0}")]
    Vault(#[from] VaultError),
    #[error("invalid target '{0}'")]
    InvalidTarget(String),
    #[error("invalid action '{0}'")]
    InvalidAction(String),
    #[error("workspace '{0}' is not installed")]
    NotInstalled(AppId),
    #[error("node runtime not found; install Node.js 22+ or bundle bell-runtime")]
    MissingRuntime,
    #[error("failed to spawn '{0}': {1}")]
    SpawnFailed(AppId, String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceState {
    Stopped,
    Starting,
    Running,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub kind: String,
    pub path: String,
    pub claude_cli: bool,
    pub codex_cli: bool,
    pub gh_cli: bool,
    pub git_cli: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatus {
    pub id: AppId,
    pub name: String,
    pub shortcut_index: u8,
    pub accent_light: String,
    pub accent_dark: String,
    pub port: u16,
    pub url: String,
    pub state: WorkspaceState,
    pub pid: Option<u32>,
    pub manifest_valid: bool,
    pub installed: bool,
    pub db_bytes: u64,
    pub restart_count: u32,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopStatus {
    pub edition: Edition,
    pub version: String,
    pub os: String,
    pub arch: String,
    pub active_target: String,
    pub runtime: RuntimeInfo,
    pub prefs: DesktopPrefs,
    pub workspaces: Vec<WorkspaceStatus>,
    pub vault: VaultStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLogs {
    pub id: AppId,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    pub edition: Edition,
    pub base_port: u16,
    pub vault_dir: Option<PathBuf>,
    pub sidecars_dir: Option<PathBuf>,
    pub resource_dir: Option<PathBuf>,
    pub allow_dev_fallback: bool,
    pub ready_timeout: Duration,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            edition: Edition::active(),
            base_port: 4600,
            vault_dir: None,
            sidecars_dir: None,
            resource_dir: None,
            allow_dev_fallback: true,
            ready_timeout: Duration::from_secs(25),
        }
    }
}

struct RunningHandle {
    generation: u64,
    kill_tx: Option<oneshot::Sender<()>>,
    exited_rx: watch::Receiver<Option<String>>,
}

struct WorkspaceSlot {
    port: u16,
    state: WorkspaceState,
    pid: Option<u32>,
    manifest_valid: bool,
    restart_count: u32,
    last_error: Option<String>,
    generation: u64,
    handle: Option<RunningHandle>,
    logs: Arc<Mutex<VecDeque<String>>>,
}

struct InnerState {
    active_target: String,
    prefs: DesktopPrefs,
    slots: BTreeMap<AppId, WorkspaceSlot>,
}

#[derive(Debug, Clone)]
enum AppLaunchSource {
    Sidecar { dir: PathBuf, entry: PathBuf },
    CheckoutBin { dir: PathBuf, bin_path: PathBuf },
    CheckoutTsx { dir: PathBuf, tsx_bin: PathBuf },
}

pub struct Supervisor {
    config: SupervisorConfig,
    vault: Vault,
    sidecars_dir: Option<PathBuf>,
    suite_root: Option<PathBuf>,
    state: Arc<Mutex<InnerState>>,
    sidecars_unpacked: Mutex<bool>,
    runtime_cache: Mutex<Option<RuntimeInfo>>,
    vault_cache: Mutex<Option<(Instant, VaultStatus, bool)>>,
    today_cache: Mutex<Option<(Instant, TodayOverview)>>,
    op_locks: BTreeMap<AppId, Arc<AsyncMutex<()>>>,
    http_client: reqwest::Client,
}

impl Supervisor {
    pub fn new(config: SupervisorConfig) -> Result<Arc<Self>, SupervisorError> {
        let vault_root = config.vault_dir.clone().unwrap_or_else(default_vault_root);
        let vault = Vault::open(vault_root)?;
        let mut prefs = vault.load_prefs().unwrap_or_default();
        let initial_target = if is_valid_target(&prefs.active_target) {
            prefs.active_target.clone()
        } else {
            "hub".to_string()
        };
        prefs.active_target = initial_target.clone();

        let sidecars_dir = resolve_sidecars_dir(&config);
        let suite_root = if config.allow_dev_fallback && sidecars_dir.is_none() {
            discover_suite_root()
        } else {
            None
        };

        let mut slots = BTreeMap::new();
        let mut op_locks = BTreeMap::new();
        for app in AppId::ALL {
            slots.insert(
                app,
                WorkspaceSlot {
                    port: config.base_port.saturating_add(app.port_offset()),
                    state: WorkspaceState::Stopped,
                    pid: None,
                    manifest_valid: false,
                    restart_count: 0,
                    last_error: None,
                    generation: 0,
                    handle: None,
                    logs: Arc::new(Mutex::new(VecDeque::with_capacity(LOG_RING_CAPACITY))),
                },
            );
            op_locks.insert(app, Arc::new(AsyncMutex::new(())));
        }

        let http_client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_millis(1500))
            .build()
            .unwrap_or_default();

        Ok(Arc::new(Self {
            config,
            vault,
            sidecars_dir,
            suite_root,
            state: Arc::new(Mutex::new(InnerState {
                active_target: initial_target,
                prefs,
                slots,
            })),
            sidecars_unpacked: Mutex::new(false),
            runtime_cache: Mutex::new(None),
            vault_cache: Mutex::new(None),
            today_cache: Mutex::new(None),
            op_locks,
            http_client,
        }))
    }

    pub fn edition(&self) -> Edition {
        self.config.edition
    }

    pub fn vault(&self) -> &Vault {
        &self.vault
    }

    pub fn cached_theme_pref(&self) -> String {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .prefs
            .theme
            .clone()
    }

    pub fn cached_auto_start_pref(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .prefs
            .auto_start
    }

    pub fn set_active_target_in_memory(&self, target: &str) {
        let norm = target.trim().to_ascii_lowercase();
        if is_valid_target(&norm) {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            guard.active_target = norm.clone();
            guard.prefs.active_target = norm;
        }
    }

    pub fn pr_api_token(&self) -> Option<String> {
        self.vault.read_pr_api_token()
    }

    pub fn ensure_pr_api_token(&self) -> Result<String, SupervisorError> {
        Ok(self.vault.ensure_pr_api_token()?)
    }

    pub fn preallocate_all_ports(&self) {
        self.preallocate_ports(&AppId::ALL);
    }

    fn extracted_cache_root(&self) -> PathBuf {
        self.vault.root_dir().join("runtime-cache").join(format!(
            "{}-{}",
            self.config.edition.as_str(),
            env!("CARGO_PKG_VERSION")
        ))
    }

    /// Idempotently unpacks `<sidecars_dir>/apps.tar.gz` into `<vault_root>/runtime-cache/<edition>-<version>/`
    /// when loose `apps/<id>/server.cjs` files are not present on disk.
    pub fn ensure_sidecars_ready(&self) {
        let mut unpacked = self
            .sidecars_unpacked
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if *unpacked {
            return;
        }
        let Some(ref sc_dir) = self.sidecars_dir else {
            *unpacked = true;
            return;
        };

        if AppId::ALL
            .iter()
            .any(|app| sc_dir.join("apps").join(app.as_str()).join("server.cjs").is_file())
        {
            *unpacked = true;
            return;
        }

        let archive_path = sc_dir.join("apps.tar.gz");
        if !archive_path.is_file() {
            *unpacked = true;
            return;
        }

        let cache_dir = self.extracted_cache_root();
        let ready_marker = cache_dir.join(".ready");
        if ready_marker.is_file() && cache_dir.join("apps").is_dir() {
            *unpacked = true;
            return;
        }

        if unpack_apps_archive(&archive_path, &cache_dir).is_ok() {
            let _ = fs::write(&ready_marker, env!("CARGO_PKG_VERSION"));
            *unpacked = true;
        }
    }

    fn invalidate_vault_cache(&self) {
        *self.vault_cache.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    fn ordered_apps(&self) -> Vec<AppId> {
        if let Some(primary) = self.config.edition.primary_app() {
            let mut list = vec![primary];
            for app in AppId::ALL {
                if app != primary {
                    list.push(app);
                }
            }
            list
        } else {
            AppId::ALL.to_vec()
        }
    }

    fn resolve_bundled_runtime(&self) -> Option<(String, String)> {
        if let Ok(raw) = std::env::var("BELL_DESKTOP_RUNTIME_BIN") {
            let p = PathBuf::from(raw.trim());
            if p.is_file() {
                return Some(("bundled".to_string(), p.to_string_lossy().into_owned()));
            }
        }
        if let Some(ref sc_dir) = self.sidecars_dir {
            let bin_name = if cfg!(windows) {
                "bell-runtime.exe"
            } else {
                "bell-runtime"
            };
            let bundled = sc_dir.join("bin").join(bin_name);
            if bundled.is_file() {
                return Some((
                    "bundled".to_string(),
                    bundled.to_string_lossy().into_owned(),
                ));
            }
        }
        None
    }

    fn resolve_runtime(&self) -> (String, String) {
        if let Some(bundled) = self.resolve_bundled_runtime() {
            return bundled;
        }
        if let Some(cached) = self
            .runtime_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            if !cached.path.is_empty() {
                return (cached.kind.clone(), cached.path.clone());
            }
        }
        if let Some(node) = find_executable("node") {
            return ("system".to_string(), node.to_string_lossy().into_owned());
        }
        ("missing".to_string(), String::new())
    }

    pub fn runtime_info_fast(&self) -> RuntimeInfo {
        if let Some(cached) = self
            .runtime_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            return cached;
        }
        if let Some((kind, path)) = self.resolve_bundled_runtime() {
            return RuntimeInfo {
                kind,
                path,
                claude_cli: false,
                codex_cli: false,
                gh_cli: false,
                git_cli: false,
            };
        }
        let (kind, path) = match find_executable("node") {
            Some(node) => ("system".to_string(), node.to_string_lossy().into_owned()),
            None => ("missing".to_string(), String::new()),
        };
        RuntimeInfo {
            kind,
            path,
            claude_cli: false,
            codex_cli: false,
            gh_cli: false,
            git_cli: false,
        }
    }

    pub fn runtime_info(&self) -> RuntimeInfo {
        let (kind, path) = self.resolve_runtime();
        let info = RuntimeInfo {
            kind,
            path,
            claude_cli: find_executable("claude").is_some(),
            codex_cli: find_executable("codex").is_some(),
            gh_cli: find_executable("gh").is_some(),
            git_cli: find_executable("git").is_some(),
        };
        *self
            .runtime_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(info.clone());
        info
    }

    pub fn refresh_caches_blocking(&self) {
        let _ = self.runtime_info();
        let deep = self.vault.scan();
        *self.vault_cache.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((Instant::now(), deep, true));
    }

    fn discover_app_source(&self, app: AppId) -> Option<AppLaunchSource> {
        if let Some(ref sc_dir) = self.sidecars_dir {
            let app_dir = sc_dir.join("apps").join(app.as_str());
            let entry = app_dir.join("server.cjs");
            if entry.is_file() {
                return Some(AppLaunchSource::Sidecar {
                    dir: app_dir,
                    entry,
                });
            }

            let cache_app_dir = self.extracted_cache_root().join("apps").join(app.as_str());
            let cache_entry = cache_app_dir.join("server.cjs");
            if cache_entry.is_file() {
                return Some(AppLaunchSource::Sidecar {
                    dir: cache_app_dir,
                    entry: cache_entry,
                });
            }
        }

        if !self.config.allow_dev_fallback {
            return None;
        }

        let mut candidate_dirs = Vec::new();
        let env_key = format!("BELL_SUITE_APP_{}", app.as_str().to_ascii_uppercase());
        if let Ok(override_dir) = std::env::var(&env_key) {
            if !override_dir.trim().is_empty() {
                candidate_dirs.push(PathBuf::from(override_dir.trim()));
            }
        }

        let root_opt = self.suite_root.clone().or_else(discover_suite_root);
        if let Some(ref root) = root_opt {
            let rels: &[&str] = match app {
                AppId::Personal => &[
                    "worktrees/personal-bot-suite",
                    "../bell-personal",
                    "../BELL-Personal",
                ],
                AppId::Explain => &[
                    "worktrees/explain-launch",
                    "../bell-explain",
                    "../BELL-Explain",
                ],
                AppId::Pr => &["../bell-pr", "../BELL-PR"],
                AppId::Reader => &["../bell-reader", "../BELL-Reader"],
                AppId::Campaign => &["../bell-campaign", "../BELL-Campaign"],
            };
            for rel in rels {
                candidate_dirs.push(root.join(rel));
            }
        }

        if let Ok(home) = std::env::var("HOME") {
            candidate_dirs.push(
                PathBuf::from(home)
                    .join(".local")
                    .join("share")
                    .join(format!("bell-{}", app.as_str()))
                    .join("app"),
            );
        }

        let bin_rel = match app {
            AppId::Personal => "bin/bell-personal.mjs",
            AppId::Explain => "bin/bell-explain.mjs",
            AppId::Pr => "bin/bellpr.mjs",
            AppId::Reader => "bin/bell-reader.mjs",
            AppId::Campaign => "bin/bell-campaign.mjs",
        };

        for dir in &candidate_dirs {
            if dir.join("package.json").is_file() && dir.join(bin_rel).is_file() {
                return Some(AppLaunchSource::CheckoutBin {
                    dir: dir.clone(),
                    bin_path: dir.join(bin_rel),
                });
            }
        }
        for dir in &candidate_dirs {
            let tsx_name = if cfg!(windows) { "tsx.cmd" } else { "tsx" };
            let tsx_bin = dir.join("node_modules").join(".bin").join(tsx_name);
            if dir.join("package.json").is_file()
                && tsx_bin.is_file()
                && dir.join("server").join("index.ts").is_file()
            {
                return Some(AppLaunchSource::CheckoutTsx {
                    dir: dir.clone(),
                    tsx_bin,
                });
            }
        }

        None
    }

    fn is_app_installed(&self, app: AppId) -> bool {
        if self.discover_app_source(app).is_some() {
            return true;
        }
        if self
            .sidecars_dir
            .as_ref()
            .is_some_and(|d| d.join("apps.tar.gz").is_file())
        {
            return match self.config.edition.primary_app() {
                None => true,
                Some(primary) => primary == app,
            };
        }
        false
    }

    fn cached_or_fast_vault_status(&self) -> VaultStatus {
        {
            let guard = self.vault_cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((ts, ref cached, _)) = *guard {
                if ts.elapsed() < VAULT_CACHE_TTL {
                    return cached.clone();
                }
            }
        }
        let fast = self.vault.scan_fast();
        *self.vault_cache.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((Instant::now(), fast.clone(), false));
        fast
    }

    fn cached_or_deep_vault_status(&self) -> VaultStatus {
        {
            let guard = self.vault_cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((ts, ref cached, is_deep)) = *guard {
                if is_deep && ts.elapsed() < VAULT_CACHE_TTL {
                    return cached.clone();
                }
            }
        }
        let deep = self.vault.scan();
        *self.vault_cache.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((Instant::now(), deep.clone(), true));
        deep
    }

    fn build_status(&self, vault_status: VaultStatus) -> DesktopStatus {
        let mut db_bytes_by_app: BTreeMap<AppId, u64> = BTreeMap::new();
        for db in &vault_status.databases {
            *db_bytes_by_app.entry(db.app_id).or_insert(0) += db.bytes;
        }

        let runtime = self.runtime_info_fast();
        let apps = self.ordered_apps();
        let installed_flags: BTreeMap<AppId, bool> = apps
            .iter()
            .map(|&app| (app, self.is_app_installed(app)))
            .collect();

        let guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let active_target = guard.active_target.clone();
        let mut prefs = guard.prefs.clone();
        prefs.active_target = active_target.clone();

        let mut workspaces = Vec::with_capacity(apps.len());
        for app in apps {
            let slot = &guard.slots[&app];
            workspaces.push(WorkspaceStatus {
                id: app,
                name: app.display_name().to_string(),
                shortcut_index: app.shortcut_index(),
                accent_light: app.accent_light().to_string(),
                accent_dark: app.accent_dark().to_string(),
                port: slot.port,
                url: format!("http://127.0.0.1:{}", slot.port),
                state: slot.state,
                pid: slot.pid,
                manifest_valid: slot.manifest_valid,
                installed: installed_flags.get(&app).copied().unwrap_or(false),
                db_bytes: db_bytes_by_app.get(&app).copied().unwrap_or(0),
                restart_count: slot.restart_count,
                last_error: slot.last_error.clone(),
            });
        }
        drop(guard);

        DesktopStatus {
            edition: self.config.edition,
            version: env!("CARGO_PKG_VERSION").to_string(),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            active_target,
            runtime,
            prefs,
            workspaces,
            vault: vault_status,
        }
    }

    pub fn status(&self) -> DesktopStatus {
        self.build_status(self.cached_or_fast_vault_status())
    }

    pub fn status_with_deep_vault(&self) -> DesktopStatus {
        self.build_status(self.cached_or_deep_vault_status())
    }

    pub fn cached_today(&self) -> Option<TodayOverview> {
        self.today_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|(_, ov)| ov.clone())
    }

    pub fn is_today_cache_fresh(&self, max_age: Duration) -> bool {
        self.today_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|(ts, _)| ts.elapsed() < max_age)
    }

    pub async fn refresh_today(&self) -> TodayOverview {
        let running: Vec<(AppId, u16)> = {
            let guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            guard
                .slots
                .iter()
                .filter(|(_, s)| s.state == WorkspaceState::Running)
                .map(|(&id, s)| (id, s.port))
                .collect()
        };
        let mut tokens = BTreeMap::new();
        if let Some(tok) = self.pr_api_token() {
            tokens.insert(AppId::Pr, tok);
        }
        let overview = aggregate_today_with_tokens(&running, &tokens).await;
        *self.today_cache.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((Instant::now(), overview.clone()));
        overview
    }

    pub async fn today(&self) -> TodayOverview {
        self.refresh_today().await
    }

    pub fn activate(&self, target: &str) -> Result<DesktopStatus, SupervisorError> {
        let normalized = target.trim().to_ascii_lowercase();
        if !is_valid_target(&normalized) {
            return Err(SupervisorError::InvalidTarget(target.to_string()));
        }
        {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            guard.active_target = normalized.clone();
            guard.prefs.active_target = normalized.clone();
        }
        let _ = self.vault.update_prefs(PrefsPatch {
            active_target: Some(normalized),
            ..Default::default()
        });
        Ok(self.status())
    }

    pub fn save_prefs(&self, mut patch: PrefsPatch) -> Result<DesktopStatus, SupervisorError> {
        if let Some(ref target) = patch.active_target {
            let norm = target.trim().to_ascii_lowercase();
            if !is_valid_target(&norm) {
                return Err(SupervisorError::InvalidTarget(target.clone()));
            }
            patch.active_target = Some(norm);
        }
        let updated = self.vault.update_prefs(patch)?;
        {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            guard.active_target = updated.active_target.clone();
            guard.prefs = updated;
        }
        Ok(self.status())
    }

    pub fn backup_vault(&self) -> Result<VaultBackupResult, SupervisorError> {
        let res = self.vault.backup_vault()?;
        self.invalidate_vault_cache();
        Ok(res)
    }

    pub fn logs(&self, id: AppId) -> WorkspaceLogs {
        let logs_arc = {
            let guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            guard.slots[&id].logs.clone()
        };
        let lines = logs_arc
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect();
        WorkspaceLogs { id, lines }
    }

    pub async fn workspace_action(
        self: &Arc<Self>,
        id: AppId,
        action: &str,
    ) -> Result<DesktopStatus, SupervisorError> {
        match action {
            "start" => self.start_workspace(id).await,
            "stop" => self.stop_workspace(id).await,
            "restart" => self.restart_workspace(id).await,
            other => Err(SupervisorError::InvalidAction(other.to_string())),
        }
    }

    pub async fn start_workspace(
        self: &Arc<Self>,
        id: AppId,
    ) -> Result<DesktopStatus, SupervisorError> {
        let op_lock = self.op_locks[&id].clone();
        let _op_guard = op_lock.lock().await;
        self.start_workspace_locked(id).await?;
        Ok(self.status())
    }

    async fn start_workspace_locked(self: &Arc<Self>, id: AppId) -> Result<(), SupervisorError> {
        {
            let guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if guard.slots[&id].state == WorkspaceState::Running {
                return Ok(());
            }
        }

        self.ensure_sidecars_ready();

        let Some(source) = self.discover_app_source(id) else {
            return Err(SupervisorError::NotInstalled(id));
        };

        let (_, runtime_path) = self.resolve_runtime();
        if matches!(
            source,
            AppLaunchSource::Sidecar { .. } | AppLaunchSource::CheckoutBin { .. }
        ) && runtime_path.is_empty()
        {
            return Err(SupervisorError::MissingRuntime);
        }

        // Allocate collision-free port on 127.0.0.1 (preserving pre-allocated fallback port if bindable)
        let preferred_port = self.config.base_port.saturating_add(id.port_offset());
        let port = {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let current_port = guard.slots[&id].port;
            let is_taken_by_sibling = |candidate: u16| {
                guard.slots.iter().any(|(&other, s)| {
                    other != id
                        && matches!(s.state, WorkspaceState::Starting | WorkspaceState::Running)
                        && s.port == candidate
                })
            };
            let chosen = if !is_taken_by_sibling(current_port) && can_bind_loopback(current_port) {
                current_port
            } else if !is_taken_by_sibling(preferred_port) && can_bind_loopback(preferred_port) {
                preferred_port
            } else {
                allocate_ephemeral_port().unwrap_or(preferred_port)
            };
            guard.slots.get_mut(&id).unwrap().port = chosen;
            chosen
        };

        let ports_by_app: BTreeMap<AppId, u16> = {
            let guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            guard.slots.iter().map(|(&k, s)| (k, s.port)).collect()
        };

        let app_vault_dir = self.vault.root_dir().join(id.as_str());
        let _ = std::fs::create_dir_all(&app_vault_dir);
        if id == AppId::Pr {
            let _ = self.vault.ensure_pr_initial_store();
        }
        let explicit_personal_key = std::env::var("BELLPERSONAL_MASTER_KEY")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        if id == AppId::Personal && explicit_personal_key.is_none() {
            let _ = crate::vault::ensure_personal_master_key_at_root(self.vault.root_dir());
        }
        let mut suite_env = build_suite_env(id, &ports_by_app, "127.0.0.1", self.vault.root_dir());
        if let Some(explicit_key) = explicit_personal_key {
            suite_env.insert("BELLPERSONAL_MASTER_KEY".into(), explicit_key);
        }

        let mut cmd = match source {
            AppLaunchSource::Sidecar { dir, entry } => {
                let mut c = Command::new(&runtime_path);
                c.arg(entry).current_dir(dir);
                c
            }
            AppLaunchSource::CheckoutBin { dir, bin_path } => {
                let mut c = Command::new(&runtime_path);
                c.arg(bin_path)
                    .arg("start")
                    .arg("--port")
                    .arg(port.to_string())
                    .arg("--host")
                    .arg("127.0.0.1")
                    .arg("--no-open");
                if id == AppId::Pr {
                    c.arg("--force");
                }
                c.current_dir(dir);
                c
            }
            AppLaunchSource::CheckoutTsx { dir, tsx_bin } => {
                let mut c = Command::new(tsx_bin);
                c.arg("server/index.ts").current_dir(dir);
                c
            }
        };

        cmd.envs(&suite_env)
            .env("PATH", augmented_path_env())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        #[cfg(windows)]
        {
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| SupervisorError::SpawnFailed(id, e.to_string()))?;

        let pid = child.id();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let (kill_tx, mut kill_rx) = oneshot::channel::<()>();
        let (exited_tx, mut exited_rx) = watch::channel::<Option<String>>(None);

        let (generation, logs_buf) = {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let slot = guard.slots.get_mut(&id).unwrap();
            slot.generation += 1;
            let gen = slot.generation;
            slot.state = WorkspaceState::Starting;
            slot.pid = pid;
            slot.manifest_valid = false;
            slot.last_error = None;
            slot.handle = Some(RunningHandle {
                generation: gen,
                kill_tx: Some(kill_tx),
                exited_rx: exited_rx.clone(),
            });
            (gen, slot.logs.clone())
        };

        if let Some(out) = stdout {
            let buf = logs_buf.clone();
            tokio::spawn(async move {
                let mut lines = TokioBufReader::new(out).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    push_log_line(&buf, line);
                }
            });
        }
        if let Some(err) = stderr {
            let buf = logs_buf.clone();
            tokio::spawn(async move {
                let mut lines = TokioBufReader::new(err).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    push_log_line(&buf, format!("[stderr] {line}"));
                }
            });
        }

        let state_ref = self.state.clone();
        tokio::spawn(async move {
            tokio::select! {
                res = child.wait() => {
                    let msg = match res {
                        Ok(st) => format!("Process exited ({st})"),
                        Err(e) => format!("Process wait error: {e}"),
                    };
                    let _ = exited_tx.send(Some(msg.clone()));
                    let mut guard = state_ref.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(slot) = guard.slots.get_mut(&id) {
                        if slot.generation == generation {
                            slot.state = WorkspaceState::Error;
                            slot.pid = None;
                            slot.manifest_valid = false;
                            slot.last_error = Some(msg);
                            slot.handle = None;
                        }
                    }
                }
                _ = &mut kill_rx => {
                    terminate_child(&mut child).await;
                    let _ = exited_tx.send(Some("stopped".to_string()));
                }
            }
        });

        // Wait 45ms before first HTTP probe so Node/V8 binds its listener without a wasted SYN
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(45)) => {}
            _ = exited_rx.changed() => {}
        }

        let deadline = Instant::now() + self.config.ready_timeout;
        loop {
            if let Some(reason) = exited_rx.borrow().clone() {
                let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
                let slot = guard.slots.get_mut(&id).unwrap();
                if slot.generation == generation && slot.state != WorkspaceState::Stopped {
                    slot.state = WorkspaceState::Error;
                    slot.pid = None;
                    slot.last_error = Some(reason);
                }
                return Ok(());
            }

            if let Some(manifest_valid) = self.probe_ready(id, port).await {
                {
                    let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
                    let slot = guard.slots.get_mut(&id).unwrap();
                    if slot.generation == generation {
                        slot.state = WorkspaceState::Running;
                        slot.manifest_valid = manifest_valid;
                        slot.last_error = None;
                    }
                }
                self.invalidate_vault_cache();
                return Ok(());
            }

            if Instant::now() >= deadline {
                let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
                let slot = guard.slots.get_mut(&id).unwrap();
                if slot.generation == generation {
                    slot.state = WorkspaceState::Error;
                    slot.last_error = Some(format!(
                        "Timed out waiting for workspace on 127.0.0.1:{port}"
                    ));
                }
                return Ok(());
            }

            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(35)) => {}
                _ = exited_rx.changed() => {}
            }
        }
    }

    async fn probe_ready(&self, id: AppId, port: u16) -> Option<bool> {
        let manifest_url = format!("http://127.0.0.1:{port}/api/suite/manifest");
        let own_origin = format!("http://127.0.0.1:{port}");
        let mut req = self
            .http_client
            .get(&manifest_url)
            .header("Accept", "application/json");
        if id == AppId::Pr {
            if let Some(tok) = self.pr_api_token() {
                req = req.bearer_auth(tok);
            }
        }
        let Ok(res) = req.send().await else {
            return None;
        };

        let code = res.status().as_u16();
        if code == 200 {
            let Ok(bytes) = read_bounded_bytes(res, MAX_FETCH_BYTES).await else {
                return None;
            };
            if let Ok(json) = serde_json::from_slice::<Value>(&bytes) {
                if validate_manifest(&json, &[own_origin]).is_ok() {
                    return Some(true);
                }
            }
        }

        if matches!(code, 200 | 401 | 403 | 404) {
            let health_url = format!("http://127.0.0.1:{port}/api/health");
            if let Ok(hres) = self.http_client.get(&health_url).send().await {
                if hres.status().as_u16() == 200 {
                    return Some(false);
                }
            }
        }
        None
    }

    pub async fn stop_workspace(
        self: &Arc<Self>,
        id: AppId,
    ) -> Result<DesktopStatus, SupervisorError> {
        let op_lock = self.op_locks[&id].clone();
        let _op_guard = op_lock.lock().await;
        self.stop_workspace_locked(id).await;
        Ok(self.status())
    }

    async fn stop_workspace_locked(&self, id: AppId) {
        let handle = {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if guard.active_target == id.as_str() {
                guard.active_target = "hub".to_string();
                guard.prefs.active_target = "hub".to_string();
            }
            let slot = guard.slots.get_mut(&id).unwrap();
            slot.generation += 1;
            slot.state = WorkspaceState::Stopped;
            slot.pid = None;
            slot.manifest_valid = false;
            slot.handle.take()
        };

        if let Some(mut h) = handle {
            let _ = h.generation;
            if let Some(tx) = h.kill_tx.take() {
                let _ = tx.send(());
            }
            while h.exited_rx.borrow().is_none() {
                if h.exited_rx.changed().await.is_err() {
                    break;
                }
            }
        }
    }

    pub async fn restart_workspace(
        self: &Arc<Self>,
        id: AppId,
    ) -> Result<DesktopStatus, SupervisorError> {
        let op_lock = self.op_locks[&id].clone();
        let _op_guard = op_lock.lock().await;
        self.stop_workspace_locked(id).await;
        {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            guard.slots.get_mut(&id).unwrap().restart_count += 1;
        }
        self.start_workspace_locked(id).await?;
        Ok(self.status())
    }

    fn preallocate_ports(&self, targets: &[AppId]) {
        let installed_targets: Vec<AppId> = targets
            .iter()
            .copied()
            .filter(|&app| self.is_app_installed(app))
            .collect();

        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let mut held_listeners: Vec<TcpListener> = Vec::with_capacity(installed_targets.len());
        let mut reserved_ports: Vec<u16> = guard
            .slots
            .values()
            .filter(|s| matches!(s.state, WorkspaceState::Starting | WorkspaceState::Running))
            .map(|s| s.port)
            .collect();

        for app in installed_targets {
            if matches!(
                guard.slots[&app].state,
                WorkspaceState::Starting | WorkspaceState::Running
            ) {
                continue;
            }
            let preferred = self.config.base_port.saturating_add(app.port_offset());
            let chosen = if !reserved_ports.contains(&preferred) {
                if let Ok(listener) = TcpListener::bind(("127.0.0.1", preferred)) {
                    held_listeners.push(listener);
                    preferred
                } else if let Ok(listener) = TcpListener::bind(("127.0.0.1", 0)) {
                    let ephem = listener.local_addr().map(|a| a.port()).unwrap_or(preferred);
                    held_listeners.push(listener);
                    ephem
                } else {
                    preferred
                }
            } else if let Ok(listener) = TcpListener::bind(("127.0.0.1", 0)) {
                let ephem = listener.local_addr().map(|a| a.port()).unwrap_or(preferred);
                held_listeners.push(listener);
                ephem
            } else {
                preferred
            };
            reserved_ports.push(chosen);
            guard.slots.get_mut(&app).unwrap().port = chosen;
        }
        drop(held_listeners);
    }

    fn maybe_focus_standalone_primary(&self) {
        if let Some(primary) = self.config.edition.primary_app() {
            let should_focus_primary = {
                let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
                if guard.active_target == "hub"
                    && guard.slots[&primary].state == WorkspaceState::Running
                {
                    guard.active_target = primary.as_str().to_string();
                    guard.prefs.active_target = primary.as_str().to_string();
                    true
                } else {
                    false
                }
            };
            if should_focus_primary {
                let _ = self.vault.update_prefs(PrefsPatch {
                    active_target: Some(primary.as_str().to_string()),
                    ..Default::default()
                });
            }
        }
    }

    pub async fn start_all(self: &Arc<Self>) -> Result<DesktopStatus, SupervisorError> {
        let sup_unpack = self.clone();
        let _ = tokio::task::spawn_blocking(move || sup_unpack.ensure_sidecars_ready()).await;

        let targets: Vec<AppId> = match self.config.edition.primary_app() {
            Some(primary) => vec![primary],
            None => AppId::ALL.to_vec(),
        };

        self.preallocate_ports(&targets);

        let mut set = tokio::task::JoinSet::new();
        for app in targets {
            if self.discover_app_source(app).is_some() {
                let sup = self.clone();
                set.spawn(async move {
                    let _ = sup.start_workspace(app).await;
                });
            }
        }
        while set.join_next().await.is_some() {}

        self.maybe_focus_standalone_primary();
        self.invalidate_vault_cache();
        Ok(self.status())
    }

    /// Starts bundled workspaces sequentially one-by-one (eliminating the 5-way macOS `amfid`/V8
    /// cold-boot stampede while keeping UI responsive) and invokes `on_each` after each workspace boots.
    pub async fn start_all_staggered<F>(
        self: &Arc<Self>,
        on_each: F,
    ) -> Result<DesktopStatus, SupervisorError>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let sup_unpack = self.clone();
        let _ = tokio::task::spawn_blocking(move || sup_unpack.ensure_sidecars_ready()).await;

        let targets: Vec<AppId> = match self.config.edition.primary_app() {
            Some(primary) => vec![primary],
            None => AppId::ALL.to_vec(),
        };

        self.preallocate_ports(&targets);

        for app in targets {
            if self.discover_app_source(app).is_some() {
                let _ = self.start_workspace(app).await;
                on_each();
            }
        }

        self.maybe_focus_standalone_primary();
        self.invalidate_vault_cache();
        Ok(self.status())
    }

    pub async fn shutdown_all(self: &Arc<Self>) {
        let mut set = tokio::task::JoinSet::new();
        for app in AppId::ALL {
            let sup = self.clone();
            set.spawn(async move {
                let _ = sup.stop_workspace(app).await;
            });
        }
        while set.join_next().await.is_some() {}
    }
}

fn unpack_apps_archive(archive_path: &Path, cache_dir: &Path) -> io::Result<()> {
    let _ = fs::remove_dir_all(cache_dir);
    let apps_root = cache_dir.join("apps");
    fs::create_dir_all(&apps_root)?;

    let file = File::open(archive_path)?;
    let decoder = GzDecoder::new(BufReader::new(file));
    let mut archive = Archive::new(decoder);

    for entry_res in archive.entries()? {
        let mut entry = entry_res?;
        let entry_type = entry.header().entry_type();
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            continue;
        }

        let raw_path = entry.path()?.into_owned();
        let mut normals: Vec<&std::ffi::OsStr> = Vec::new();
        for comp in raw_path.components() {
            match comp {
                Component::Normal(seg) => normals.push(seg),
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unsafe path component in apps.tar.gz",
                    ));
                }
            }
        }
        if normals.is_empty() {
            continue;
        }

        let mut out_path = if normals[0] == "apps" {
            cache_dir.to_path_buf()
        } else {
            apps_root.clone()
        };
        for seg in normals {
            out_path.push(seg);
        }

        if entry_type.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else if entry_type.is_file() {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            entry.unpack(&out_path)?;
        }
    }
    Ok(())
}

fn push_log_line(buf: &Arc<Mutex<VecDeque<String>>>, line: String) {
    let trimmed = line.trim_end_matches(&['\r', '\n'][..]).to_string();
    if trimmed.is_empty() {
        return;
    }
    let mut guard = buf.lock().unwrap_or_else(|e| e.into_inner());
    if guard.len() >= LOG_RING_CAPACITY {
        guard.pop_front();
    }
    guard.push_back(trimmed);
}

async fn terminate_child(child: &mut Child) {
    #[cfg(unix)]
    {
        if let Some(pid) = child.id() {
            unsafe {
                let _ = libc::kill(pid as libc::pid_t, libc::SIGTERM);
            }
            if tokio::time::timeout(Duration::from_millis(1500), child.wait())
                .await
                .is_ok()
            {
                return;
            }
        }
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

fn can_bind_loopback(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn allocate_ephemeral_port() -> Option<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).ok()?;
    listener.local_addr().ok().map(|a| a.port())
}

fn resolve_sidecars_dir(config: &SupervisorConfig) -> Option<PathBuf> {
    if let Some(ref p) = config.sidecars_dir {
        return Some(p.clone());
    }
    if let Ok(raw) = std::env::var("BELL_DESKTOP_SIDECARS_DIR") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }
    if let Some(ref res_dir) = config.resource_dir {
        let cand = res_dir.join("dist-sidecars");
        if cand.is_dir() {
            return Some(cand);
        }
    }
    if let Some(root) = discover_suite_root() {
        let cand = root.join("desktop").join("dist-sidecars");
        if cand.is_dir() {
            return Some(cand);
        }
    }
    None
}

fn discover_suite_root() -> Option<PathBuf> {
    let mut seeds = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        seeds.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(p) = exe.parent() {
            seeds.push(p.to_path_buf());
        }
    }
    seeds.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")));

    for seed in seeds {
        let mut cur: Option<&Path> = Some(seed.as_path());
        while let Some(dir) = cur {
            if dir.join("contracts").join("v1").join("reference.mjs").is_file() {
                return Some(dir.to_path_buf());
            }
            cur = dir.parent();
        }
    }
    None
}

fn extra_bin_dirs() -> Vec<PathBuf> {
    #[cfg(unix)]
    {
        let mut extra = vec![
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
        ];
        if let Ok(home) = std::env::var("HOME") {
            let h = PathBuf::from(home);
            extra.push(h.join(".local").join("bin"));
            extra.push(h.join(".cargo").join("bin"));
        }
        extra
    }
    #[cfg(windows)]
    {
        let mut extra = Vec::new();
        if let Ok(pf) = std::env::var("ProgramFiles") {
            let p = PathBuf::from(pf);
            extra.push(p.join("nodejs"));
            extra.push(p.join("Git").join("cmd"));
            extra.push(p.join("GitHub CLI"));
        }
        extra
    }
    #[cfg(not(any(unix, windows)))]
    {
        Vec::new()
    }
}

fn augmented_path_env() -> std::ffi::OsString {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    for extra in extra_bin_dirs() {
        if extra.is_dir() && !dirs.contains(&extra) {
            dirs.push(extra);
        }
    }
    std::env::join_paths(dirs).unwrap_or_default()
}

pub fn find_executable(name: &str) -> Option<PathBuf> {
    let path_os = augmented_path_env();
    let exts: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".bat", ""]
    } else {
        &[""]
    };
    for dir in std::env::split_paths(&path_os) {
        for ext in exts {
            let cand = dir.join(format!("{name}{ext}"));
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}
