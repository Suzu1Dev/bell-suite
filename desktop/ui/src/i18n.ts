import type { AppId, LangPref, TodayItemState, WorkspaceState } from "./types";

export interface UiCopy {
  hubButtonPrefix: string;
  hubButtonShort: string;
  hubVaultToggle: string;
  backToWorkspaces: string;
  switchInSuite: string;
  heroKickerSuite: string;
  heroKickerStandalone: string;
  heroSubtitleSuite: string;
  heroSubtitleStandalone: string;
  runningSummary: (running: number, total: number) => string;
  standaloneHeroState: Record<WorkspaceState, string>;
  standaloneOpenWorkspace: string;
  standaloneStartWorkspace: string;
  startAll: string;
  startingAll: string;
  autoStart: string;
  themeLight: string;
  themeDark: string;
  themeSystem: string;
  langSwitchTooltip: string;
  themeSwitchTooltip: (cur: string, next: string) => string;
  runtimeTitle: string;
  zeroNodeReady: string;
  systemNode: string;
  cliReady: string;
  cliMissing: string;
  workspacesSectionTitle: string;
  workspacesSectionSub: string;
  siblingSectionTitle: string;
  manifestValid: string;
  manifestPending: string;
  notInstalled: string;
  openWorkspace: string;
  startAndOpen: string;
  restart: string;
  stop: string;
  start: string;
  logs: string;
  portLabel: string;
  pidLabel: string;
  dbSizeLabel: string;
  restartsLabel: string;
  contractCardKicker: string;
  contractCardTitle: string;
  contractCardBody: string;
  contractLoopback: string;
  contractIsolatedDb: string;
  contractCsp: string;
  contractManifests: (valid: number, total: number) => string;
  todayTitle: string;
  todaySub: string;
  todayRefresh: string;
  todayFilterAll: string;
  todayAggregatorBadge: string;
  todayEmptyTitle: string;
  todayEmptyBody: string;
  todayJump: string;
  vaultTitle: string;
  vaultSub: string;
  vaultRootLabel: string;
  vaultCopyPath: string;
  vaultSyncCap: (usedMb: string, quotaGb: string) => string;
  vaultBackupsSummary: (count: number, lastAt: string) => string;
  vaultNeverBackedUp: string;
  vaultIntegrityOk: string;
  vaultIntegrityFail: string;
  vaultJsonOk: string;
  vaultJsonFail: string;
  vaultTables: (n: number) => string;
  vaultJsonEntries: (n: number) => string;
  vaultDbEmpty: string;
  vaultBackupNow: string;
  vaultBackingUp: string;
  backupToastTitle: string;
  backupToastDetail: (dir: string, files: number, size: string) => string;
  stageKicker: string;
  stageLaunch: (originAndRoute: string) => string;
  stageBackHub: string;
  stageRoutesTitle: string;
  stageIsolationNote: string;
  stageHealthTitle: string;
  stageConsoleTitle: string;
  logsModalTitle: (name: string) => string;
  logsRefresh: string;
  logsCopy: string;
  logsCopied: string;
  logsClose: string;
  logsEmpty: string;
  errorTitle: string;
  errorRetry: string;
  winMinimize: string;
  winMaximize: string;
  winClose: string;
  stateLabel: Record<WorkspaceState, string>;
  todayStateLabel: Record<TodayItemState, string>;
  taglines: Record<AppId, string>;
}

