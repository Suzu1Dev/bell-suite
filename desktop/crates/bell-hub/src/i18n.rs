use bell_core::{AppId, WorkspaceState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    ZhTw,
    En,
}

impl Lang {
    pub fn from_pref(raw: &str) -> Self {
        if raw.trim().to_ascii_lowercase().starts_with("en") {
            Lang::En
        } else {
            Lang::ZhTw
        }
    }

    pub const fn html_lang(self) -> &'static str {
        match self {
            Lang::ZhTw => "zh-Hant",
            Lang::En => "en",
        }
    }
}

pub struct Copy {
    pub hub_name: &'static str,
    pub cmd_trigger: &'static str,
    pub nav_overview: &'static str,
    pub nav_today: &'static str,
    pub nav_workspaces: &'static str,
    pub nav_vault: &'static str,
    pub nav_logs: &'static str,
    pub rail_workspaces: &'static str,
    pub theme_dark: &'static str,
    pub theme_light: &'static str,
    pub theme_auto: &'static str,
    pub start_all: &'static str,
    pub snapshot_short: &'static str,
    pub snapshot_full: &'static str,
    pub open_enter: &'static str,
    pub open_arrow: &'static str,
    pub op_start: &'static str,
    pub op_stop: &'static str,
    pub op_restart: &'static str,
    pub op_logs: &'static str,
    pub sec_workspaces: &'static str,
    pub sec_today: &'static str,
    pub sec_vault_runtime: &'static str,
    pub view_inbox_link: &'static str,
    pub filter_all: &'static str,
    pub filter_due: &'static str,
    pub filter_needs_you: &'static str,
    pub filter_waiting: &'static str,
    pub filter_new: &'static str,
    pub today_empty_title: &'static str,
    pub today_empty_sub: &'static str,
    pub today_aggregator: &'static str,
    pub inspector_empty: &'static str,
    pub inspector_route: &'static str,
    pub inspector_updated: &'static str,
    pub inspector_state: &'static str,
    pub inspector_type: &'static str,
    pub vault_storage: &'static str,
    pub vault_root: &'static str,
    pub copy_path: &'static str,
    pub copy_logs: &'static str,
    pub refresh: &'static str,
    pub col_app: &'static str,
    pub col_path: &'static str,
    pub col_count: &'static str,
    pub col_size: &'static str,
    pub col_health: &'static str,
    pub wal_ok: &'static str,
    pub wal_fail: &'static str,
    pub json_ok: &'static str,
    pub json_fail: &'static str,
    pub vault_empty: &'static str,
    pub runtime_bundled: &'static str,
    pub runtime_system: &'static str,
    pub manifest_ok: &'static str,
    pub manifest_idle: &'static str,
    pub primary_badge: &'static str,
    pub logs_empty: &'static str,
    pub palette_placeholder: &'static str,
    pub palette_empty: &'static str,
    pub palette_group_ws: &'static str,
    pub palette_group_routes: &'static str,
    pub palette_group_today: &'static str,
    pub palette_group_actions: &'static str,
    pub palette_sub_switch_view: &'static str,
    pub palette_sub_sys_cmd: &'static str,
    pub palette_act_start_all: &'static str,
    pub palette_act_snapshot: &'static str,
    pub palette_act_theme: &'static str,
    pub palette_act_lang: &'static str,
}

