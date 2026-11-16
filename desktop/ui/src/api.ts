import {
  PRODUCTS,
  WORKSPACE_ORDER,
  type AppId,
  type ChromePlatform,
  type DesktopPrefs,
  type DesktopStatus,
  type Edition,
  type LangPref,
  type TargetId,
  type ThemePref,
  type TodayOverview,
  type VaultBackupResult,
  type WindowAction,
  type WorkspaceAction,
  type WorkspaceLogs,
} from "./types";

interface TauriInternals {
  invoke: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
}

function getTauriInternals(win: unknown = typeof window !== "undefined" ? window : undefined): TauriInternals | null {
  if (!win || typeof win !== "object") return null;
  const internals = (win as { __TAURI_INTERNALS__?: Partial<TauriInternals> }).__TAURI_INTERNALS__;
  if (internals && typeof internals.invoke === "function") {
    return internals as TauriInternals;
  }
  return null;
}

export function isTauriRuntime(win?: unknown): boolean {
  return getTauriInternals(win) !== null;
}

export function resolveChromePlatform(
  osFromStatus?: string | null,
  search: string = typeof window !== "undefined" ? window.location.search : "",
  userAgent: string = typeof navigator !== "undefined" ? navigator.userAgent : "",
): ChromePlatform {
  const params = new URLSearchParams(search);
  const override = params.get("platform")?.toLowerCase();
  if (override === "macos" || override === "darwin" || override === "mac") return "macos";
  if (override === "windows" || override === "win" || override === "win32") return "windows";

  const os = (osFromStatus ?? "").toLowerCase();
  if (os === "macos" || os === "darwin") return "macos";
  if (os === "windows" || os === "win32") return "windows";

  const ua = userAgent.toLowerCase();
  if (ua.includes("mac os") || ua.includes("macintosh")) return "macos";
  return "windows";
}

export function shouldShowPreviewLights(
  platform: ChromePlatform,
  isTauri: boolean,
  search: string = typeof window !== "undefined" ? window.location.search : "",
): boolean {
  if (isTauri || platform !== "macos") return false;
  const v = new URLSearchParams(search).get("previewLights");
  return v === "1" || v === "true";
}

