pub mod i18n;
pub mod icons;
pub mod palette;

use crate::i18n::{
    app_badge_name, app_short_name, app_summary, copy_for, deep_routes, esc, format_backup_meta,
    format_bytes, format_quota_pair, format_shortcut, format_table_or_entry_count, header_summary,
    open_in_app_cta, open_primary_cta, sanitize_hash_route, short_iso, today_state_label,
    ws_state_label, ws_state_slug, Copy, Lang,
};
use bell_core::{
    AppId, DesktopPrefs, DesktopStatus, Edition, RuntimeInfo, TodayItem, TodayOverview,
    TodaySource, VaultDatabase, VaultStatus, WorkspaceLogs, WorkspaceState, WorkspaceStatus,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub use icons::{build_favicon_ico_bytes, FAVICON_SVG};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HubToast {
    pub kind: String,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HubUiClientState {
    #[serde(default)]
    pub view: String,
    #[serde(default)]
    pub today_filter: String,
    #[serde(default)]
    pub selected_item_key: Option<String>,
    #[serde(default)]
    pub logs_app: Option<AppId>,
    #[serde(default)]
    pub palette_open: bool,
    #[serde(default)]
    pub palette_query: String,
    #[serde(default)]
    pub palette_index: usize,
    #[serde(default)]
    pub busy_label: Option<String>,
    #[serde(default)]
    pub toast: Option<HubToast>,
    #[serde(default)]
    pub theme_polarity_hint: Option<String>,
    #[serde(default)]
    pub cached_today_count: Option<usize>,
}

pub struct HubRenderModel<'a> {
    pub status: &'a DesktopStatus,
    pub today: Option<&'a TodayOverview>,
    pub logs: Option<&'a WorkspaceLogs>,
    pub ui: &'a HubUiClientState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HubViewPatch {
    pub html: String,
    pub resolved_theme: String,
    pub lang: String,
    pub title: String,
    pub navigate_url: Option<String>,
}

pub const HUB_CSS: &str = include_str!("hub.css");

pub const HUB_BOOT_JS: &str = r###"(function () {
  "use strict";
  const qs = new URLSearchParams(location.search);
  const isDemo = qs.get("demo") === "1" || qs.get("demo") === "true";
  const isTauri = typeof window.__TAURI_INTERNALS__?.invoke === "function";
  const invoke = (cmd, args) => window.__TAURI_INTERNALS__.invoke(cmd, args || {});

  if (qs.get("theme") === "light" || qs.get("theme") === "dark") {
    document.documentElement.dataset.theme = qs.get("theme");
  } else if (!qs.get("theme")) {
    document.documentElement.dataset.theme = matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
  }

  const initShell = document.querySelector("#hub-root .hub-shell");
  const initCount = initShell && initShell.dataset.todayCount ? Number(initShell.dataset.todayCount) : null;

  const uiState = {
    view: qs.get("view") || "overview",
    todayFilter: qs.get("filter") || "all",
    selectedItemKey: null,
    logsApp: "personal",
    paletteOpen: qs.get("palette") === "1",
    paletteQuery: "",
    paletteIndex: 0,
    busyLabel: null,
    toast: null,
    themePolarityHint: matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark",
    cachedTodayCount: Number.isFinite(initCount) ? initCount : null
  };

  let reportedReady = false;
  let readyScheduled = false;
  let toastTimer = null;
  let renderSeq = 0;

  function measureReady(attempt) {
    if (reportedReady) return;
    const r = document.getElementById("hub-root");
    if (!r) return;
    const sEl = r.querySelector(".hub-sheet") || r;
    const rRect = r.getBoundingClientRect();
    const sRect = sEl.getBoundingClientRect();
    if ((rRect.height <= 0 || sRect.height <= 0) && attempt < 1) {
      setTimeout(() => measureReady(attempt + 1), 80);
      return;
    }
    if (reportedReady) return;
    reportedReady = true;
    const m = {
      chromeHeight: Math.round(rRect.height),
      mainHeight: Math.round(sRect.height),
      textLength: (r.textContent || "").trim().length,
      theme: document.documentElement.dataset.theme || "dark"
    };
    invoke("desktop_report_ready", { report: m }).catch(() => {});
  }

  function showToast(kind, title, detail) {
    uiState.toast = { kind, title, detail };
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      uiState.toast = null;
      void syncHub();
    }, 5200);
  }

  async function apiPost(path, body) {
    const r = await fetch(path, {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json", "Accept": "application/json" },
      body: JSON.stringify(body || {})
    });
    const data = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(data.error || (r.status + " " + r.statusText));
    return data;
  }

  function applyPatch(patch) {
    if (!patch) return;
    if (patch.navigateUrl) {
      location.assign(patch.navigateUrl);
      return;
    }
    if (patch.resolvedTheme) document.documentElement.dataset.theme = patch.resolvedTheme;
    if (patch.lang) document.documentElement.lang = patch.lang;
    if (patch.title) document.title = patch.title;

    const root = document.getElementById("hub-root");
    if (!root || typeof patch.html !== "string") return;

    const prevInput = uiState.paletteOpen ? document.getElementById("palette-input") : null;
    const liveVal = prevInput ? prevInput.value : null;
    const selStart = prevInput ? prevInput.selectionStart : null;
    const selEnd = prevInput ? prevInput.selectionEnd : null;
    const prevBody = root.querySelector(".sheet-body");
    const prevScroll = prevBody ? prevBody.scrollTop : 0;

    root.innerHTML = patch.html;

    const shellEl = root.querySelector(".hub-shell");
    if (shellEl && shellEl.dataset.todayLive === "1") {
      const cnt = Number(shellEl.dataset.todayCount);
      if (Number.isFinite(cnt)) uiState.cachedTodayCount = cnt;
    }

    const nextBody = root.querySelector(".sheet-body");
    if (nextBody && prevScroll > 0) nextBody.scrollTop = prevScroll;

    if (uiState.paletteOpen) {
      const input = document.getElementById("palette-input");
      if (input) {
        if (liveVal !== null && liveVal !== uiState.paletteQuery) input.value = liveVal;
        input.focus({ preventScroll: true });
        const pos = selStart !== null ? selStart : input.value.length;
        try { input.setSelectionRange(pos, selEnd !== null ? selEnd : pos); } catch {}
      }
      const active = document.getElementById("palette-active-item");
      if (active) active.scrollIntoView({ block: "nearest" });
    }

    if (isTauri && !reportedReady && !readyScheduled) {
      readyScheduled = true;
      requestAnimationFrame(() => measureReady(0));
      setTimeout(() => measureReady(0), 50);
    }
  }

  async function syncHub() {
    const seq = ++renderSeq;
    try {
      const patch = isTauri
        ? await invoke("desktop_hub_render", { ui: uiState })
        : await apiPost("/api/desktop/hub-render" + location.search, uiState);
      if (seq === renderSeq) applyPatch(patch);
    } catch {}
  }

  async function openWorkspace(wsId, route) {
    uiState.paletteOpen = false;
    uiState.busyLabel = wsId.toUpperCase();
    await syncHub();
    try {
      if (isTauri) {
        await invoke("desktop_activate", { target: wsId, route: route || null });
        uiState.busyLabel = null;
        return;
      }
      if (isDemo) {
        uiState.busyLabel = null;
        showToast("ok", "BELL " + wsId.toUpperCase(), "Demo → " + wsId + " " + (route || "#/"));
        await syncHub();
        return;
      }
      let st = await apiPost("/api/desktop/activate", route ? { target: wsId, route } : { target: wsId });
      if (st.navigateUrl) { location.assign(st.navigateUrl); return; }
      let ws = (st.workspaces || []).find((w) => w.id === wsId);
      if (ws && ws.installed && ws.state !== "running") {
        st = await apiPost("/api/desktop/workspaces/" + encodeURIComponent(wsId) + "/start", {});
        ws = (st.workspaces || []).find((w) => w.id === wsId);
      }
      uiState.busyLabel = null;
      if (ws && ws.state === "running") {
        if (!route && wsId === "pr") location.assign("/api/desktop/open/pr");
        else location.assign(String(ws.url).replace(/\/+$/, "") + "/" + (route || "#/"));
      } else {
        await syncHub();
      }
    } catch (err) {
      uiState.busyLabel = null;
      showToast("error", "Activation Failed", err.message || String(err));
      await syncHub();
    }
  }

  async function setPrefs(patch) {
    if (patch.theme && (isDemo || qs.has("theme"))) {
      qs.set("theme", patch.theme === "system" ? uiState.themePolarityHint : patch.theme);
      history.replaceState(null, "", "?" + qs.toString());
    }
    if (patch.lang && (isDemo || qs.has("lang"))) {
      qs.set("lang", patch.lang);
      history.replaceState(null, "", "?" + qs.toString());
    }
    if (!isDemo) {
      if (isTauri) {
        await invoke("desktop_save_prefs", {
          theme: patch.theme ?? null,
          lang: patch.lang ?? null,
          autoStart: patch.autoStart ?? null
        }).catch(() => {});
      } else {
        await apiPost("/api/desktop/prefs", patch).catch(() => {});
      }
    }
    await syncHub();
  }

  async function execPaletteEl(el) {
    if (!el) return;
    const kind = el.dataset.execKind;
    uiState.paletteOpen = false;
    uiState.paletteQuery = "";
    uiState.paletteIndex = 0;
    if (kind === "open-ws") await openWorkspace(el.dataset.ws, null);
    else if (kind === "jump-route") await openWorkspace(el.dataset.ws, el.dataset.route || "#/");
    else if (kind === "set-view") { uiState.view = el.dataset.view || "overview"; await syncHub(); }
    else if (kind === "start-all") await runStartAll();
    else if (kind === "vault-backup") await runBackup();
    else if (kind === "toggle-theme") {
      const next = document.documentElement.dataset.theme === "light" ? "dark" : "light";
      await setPrefs({ theme: next });
    } else if (kind === "toggle-lang") {
      const next = document.documentElement.lang === "en" ? "zh-tw" : "en";
      await setPrefs({ lang: next });
    }
  }

  async function runStartAll() {
    uiState.busyLabel = "Starting…";
    await syncHub();
    try {
      if (!isDemo) {
        if (isTauri) await invoke("desktop_start_all", {});
        else await apiPost("/api/desktop/workspaces/start-all", {});
      }
      uiState.busyLabel = null;
      showToast("ok", "BELL Suite", "All installed workspaces started");
    } catch (err) {
      uiState.busyLabel = null;
      showToast("error", "Start All Failed", err.message || String(err));
    }
    await syncHub();
  }

  async function runBackup() {
    uiState.busyLabel = "Snapshot…";
    await syncHub();
    try {
      let detail = "Atomic SQLite snapshot saved";
      if (!isDemo) {
        const res = isTauri
          ? await invoke("desktop_vault_backup", {})
          : await apiPost("/api/desktop/vault/backup", {});
        const cnt = Array.isArray(res.files) ? res.files.length : (res.files || 0);
        detail = cnt + " files → " + (res.backupDir || "vault/backups");
      }
      uiState.busyLabel = null;
      showToast("ok", "Vault Snapshot Complete", detail);
    } catch (err) {
      uiState.busyLabel = null;
      showToast("error", "Snapshot Failed", err.message || String(err));
    }
    await syncHub();
  }

  document.addEventListener("click", (ev) => {
    const el = ev.target.closest("[data-action]");
    if (!el) return;
    const act = el.dataset.action;
    if (act === "close-palette" && ev.target !== el) return;

    if (act === "set-view") { uiState.view = el.dataset.view || "overview"; void syncHub(); }
    else if (act === "set-filter") { uiState.todayFilter = el.dataset.filter || "all"; void syncHub(); }
    else if (act === "select-item") {
      uiState.selectedItemKey = el.dataset.itemKey || null;
      if (el.dataset.switchToday === "1") uiState.view = "today";
      void syncHub();
    } else if (act === "open-ws") {
      if (el.dataset.ws) void openWorkspace(el.dataset.ws, null);
    } else if (act === "jump-today") {
      if (el.dataset.ws) void openWorkspace(el.dataset.ws, el.dataset.route || "#/");
    } else if (act === "ws-op") {
      const id = el.dataset.ws;
      const op = el.dataset.op;
      if (!id || !op) return;
      uiState.busyLabel = id + ":" + op;
      void syncHub();
      const p = isDemo
        ? Promise.resolve()
        : (isTauri ? invoke("desktop_workspace_action", { id, action: op }) : apiPost("/api/desktop/workspaces/" + encodeURIComponent(id) + "/" + encodeURIComponent(op), {}));
      p.catch((e) => showToast("error", id.toUpperCase(), e.message || String(e)))
        .finally(() => { uiState.busyLabel = null; void syncHub(); });
    } else if (act === "start-all") void runStartAll();
    else if (act === "vault-backup") void runBackup();
    else if (act === "set-theme") void setPrefs({ theme: el.dataset.themePref || "dark" });
    else if (act === "set-lang") void setPrefs({ lang: el.dataset.lang || "zh-tw" });
    else if (act === "select-logs-app") {
      uiState.logsApp = el.dataset.ws || "personal";
      if (el.dataset.switchLogs === "1") uiState.view = "logs";
      void syncHub();
    } else if (act === "refresh-logs") void syncHub();
    else if (act === "copy-text") {
      const txt = el.dataset.copy || "";
      navigator.clipboard?.writeText(txt).catch(() => {});
      showToast("ok", "Copied", txt.length > 72 ? txt.slice(0, 72) + "…" : txt);
      void syncHub();
    } else if (act === "open-palette") {
      uiState.paletteOpen = true;
      uiState.paletteQuery = "";
      uiState.paletteIndex = 0;
      void syncHub();
    } else if (act === "close-palette") {
      uiState.paletteOpen = false;
      void syncHub();
    } else if (act === "palette-exec") void execPaletteEl(el);
    else if (act === "dismiss-toast") { uiState.toast = null; void syncHub(); }
  });

  document.addEventListener("input", (ev) => {
    if (ev.target && ev.target.id === "palette-input") {
      uiState.paletteQuery = ev.target.value || "";
      uiState.paletteIndex = 0;
      void syncHub();
    }
  });

  window.addEventListener("keydown", (ev) => {
    const mod = ev.metaKey || ev.ctrlKey;
    if (mod && !ev.altKey && !ev.shiftKey && ev.key.toLowerCase() === "k") {
      ev.preventDefault();
      uiState.paletteOpen = !uiState.paletteOpen;
      if (uiState.paletteOpen) { uiState.paletteQuery = ""; uiState.paletteIndex = 0; }
      void syncHub();
      return;
    }
    if (uiState.paletteOpen) {
      const listEl = document.querySelector(".palette-list");
      const count = Number(listEl?.dataset.paletteCount || "0");
      if (ev.key === "Escape") {
        ev.preventDefault();
        uiState.paletteOpen = false;
        void syncHub();
      } else if (ev.key === "ArrowDown" && count > 0) {
        ev.preventDefault();
        uiState.paletteIndex = (uiState.paletteIndex + 1) % count;
        void syncHub();
      } else if (ev.key === "ArrowUp" && count > 0) {
        ev.preventDefault();
        uiState.paletteIndex = (uiState.paletteIndex + count - 1) % count;
        void syncHub();
      } else if (ev.key === "Enter") {
        ev.preventDefault();
        void execPaletteEl(document.getElementById("palette-active-item"));
      }
      return;
    }
    if (mod && !ev.altKey && !ev.shiftKey && /^[1-5]$/.test(ev.key)) {
      const tag = ev.target?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA") return;
      ev.preventDefault();
      const apps = ["personal", "explain", "pr", "reader", "campaign"];
      void openWorkspace(apps[Number(ev.key) - 1], null);
    }
  });

  matchMedia("(prefers-color-scheme: light)").addEventListener("change", (e) => {
    uiState.themePolarityHint = e.matches ? "light" : "dark";
    void syncHub();
  });

  window.addEventListener("bell:sync", () => { void syncHub(); });
  setInterval(() => { if (!document.hidden && !uiState.paletteOpen && !uiState.busyLabel) void syncHub(); }, 5000);
  void syncHub();
})();
"###;