pub const ZH_TW: Copy = Copy {
    hub_name: "BELL Hub",
    cmd_trigger: "搜尋或跳轉…",
    nav_overview: "總覽",
    nav_today: "今日收件匣",
    nav_workspaces: "產品工作區",
    nav_vault: "資料保險箱",
    nav_logs: "系統與日誌",
    rail_workspaces: "WORKSPACES",
    theme_dark: "深色",
    theme_light: "淺色",
    theme_auto: "自動",
    start_all: "全部啟動",
    snapshot_short: "建立快照",
    snapshot_full: "立即建立原子熱備份",
    open_enter: "開啟 ↵",
    open_arrow: "開啟 →",
    op_start: "啟動",
    op_stop: "停止",
    op_restart: "重啟",
    op_logs: "日誌",
    sec_workspaces: "本機工作區",
    sec_today: "今日待處理",
    sec_vault_runtime: "本機保險箱與引擎",
    view_inbox_link: "收件匣檢視 →",
    filter_all: "全部",
    filter_due: "到期",
    filter_needs_you: "待審",
    filter_waiting: "等待",
    filter_new: "新進",
    today_empty_title: "目前沒有待處理項目",
    today_empty_sub: "啟動工作區後，到期複習、待審 PR、收件匣與活動待辦將自動匯流於此。",
    today_aggregator: "聚合端",
    inspector_empty: "請從左側清單選取項目以檢視詳情",
    inspector_route: "目標路由",
    inspector_updated: "最後更新",
    inspector_state: "目前狀態",
    inspector_type: "物件類型",
    vault_storage: "儲存配額",
    vault_root: "保險箱路徑",
    copy_path: "複製路徑",
    copy_logs: "複製日誌",
    refresh: "重新整理",
    col_app: "工作區",
    col_path: "資料庫檔案",
    col_count: "資料表 / 紀錄",
    col_size: "大小",
    col_health: "完整性檢查",
    wal_ok: "WAL 正常 ✓",
    wal_fail: "檢查異常 ✕",
    json_ok: "JSON 正常 ✓",
    json_fail: "JSON 異常 ✕",
    vault_empty: "尚未初始化資料庫檔案（首次啟動工作區時自動建立）",
    runtime_bundled: "內建 bell-runtime",
    runtime_system: "系統 Node.js",
    manifest_ok: "Manifest v1 ✓",
    manifest_idle: "待命",
    primary_badge: "主工作區",
    logs_empty: "尚無程序日誌輸出。",
    palette_placeholder: "跳轉工作區、深層路由、今日待辦或執行系統指令…",
    palette_empty: "找不到符合的指令或工作區項目",
    palette_group_ws: "開啟工作區",
    palette_group_routes: "深層路由快速跳轉",
    palette_group_today: "今日待處理項目",
    palette_group_actions: "Hub 檢視與系統指令",
    palette_sub_switch_view: "BELL Hub · 切換檢視",
    palette_sub_sys_cmd: "BELL Hub · 系統指令",
    palette_act_start_all: "啟動全部工作區",
    palette_act_snapshot: "建立資料保險箱原子快照",
    palette_act_theme: "切換深色／淺色主題",
    palette_act_lang: "切換語言（繁體中文／English）",
};