const ZH_TW: UiCopy = {
  hubButtonPrefix: "總覽 ",
  hubButtonShort: "Hub",
  hubVaultToggle: "Hub / 保險箱",
  backToWorkspaces: "返回工作區",
  switchInSuite: "切換工作區…",
  heroKickerSuite: "LOCAL-FIRST WORKSPACE ORCHESTRATOR · SUITE CONTRACT V1",
  heroKickerStandalone: "STANDALONE LOCAL-FIRST WORKSPACE · SUITE CONTRACT V1",
  heroSubtitleSuite:
    "本機優先的五合一工作空間：獨立 SQLite 儲存、127.0.0.1 嚴格迴路隔離、跨產品零自動寫入交接。",
  heroSubtitleStandalone:
    "獨立運作的本機優先工作空間，內建 SQLite 原子備份與 Suite Contract v1 跨工具相容能力。",
  runningSummary: (running, total) => `${running} / ${total} 運作中`,
  standaloneHeroState: {
    running: "主工作區運作中",
    starting: "主工作區啟動中",
    stopped: "已停止",
    error: "執行異常",
  },
  standaloneOpenWorkspace: "開啟工作區",
  standaloneStartWorkspace: "啟動工作區",
  startAll: "全部啟動",
  startingAll: "啟動中…",
  autoStart: "啟動時自動執行",
  themeLight: "淺色",
  themeDark: "深色",
  themeSystem: "系統",
  langSwitchTooltip: "目前語言：繁體中文（點擊切換為 English）",
  themeSwitchTooltip: (cur, next) => `目前主題：${cur}（點擊切換為${next}）`,
  runtimeTitle: "執行環境與 AI CLI 就緒狀態",
  zeroNodeReady: "Zero-Node 內建引擎就緒",
  systemNode: "系統 Node.js 引擎",
  cliReady: "已就緒",
  cliMissing: "未偵測",
  workspacesSectionTitle: "五大工作空間",
  workspacesSectionSub: "每個工作區皆以獨立本機程序與專屬 SQLite 資料庫運作",
  siblingSectionTitle: "BELL Suite 同套件工作空間",
  manifestValid: "Manifest v1 ✓",
  manifestPending: "Manifest 待命",
  notInstalled: "未安裝",
  openWorkspace: "開啟工作區",
  startAndOpen: "啟動並開啟",
  restart: "重啟",
  stop: "停止",
  start: "啟動",
  logs: "日誌",
  portLabel: "埠號",
  pidLabel: "PID",
  dbSizeLabel: "資料庫",
  restartsLabel: "重啟次數",
  contractCardKicker: "ARCHITECTURE · ISOLATION",
  contractCardTitle: "Suite Contract v1 安全邊界",
  contractCardBody:
    "各工具嚴格綁定 127.0.0.1 本機迴路，絕不共用資料庫或跨服務直接寫入；草稿交接須經使用者確認後才落盤。",
  contractLoopback: "127.0.0.1 Loopback-Only",
  contractIsolatedDb: "5 座隔離 SQLite Vaults",
  contractCsp: "CSP frame-ancestors 'none'",
  contractManifests: (valid, total) => `${valid}/${total} Manifest 已驗證`,
  todayTitle: "Today 今日摘要彙整",
  todaySub: "跨工作空間唯讀最小化摘要（僅標題與 #/ 路由，不含私密內文）",
  todayRefresh: "重新整理",
  todayFilterAll: "全部",
  todayAggregatorBadge: "聚合端",
  todayEmptyTitle: "目前沒有待處理的今日項目",
  todayEmptyBody: "啟動工作空間後，到期複習、待審 PR、收件匣與活動時程將自動彙整於此。",
  todayJump: "開啟 →",
  vaultTitle: "本機 SQLite 資料保險箱",
  vaultSub: "完整性檢查（PRAGMA integrity_check）、同步配額與一鍵原子備份",
  vaultRootLabel: "保險箱根目錄",
  vaultCopyPath: "複製路徑",
  vaultSyncCap: (usedMb, quotaGb) => `${usedMb} / ${quotaGb} 同步上限`,
  vaultBackupsSummary: (count, lastAt) => `已保存 ${count} 份快照 · 最近備份 ${lastAt}`,
  vaultNeverBackedUp: "尚無備份紀錄",
  vaultIntegrityOk: "PRAGMA integrity_check: 正常",
  vaultIntegrityFail: "PRAGMA integrity_check: 異常",
  vaultJsonOk: "JSON 正常",
  vaultJsonFail: "JSON 異常",
  vaultTables: (n) => `${n} 張表`,
  vaultJsonEntries: (n) => `${n} 筆項目`,
  vaultDbEmpty: "尚未建立 SQLite 資料庫檔案（首次啟動工作空間時自動初始化）",
  vaultBackupNow: "立即建立原子備份",
  vaultBackingUp: "正在建立原子備份…",
  backupToastTitle: "SQLite 原子備份已完成",
  backupToastDetail: (dir, files, size) => `已備份 ${files} 個檔案（${size}）至 ${dir}`,
  stageKicker: "ACTIVE WORKSPACE STAGE · BROWSER PREVIEW",
  stageLaunch: (originAndRoute) => `開啟 ${originAndRoute} ↗`,
  stageBackHub: "返回總覽 Hub",
  stageRoutesTitle: "快速路由跳轉（Suite Contract v1）",
  stageIsolationNote:
    "Tauri 原生桌面模式將於 y ≥ 44px 嵌入原生子 WebView；瀏覽器預覽模式嚴格保留各應用 X-Frame-Options: DENY / frame-ancestors 'none' 安全隔離，請透過上方連結直接開啟。",
  stageHealthTitle: "即時健康度與資料庫狀態",
  stageConsoleTitle: "即時程序日誌（stdout / stderr）",
  logsModalTitle: (name) => `${name} · 程序日誌`,
  logsRefresh: "重新整理",
  logsCopy: "複製日誌",
  logsCopied: "已複製 ✓",
  logsClose: "關閉",
  logsEmpty: "目前尚無日誌輸出。",
  errorTitle: "無法連線至本機 BELL 服務",
  errorRetry: "重新嘗試",
  winMinimize: "最小化",
  winMaximize: "最大化 / 還原",
  winClose: "關閉視窗",
  stateLabel: {
    running: "運作中",
    starting: "啟動中",
    stopped: "已停止",
    error: "異常",
  },
  todayStateLabel: {
    due: "今日到期",
    "needs-you": "需要處理",
    waiting: "等待中",
    new: "新項目",
  },
  taglines: {
    personal: "今日總覽、待辦事項、行事曆、靈感與收件匣",
    explain: "互動式概念拆解、程式碼逐步推演與 FSRS 間隔複習",
    pr: "AI 輔助開源貢獻、程式碼理解與審查工作台",
    reader: "純淨網頁擷取、紙本感螢光筆標註與手繪塗鴉筆記",
    campaign: "活動專案倒數時程、各組待辦、招工表單與行前手冊生成",
  },
};