fn resolve_theme(pref: &str, hint: Option<&str>) -> &'static str {
    let p = pref.trim().to_ascii_lowercase();
    if p == "light" {
        "light"
    } else if p == "dark" {
        "dark"
    } else if hint.is_some_and(|h| h.trim().eq_ignore_ascii_case("light")) {
        "light"
    } else {
        "dark"
    }
}

fn normalize_view(raw: &str) -> &'static str {
    match raw.trim() {
        "today" => "today",
        "workspaces" => "workspaces",
        "vault" => "vault",
        "logs" => "logs",
        _ => "overview",
    }
}

fn normalize_filter(raw: &str) -> &'static str {
    match raw.trim() {
        "due" => "due",
        "needs-you" => "needs-you",
        "waiting" => "waiting",
        "new" => "new",
        _ => "all",
    }
}

fn ordered_workspaces(status: &DesktopStatus) -> Vec<WorkspaceStatus> {
    let primary = status.edition.primary_app();
    let mut order = Vec::with_capacity(5);
    if let Some(p) = primary {
        order.push(p);
    }
    for app in AppId::ALL {
        if Some(app) != primary {
            order.push(app);
        }
    }

    let mut out = Vec::with_capacity(5);
    for app in order {
        if let Some(found) = status.workspaces.iter().find(|w| w.id == app) {
            out.push(found.clone());
        } else {
            out.push(WorkspaceStatus {
                id: app,
                name: app.display_name().to_string(),
                shortcut_index: app.shortcut_index(),
                accent_light: app.accent_light().to_string(),
                accent_dark: app.accent_dark().to_string(),
                port: app.default_port(),
                url: format!("http://127.0.0.1:{}", app.default_port()),
                state: WorkspaceState::Stopped,
                pid: None,
                manifest_valid: false,
                installed: false,
                db_bytes: 0,
                restart_count: 0,
                last_error: None,
            });
        }
    }
    out
}