pub const EN: Copy = Copy {
    hub_name: "BELL Hub",
    cmd_trigger: "Jump to…",
    nav_overview: "Overview",
    nav_today: "Today",
    nav_workspaces: "Workspaces",
    nav_vault: "Data Vault",
    nav_logs: "Logs & Runtime",
    rail_workspaces: "WORKSPACES",
    theme_dark: "Dark",
    theme_light: "Light",
    theme_auto: "Auto",
    start_all: "Start All",
    snapshot_short: "Snapshot",
    snapshot_full: "Snapshot Vault Now",
    open_enter: "Open ↵",
    open_arrow: "Open →",
    op_start: "Start",
    op_stop: "Stop",
    op_restart: "Restart",
    op_logs: "Logs",
    sec_workspaces: "Workspaces",
    sec_today: "Today Stream",
    sec_vault_runtime: "Vault & Runtime",
    view_inbox_link: "Open Inbox →",
    filter_all: "All",
    filter_due: "Due",
    filter_needs_you: "Needs You",
    filter_waiting: "Waiting",
    filter_new: "New",
    today_empty_title: "All clear for today",
    today_empty_sub: "Start workspaces to stream due reviews, PR checks, reading notes, and campaign tasks.",
    today_aggregator: "Aggregator",
    inspector_empty: "Select an item on the left to inspect details",
    inspector_route: "Deep Route",
    inspector_updated: "Updated",
    inspector_state: "State",
    inspector_type: "Type",
    vault_storage: "Vault Quota",
    vault_root: "Vault Root",
    copy_path: "Copy Path",
    copy_logs: "Copy Logs",
    refresh: "Refresh",
    col_app: "App",
    col_path: "Database File",
    col_count: "Tables / Entries",
    col_size: "Size",
    col_health: "Integrity",
    wal_ok: "WAL OK ✓",
    wal_fail: "Corrupt ✕",
    json_ok: "JSON OK ✓",
    json_fail: "JSON Invalid ✕",
    vault_empty: "No database files initialized yet (created on first workspace start).",
    runtime_bundled: "Bundled bell-runtime",
    runtime_system: "System Node.js",
    manifest_ok: "Manifest v1 ✓",
    manifest_idle: "Idle",
    primary_badge: "Primary",
    logs_empty: "No sidecar output recorded yet.",
    palette_placeholder: "Jump to workspace, deep route, Today item, or run command…",
    palette_empty: "No matching workspaces, routes, or commands",
    palette_group_ws: "Workspaces",
    palette_group_routes: "Deep Routes",
    palette_group_today: "Today Stream",
    palette_group_actions: "Hub Views & System Commands",
    palette_sub_switch_view: "BELL Hub · Switch view",
    palette_sub_sys_cmd: "BELL Hub · System command",
    palette_act_start_all: "Start All Workspaces",
    palette_act_snapshot: "Create Atomic Vault Snapshot",
    palette_act_theme: "Toggle Dark / Light Theme",
    palette_act_lang: "Switch Language (繁中 / EN)",
};

pub const fn copy_for(lang: Lang) -> &'static Copy {
    match lang {
        Lang::ZhTw => &ZH_TW,
        Lang::En => &EN,
    }
}

pub const fn app_short_name(app: AppId) -> &'static str {
    match app {
        AppId::Personal => "Personal",
        AppId::Explain => "Explain",
        AppId::Pr => "PR",
        AppId::Reader => "Web Reader",
        AppId::Campaign => "Campaign",
    }
}

pub const fn app_badge_name(app: AppId) -> &'static str {
    match app {
        AppId::Personal => "Personal",
        AppId::Explain => "Explain",
        AppId::Pr => "PR",
        AppId::Reader => "Reader",
        AppId::Campaign => "Campaign",
    }
}

pub const fn app_summary(app: AppId, lang: Lang) -> &'static str {
    match (app, lang) {
        (AppId::Personal, Lang::ZhTw) => "今日總覽、待辦事項、行事曆、靈感與收件匣",
        (AppId::Explain, Lang::ZhTw) => "概念拆解、程式碼逐步推演與 FSRS 間隔複習",
        (AppId::Pr, Lang::ZhTw) => "AI 輔助開源貢獻、程式碼理解與 PR 審查工作台",
        (AppId::Reader, Lang::ZhTw) => "純淨網頁擷取、螢光筆標註與手繪筆記",
        (AppId::Campaign, Lang::ZhTw) => "活動倒數時程、各組待辦、表單與行前手冊",
        (AppId::Personal, Lang::En) => "Today dashboard, tasks, calendar, ideas & inbox",
        (AppId::Explain, Lang::En) => "Interactive explanations, code walkthroughs & FSRS review",
        (AppId::Pr, Lang::En) => "Local AI open-source contribution & code review workbench",
        (AppId::Reader, Lang::En) => "Clean web capture, paper highlights & ink sketches",
        (AppId::Campaign, Lang::En) => "Event countdown timeline, team tasks, forms & handbook",
    }
}

