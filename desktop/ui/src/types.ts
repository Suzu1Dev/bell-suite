export type AppId = "personal" | "explain" | "pr" | "reader" | "campaign";
export type Edition = "suite" | AppId;
export type TargetId = "hub" | AppId;
export type WorkspaceState = "stopped" | "starting" | "running" | "error";
export type WorkspaceAction = "start" | "stop" | "restart";
export type ThemePref = "light" | "dark" | "system";
export type LangPref = "zh-tw" | "en";
export type WindowAction = "minimize" | "toggleMaximize" | "close";
export type ChromePlatform = "macos" | "windows";
export type TodayItemState = "due" | "needs-you" | "waiting" | "new";

export interface RuntimeInfo {
  kind: string;
  path: string;
  claudeCli: boolean;
  codexCli: boolean;
  ghCli: boolean;
  gitCli: boolean;
}

export interface DesktopPrefs {
  theme: ThemePref;
  lang: LangPref;
  activeTarget: TargetId;
  autoStart: boolean;
}

export interface WorkspaceInfo {
  id: AppId;
  name: string;
  shortcutIndex: number;
  accentLight: string;
  accentDark: string;
  port: number;
  url: string;
  state: WorkspaceState;
  pid: number | null;
  manifestValid: boolean;
  installed: boolean;
  dbBytes: number;
  restartCount: number;
  lastError: string | null;
}

export interface VaultDatabaseInfo {
  appId: AppId;
  path: string;
  bytes: number;
  healthy: boolean;
  tables: number;
}

export interface VaultInfo {
  rootDir: string;
  totalBytes: number;
  quotaBytes: number;
  backupsCount: number;
  lastBackupAt: string | null;
  databases: VaultDatabaseInfo[];
}

export interface DesktopStatus {
  edition: Edition;
  version: string;
  os: string;
  arch: string;
  activeTarget: TargetId;
  runtime: RuntimeInfo;
  prefs: DesktopPrefs;
  workspaces: WorkspaceInfo[];
  vault: VaultInfo;
}

export interface TodayItem {
  sourceApp: AppId;
  type: string;
  id: string;
  title: string;
  href: string;
  state: TodayItemState;
  updatedAt: string;
  dueAt?: string;
  detail?: string;
}

export interface TodaySource {
  appId: AppId;
  status: string;
  counts: Record<string, number>;
  items: TodayItem[];
}

export interface TodayOverview {
  generatedAt: string;
  totalItems: number;
  sources: TodaySource[];
}

export interface VaultBackupResult {
  backupDir: string;
  files: string[] | number;
  totalBytes: number;
  createdAt: string;
}

export interface WorkspaceLogs {
  id: AppId;
  lines: string[];
}

export interface ProductMeta {
  id: Edition;
  shortLabel: string;
  displayName: string;
  bundleId: string;
  shortcutIndex: number;
  defaultPort: number;
  accentLight: string;
  accentDark: string;
}

export const WORKSPACE_ORDER: readonly AppId[] = [
  "personal",
  "explain",
  "pr",
  "reader",
  "campaign",
] as const;

export const PRODUCTS: Record<Edition, ProductMeta> = {
  suite: {
    id: "suite",
    shortLabel: "Hub",
    displayName: "BELL Suite",
    bundleId: "com.bell1dev.suite",
    shortcutIndex: 0,
    defaultPort: 4600,
    accentLight: "#0f8a7d",
    accentDark: "#43cabc",
  },
  personal: {
    id: "personal",
    shortLabel: "Personal",
    displayName: "BELL Personal",
    bundleId: "com.bell1dev.personal",
    shortcutIndex: 1,
    defaultPort: 4601,
    accentLight: "#a4532c",
    accentDark: "#e08a5c",
  },
  explain: {
    id: "explain",
    shortLabel: "Explain",
    displayName: "BELL Explain",
    bundleId: "com.bell1dev.explain",
    shortcutIndex: 2,
    defaultPort: 4602,
    accentLight: "#5b45e0",
    accentDark: "#a799ff",
  },
  pr: {
    id: "pr",
    shortLabel: "PR",
    displayName: "BELL PR",
    bundleId: "com.bell1dev.pr",
    shortcutIndex: 3,
    defaultPort: 4603,
    accentLight: "#1b6b4c",
    accentDark: "#46c289",
  },
  reader: {
    id: "reader",
    shortLabel: "Reader",
    displayName: "BELL Web Reader",
    bundleId: "com.bell1dev.reader",
    shortcutIndex: 4,
    defaultPort: 4604,
    accentLight: "#0b6bcb",
    accentDark: "#38aef8",
  },
  campaign: {
    id: "campaign",
    shortLabel: "Campaign",
    displayName: "BELL Campaign",
    bundleId: "com.bell1dev.campaign",
    shortcutIndex: 5,
    defaultPort: 4605,
    accentLight: "#9e2b5c",
    accentDark: "#e25b91",
  },
};

export const WORKSPACE_ROUTES: Record<AppId, readonly string[]> = {
  personal: ["#/today", "#/tasks", "#/events", "#/ideas", "#/inbox"],
  explain: ["#/", "#/library", "#/graph", "#/favorites", "#/review"],
  pr: ["#/", "#/discover", "#/repos", "#/learning", "#/settings"],
  reader: ["#/", "#/library", "#/topics"],
  campaign: ["#/", "#/schedule", "#/tasks", "#/forms", "#/meetings"],
};