fn render_primary_cta_button(status: &DesktopStatus, lang: Lang, c: &Copy) -> String {
    match status.edition.primary_app() {
        None => format!(
            r#"<button type="button" class="btn btn-primary" data-action="start-all">{ico}<span>{lbl}</span></button>"#,
            ico = icons::bolt(),
            lbl = esc(c.start_all),
        ),
        Some(primary) => format!(
            r#"<button type="button" class="btn btn-primary" data-action="open-ws" data-ws="{app}">{ico}<span>{lbl}</span></button>"#,
            app = primary.as_str(),
            ico = icons::app_icon(primary),
            lbl = esc(&open_primary_cta(primary, lang)),
        ),
    }
}

fn flatten_today_items(today: Option<&TodayOverview>) -> Vec<&TodayItem> {
    let Some(ov) = today else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for src in &ov.sources {
        for it in &src.items {
            out.push(it);
        }
    }
    out
}

fn item_key(item: &TodayItem) -> String {
    format!("{}:{}", item.source_app.as_str(), item.id)
}

fn render_rail(
    status: &DesktopStatus,
    workspaces: &[WorkspaceStatus],
    total_today: usize,
    active_view: &str,
    lang: Lang,
    c: &Copy,
) -> String {
    let installed = workspaces.iter().filter(|w| w.installed).count().max(1);
    let running = workspaces
        .iter()
        .filter(|w| w.state == WorkspaceState::Running)
        .count();
    let brand_title = match status.edition {
        Edition::Suite => c.hub_name,
        other => other.display_name(),
    };
    let online_dot = if running > 0 {
        "dot-running"
    } else {
        "dot-stopped"
    };
    let cmd_k = format_shortcut(&status.os, "K");

    let nav_items: [(&str, &str, &'static str, Option<String>); 5] = [
        ("overview", c.nav_overview, icons::overview(), None),
        (
            "today",
            c.nav_today,
            icons::inbox(),
            Some(total_today.to_string()),
        ),
        (
            "workspaces",
            c.nav_workspaces,
            icons::layers(),
            Some(format!("{running}/{installed}")),
        ),
        (
            "vault",
            c.nav_vault,
            icons::database(),
            Some(status.vault.databases.len().to_string()),
        ),
        ("logs", c.nav_logs, icons::terminal(), None),
    ];

    let mut nav_html = String::with_capacity(1200);
    for (vid, label, ico, count) in nav_items {
        let cur = if active_view == vid {
            r#" aria-current="page""#
        } else {
            ""
        };
        let cnt_html = match count {
            Some(v) => format!(r#"<span class="rail-count">{v}</span>"#),
            None => String::new(),
        };
        let _ = write!(
            nav_html,
            r#"<button type="button" class="rail-row" data-action="set-view" data-view="{vid}"{cur}>{ico}<span class="rail-row-label">{lbl}</span>{cnt_html}</button>"#,
            lbl = esc(label),
        );
    }

    let mut ws_html = String::with_capacity(1200);
    for ws in workspaces {
        let slug = ws_state_slug(ws.state);
        let idx_str = ws.shortcut_index.to_string();
        let kbd = format_shortcut(&status.os, &idx_str);
        let _ = write!(
            ws_html,
            r#"<button type="button" class="rail-row" data-app="{app}" data-action="open-ws" data-ws="{app}" title="{full} ({kbd})"><span class="dot dot-{slug}" aria-hidden="true"></span><span class="rail-row-label">{short}</span><kbd class="kbd">{kbd}</kbd></button>"#,
            app = ws.id.as_str(),
            full = esc(&ws.name),
            short = esc(app_short_name(ws.id)),
            kbd = esc(&kbd),
        );
    }

    let pref_theme = status.prefs.theme.trim().to_ascii_lowercase();
    let t_dark = pref_theme == "dark";
    let t_light = pref_theme == "light";
    let t_sys = !t_dark && !t_light;

    format!(
        r#"<aside class="hub-rail" aria-label="BELL Hub Navigation"><div class="rail-brand"><span class="rail-brand-mark">{bell_ico}</span><span class="rail-brand-title">{brand}</span><span class="rail-online-pill"><span class="dot {online_dot}" aria-hidden="true"></span>{running}/{installed}</span></div><button type="button" class="rail-cmd-btn" data-action="open-palette">{search_ico}<span class="rail-cmd-label">{cmd_lbl}</span><kbd class="kbd">{cmd_k}</kbd></button><nav class="rail-nav-group" aria-label="Views">{nav_html}</nav><div class="rail-section-label">{ws_sec}</div><div class="rail-ws-section">{ws_html}</div><div class="rail-footer"><div class="rail-seg" role="group" aria-label="Theme"><button type="button" class="rail-seg-btn" data-action="set-theme" data-theme-pref="dark" aria-pressed="{p_dark}">{lbl_dark}</button><button type="button" class="rail-seg-btn" data-action="set-theme" data-theme-pref="light" aria-pressed="{p_light}">{lbl_light}</button><button type="button" class="rail-seg-btn" data-action="set-theme" data-theme-pref="system" aria-pressed="{p_sys}">{lbl_auto}</button></div><div class="rail-seg rail-seg-2" role="group" aria-label="Language"><button type="button" class="rail-seg-btn" data-action="set-lang" data-lang="zh-tw" aria-pressed="{p_zh}">繁中</button><button type="button" class="rail-seg-btn" data-action="set-lang" data-lang="en" aria-pressed="{p_en}">EN</button></div></div></aside>"#,
        bell_ico = icons::bell(),
        brand = esc(brand_title),
        search_ico = icons::search(),
        cmd_lbl = esc(c.cmd_trigger),
        cmd_k = esc(&cmd_k),
        ws_sec = esc(c.rail_workspaces),
        p_dark = if t_dark { "true" } else { "false" },
        p_light = if t_light { "true" } else { "false" },
        p_sys = if t_sys { "true" } else { "false" },
        lbl_dark = esc(c.theme_dark),
        lbl_light = esc(c.theme_light),
        lbl_auto = esc(c.theme_auto),
        p_zh = if lang == Lang::ZhTw { "true" } else { "false" },
        p_en = if lang == Lang::En { "true" } else { "false" },
    )
}

fn render_sheet_header(
    title: &str,
    sub: &str,
    busy: Option<&str>,
    actions_html: &str,
) -> String {
    let busy_html = match busy {
        Some(b) if !b.trim().is_empty() => format!(
            r#"<span class="busy-chip"><span class="dot dot-starting" aria-hidden="true"></span>{}</span>"#,
            esc(b)
        ),
        _ => String::new(),
    };
    format!(
        r#"<header class="sheet-header"><div class="sheet-title-group"><h1 class="sheet-title">{t}</h1><span class="sheet-sub">{s}</span>{busy_html}</div><div class="sheet-actions">{actions_html}</div></header>"#,
        t = esc(title),
        s = esc(sub),
    )
}

fn render_ws_inline_ops(ws: &WorkspaceStatus, c: &Copy) -> String {
    let app = ws.id.as_str();
    let disabled = if !ws.installed { " disabled" } else { "" };
    match ws.state {
        WorkspaceState::Running => format!(
            r#"<button type="button" class="btn btn-xs btn-ghost" data-action="ws-op" data-ws="{app}" data-op="restart"{disabled}>{restart}</button><button type="button" class="btn btn-xs btn-ghost" data-action="ws-op" data-ws="{app}" data-op="stop"{disabled}>{stop}</button><button type="button" class="btn btn-xs" data-action="open-ws" data-ws="{app}"{disabled}>{open}</button>"#,
            restart = esc(c.op_restart),
            stop = esc(c.op_stop),
            open = esc(c.open_enter),
        ),
        WorkspaceState::Starting => format!(
            r#"<button type="button" class="btn btn-xs btn-ghost" data-action="ws-op" data-ws="{app}" data-op="stop"{disabled}>{stop}</button><button type="button" class="btn btn-xs" data-action="open-ws" data-ws="{app}"{disabled}>{open}</button>"#,
            stop = esc(c.op_stop),
            open = esc(c.open_enter),
        ),
        WorkspaceState::Stopped | WorkspaceState::Error => format!(
            r#"<button type="button" class="btn btn-xs btn-ghost" data-action="ws-op" data-ws="{app}" data-op="start"{disabled}>{start}</button><button type="button" class="btn btn-xs" data-action="open-ws" data-ws="{app}"{disabled}>{open}</button>"#,
            start = esc(c.op_start),
            open = esc(c.open_enter),
        ),
    }
}

fn render_filter_tabs(active_filter: &str, c: &Copy) -> String {
    let tabs = [
        ("all", c.filter_all),
        ("due", c.filter_due),
        ("needs-you", c.filter_needs_you),
        ("waiting", c.filter_waiting),
        ("new", c.filter_new),
    ];
    let mut out = String::with_capacity(512);
    out.push_str(r#"<div class="filter-tabs" role="tablist">"#);
    for (fid, lbl) in tabs {
        let pressed = if active_filter == fid { "true" } else { "false" };
        let _ = write!(
            out,
            r#"<button type="button" class="filter-tab" data-action="set-filter" data-filter="{fid}" aria-pressed="{pressed}">{}</button>"#,
            esc(lbl)
        );
    }
    out.push_str("</div>");
    out
}

fn render_overview_view(
    status: &DesktopStatus,
    workspaces: &[WorkspaceStatus],
    all_today: &[&TodayItem],
    active_filter: &str,
    busy: Option<&str>,
    lang: Lang,
    c: &Copy,
) -> String {
    let running = workspaces
        .iter()
        .filter(|w| w.state == WorkspaceState::Running)
        .count();
    let title = format!("{} · {}", status.edition.display_name(), c.nav_overview);
    let sub = header_summary(running, all_today.len(), lang);
    let primary_cta = render_primary_cta_button(status, lang, c);
    let actions = format!(
        r#"<button type="button" class="btn" data-action="vault-backup">{ico_db}<span>{snap}</span></button>{primary_cta}"#,
        ico_db = icons::database(),
        snap = esc(c.snapshot_short),
    );
    let header = render_sheet_header(&title, &sub, busy, &actions);

    let primary_app = status.edition.primary_app();
    let mut ws_rows = String::with_capacity(3000);
    for ws in workspaces {
        let slug = ws_state_slug(ws.state);
        let ops = render_ws_inline_ops(ws, c);
        let primary_pill = if primary_app == Some(ws.id) {
            format!(
                r#"<span class="badge-subtle badge-ok">{}</span>"#,
                esc(c.primary_badge)
            )
        } else {
            String::new()
        };
        let disabled = if !ws.installed { " disabled" } else { "" };
        let _ = write!(
            ws_rows,
            r#"<div class="ws-row" data-app="{app}"><button type="button" class="ws-row-hit" data-action="open-ws" data-ws="{app}"{disabled}><span class="app-tile">{ico}</span><span class="ws-row-main"><span class="ws-row-name">{name}</span>{primary_pill}<span class="ws-row-summary">{summary}</span></span></button><div class="ws-row-metrics"><span class="metric-mono">:{port}</span><span class="metric-mono">{db}</span><span class="ws-status-tag"><span class="dot dot-{slug}" aria-hidden="true"></span><span>{st_lbl}</span></span></div><div class="ws-row-actions">{ops}</div></div>"#,
            app = ws.id.as_str(),
            ico = icons::app_icon(ws.id),
            name = esc(&ws.name),
            summary = esc(app_summary(ws.id, lang)),
            port = ws.port,
            db = esc(&format_bytes(ws.db_bytes)),
            st_lbl = esc(ws_state_label(ws.state, lang)),
            ops = ops,
        );
    }

    let filtered: Vec<&TodayItem> = all_today
        .iter()
        .copied()
        .filter(|it| active_filter == "all" || it.state == active_filter)
        .collect();

    let mut today_rows = String::with_capacity(2048);
    if filtered.is_empty() {
        let _ = write!(
            today_rows,
            r#"<div class="empty-state"><div class="empty-state-title">{t}</div><div class="empty-state-sub">{s}</div></div>"#,
            t = esc(c.today_empty_title),
            s = esc(c.today_empty_sub),
        );
    } else {
        for item in filtered.iter().take(6) {
            let pip = match item.state.as_str() {
                "due" => "state-pip-due",
                "needs-you" => "state-pip-needs-you",
                "new" => "state-pip-new",
                _ => "state-pip-waiting",
            };
            let clean_href = sanitize_hash_route(&item.href);
            let detail_txt = item.detail.as_deref().unwrap_or(clean_href);
            let key = item_key(item);
            let _ = write!(
                today_rows,
                r#"<div class="issue-row" data-app="{app}"><button type="button" class="issue-select-hit" data-action="select-item" data-item-key="{key}" data-switch-today="1"><span class="state-pip {pip}" aria-hidden="true"></span><span class="app-badge">{badge}</span><bdi class="issue-title">{title}</bdi><span class="issue-detail tabular">{detail}</span></button><button type="button" class="btn btn-xs" data-action="jump-today" data-ws="{app}" data-route="{href}">{open}</button></div>"#,
                app = item.source_app.as_str(),
                key = esc(&key),
                badge = esc(app_badge_name(item.source_app)),
                title = esc(&item.title),
                detail = esc(detail_txt),
                href = esc(clean_href),
                open = esc(c.open_arrow),
            );
        }
    }

    let used = status.vault.total_bytes;
    let quota = status.vault.quota_bytes.max(1);
    let pct =
        ((used as f64 / quota as f64) * 100.0).clamp(if used > 0 { 1.4 } else { 0.0 }, 100.0);
    let quota_pair = format_quota_pair(used, quota);
    let backup_meta = format_backup_meta(
        status.vault.backups_count,
        status.vault.last_backup_at.as_deref(),
        lang,
    );

    let mut db_rows = String::with_capacity(1400);
    if status.vault.databases.is_empty() {
        let _ = write!(
            db_rows,
            r#"<div class="empty-state"><div class="empty-state-sub">{}</div></div>"#,
            esc(c.vault_empty)
        );
    } else {
        for db in &status.vault.databases {
            let file_name = db
                .path
                .rsplit(['/', '\\'])
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or("store.db");
            let mark = if db.healthy {
                r#"<span class="health-mark-ok">✓</span>"#
            } else {
                r#"<span class="health-mark-err">✕</span>"#
            };
            let _ = write!(
                db_rows,
                r#"<div class="db-mini-row" data-app="{app}"><span class="app-badge">{badge}</span><span class="db-mini-file" title="{full}">{fname}</span><span class="db-mini-size">{sz}</span>{mark}</div>"#,
                app = db.app_id.as_str(),
                badge = esc(app_badge_name(db.app_id)),
                full = esc(&db.path),
                fname = esc(file_name),
                sz = esc(&format_bytes(db.bytes)),
            );
        }
    }

    let rt = &status.runtime;
    let rt_lbl = if rt.kind == "bundled" {
        c.runtime_bundled
    } else {
        c.runtime_system
    };
    let clis = [
        ("claude", rt.claude_cli),
        ("codex", rt.codex_cli),
        ("gh", rt.gh_cli),
        ("git", rt.git_cli),
    ];
    let mut cli_html = String::with_capacity(400);
    for (name, ok) in clis {
        let cls = if ok { "cli-tag cli-tag-ok" } else { "cli-tag" };
        let sym = if ok { "✓" } else { "·" };
        let _ = write!(cli_html, r#"<span class="{cls}">{sym} {name}</span>"#);
    }

    format!(
        r#"{header}<div class="sheet-body"><section><div class="sec-bar"><h2 class="sec-heading">{sec_ws}</h2><button type="button" class="sec-link" data-action="set-view" data-view="workspaces">{nav_ws} →</button></div><div class="linear-group">{ws_rows}</div></section><section class="overview-split"><div class="split-card"><div class="split-card-head"><h2 class="sec-heading">{sec_today}</h2>{filters}<button type="button" class="sec-link" data-action="set-view" data-view="today">{inbox_lnk}</button></div><div>{today_rows}</div></div><div class="split-card"><div class="split-card-head"><h2 class="sec-heading">{sec_vault}</h2><button type="button" class="sec-link" data-action="set-view" data-view="vault">{nav_vault} →</button></div><div class="vault-mini-body"><div class="quota-head"><span class="quota-nums">{quota_pair}</span><span class="quota-meta">{backup_meta}</span></div><svg class="quota-svg" viewBox="0 0 100 5" preserveAspectRatio="none" aria-hidden="true"><rect class="quota-track" x="0" y="0" width="100" height="5" rx="2.5"/><rect class="quota-fill" x="0" y="0" width="{pct:.2}" height="5" rx="2.5"/></svg><div class="db-mini-list">{db_rows}</div><div class="runtime-mini-bar"><span class="badge-subtle badge-ok">{rt_lbl}</span><div class="cli-row">{cli_html}</div></div></div></div></section></div>"#,
        sec_ws = esc(c.sec_workspaces),
        nav_ws = esc(c.nav_workspaces),
        sec_today = esc(c.sec_today),
        filters = render_filter_tabs(active_filter, c),
        inbox_lnk = esc(c.view_inbox_link),
        sec_vault = esc(c.sec_vault_runtime),
        nav_vault = esc(c.nav_vault),
        quota_pair = esc(&quota_pair),
        backup_meta = esc(&backup_meta),
        rt_lbl = esc(rt_lbl),
    )
}

fn render_today_view(
    model: &HubRenderModel<'_>,
    workspaces: &[WorkspaceStatus],
    all_today: &[&TodayItem],
    active_filter: &str,
    lang: Lang,
    c: &Copy,
) -> String {
    let today = model.today;
    let selected_key = model.ui.selected_item_key.as_deref();
    let busy = model.ui.busy_label.as_deref();
    let running = workspaces
        .iter()
        .filter(|w| w.state == WorkspaceState::Running)
        .count();
    let sub = header_summary(running, all_today.len(), lang);
    let actions = render_primary_cta_button(model.status, lang, c);
    let header = render_sheet_header(c.nav_today, &sub, busy, &actions);

    let mut src_pills = String::with_capacity(800);
    if let Some(ov) = today {
        for src in &ov.sources {
            let ws = workspaces.iter().find(|w| w.id == src.app_id);
            let dot_slug = ws
                .map(|w| ws_state_slug(w.state))
                .unwrap_or(if src.status == "ready" {
                    "running"
                } else {
                    "stopped"
                });
            let count_txt = if src.status == "unsupported"
                && ws.is_some_and(|w| w.state == WorkspaceState::Running)
                && src.items.is_empty()
            {
                c.today_aggregator.to_string()
            } else {
                src.items.len().to_string()
            };
            let _ = write!(
                src_pills,
                r#"<span class="src-pill" data-app="{app}"><span class="dot dot-{dot_slug}" aria-hidden="true"></span><span>{name}</span><span class="mono">{cnt}</span></span>"#,
                app = src.app_id.as_str(),
                name = esc(app_badge_name(src.app_id)),
                cnt = esc(&count_txt),
            );
        }
    }

    let filtered: Vec<&TodayItem> = all_today
        .iter()
        .copied()
        .filter(|it| active_filter == "all" || it.state == active_filter)
        .collect();

    let selected_item: Option<&TodayItem> = selected_key
        .and_then(|k| filtered.iter().copied().find(|it| item_key(it) == k))
        .or_else(|| filtered.first().copied());
    let active_key = selected_item.map(item_key);

    let mut list_html = String::with_capacity(2400);
    if filtered.is_empty() {
        let _ = write!(
            list_html,
            r#"<div class="empty-state"><div class="empty-state-title">{t}</div><div class="empty-state-sub">{s}</div></div>"#,
            t = esc(c.today_empty_title),
            s = esc(c.today_empty_sub),
        );
    } else {
        for item in &filtered {
            let k = item_key(item);
            let is_sel = active_key.as_deref() == Some(k.as_str());
            let pip = match item.state.as_str() {
                "due" => "state-pip-due",
                "needs-you" => "state-pip-needs-you",
                "new" => "state-pip-new",
                _ => "state-pip-waiting",
            };
            let clean_href = sanitize_hash_route(&item.href);
            let detail_txt = item.detail.as_deref().unwrap_or(clean_href);
            let _ = write!(
                list_html,
                r#"<div class="issue-row" data-app="{app}" aria-selected="{sel}"><button type="button" class="issue-select-hit" data-action="select-item" data-item-key="{k}"><span class="state-pip {pip}" aria-hidden="true"></span><span class="app-badge">{badge}</span><bdi class="issue-title">{title}</bdi><span class="issue-detail tabular">{detail}</span></button><button type="button" class="btn btn-xs" data-action="jump-today" data-ws="{app}" data-route="{href}">{open}</button></div>"#,
                app = item.source_app.as_str(),
                sel = if is_sel { "true" } else { "false" },
                k = esc(&k),
                badge = esc(app_badge_name(item.source_app)),
                title = esc(&item.title),
                detail = esc(detail_txt),
                href = esc(clean_href),
                open = esc(c.open_arrow),
            );
        }
    }

    let inspector_html = match selected_item {
        None => format!(
            r#"<aside class="inbox-inspector"><div class="empty-state"><div class="empty-state-sub">{}</div></div></aside>"#,
            esc(c.inspector_empty)
        ),
        Some(item) => {
            let clean_href = sanitize_hash_route(&item.href);
            let cta = open_in_app_cta(item.source_app, lang);
            let detail_block = match &item.detail {
                Some(d) if !d.is_empty() => {
                    format!(r#"<div class="inspector-detail-box">{}</div>"#, esc(d))
                }
                _ => String::new(),
            };
            format!(
                r#"<aside class="inbox-inspector" data-app="{app}"><div class="inspector-head"><span class="app-tile">{ico}</span><span class="app-badge">{app_name}</span></div><h2 class="inspector-title"><bdi>{title}</bdi></h2>{detail_block}<div class="inspector-meta-list"><div class="inspector-meta-row"><span class="inspector-meta-key">{k_state}</span><span class="inspector-meta-val">{v_state}</span></div><div class="inspector-meta-row"><span class="inspector-meta-key">{k_type}</span><span class="inspector-meta-val mono">{v_type}</span></div><div class="inspector-meta-row"><span class="inspector-meta-key">{k_route}</span><code class="inspector-meta-val">{href}</code></div><div class="inspector-meta-row"><span class="inspector-meta-key">{k_upd}</span><span class="inspector-meta-val tabular">{v_upd}</span></div></div><button type="button" class="btn btn-primary" data-action="jump-today" data-ws="{app}" data-route="{href}">{cta}</button></aside>"#,
                app = item.source_app.as_str(),
                ico = icons::app_icon(item.source_app),
                app_name = esc(item.source_app.display_name()),
                title = esc(&item.title),
                k_state = esc(c.inspector_state),
                v_state = esc(today_state_label(&item.state, lang)),
                k_type = esc(c.inspector_type),
                v_type = esc(&item.item_type),
                k_route = esc(c.inspector_route),
                href = esc(clean_href),
                k_upd = esc(c.inspector_updated),
                v_upd = esc(&short_iso(&item.updated_at)),
                cta = esc(&cta),
            )
        }
    };

    format!(
        r#"{header}<div class="sheet-body sheet-body-flush"><div class="inbox-subbar">{filters}<div class="src-pills">{src_pills}</div></div><div class="inbox-layout"><div class="inbox-list-col">{list_html}</div>{inspector_html}</div></div>"#,
        filters = render_filter_tabs(active_filter, c),
    )
}

fn render_workspaces_view(
    status: &DesktopStatus,
    workspaces: &[WorkspaceStatus],
    busy: Option<&str>,
    lang: Lang,
    c: &Copy,
) -> String {
    let running = workspaces
        .iter()
        .filter(|w| w.state == WorkspaceState::Running)
        .count();
    let installed = workspaces.iter().filter(|w| w.installed).count();
    let sub = format!("{running}/{installed} · 127.0.0.1 Loopback-Only");
    let actions = render_primary_cta_button(status, lang, c);
    let header = render_sheet_header(c.nav_workspaces, &sub, busy, &actions);
    let primary_app = status.edition.primary_app();

    let mut rows = String::with_capacity(4096);
    for ws in workspaces {
        let slug = ws_state_slug(ws.state);
        let app = ws.id.as_str();
        let pid_str = ws
            .pid
            .map(|p| format!("PID {p}"))
            .unwrap_or_else(|| "PID —".to_string());
        let manifest_cls = if ws.manifest_valid {
            "badge-subtle badge-ok"
        } else {
            "badge-subtle"
        };
        let manifest_txt = if ws.manifest_valid {
            c.manifest_ok
        } else {
            c.manifest_idle
        };
        let primary_pill = if primary_app == Some(ws.id) {
            format!(
                r#"<span class="badge-subtle badge-ok">{}</span>"#,
                esc(c.primary_badge)
            )
        } else {
            String::new()
        };
        let kbd = format_shortcut(&status.os, &ws.shortcut_index.to_string());
        let disabled = if !ws.installed { " disabled" } else { "" };

        let mut route_pills = String::with_capacity(512);
        for &(route, r_lbl) in deep_routes(ws.id, lang) {
            let _ = write!(
                route_pills,
                r#"<button type="button" class="route-pill" data-action="jump-today" data-ws="{app}" data-route="{r}"><span>{lbl}</span><code>{r}</code></button>"#,
                r = esc(route),
                lbl = esc(r_lbl),
            );
        }

        let err_html = match &ws.last_error {
            Some(e) if !e.is_empty() => format!(r#"<div class="ws-err-banner">{}</div>"#, esc(e)),
            _ => String::new(),
        };

        let ops = render_ws_inline_ops(ws, c);
        let _ = write!(
            rows,
            r#"<div class="ws-detail-row" data-app="{app}"><div class="ws-detail-top"><button type="button" class="ws-row-hit" data-action="open-ws" data-ws="{app}"{disabled}><span class="app-tile">{ico}</span><span class="ws-row-name">{name}</span>{primary_pill}<kbd class="kbd">{kbd}</kbd><span class="{manifest_cls}">{manifest_txt}</span><span class="ws-row-summary">{summary}</span></button><div class="ws-row-metrics"><span class="metric-mono">:{port}</span><span class="metric-mono">{pid}</span><span class="metric-mono">{db}</span><span class="ws-status-tag"><span class="dot dot-{slug}" aria-hidden="true"></span><span>{st_lbl}</span></span></div><div class="ws-row-actions"><button type="button" class="btn btn-xs btn-ghost" data-action="select-logs-app" data-ws="{app}" data-switch-logs="1">{logs_lbl}</button>{ops}</div></div><div class="ws-detail-routes">{route_pills}</div>{err_html}</div>"#,
            ico = icons::app_icon(ws.id),
            name = esc(&ws.name),
            kbd = esc(&kbd),
            summary = esc(app_summary(ws.id, lang)),
            port = ws.port,
            pid = esc(&pid_str),
            db = esc(&format_bytes(ws.db_bytes)),
            st_lbl = esc(ws_state_label(ws.state, lang)),
            logs_lbl = esc(c.op_logs),
        );
    }

    format!(r#"{header}<div class="sheet-body"><div class="linear-group">{rows}</div></div>"#)
}

fn render_vault_view(status: &DesktopStatus, busy: Option<&str>, lang: Lang, c: &Copy) -> String {
    let vault = &status.vault;
    let quota_pair = format_quota_pair(vault.total_bytes, vault.quota_bytes);
    let backup_meta = format_backup_meta(
        vault.backups_count,
        vault.last_backup_at.as_deref(),
        lang,
    );
    let sub = format!("{quota_pair} · {backup_meta}");
    let actions = format!(
        r#"<button type="button" class="btn btn-primary" data-action="vault-backup">{ico}<span>{lbl}</span></button>"#,
        ico = icons::database(),
        lbl = esc(c.snapshot_full),
    );
    let header = render_sheet_header(c.nav_vault, &sub, busy, &actions);

    let used = vault.total_bytes;
    let quota = vault.quota_bytes.max(1);
    let pct =
        ((used as f64 / quota as f64) * 100.0).clamp(if used > 0 { 1.4 } else { 0.0 }, 100.0);

    let table_html = if vault.databases.is_empty() {
        format!(
            r#"<div class="linear-group"><div class="empty-state"><div class="empty-state-sub">{}</div></div></div>"#,
            esc(c.vault_empty)
        )
    } else {
        let mut trs = String::with_capacity(2048);
        for db in &vault.databases {
            let fname = db
                .path
                .rsplit(['/', '\\'])
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or("store.db");
            let is_json = fname.to_ascii_lowercase().ends_with(".json");
            let count_str = format_table_or_entry_count(db.tables, is_json, lang);
            let health_txt = match (is_json, db.healthy) {
                (true, true) => c.json_ok,
                (true, false) => c.json_fail,
                (false, true) => c.wal_ok,
                (false, false) => c.wal_fail,
            };
            let health_cls = if db.healthy {
                "badge-subtle badge-ok"
            } else {
                "badge-subtle"
            };
            let _ = write!(
                trs,
                r#"<tr data-app="{app}"><td><span class="app-badge">{badge}</span></td><td><strong class="mono">{fname}</strong><span class="db-path-sub" title="{full}">{full}</span></td><td class="tabular">{cnt}</td><td class="mono tabular">{sz}</td><td><span class="{health_cls}">{htxt}</span></td></tr>"#,
                app = db.app_id.as_str(),
                badge = esc(app_badge_name(db.app_id)),
                fname = esc(fname),
                full = esc(&db.path),
                cnt = esc(&count_str),
                sz = esc(&format_bytes(db.bytes)),
                htxt = esc(health_txt),
            );
        }
        format!(
            r#"<div class="linear-group"><table class="vault-table"><thead><tr><th>{c_app}</th><th>{c_path}</th><th>{c_cnt}</th><th>{c_sz}</th><th>{c_hlth}</th></tr></thead><tbody>{trs}</tbody></table></div>"#,
            c_app = esc(c.col_app),
            c_path = esc(c.col_path),
            c_cnt = esc(c.col_count),
            c_sz = esc(c.col_size),
            c_hlth = esc(c.col_health),
        )
    };

    format!(
        r#"{header}<div class="sheet-body"><div class="vault-summary-grid"><div class="vault-box"><span class="sec-heading">{lbl_quota}</span><div class="quota-head"><span class="quota-nums">{quota_pair}</span><span class="quota-meta">{backup_meta}</span></div><svg class="quota-svg" viewBox="0 0 100 5" preserveAspectRatio="none" aria-hidden="true"><rect class="quota-track" x="0" y="0" width="100" height="5" rx="2.5"/><rect class="quota-fill" x="0" y="0" width="{pct:.2}" height="5" rx="2.5"/></svg></div><div class="vault-box"><span class="sec-heading">{lbl_root}</span><div class="vault-root-line"><span class="vault-root-path" title="{root}">{root}</span><button type="button" class="btn btn-xs" data-action="copy-text" data-copy="{root}">{ico_copy}<span>{lbl_copy}</span></button></div></div></div>{table_html}</div>"#,
        lbl_quota = esc(c.vault_storage),
        quota_pair = esc(&quota_pair),
        backup_meta = esc(&backup_meta),
        lbl_root = esc(c.vault_root),
        root = esc(&vault.root_dir),
        ico_copy = icons::copy(),
        lbl_copy = esc(c.copy_path),
    )
}

fn render_logs_view(
    status: &DesktopStatus,
    workspaces: &[WorkspaceStatus],
    logs: Option<&WorkspaceLogs>,
    requested_app: Option<AppId>,
    busy: Option<&str>,
    c: &Copy,
) -> String {
    let active_app = requested_app
        .or_else(|| logs.map(|l| l.id))
        .unwrap_or_else(|| status.edition.primary_app().unwrap_or(AppId::Personal));
    let lines: &[String] = logs
        .filter(|l| l.id == active_app)
        .map(|l| l.lines.as_slice())
        .unwrap_or(&[]);
    let joined_for_copy = lines.join("\n");

    let sub = format!("{} · {} lines", active_app.display_name(), lines.len());
    let actions = format!(
        r#"<button type="button" class="btn" data-action="refresh-logs">{ico_ref}<span>{lbl_ref}</span></button><button type="button" class="btn" data-action="copy-text" data-copy="{copy_payload}">{ico_cp}<span>{lbl_cp}</span></button>"#,
        ico_ref = icons::refresh(),
        lbl_ref = esc(c.refresh),
        copy_payload = esc(&joined_for_copy),
        ico_cp = icons::copy(),
        lbl_cp = esc(c.copy_logs),
    );
    let header = render_sheet_header(c.nav_logs, &sub, busy, &actions);

    let mut tabs_html = String::with_capacity(800);
    tabs_html.push_str(r#"<div class="logs-tabs" role="tablist">"#);
    for ws in workspaces {
        let pressed = if ws.id == active_app { "true" } else { "false" };
        let slug = ws_state_slug(ws.state);
        let _ = write!(
            tabs_html,
            r#"<button type="button" class="filter-tab" data-app="{app}" data-action="select-logs-app" data-ws="{app}" aria-pressed="{pressed}"><span class="dot dot-{slug}" aria-hidden="true"></span><span>{lbl}</span></button>"#,
            app = ws.id.as_str(),
            lbl = esc(app_short_name(ws.id)),
        );
    }
    tabs_html.push_str("</div>");

    let rt = &status.runtime;
    let rt_lbl = if rt.kind == "bundled" {
        c.runtime_bundled
    } else {
        c.runtime_system
    };
    let clis = [
        ("claude", rt.claude_cli),
        ("codex", rt.codex_cli),
        ("gh", rt.gh_cli),
        ("git", rt.git_cli),
    ];
    let mut cli_html = String::with_capacity(400);
    for (name, ok) in clis {
        let cls = if ok { "cli-tag cli-tag-ok" } else { "cli-tag" };
        let sym = if ok { "✓" } else { "·" };
        let _ = write!(cli_html, r#"<span class="{cls}">{sym} {name}</span>"#);
    }

    let mut log_body = String::with_capacity(2048);
    if lines.is_empty() {
        let _ = write!(
            log_body,
            r#"<div class="empty-state"><div class="empty-state-sub">{}</div></div>"#,
            esc(c.logs_empty)
        );
    } else {
        for (idx, ln) in lines.iter().enumerate() {
            let _ = write!(
                log_body,
                r#"<div class="log-line"><span class="log-ln">{n}</span><span>{txt}</span></div>"#,
                n = idx + 1,
                txt = esc(ln),
            );
        }
    }

    format!(
        r#"{header}<div class="sheet-body"><div class="split-card-head"><div>{tabs_html}</div><div class="cli-row"><span class="badge-subtle badge-ok" title="{rt_path}">{rt_lbl}</span>{cli_html}</div></div><div class="terminal-pane">{log_body}</div></div>"#,
        rt_path = esc(&rt.path),
        rt_lbl = esc(rt_lbl),
    )
}

pub fn render_hub_patch(model: &HubRenderModel<'_>) -> HubViewPatch {
    let resolved_theme = resolve_theme(
        &model.status.prefs.theme,
        model.ui.theme_polarity_hint.as_deref(),
    );
    let lang = Lang::from_pref(&model.status.prefs.lang);
    let c = copy_for(lang);
    let active_view = normalize_view(&model.ui.view);
    let active_filter = normalize_filter(&model.ui.today_filter);
    let workspaces = ordered_workspaces(model.status);
    let all_today = flatten_today_items(model.today);
    let (total_today, today_live_attr) = match model.today {
        Some(t) => (
            t.total_items.max(all_today.len()),
            r#" data-today-live="1""#,
        ),
        None => (model.ui.cached_today_count.unwrap_or(0), ""),
    };
    let busy = model.ui.busy_label.as_deref();

    let rail_html = render_rail(
        model.status,
        &workspaces,
        total_today,
        active_view,
        lang,
        c,
    );

    let sheet_inner = match active_view {
        "today" => render_today_view(model, &workspaces, &all_today, active_filter, lang, c),
        "workspaces" => render_workspaces_view(model.status, &workspaces, busy, lang, c),
        "vault" => render_vault_view(model.status, busy, lang, c),
        "logs" => render_logs_view(
            model.status,
            &workspaces,
            model.logs,
            model.ui.logs_app,
            busy,
            c,
        ),
        _ => render_overview_view(
            model.status,
            &workspaces,
            &all_today,
            active_filter,
            busy,
            lang,
            c,
        ),
    };

    let palette_html = if model.ui.palette_open {
        palette::render_palette_modal(
            model.status,
            model.today,
            &model.ui.palette_query,
            model.ui.palette_index,
            lang,
        )
    } else {
        String::new()
    };

    let toast_html = match &model.ui.toast {
        Some(t) => {
            let err_cls = if t.kind == "error" {
                " hub-toast-error"
            } else {
                ""
            };
            format!(
                r#"<div class="hub-toast{err_cls}" role="status"><div><div class="hub-toast-title">{title}</div><div class="hub-toast-detail">{detail}</div></div><button type="button" class="btn btn-xs btn-ghost" data-action="dismiss-toast" aria-label="Close">✕</button></div>"#,
                title = esc(&t.title),
                detail = esc(&t.detail),
            )
        }
        None => String::new(),
    };

    let view_title = match active_view {
        "today" => c.nav_today,
        "workspaces" => c.nav_workspaces,
        "vault" => c.nav_vault,
        "logs" => c.nav_logs,
        _ => c.nav_overview,
    };
    let title = format!("{} · {}", model.status.edition.display_name(), view_title);

    let html = format!(
        r#"<div class="hub-shell" data-today-count="{total_today}"{today_live_attr}>{rail_html}<main class="hub-sheet">{sheet_inner}</main></div>{palette_html}{toast_html}"#
    );

    HubViewPatch {
        html,
        resolved_theme: resolved_theme.to_string(),
        lang: lang.html_lang().to_string(),
        title,
        navigate_url: None,
    }
}

pub fn render_initial_html() -> String {
    let (demo_status, demo_today, demo_logs) = build_demo_status(Edition::Suite, "dark", "zh-tw");
    let default_ui = HubUiClientState::default();
    let initial_patch = render_hub_patch(&HubRenderModel {
        status: &demo_status,
        today: Some(&demo_today),
        logs: Some(&demo_logs),
        ui: &default_ui,
    });

    format!(
        r#"<!doctype html>
<html lang="zh-Hant" data-theme="dark">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <meta name="color-scheme" content="dark light" />
    <link rel="icon" type="image/svg+xml" href="./favicon.svg" />
    <link rel="alternate icon" href="./favicon.ico" />
    <link rel="stylesheet" href="./hub.css" />
    <title>{title}</title>
  </head>
  <body>
    <div id="hub-root">{body}</div>
    <script defer src="./hub.js"></script>
  </body>
</html>
"#,
        title = esc(&initial_patch.title),
        body = initial_patch.html,
    )
}

pub fn build_demo_status(
    edition: Edition,
    theme: &str,
    lang: &str,
) -> (DesktopStatus, TodayOverview, WorkspaceLogs) {
    struct DemoSpec {
        id: AppId,
        state: WorkspaceState,
        pid: Option<u32>,
        db_bytes: u64,
        rel_path: &'static str,
        tables: usize,
    }

    let vault_root = "/Users/bell/Library/Application Support/com.bell1dev.suite/vault";
    let specs = [
        DemoSpec {
            id: AppId::Personal,
            state: WorkspaceState::Running,
            pid: Some(28410),
            db_bytes: 425_984,
            rel_path: "personal/settings.sqlite",
            tables: 9,
        },
        DemoSpec {
            id: AppId::Explain,
            state: WorkspaceState::Running,
            pid: Some(28427),
            db_bytes: 1_887_436,
            rel_path: "explain/settings.sqlite",
            tables: 12,
        },
        DemoSpec {
            id: AppId::Pr,
            state: WorkspaceState::Running,
            pid: Some(28444),
            db_bytes: 950_272,
            rel_path: "pr/state.json",
            tables: 14,
        },
        DemoSpec {
            id: AppId::Reader,
            state: WorkspaceState::Running,
            pid: Some(28461),
            db_bytes: 688_128,
            rel_path: "reader/reader.db",
            tables: 7,
        },
        DemoSpec {
            id: AppId::Campaign,
            state: WorkspaceState::Stopped,
            pid: None,
            db_bytes: 311_296,
            rel_path: "campaign/campaign.db",
            tables: 11,
        },
    ];

    let workspaces: Vec<WorkspaceStatus> = specs
        .iter()
        .map(|s| WorkspaceStatus {
            id: s.id,
            name: s.id.display_name().to_string(),
            shortcut_index: s.id.shortcut_index(),
            accent_light: s.id.accent_light().to_string(),
            accent_dark: s.id.accent_dark().to_string(),
            port: s.id.default_port(),
            url: format!("http://127.0.0.1:{}", s.id.default_port()),
            state: s.state,
            pid: s.pid,
            manifest_valid: s.state == WorkspaceState::Running,
            installed: true,
            db_bytes: s.db_bytes,
            restart_count: 0,
            last_error: None,
        })
        .collect();

    let databases: Vec<VaultDatabase> = specs
        .iter()
        .map(|s| VaultDatabase {
            app_id: s.id,
            path: format!("{vault_root}/{}", s.rel_path),
            bytes: s.db_bytes,
            healthy: true,
            tables: s.tables,
        })
        .collect();

    let total_bytes: u64 = databases.iter().map(|d| d.bytes).sum();

    let status = DesktopStatus {
        edition,
        version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        active_target: "hub".to_string(),
        runtime: RuntimeInfo {
            kind: "bundled".to_string(),
            path: "/Applications/BELL Suite.app/Contents/Resources/dist-sidecars/bin/bell-runtime"
                .to_string(),
            claude_cli: true,
            codex_cli: true,
            gh_cli: true,
            git_cli: true,
        },
        prefs: DesktopPrefs {
            theme: if theme.trim().is_empty() {
                "dark".to_string()
            } else {
                theme.to_string()
            },
            lang: if lang.trim().is_empty() {
                "zh-tw".to_string()
            } else {
                lang.to_string()
            },
            active_target: "hub".to_string(),
            auto_start: true,
        },
        workspaces,
        vault: VaultStatus {
            root_dir: vault_root.to_string(),
            total_bytes,
            quota_bytes: 5 * 1024 * 1024 * 1024,
            backups_count: 3,
            last_backup_at: Some("2026-11-16T19:45:00Z".to_string()),
            databases,
        },
    };

    let is_en = Lang::from_pref(lang) == Lang::En;
    let today = TodayOverview {
        generated_at: "2026-11-16T20:15:00Z".to_string(),
        total_items: 4,
        sources: vec![
            TodaySource {
                app_id: AppId::Personal,
                status: "unsupported".to_string(),
                counts: BTreeMap::new(),
                items: Vec::new(),
            },
            TodaySource {
                app_id: AppId::Explain,
                status: "ready".to_string(),
                counts: BTreeMap::from([("dueReviews".to_string(), 2)]),
                items: vec![TodayItem {
                    source_app: AppId::Explain,
                    item_type: "review".to_string(),
                    id: "note-tauri-v2".to_string(),
                    title: if is_en {
                        "Tauri v2 Single-Webview Navigation & SuiteSwitcher Handoff"
                    } else {
                        "Tauri v2 單一視窗導覽與 SuiteSwitcher 跨應用交接"
                    }
                    .to_string(),
                    href: "#/note/note-tauri-v2".to_string(),
                    state: "due".to_string(),
                    updated_at: "2026-11-16T19:50:00Z".to_string(),
                    due_at: Some("2026-11-16T23:59:00Z".to_string()),
                    detail: Some(
                        if is_en {
                            "FSRS review due today"
                        } else {
                            "FSRS 間隔複習今日到期"
                        }
                        .to_string(),
                    ),
                }],
            },
            TodaySource {
                app_id: AppId::Pr,
                status: "ready".to_string(),
                counts: BTreeMap::from([("needsReview".to_string(), 1)]),
                items: vec![TodayItem {
                    source_app: AppId::Pr,
                    item_type: "run".to_string(),
                    id: "pr-148".to_string(),
                    title: if is_en {
                        "bell-hub: Pure Rust Linear workspace orchestrator & command palette"
                    } else {
                        "bell-hub：純 Rust Linear 工作區協調器與快捷指令面板"
                    }
                    .to_string(),
                    href: "#/discover".to_string(),
                    state: "needs-you".to_string(),
                    updated_at: "2026-11-16T20:10:00Z".to_string(),
                    due_at: None,
                    detail: Some(
                        if is_en {
                            "PR #148 · Ready for sign-off"
                        } else {
                            "PR #148 · 等待最終簽核"
                        }
                        .to_string(),
                    ),
                }],
            },
            TodaySource {
                app_id: AppId::Reader,
                status: "ready".to_string(),
                counts: BTreeMap::from([("unread".to_string(), 3)]),
                items: vec![TodayItem {
                    source_app: AppId::Reader,
                    item_type: "page".to_string(),
                    id: "sqlite-wal".to_string(),
                    title: if is_en {
                        "SQLite Online Backup API & WAL Checkpoint Internals"
                    } else {
                        "SQLite 線上原子熱備份與 WAL Checkpoint 內部機制"
                    }
                    .to_string(),
                    href: "#/page/sqlite-wal".to_string(),
                    state: "new".to_string(),
                    updated_at: "2026-11-16T18:22:00Z".to_string(),
                    due_at: None,
                    detail: Some(
                        if is_en {
                            "4 ink highlights"
                        } else {
                            "4 處螢光筆標註"
                        }
                        .to_string(),
                    ),
                }],
            },
            TodaySource {
                app_id: AppId::Campaign,
                status: "ready".to_string(),
                counts: BTreeMap::from([("progressPercent".to_string(), 72)]),
                items: vec![TodayItem {
                    source_app: AppId::Campaign,
                    item_type: "task".to_string(),
                    id: "release-v103".to_string(),
                    title: if is_en {
                        "Verify BELL Desktop v1.0.3 macOS DMG & Windows NSIS release checklist"
                    } else {
                        "確認 BELL Desktop v1.0.3 macOS 與 Windows 封裝檢核表"
                    }
                    .to_string(),
                    href: "#/tasks".to_string(),
                    state: "due".to_string(),
                    updated_at: "2026-11-16T20:05:00Z".to_string(),
                    due_at: None,
                    detail: Some(
                        if is_en {
                            "Due today 18:00"
                        } else {
                            "今日 18:00 截止"
                        }
                        .to_string(),
                    ),
                }],
            },
        ],
    };

    let log_app = edition.primary_app().unwrap_or(AppId::Personal);
    let logs = WorkspaceLogs {
        id: log_app,
        lines: vec![
            format!(
                "[2026-11-16T20:00:01Z] [bell-core] spawning sidecar '{}' via bundled bell-runtime",
                log_app.as_str()
            ),
            format!(
                "[2026-11-16T20:00:01Z] [{}] listening on http://127.0.0.1:{}",
                log_app.as_str(),
                log_app.default_port()
            ),
            format!(
                "[2026-11-16T20:00:02Z] [{}] SQLite WAL journal initialized (PRAGMA integrity_check = ok)",
                log_app.as_str()
            ),
            format!(
                "[2026-11-16T20:00:02Z] [{}] GET /api/suite/manifest -> 200 OK (schemaVersion: 1)",
                log_app.as_str()
            ),
            format!(
                "[2026-11-16T20:15:00Z] [{}] GET /api/suite/today -> 200 OK",
                log_app.as_str()
            ),
        ],
    };

    (status, today, logs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bell_core::allowed_route;

    #[test]
    fn zero_top_chrome_bar_and_all_views_render_cleanly() {
        let (st, today, logs) = build_demo_status(Edition::Suite, "dark", "zh-tw");
        assert!(!HUB_CSS.contains("chrome-bar"));
        assert!(!HUB_BOOT_JS.contains("Math.max(44"));
        assert!(!HUB_BOOT_JS.contains("|| 600"));

        for view in ["overview", "today", "workspaces", "vault", "logs"] {
            let ui = HubUiClientState {
                view: view.to_string(),
                palette_open: view == "overview",
                palette_query: if view == "overview" {
                    "explain".to_string()
                } else {
                    String::new()
                },
                ..Default::default()
            };
            let patch = render_hub_patch(&HubRenderModel {
                status: &st,
                today: Some(&today),
                logs: Some(&logs),
                ui: &ui,
            });
            assert!(!patch.html.contains("chrome-bar"));
            assert!(!patch.html.contains(" style="));
            assert!(!patch.html.contains("view:overview"));
            assert!(patch.html.contains("hub-rail"));
            assert!(patch.html.contains("hub-sheet"));
            assert_eq!(patch.resolved_theme, "dark");
            assert_eq!(patch.lang, "zh-Hant");
        }
    }

    #[test]
    fn cached_today_count_persists_across_non_today_views() {
        let (st, _, logs) = build_demo_status(Edition::Suite, "dark", "en");
        for view in ["workspaces", "vault", "logs"] {
            let ui = HubUiClientState {
                view: view.to_string(),
                cached_today_count: Some(6),
                ..Default::default()
            };
            let patch = render_hub_patch(&HubRenderModel {
                status: &st,
                today: None,
                logs: Some(&logs),
                ui: &ui,
            });
            assert!(
                patch
                    .html
                    .contains(r#"data-view="today"><svg class="ico""#)
                    && patch
                        .html
                        .contains(r#"<span class="rail-row-label">Today</span><span class="rail-count">6</span>"#),
                "Left rail Today badge must retain cached count 6 on view {view}"
            );
            assert!(patch.html.contains(r#"data-today-count="6""#));
            assert!(!patch.html.contains(r#"data-today-live="1""#));
        }
    }

    #[test]
    fn standalone_primary_ordering_and_platform_shortcuts_and_brand_tokens() {
        let (mut st, today, logs) = build_demo_status(Edition::Explain, "system", "en");
        st.os = "macos".to_string();
        let ui = HubUiClientState {
            view: "overview".to_string(),
            palette_open: true,
            theme_polarity_hint: Some("light".to_string()),
            toast: Some(HubToast {
                kind: "ok".to_string(),
                title: "Snapshot <Ready>".to_string(),
                detail: "/tmp/vault & backup".to_string(),
            }),
            ..Default::default()
        };
        let mac_patch = render_hub_patch(&HubRenderModel {
            status: &st,
            today: Some(&today),
            logs: Some(&logs),
            ui: &ui,
        });
        assert_eq!(mac_patch.resolved_theme, "light");
        assert_eq!(mac_patch.lang, "en");
        assert!(mac_patch.html.contains("⌘K"));
        assert!(mac_patch.html.contains("⌘2"));
        assert!(!mac_patch.html.contains("Ctrl+K"));
        assert!(mac_patch.html.contains("Open BELL Explain"));
        assert!(mac_patch.html.contains(r#"class="badge-subtle badge-ok">Primary</span>"#));
        // Primary app Explain must appear before Personal in workspace rows
        let explain_pos = mac_patch.html.find(r#"class="ws-row" data-app="explain""#).unwrap();
        let personal_pos = mac_patch
            .html
            .find(r#"class="ws-row" data-app="personal""#)
            .unwrap();
        assert!(explain_pos < personal_pos);

        // Windows / Linux shortcut formatting
        st.os = "windows".to_string();
        let win_patch = render_hub_patch(&HubRenderModel {
            status: &st,
            today: Some(&today),
            logs: Some(&logs),
            ui: &ui,
        });
        assert!(win_patch.html.contains("Ctrl+K"));
        assert!(win_patch.html.contains("Ctrl+2"));
        assert!(!win_patch.html.contains("⌘K"));

        // Verify HTML escaping
        assert!(win_patch.html.contains("Snapshot &lt;Ready&gt;"));
        assert!(win_patch.html.contains("/tmp/vault &amp; backup"));

        // Verify CONTRACT.md §1 ground-truth brand hex tokens in HUB_CSS
        for hex in [
            "#E08A5C", "#A4532C", // personal
            "#A799FF", "#5B45E0", // explain
            "#46C289", "#1B6B4C", // pr
            "#38AEF8", "#0B6BCB", // reader
            "#E25B91", "#9E2B5C", // campaign
        ] {
            assert!(
                HUB_CSS.contains(hex),
                "HUB_CSS must include ground-truth brand token {hex}"
            );
        }
    }

    #[test]
    fn every_deep_route_pill_passes_suite_contract_v1() {
        for app in AppId::ALL {
            for lang in [Lang::ZhTw, Lang::En] {
                for &(route, label) in deep_routes(app, lang) {
                    assert!(!label.is_empty());
                    assert!(
                        allowed_route(app, route),
                        "route {route} for {app} must satisfy Suite Contract v1"
                    );
                }
            }
        }
    }

    #[test]
    fn initial_html_and_favicon_ico_are_valid() {
        let html = render_initial_html();
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains(r#"id="hub-root""#));
        assert!(html.contains("./hub.css"));
        assert!(html.contains("./favicon.svg"));
        assert!(FAVICON_SVG.contains("<svg"));
        assert!(!html.contains("chrome-bar"));

        let ico = build_favicon_ico_bytes();
        assert_eq!(&ico[0..6], &[0, 0, 1, 0, 2, 0]);
        assert!(ico.len() > 4000);
    }
}
