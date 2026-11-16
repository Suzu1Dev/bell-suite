function assertEqual<T>(actual: T, expected: T, msg?: string): void {
  const a = JSON.stringify(actual);
  const b = JSON.stringify(expected);
  if (a !== b) {
    throw new Error(`${msg ?? "Assertion failed"}: expected ${b}, got ${a}`);
  }
}

const assert = { equal: assertEqual, deepEqual: assertEqual };
import {
  activateTarget,
  backupVault,
  buildSafeWorkspaceLaunchUrl,
  escHtml,
  getDesktopStatus,
  getTodayOverview,
  getWorkspaceLogs,
  isTauriRuntime,
  resolveChromePlatform,
  sanitizeRoute,
  savePrefs,
  shouldShowPreviewLights,
  startAllWorkspaces,
  windowAction,
  workspaceAction,
} from "./api";
import { getCopy } from "./i18n";
import { PRODUCTS } from "./types";

// 1. Verify Ground-Truth Brand Tokens (CONTRACT.md §1)
assert.equal(PRODUCTS.suite.accentLight, "#0f8a7d");
assert.equal(PRODUCTS.suite.accentDark, "#43cabc");
assert.equal(PRODUCTS.personal.accentLight, "#a4532c");
assert.equal(PRODUCTS.personal.accentDark, "#e08a5c");
assert.equal(PRODUCTS.explain.accentLight, "#5b45e0");
assert.equal(PRODUCTS.explain.accentDark, "#a799ff");
assert.equal(PRODUCTS.pr.accentLight, "#1b6b4c");
assert.equal(PRODUCTS.pr.accentDark, "#46c289");
assert.equal(PRODUCTS.reader.accentLight, "#0b6bcb");
assert.equal(PRODUCTS.reader.accentDark, "#38aef8");
assert.equal(PRODUCTS.campaign.accentLight, "#9e2b5c");
assert.equal(PRODUCTS.campaign.accentDark, "#e25b91");

// 2. Verify Cross-OS Chrome Platform Detection & Traffic-Light Rules
assert.equal(resolveChromePlatform("linux", "?platform=macos", "Mozilla/5.0"), "macos");
assert.equal(resolveChromePlatform("macos", "?platform=windows", "Macintosh"), "windows");
assert.equal(resolveChromePlatform("macos", "", "Mozilla/5.0"), "macos");
assert.equal(resolveChromePlatform("windows", "", "Mozilla/5.0"), "windows");
assert.equal(shouldShowPreviewLights("macos", false, "?previewLights=1"), true);
assert.equal(shouldShowPreviewLights("macos", true, "?previewLights=1"), false);
assert.equal(shouldShowPreviewLights("windows", false, "?previewLights=1"), false);

// 3. Verify Security Sanitizers (Loopback-only & Hash Route & XSS)
assert.equal(
  buildSafeWorkspaceLaunchUrl("https://evil.example:4601", 4601, "#/today"),
  "http://127.0.0.1:4601/#/today",
);
assert.equal(
  buildSafeWorkspaceLaunchUrl("http://127.0.0.1:4602", 4602, "//evil.example"),
  "http://127.0.0.1:4602",
);
assert.equal(sanitizeRoute("#/note/note-1"), "#/note/note-1");
assert.equal(sanitizeRoute("javascript:alert(1)"), "");
assert.equal(escHtml(`<img src=x onerror="1">`), "&lt;img src=x onerror=&quot;1&quot;&gt;");

// 4. Verify Tauri v2 Transport (`__TAURI_INTERNALS__.invoke`)
const tauriCalls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
(globalThis as Record<string, unknown>).window = {
  location: { search: "" },
  __TAURI_INTERNALS__: {
    invoke: async (cmd: string, args?: Record<string, unknown>) => {
      tauriCalls.push({ cmd, args });
      return { ok: true };
    },
  },
};

assert.equal(isTauriRuntime(), true);
await getDesktopStatus();
await getTodayOverview();
await activateTarget("explain", "#/note/note-1");
await workspaceAction("pr", "restart");
await startAllWorkspaces();
await savePrefs({ theme: "dark", lang: "zh-tw", autoStart: true });
await backupVault();
await getWorkspaceLogs("reader");
await windowAction("toggleMaximize");

assert.deepEqual(
  tauriCalls.map((c) => c.cmd),
  [
    "desktop_status",
    "desktop_today",
    "desktop_activate",
    "desktop_workspace_action",
    "desktop_start_all",
    "desktop_save_prefs",
    "desktop_vault_backup",
    "desktop_logs",
    "desktop_window_action",
  ],
);
assert.deepEqual(tauriCalls[2]?.args, { target: "explain", route: "#/note/note-1" });
assert.deepEqual(tauriCalls[3]?.args, { id: "pr", action: "restart" });
assert.deepEqual(tauriCalls[5]?.args, { theme: "dark", lang: "zh-tw", autoStart: true });
assert.deepEqual(tauriCalls[7]?.args, { id: "reader" });
assert.deepEqual(tauriCalls[8]?.args, { action: "toggleMaximize" });

// 5. Verify Browser Daemon HTTP Transport (`fetch("/api/desktop/...")`)
const httpCalls: Array<{ path: string; method: string; body?: string }> = [];
(globalThis as Record<string, unknown>).window = { location: { search: "" } };
(globalThis as Record<string, unknown>).fetch = async (path: string, init?: RequestInit) => {
  httpCalls.push({
    path,
    method: init?.method ?? "GET",
    body: typeof init?.body === "string" ? init.body : undefined,
  });
  return {
    ok: true,
    status: 200,
    statusText: "OK",
    json: async () => ({ ok: true }),
  } as Response;
};

assert.equal(isTauriRuntime(), false);
await getDesktopStatus();
await getTodayOverview();
await activateTarget("personal", "#/today");
await workspaceAction("campaign", "start");
await startAllWorkspaces();
await savePrefs({ theme: "light" });
await backupVault();
await getWorkspaceLogs("explain");
await windowAction("close"); // must be no-op in browser

assert.deepEqual(
  httpCalls.map((c) => `${c.method} ${c.path}`),
  [
    "GET /api/desktop/status",
    "GET /api/desktop/today",
    "POST /api/desktop/activate",
    "POST /api/desktop/workspaces/campaign/start",
    "POST /api/desktop/workspaces/start-all",
    "POST /api/desktop/prefs",
    "POST /api/desktop/vault/backup",
    "GET /api/desktop/logs/explain",
  ],
);
assert.deepEqual(JSON.parse(httpCalls[2]?.body ?? "{}"), {
  target: "personal",
  route: "#/today",
});

// 6. Verify i18n Round 2 copy polish (singular snapshot, capitalized Today states, JSON labels)
const enCopy = getCopy("en");
assert.equal(enCopy.vaultBackupsSummary(1, "10:00"), "1 snapshot · Last backup 10:00");
assert.equal(enCopy.vaultBackupsSummary(2, "10:00"), "2 snapshots · Last backup 10:00");
assert.deepEqual(enCopy.todayStateLabel, {
  due: "Due",
  "needs-you": "Needs You",
  waiting: "Waiting",
  new: "New",
});
assert.equal(enCopy.vaultJsonOk, "JSON OK");
assert.equal(enCopy.vaultJsonEntries(1), "1 entry");
assert.equal(enCopy.vaultJsonEntries(14), "14 entries");

console.log("desktop-ui contract & transport tests passed (9 Tauri cmds + 8 HTTP routes + token parity + i18n)");