export function escHtml(raw: unknown): string {
  const str = String(raw ?? "");
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

const LOOPBACK_URL_RE = /^https?:\/\/(127\.0\.0\.1|localhost)(:\d{1,5})?(\/|$)/;
const SAFE_HASH_ROUTE_RE = /^#\/[A-Za-z0-9/_\-.~%:?=&+]*$/;

export function sanitizeRoute(route?: string | null): string {
  if (!route) return "";
  const trimmed = route.trim();
  if (!SAFE_HASH_ROUTE_RE.test(trimmed) || trimmed.includes("//") || trimmed.includes("\\")) {
    return "";
  }
  return trimmed;
}

export function buildSafeWorkspaceLaunchUrl(baseUrl: string, port: number, route?: string | null): string {
  const fallback = `http://127.0.0.1:${Number.isInteger(port) && port > 0 ? port : 4601}`;
  const cleanBase = LOOPBACK_URL_RE.test(baseUrl) ? baseUrl.replace(/\/+$/, "") : fallback;
  const cleanRoute = sanitizeRoute(route);
  return cleanRoute ? `${cleanBase}/${cleanRoute}` : cleanBase;
}

async function fetchJson<T>(path: string, method: "GET" | "POST" = "GET", body?: unknown): Promise<T> {
  const init: RequestInit = {
    method,
    credentials: "same-origin",
    headers:
      method === "POST"
        ? { "Content-Type": "application/json", Accept: "application/json" }
        : { Accept: "application/json" },
  };
  if (method === "POST") {
    init.body = JSON.stringify(body ?? {});
  }
  const res = await fetch(path, init);
  if (!res.ok) {
    let detail = `${res.status} ${res.statusText}`;
    try {
      const errJson = (await res.json()) as { error?: string; message?: string };
      if (errJson?.error) detail = errJson.error;
      else if (errJson?.message) detail = errJson.message;
    } catch {
      // keep status text
    }
    throw new Error(detail);
  }
  return (await res.json()) as T;
}

// Opt-in deterministic preview mode strictly for `?demo=1` headless UI inspection.
function isDemoMode(): boolean {
  if (typeof window === "undefined") return false;
  return new URLSearchParams(window.location.search).get("demo") === "1";
}

let demoState: DesktopStatus | null = null;

function getDemoState(): DesktopStatus {
  if (demoState) return demoState;
  const params = new URLSearchParams(window.location.search);
  const rawEdition = params.get("edition") ?? "suite";
  const edition: Edition =
    rawEdition === "suite" || (WORKSPACE_ORDER as readonly string[]).includes(rawEdition)
      ? (rawEdition as Edition)
      : "suite";
  const rawTarget = params.get("target") ?? "hub";
  const activeTarget: TargetId =
    rawTarget === "hub" || (WORKSPACE_ORDER as readonly string[]).includes(rawTarget)
      ? (rawTarget as TargetId)
      : "hub";
  const themeParam = params.get("theme");
  const theme: ThemePref =
    themeParam === "dark" || themeParam === "light" || themeParam === "system" ? themeParam : "light";
  const langParam = params.get("lang");
  const lang: LangPref = langParam === "en" ? "en" : "zh-tw";

  demoState = {
    edition,
    version: "1.0.1",
    os: params.get("platform") === "windows" ? "windows" : "macos",
    arch: "aarch64",
    activeTarget,
    runtime: {
      kind: "bundled",
      path: "/Applications/BELL Suite.app/Contents/Resources/dist-sidecars/bin/bell-runtime",
      claudeCli: true,
      codexCli: true,
      ghCli: true,
      gitCli: true,
    },
    prefs: {
      theme,
      lang,
      activeTarget,
      autoStart: true,
    },
    workspaces: WORKSPACE_ORDER.map((id, idx) => {
      const meta = PRODUCTS[id];
      const running = idx < 3;
      const starting = idx === 3;
      return {
        id,
        name: meta.displayName,
        shortcutIndex: meta.shortcutIndex,
        accentLight: meta.accentLight,
        accentDark: meta.accentDark,
        port: meta.defaultPort,
        url: `http://127.0.0.1:${meta.defaultPort}`,
        state: running ? "running" : starting ? "starting" : "stopped",
        pid: running ? 28410 + idx * 17 : starting ? 28499 : null,
        manifestValid: running,
        installed: true,
        dbBytes: [425984, 1835008, 950272, 688128, 311296][idx] ?? 131072,
        restartCount: idx === 2 ? 1 : 0,
        lastError: null,
      };
    }),
    vault: {
      rootDir: "/Users/bell/Library/Application Support/com.bell1dev.suite/vault",
      totalBytes: 4210688,
      quotaBytes: 5368709120,
      backupsCount: 3,
      lastBackupAt: "2026-11-16T01:24:00Z",
      databases: [
        {
          appId: "personal",
          path: "/Users/bell/Library/Application Support/com.bell1dev.suite/vault/personal/settings.sqlite",
          bytes: 425984,
          healthy: true,
          tables: 9,
        },
        {
          appId: "explain",
          path: "/Users/bell/Library/Application Support/com.bell1dev.suite/vault/explain/settings.sqlite",
          bytes: 1835008,
          healthy: true,
          tables: 12,
        },
        {
          appId: "pr",
          path: "/Users/bell/Library/Application Support/com.bell1dev.suite/vault/pr/store.json",
          bytes: 950272,
          healthy: true,
          tables: 14,
        },
        {
          appId: "reader",
          path: "/Users/bell/Library/Application Support/com.bell1dev.suite/vault/reader/reader.db",
          bytes: 688128,
          healthy: true,
          tables: 7,
        },
        {
          appId: "campaign",
          path: "/Users/bell/Library/Application Support/com.bell1dev.suite/vault/campaign/campaign.db",
          bytes: 311296,
          healthy: true,
          tables: 11,
        },
      ],
    },
  };
  return demoState;
}

export async function getDesktopStatus(): Promise<DesktopStatus> {
  const tauri = getTauriInternals();
  if (tauri) return tauri.invoke<DesktopStatus>("desktop_status");
  if (isDemoMode()) return structuredClone(getDemoState());
  return fetchJson<DesktopStatus>("/api/desktop/status");
}

export async function getTodayOverview(): Promise<TodayOverview> {
  const tauri = getTauriInternals();
  if (tauri) return tauri.invoke<TodayOverview>("desktop_today");
  if (isDemoMode()) {
    return {
      generatedAt: "2026-11-16T01:30:00Z",
      totalItems: 4,
      sources: [
        {
          appId: "personal",
          status: "unsupported",
          counts: {},
          items: [],
        },
        {
          appId: "explain",
          status: "ready",
          counts: { dueReviews: 2 },
          items: [
            {
              sourceApp: "explain",
              type: "review",
              id: "note-tauri-ipc",
              title: "Tauri v2 Multi-Webview Bounds & Loopback Isolation",
              href: "#/note/note-tauri-ipc",
              state: "due",
              updatedAt: "2026-11-16T00:55:00Z",
              detail: "FSRS 間隔複習到期",
            },
          ],
        },
        {
          appId: "pr",
          status: "ready",
          counts: { needsReview: 1 },
          items: [
            {
              sourceApp: "pr",
              type: "run",
              id: "pr-142",
              title: "bell-core: SQLite Backup API atomic snapshot verification",
              href: "#/discover",
              state: "needs-you",
              updatedAt: "2026-11-16T01:20:00Z",
              detail: "等待最終審查確認",
            },
          ],
        },
        {
          appId: "reader",
          status: "ready",
          counts: { unread: 3 },
          items: [
            {
              sourceApp: "reader",
              type: "page",
              id: "sqlite-wal",
              title: "SQLite Write-Ahead Logging & Online Backup Internals",
              href: "#/page/sqlite-wal",
              state: "new",
              updatedAt: "2026-11-15T22:40:00Z",
              detail: "3 處螢光筆標註",
            },
          ],
        },
        {
          appId: "campaign",
          status: "ready",
          counts: { progressPercent: 58, dueTasks: 12 },
          items: [
            {
              sourceApp: "campaign",
              type: "task",
              id: "task-launch-checklist",
              title: "審核 BELL Desktop v1.0.1 macOS 公證與封裝清單",
              href: "#/tasks",
              state: "due",
              updatedAt: "2026-11-16T01:12:00Z",
              detail: "今日 18:00 到期",
            },
          ],
        },
      ],
    };
  }
  return fetchJson<TodayOverview>("/api/desktop/today");
}

export async function activateTarget(target: TargetId, route?: string): Promise<DesktopStatus> {
  const cleanRoute = sanitizeRoute(route) || undefined;
  const tauri = getTauriInternals();
  if (tauri) {
    return tauri.invoke<DesktopStatus>("desktop_activate", {
      target,
      route: cleanRoute ?? null,
    });
  }
  if (isDemoMode()) {
    const st = getDemoState();
    st.activeTarget = target;
    st.prefs.activeTarget = target;
    if (target !== "hub") {
      const ws = st.workspaces.find((w) => w.id === target);
      if (ws && ws.state !== "running") {
        ws.state = "running";
        ws.pid = 28512;
        ws.manifestValid = true;
      }
    }
    return structuredClone(st);
  }
  return fetchJson<DesktopStatus>(
    "/api/desktop/activate",
    "POST",
    cleanRoute ? { target, route: cleanRoute } : { target },
  );
}

export async function workspaceAction(id: AppId, action: WorkspaceAction): Promise<DesktopStatus> {
  const tauri = getTauriInternals();
  if (tauri) {
    return tauri.invoke<DesktopStatus>("desktop_workspace_action", { id, action });
  }
  if (isDemoMode()) {
    const st = getDemoState();
    const ws = st.workspaces.find((w) => w.id === id);
    if (ws) {
      if (action === "stop") {
        ws.state = "stopped";
        ws.pid = null;
        ws.manifestValid = false;
      } else {
        ws.state = "running";
        ws.pid = (ws.pid ?? 28600) + 1;
        ws.manifestValid = true;
        if (action === "restart") ws.restartCount += 1;
      }
    }
    return structuredClone(st);
  }
  return fetchJson<DesktopStatus>(
    `/api/desktop/workspaces/${encodeURIComponent(id)}/${encodeURIComponent(action)}`,
    "POST",
    {},
  );
}

export async function startAllWorkspaces(): Promise<DesktopStatus> {
  const tauri = getTauriInternals();
  if (tauri) return tauri.invoke<DesktopStatus>("desktop_start_all");
  if (isDemoMode()) {
    const st = getDemoState();
    st.workspaces.forEach((ws, i) => {
      ws.state = "running";
      ws.pid = ws.pid ?? 28700 + i;
      ws.manifestValid = true;
    });
    return structuredClone(st);
  }
  return fetchJson<DesktopStatus>("/api/desktop/workspaces/start-all", "POST", {});
}

export async function savePrefs(
  prefs: Partial<Pick<DesktopPrefs, "theme" | "lang" | "autoStart">>,
): Promise<DesktopStatus> {
  const tauri = getTauriInternals();
  if (tauri) {
    return tauri.invoke<DesktopStatus>("desktop_save_prefs", {
      theme: prefs.theme ?? null,
      lang: prefs.lang ?? null,
      autoStart: prefs.autoStart ?? null,
    });
  }
  if (isDemoMode()) {
    const st = getDemoState();
    if (prefs.theme !== undefined) st.prefs.theme = prefs.theme;
    if (prefs.lang !== undefined) st.prefs.lang = prefs.lang;
    if (prefs.autoStart !== undefined) st.prefs.autoStart = prefs.autoStart;
    return structuredClone(st);
  }
  return fetchJson<DesktopStatus>("/api/desktop/prefs", "POST", prefs);
}

export async function backupVault(): Promise<VaultBackupResult> {
  const tauri = getTauriInternals();
  if (tauri) return tauri.invoke<VaultBackupResult>("desktop_vault_backup");
  if (isDemoMode()) {
    const st = getDemoState();
    const now = new Date().toISOString();
    st.vault.backupsCount += 1;
    st.vault.lastBackupAt = now;
    return {
      backupDir: `${st.vault.rootDir}/backups/snapshot-20261116-013500`,
      files: st.vault.databases.map((d) => `${d.appId}.db`),
      totalBytes: st.vault.totalBytes,
      createdAt: now,
    };
  }
  return fetchJson<VaultBackupResult>("/api/desktop/vault/backup", "POST", {});
}

export async function getWorkspaceLogs(id: AppId): Promise<WorkspaceLogs> {
  const tauri = getTauriInternals();
  if (tauri) return tauri.invoke<WorkspaceLogs>("desktop_logs", { id });
  if (isDemoMode()) {
    const meta = PRODUCTS[id];
    return {
      id,
      lines: [
        `[2026-11-16T01:28:04Z] [bell-core] spawning sidecar ${id} via bundled bell-runtime`,
        `[2026-11-16T01:28:04Z] [${id}] binding loopback listener http://127.0.0.1:${meta.defaultPort}`,
        `[2026-11-16T01:28:05Z] [${id}] SQLite WAL journal initialized (PRAGMA foreign_keys = ON)`,
        `[2026-11-16T01:28:05Z] [${id}] GET /api/suite/manifest -> 200 OK (schemaVersion: 1)`,
        `[2026-11-16T01:30:00Z] [${id}] GET /api/suite/today -> 200 OK`,
      ],
    };
  }
  return fetchJson<WorkspaceLogs>(`/api/desktop/logs/${encodeURIComponent(id)}`);
}

export async function windowAction(action: WindowAction): Promise<void> {
  const tauri = getTauriInternals();
  if (tauri) {
    await tauri.invoke<void>("desktop_window_action", { action });
  }
}

export interface DesktopRenderReport {
  chromeHeight: number;
  mainHeight: number;
  textLength: number;
  theme: string;
}

export async function reportDesktopReady(report: DesktopRenderReport): Promise<void> {
  const tauri = getTauriInternals();
  if (tauri) {
    await tauri.invoke<void>("desktop_report_ready", { report }).catch(() => {});
  }
}