pub fn deep_routes(app: AppId, lang: Lang) -> &'static [(&'static str, &'static str)] {
    match (app, lang) {
        (AppId::Personal, Lang::ZhTw) => &[
            ("#/today", "今日總覽"),
            ("#/tasks", "待辦事項"),
            ("#/events", "行事曆"),
            ("#/ideas", "靈感筆記"),
            ("#/inbox", "收件匣"),
        ],
        (AppId::Personal, Lang::En) => &[
            ("#/today", "Today"),
            ("#/tasks", "Tasks"),
            ("#/events", "Calendar"),
            ("#/ideas", "Ideas"),
            ("#/inbox", "Inbox"),
        ],
        (AppId::Explain, Lang::ZhTw) => &[
            ("#/review", "到期複習"),
            ("#/library", "知識庫"),
            ("#/graph", "概念圖譜"),
            ("#/favorites", "已收藏"),
        ],
        (AppId::Explain, Lang::En) => &[
            ("#/review", "FSRS Review"),
            ("#/library", "Library"),
            ("#/graph", "Graph"),
            ("#/favorites", "Favorites"),
        ],
        (AppId::Pr, Lang::ZhTw) => &[
            ("#/discover", "探索議題"),
            ("#/repos", "程式碼庫"),
            ("#/learning", "學習紀錄"),
            ("#/settings", "偏好設定"),
        ],
        (AppId::Pr, Lang::En) => &[
            ("#/discover", "Discover"),
            ("#/repos", "Repositories"),
            ("#/learning", "Learning"),
            ("#/settings", "Settings"),
        ],
        (AppId::Reader, Lang::ZhTw) => &[
            ("#/", "未讀閱讀"),
            ("#/library", "文章書庫"),
            ("#/topics", "主題標註"),
        ],
        (AppId::Reader, Lang::En) => &[
            ("#/", "Reading Queue"),
            ("#/library", "Library"),
            ("#/topics", "Topics"),
        ],
        (AppId::Campaign, Lang::ZhTw) => &[
            ("#/tasks", "各組待辦"),
            ("#/schedule", "倒數時程"),
            ("#/forms", "招工表單"),
            ("#/meetings", "會議紀錄"),
        ],
        (AppId::Campaign, Lang::En) => &[
            ("#/tasks", "Team Tasks"),
            ("#/schedule", "Schedule"),
            ("#/forms", "Forms"),
            ("#/meetings", "Meetings"),
        ],
    }
}

pub const fn ws_state_label(state: WorkspaceState, lang: Lang) -> &'static str {
    match (state, lang) {
        (WorkspaceState::Running, Lang::ZhTw) => "運作中",
        (WorkspaceState::Starting, Lang::ZhTw) => "啟動中",
        (WorkspaceState::Stopped, Lang::ZhTw) => "已停止",
        (WorkspaceState::Error, Lang::ZhTw) => "異常",
        (WorkspaceState::Running, Lang::En) => "Running",
        (WorkspaceState::Starting, Lang::En) => "Starting",
        (WorkspaceState::Stopped, Lang::En) => "Stopped",
        (WorkspaceState::Error, Lang::En) => "Error",
    }
}

pub const fn ws_state_slug(state: WorkspaceState) -> &'static str {
    match state {
        WorkspaceState::Running => "running",
        WorkspaceState::Starting => "starting",
        WorkspaceState::Stopped => "stopped",
        WorkspaceState::Error => "error",
    }
}

pub fn today_state_label(state: &str, lang: Lang) -> &'static str {
    match (state, lang) {
        ("due", Lang::ZhTw) => "今日到期",
        ("needs-you", Lang::ZhTw) => "需要處理",
        ("waiting", Lang::ZhTw) => "等待中",
        ("new", Lang::ZhTw) => "新項目",
        ("due", Lang::En) => "Due",
        ("needs-you", Lang::En) => "Needs You",
        ("waiting", Lang::En) => "Waiting",
        ("new", Lang::En) => "New",
        (_, Lang::ZhTw) => "待處理",
        (_, Lang::En) => "Open",
    }
}