const EN: UiCopy = {
  hubButtonPrefix: "",
  hubButtonShort: "Hub",
  hubVaultToggle: "Hub / Vault",
  backToWorkspaces: "Workspace",
  switchInSuite: "Switch / Open in Suite…",
  heroKickerSuite: "LOCAL-FIRST WORKSPACE ORCHESTRATOR · SUITE CONTRACT V1",
  heroKickerStandalone: "STANDALONE LOCAL-FIRST WORKSPACE · SUITE CONTRACT V1",
  heroSubtitleSuite:
    "Five privacy-preserving local workspaces with isolated SQLite vaults, 127.0.0.1 loopback boundaries, and zero-write draft handoffs.",
  heroSubtitleStandalone:
    "Focused standalone workspace with atomic SQLite vault backups and full Suite Contract v1 interoperability.",
  runningSummary: (running, total) => `${running} / ${total} Running`,
  standaloneHeroState: {
    running: "Primary Running",
    starting: "Primary Starting",
    stopped: "Stopped",
    error: "Workspace Error",
  },
  standaloneOpenWorkspace: "Open Workspace",
  standaloneStartWorkspace: "Start Workspace",
  startAll: "Start All",
  startingAll: "Starting…",
  autoStart: "Auto-start",
  themeLight: "Light",
  themeDark: "Dark",
  themeSystem: "System",
  langSwitchTooltip: "Language: English (Click to switch to 繁體中文)",
  themeSwitchTooltip: (cur, next) => `Theme: ${cur} (Click to switch to ${next})`,
  runtimeTitle: "Runtime & AI CLI Readiness",
  zeroNodeReady: "Zero-Node Ready",
  systemNode: "System Node",
  cliReady: "Ready",
  cliMissing: "Missing",
  workspacesSectionTitle: "Workspaces",
  workspacesSectionSub: "Each workspace runs an isolated local process and dedicated SQLite database",
  siblingSectionTitle: "Suite Sibling Workspaces",
  manifestValid: "Manifest v1 ✓",
  manifestPending: "Manifest Idle",
  notInstalled: "Not Installed",
  openWorkspace: "Open",
  startAndOpen: "Start & Open",
  restart: "Restart",
  stop: "Stop",
  start: "Start",
  logs: "Logs",
  portLabel: "Port",
  pidLabel: "PID",
  dbSizeLabel: "SQLite",
  restartsLabel: "Restarts",
  contractCardKicker: "ARCHITECTURE · ISOLATION",
  contractCardTitle: "Suite Contract v1 Guarantees",
  contractCardBody:
    "Every workspace binds strictly to 127.0.0.1 with its own SQLite file. Cross-app draft handoffs never write to disk without explicit confirmation.",
  contractLoopback: "127.0.0.1 Loopback-Only",
  contractIsolatedDb: "5 Isolated SQLite Vaults",
  contractCsp: "CSP frame-ancestors 'none'",
  contractManifests: (valid, total) => `${valid}/${total} Manifests Verified`,
  todayTitle: "Today Feed Aggregation",
  todaySub: "Data-minimized read-only feed across running workspaces (titles & #/ routes only)",
  todayRefresh: "Refresh",
  todayFilterAll: "All",
  todayAggregatorBadge: "Aggregator",
  todayEmptyTitle: "No Today items queued right now",
  todayEmptyBody: "Start workspaces to surface due reviews, PR actions, inbox items, and campaign milestones.",
  todayJump: "Open →",
  vaultTitle: "Local SQLite Vault & Sync Quota",
  vaultSub: "Live PRAGMA integrity_check health, storage cap, and atomic snapshot backups",
  vaultRootLabel: "Vault Root Path",
  vaultCopyPath: "Copy Path",
  vaultSyncCap: (usedMb, quotaGb) => `${usedMb} / ${quotaGb} Sync Cap`,
  vaultBackupsSummary: (count, lastAt) =>
    `${count} ${count === 1 ? "snapshot" : "snapshots"} · Last backup ${lastAt}`,
  vaultNeverBackedUp: "No snapshots yet",
  vaultIntegrityOk: "PRAGMA integrity_check: OK",
  vaultIntegrityFail: "PRAGMA integrity_check: FAIL",
  vaultJsonOk: "JSON OK",
  vaultJsonFail: "JSON FAIL",
  vaultTables: (n) => `${n} ${n === 1 ? "table" : "tables"}`,
  vaultJsonEntries: (n) => `${n} ${n === 1 ? "entry" : "entries"}`,
  vaultDbEmpty: "No SQLite files initialized yet (created automatically on first workspace launch).",
  vaultBackupNow: "Backup SQLite Now",
  vaultBackingUp: "Backing up SQLite…",
  backupToastTitle: "Atomic SQLite Backup Complete",
  backupToastDetail: (dir, files, size) => `Saved ${files} ${files === 1 ? "file" : "files"} (${size}) to ${dir}`,
  stageKicker: "ACTIVE WORKSPACE STAGE · BROWSER PREVIEW",
  stageLaunch: (originAndRoute) => `Open ${originAndRoute} ↗`,
  stageBackHub: "Back to Hub",
  stageRoutesTitle: "Route Quick-Links (Suite Contract v1)",
  stageIsolationNote:
    "In Tauri Desktop, a native child Webview occupies y ≥ 44px directly. In browser preview, each BELL app enforces X-Frame-Options: DENY / frame-ancestors 'none'—use the direct loopback launcher above.",
  stageHealthTitle: "Live Process & SQLite Health",
  stageConsoleTitle: "Embedded Workspace Log Console",
  logsModalTitle: (name) => `${name} · Workspace Logs`,
  logsRefresh: "Refresh",
  logsCopy: "Copy",
  logsCopied: "Copied ✓",
  logsClose: "Close",
  logsEmpty: "No log output recorded yet.",
  errorTitle: "Cannot reach local BELL service",
  errorRetry: "Retry",
  winMinimize: "Minimize",
  winMaximize: "Maximize",
  winClose: "Close",
  stateLabel: {
    running: "Running",
    starting: "Starting",
    stopped: "Stopped",
    error: "Error",
  },
  todayStateLabel: {
    due: "Due",
    "needs-you": "Needs You",
    waiting: "Waiting",
    new: "New",
  },
  taglines: {
    personal: "Today dashboard, tasks, calendar, ideas & inbox",
    explain: "Interactive concept explanations, code walkthroughs & FSRS review",
    pr: "Local AI-assisted open-source contribution & code review workbench",
    reader: "Clean web capture, paper-like ink annotations & hand-drawn sketches",
    campaign: "Event countdown timeline, team tasks, forms & handbook generator",
  },
};

export function normalizeLang(raw?: string | null): LangPref {
  if (!raw) return "zh-tw";
  return raw.toLowerCase().startsWith("en") ? "en" : "zh-tw";
}

export function getCopy(lang: LangPref): UiCopy {
  return lang === "en" ? EN : ZH_TW;
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  if (bytes < 1024) return `${Math.round(bytes)} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export function formatVaultMb(bytes: number): string {
  const mb = Math.max(0, bytes) / (1024 * 1024);
  return `${mb < 10 ? mb.toFixed(2) : mb.toFixed(1)} MB`;
}

export function formatQuotaGb(bytes: number): string {
  const gb = Math.max(1, bytes) / (1024 * 1024 * 1024);
  return `${Number.isInteger(gb) ? gb : gb.toFixed(1)} GB`;
}

export function formatShortTime(iso: string | null | undefined, lang: LangPref): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(lang === "en" ? "en-US" : "zh-TW", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}