pub fn header_summary(running: usize, total_today: usize, lang: Lang) -> String {
    match lang {
        Lang::ZhTw => format!("{running} 個工作區運作中 · 今日 {total_today} 項待處理"),
        Lang::En => format!("{running} running · {total_today} today items"),
    }
}

pub fn open_in_app_cta(app: AppId, lang: Lang) -> String {
    match lang {
        Lang::ZhTw => format!("在 {} 開啟 →", app.display_name()),
        Lang::En => format!("Open in {} →", app.display_name()),
    }
}

pub fn open_primary_cta(app: AppId, lang: Lang) -> String {
    match lang {
        Lang::ZhTw => format!("開啟 {}", app.display_name()),
        Lang::En => format!("Open {}", app.display_name()),
    }
}

pub fn is_apple_os(os: &str) -> bool {
    matches!(
        os.trim().to_ascii_lowercase().as_str(),
        "macos" | "darwin" | "ios"
    )
}

pub fn format_shortcut(os: &str, key: &str) -> String {
    if is_apple_os(os) {
        format!("⌘{key}")
    } else {
        format!("Ctrl+{key}")
    }
}

pub fn format_table_or_entry_count(n: usize, is_json: bool, lang: Lang) -> String {
    match (is_json, lang) {
        (true, Lang::ZhTw) => format!("{n} 筆項目"),
        (false, Lang::ZhTw) => format!("{n} 張表"),
        (true, Lang::En) => {
            if n == 1 {
                "1 entry".to_string()
            } else {
                format!("{n} entries")
            }
        }
        (false, Lang::En) => {
            if n == 1 {
                "1 table".to_string()
            } else {
                format!("{n} tables")
            }
        }
    }
}

pub fn format_backup_meta(count: usize, last_iso: Option<&str>, lang: Lang) -> String {
    let ts = last_iso.map(short_iso).unwrap_or_else(|| "—".to_string());
    match (count, lang) {
        (0, Lang::ZhTw) => "尚無備份快照".to_string(),
        (n, Lang::ZhTw) => format!("{n} 份快照 · 最近 {ts}"),
        (0, Lang::En) => "No snapshots yet".to_string(),
        (1, Lang::En) => format!("1 snapshot · Last {ts}"),
        (n, Lang::En) => format!("{n} snapshots · Last {ts}"),
    }
}

pub fn short_iso(iso: &str) -> String {
    // e.g. "2026-11-16T20:15:00Z" -> "11-16 20:15"
    if iso.len() >= 16 && iso.as_bytes().get(10) == Some(&b'T') {
        format!("{} {}", &iso[5..10], &iso[11..16])
    } else {
        iso.to_string()
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    if bytes == 0 {
        return "0 B".to_string();
    }
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < MB {
        let kb = b / KB;
        if kb < 10.0 {
            format!("{kb:.1} KB")
        } else {
            format!("{kb:.0} KB")
        }
    } else if b < GB {
        format!("{:.1} MB", b / MB)
    } else {
        format!("{:.1} GB", b / GB)
    }
}

pub fn format_quota_pair(used: u64, quota: u64) -> String {
    let mb = used as f64 / (1024.0 * 1024.0);
    let gb = (quota.max(1)) as f64 / (1024.0 * 1024.0 * 1024.0);
    let used_str = if mb < 0.1 && used > 0 {
        format!("{:.2} MB", mb)
    } else {
        format!("{:.1} MB", mb)
    };
    format!("{used_str} / {gb:.1} GB")
}

pub fn esc(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 8);
    for ch in raw.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn sanitize_hash_route(route: &str) -> &str {
    let trimmed = route.trim();
    if trimmed.starts_with("#/")
        && !trimmed.contains("//")
        && !trimmed.contains('\\')
        && trimmed.len() <= 512
        && trimmed[2..].bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(b, b'/' | b'_' | b'-' | b'.' | b'~' | b'%' | b':' | b'?' | b'=' | b'&' | b'+')
        })
    {
        trimmed
    } else {
        "#/"
    }
}
