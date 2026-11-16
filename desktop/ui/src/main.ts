import "./styles.css";
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
import {
  formatBytes,
  formatQuotaGb,
  formatShortTime,
  formatVaultMb,
  getCopy,
  normalizeLang,
  type UiCopy,
} from "./i18n";
import {
  PRODUCTS,
  WORKSPACE_ORDER,
  WORKSPACE_ROUTES,
  type AppId,
  type ChromePlatform,
  type DesktopStatus,
  type LangPref,
  type TargetId,
  type ThemePref,
  type TodayItem,
  type TodayItemState,
  type TodayOverview,
  type WindowAction,
  type WorkspaceAction,
  type WorkspaceInfo,
  type WorkspaceLogs,
} from "./types";

interface ToastMessage {
  kind: "ok" | "error";
  title: string;
  detail: string;
}

interface UiState {
  status: DesktopStatus | null;
  today: TodayOverview | null;
  error: string | null;
  busyKey: string | null;
  todayFilter: "all" | TodayItemState;
  activeRoutes: Partial<Record<AppId, string>>;
  stageLogs: WorkspaceLogs | null;
  logsModalApp: AppId | null;
  logsModalData: WorkspaceLogs | null;
  copiedKey: string | null;
  toast: ToastMessage | null;
  themeOverride: ThemePref | null;
  langOverride: LangPref | null;
}

const isTauri = isTauriRuntime();
const urlParams = new URLSearchParams(window.location.search);

const initialThemeParam = urlParams.get("theme");
const initialLangParam = urlParams.get("lang");

const state: UiState = {
  status: null,
  today: null,
  error: null,
  busyKey: null,
  todayFilter: "all",
  activeRoutes: {},
  stageLogs: null,
  logsModalApp: null,
  logsModalData: null,
  copiedKey: null,
  toast: null,
  themeOverride:
    initialThemeParam === "light" || initialThemeParam === "dark" || initialThemeParam === "system"
      ? initialThemeParam
      : null,
  langOverride:
    initialLangParam === "en" || initialLangParam === "zh-tw" || initialLangParam === "zh-TW"
      ? normalizeLang(initialLangParam)
      : null,
};

const appRoot = document.getElementById("app");
if (!appRoot) {
  throw new Error("Missing #app mount container");
}

appRoot.innerHTML = `
  <header id="chrome-mount" class="chrome-bar" data-tauri-drag-region></header>
  <main id="main-mount" class="shell-main"></main>
  <div id="modal-mount"></div>
  <div id="toast-mount" class="toast-layer" aria-live="polite"></div>
`;

const chromeMount = document.getElementById("chrome-mount") as HTMLElement;
const mainMount = document.getElementById("main-mount") as HTMLElement;
const modalMount = document.getElementById("modal-mount") as HTMLElement;
const toastMount = document.getElementById("toast-mount") as HTMLElement;

let lastChromeHtml = "";
let lastMainHtml = "";
let lastModalHtml = "";
let lastToastHtml = "";
let toastTimer: number | null = null;
let copiedTimer: number | null = null;

const darkMedia = window.matchMedia("(prefers-color-scheme: dark)");

function currentThemePref(): ThemePref {
  if (state.themeOverride) return state.themeOverride;
  const raw = state.status?.prefs.theme;
  if (raw === "light" || raw === "dark" || raw === "system") return raw;
  return "system";
}

function resolvedThemePolarity(pref: ThemePref): "light" | "dark" {
  if (pref === "light" || pref === "dark") return pref;
  return darkMedia.matches ? "dark" : "light";
}

function currentLang(): LangPref {
  if (state.langOverride) return state.langOverride;
  return normalizeLang(state.status?.prefs.lang);
}

function currentPlatform(): ChromePlatform {
  return resolveChromePlatform(state.status?.os);
}

function syncDocumentAttributes(): void {
  const pref = currentThemePref();
  const polarity = resolvedThemePolarity(pref);
  const lang = currentLang();
  const platform = currentPlatform();
  const activeTarget = state.status?.activeTarget ?? "hub";
  const compactBar = isTauri && activeTarget !== "hub";

  const docEl = document.documentElement;
  docEl.dataset.theme = polarity;
  docEl.dataset.platform = platform;
  docEl.dataset.compactBar = compactBar ? "true" : "false";
  docEl.lang = lang === "en" ? "en" : "zh-Hant";

  const edition = state.status?.edition ?? "suite";
  mainMount.dataset.app = edition;
  document.title = PRODUCTS[edition]?.displayName ?? "BELL Suite";
}

function showToast(toast: ToastMessage, ms = 6500): void {
  state.toast = toast;
  if (toastTimer !== null) window.clearTimeout(toastTimer);
  renderToast();
  toastTimer = window.setTimeout(() => {
    state.toast = null;
    renderToast();
  }, ms);
}

function markCopied(key: string): void {
  state.copiedKey = key;
  if (copiedTimer !== null) window.clearTimeout(copiedTimer);
  render();
  copiedTimer = window.setTimeout(() => {
    state.copiedKey = null;
    render();
  }, 1800);
}

async function copyText(text: string, key: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    markCopied(key);
  } catch {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.setAttribute("readonly", "true");
    ta.className = "sr-only";
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    document.body.removeChild(ta);
    markCopied(key);
  }
}

function shortcutBadgeText(platform: ChromePlatform, index: number): string {
  return platform === "macos" ? `⌘${index}` : `Ctrl+${index}`;
}

function bellMarkSvg(): string {
  return `<svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
    <path d="M8 1.75a4.25 4.25 0 0 0-4.25 4.25v2.7L2.4 10.5a.75.75 0 0 0 .6 1.2h10a.75.75 0 0 0 .6-1.2l-1.35-1.8V6A4.25 4.25 0 0 0 8 1.75Z" stroke="currentColor" stroke-width="1.45" stroke-linejoin="round"/>
    <path d="M6.4 13.1a1.75 1.75 0 0 0 3.2 0" stroke="currentColor" stroke-width="1.45" stroke-linecap="round"/>
  </svg>`;
}

function orderedWorkspaces(status: DesktopStatus | null): WorkspaceInfo[] {
  if (!status) {
    return WORKSPACE_ORDER.map((id) => {
      const m = PRODUCTS[id];
      return {
        id,
        name: m.displayName,
        shortcutIndex: m.shortcutIndex,
        accentLight: m.accentLight,
        accentDark: m.accentDark,
        port: m.defaultPort,
        url: `http://127.0.0.1:${m.defaultPort}`,
        state: "stopped",
        pid: null,
        manifestValid: false,
        installed: true,
        dbBytes: 0,
        restartCount: 0,
        lastError: null,
      };
    });
  }
  const byId = new Map(status.workspaces.map((w) => [w.id, w]));
  const result: WorkspaceInfo[] = [];
  for (const id of WORKSPACE_ORDER) {
    const found = byId.get(id);
    if (found) result.push(found);
  }
  for (const w of status.workspaces) {
    if (!result.some((r) => r.id === w.id)) result.push(w);
  }
  return result;
}

function renderChromeBar(copy: UiCopy): void {
  if (document.activeElement?.classList.contains("chrome-suite-select")) {
    return;
  }

  const platform = currentPlatform();
  const previewLights = shouldShowPreviewLights(platform, isTauri);
  const edition = state.status?.edition ?? "suite";
  const activeTarget = state.status?.activeTarget ?? "hub";
  const workspaces = orderedWorkspaces(state.status);
  const lang = currentLang();
  const themePref = currentThemePref();

  const leftInsetHtml =
    platform === "macos"
      ? `<div class="chrome-macos-inset" data-tauri-drag-region aria-hidden="true">
          ${
            previewLights
              ? `<span class="preview-light preview-light-close"></span>
                 <span class="preview-light preview-light-min"></span>
                 <span class="preview-light preview-light-max"></span>`
              : ""
          }
        </div>`
      : "";

  const brandEditionText = edition === "suite" ? "SUITE" : edition.toUpperCase();
  const brandHtml = `
    <div class="chrome-brand" data-app="${escHtml(edition)}" data-tauri-drag-region>
      <span class="chrome-brand-mark">${bellMarkSvg()}</span>
      <span class="chrome-brand-name">BELL</span>
      <span class="chrome-edition-badge">${escHtml(brandEditionText)}</span>
    </div>
    <span class="chrome-divider" aria-hidden="true"></span>
  `;

  let navHtml = "";
  if (edition === "suite") {
    const hubSelected = activeTarget === "hub" ? "true" : "false";
    const tabsHtml = workspaces
      .map((ws) => {
        const meta = PRODUCTS[ws.id];
        const selected = activeTarget === ws.id ? "true" : "false";
        const kbd = shortcutBadgeText(platform, ws.shortcutIndex || meta.shortcutIndex);
        return `
          <button
            type="button"
            class="chrome-tab"
            data-app="${escHtml(ws.id)}"
            data-action="activate"
            data-target="${escHtml(ws.id)}"
            aria-selected="${selected}"
            title="${escHtml(ws.name)} (${escHtml(kbd)}) · ${escHtml(copy.stateLabel[ws.state])}"
          >
            <span class="state-dot state-dot-${escHtml(ws.state)}" aria-hidden="true"></span>
            <span class="chrome-tab-index">${escHtml(ws.shortcutIndex || meta.shortcutIndex)}</span>
            <span class="chrome-tab-label-text">${escHtml(meta.shortLabel)}</span>
            <kbd class="kbd-hint">${escHtml(kbd)}</kbd>
          </button>
        `;
      })
      .join("");

    navHtml = `
      <nav class="chrome-nav" aria-label="BELL Workspaces">
        <button
          type="button"
          class="chrome-tab chrome-tab-hub"
          data-app="suite"
          data-action="activate"
          data-target="hub"
          aria-selected="${hubSelected}"
          title="${escHtml(`${copy.hubButtonPrefix}${copy.hubButtonShort}`)} (${escHtml(shortcutBadgeText(platform, 0))})"
        >
          <span class="chrome-tab-label-text"><span class="chrome-hub-prefix">${escHtml(copy.hubButtonPrefix)}</span>${escHtml(copy.hubButtonShort)}</span>
          <kbd class="kbd-hint">${escHtml(shortcutBadgeText(platform, 0))}</kbd>
        </button>
        ${tabsHtml}
      </nav>
    `;
  } else {
    const primaryWs = workspaces.find((w) => w.id === edition) ?? workspaces[0];
    const primaryState = primaryWs?.state ?? "stopped";
    const primarySelected = activeTarget === edition ? "true" : "false";
    const hubSelected = activeTarget === "hub" ? "true" : "false";
    const optionsHtml = [
      `<option value="" selected disabled>${escHtml(copy.switchInSuite)}</option>`,
      ...WORKSPACE_ORDER.map((id) => {
        const m = PRODUCTS[id];
        const ws = workspaces.find((w) => w.id === id);
        const st = ws ? copy.stateLabel[ws.state] : "";
        return `<option value="${escHtml(id)}">${m.shortcutIndex} · ${escHtml(m.displayName)}${st ? ` (${escHtml(st)})` : ""}</option>`;
      }),
      `<option value="hub">0 · ${escHtml(copy.hubVaultToggle)}</option>`,
    ].join("");

    navHtml = `
      <nav class="chrome-nav chrome-standalone-group" aria-label="BELL Workspace">
        <button
          type="button"
          class="chrome-tab"
          data-app="${escHtml(edition)}"
          data-action="activate"
          data-target="${escHtml(edition)}"
          aria-selected="${primarySelected}"
        >
          <span class="state-dot state-dot-${escHtml(primaryState)}" aria-hidden="true"></span>
          <span class="chrome-standalone-full">${escHtml(PRODUCTS[edition].displayName)}</span>
          <span class="chrome-standalone-short">${escHtml(PRODUCTS[edition].shortLabel)}</span>
          <span class="kbd-hint">${escHtml(copy.stateLabel[primaryState])}</span>
        </button>
        <select
          id="suite-workspace-select"
          name="suite-workspace"
          class="chrome-suite-select"
          data-action="suite-select"
          aria-label="${escHtml(copy.switchInSuite)}"
        >
          ${optionsHtml}
        </select>
        <button
          type="button"
          class="chrome-tab chrome-tab-hub"
          data-app="${escHtml(edition)}"
          data-action="activate"
          data-target="${activeTarget === "hub" ? escHtml(edition) : "hub"}"
          aria-selected="${hubSelected}"
        >
          <span class="chrome-tab-label-text chrome-standalone-full">${escHtml(activeTarget === "hub" ? copy.backToWorkspaces : copy.hubVaultToggle)}</span>
          <span class="chrome-tab-label-text chrome-standalone-short">${escHtml(copy.hubButtonShort)}</span>
          <kbd class="kbd-hint">${escHtml(shortcutBadgeText(platform, 0))}</kbd>
        </button>
      </nav>
    `;
  }

  const nextLang: LangPref = lang === "en" ? "zh-tw" : "en";
  const langBtnLabel = lang === "en" ? "EN" : "繁中";
  const nextTheme: ThemePref =
    themePref === "light" ? "dark" : themePref === "dark" ? "system" : "light";
  const themeLabel =
    themePref === "light"
      ? copy.themeLight
      : themePref === "dark"
        ? copy.themeDark
        : copy.themeSystem;
  const nextThemeLabel =
    nextTheme === "light"
      ? copy.themeLight
      : nextTheme === "dark"
        ? copy.themeDark
        : copy.themeSystem;

  const winControlsHtml =
    platform === "windows"
      ? `
      <div class="win-controls" role="group" aria-label="Window Controls">
        <button type="button" class="win-btn" data-action="win" data-win="minimize" title="${escHtml(copy.winMinimize)}" aria-label="${escHtml(copy.winMinimize)}">
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M1 5h8" stroke="currentColor" stroke-width="1"/></svg>
        </button>
        <button type="button" class="win-btn" data-action="win" data-win="toggleMaximize" title="${escHtml(copy.winMaximize)}" aria-label="${escHtml(copy.winMaximize)}">
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true"><rect x="1.5" y="1.5" width="7" height="7" stroke="currentColor" stroke-width="1"/></svg>
        </button>
        <button type="button" class="win-btn win-btn-close" data-action="win" data-win="close" title="${escHtml(copy.winClose)}" aria-label="${escHtml(copy.winClose)}">
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M1.5 1.5l7 7M8.5 1.5l-7 7" stroke="currentColor" stroke-width="1.1"/></svg>
        </button>
      </div>
    `
      : "";

  const html = `
    ${leftInsetHtml}
    ${brandHtml}
    ${navHtml}
    <div class="chrome-spacer" data-tauri-drag-region></div>
    <div class="chrome-right">
      <button type="button" class="chrome-mini-btn" data-action="set-lang" data-lang="${escHtml(nextLang)}" title="${escHtml(copy.langSwitchTooltip)}">
        ${escHtml(langBtnLabel)}
      </button>
      <button type="button" class="chrome-mini-btn" data-action="set-theme" data-theme-pref="${escHtml(nextTheme)}" title="${escHtml(copy.themeSwitchTooltip(themeLabel, nextThemeLabel))}">
        ${escHtml(themeLabel)}
      </button>
    </div>
    ${winControlsHtml}
  `;

  if (html !== lastChromeHtml) {
    chromeMount.innerHTML = html;
    lastChromeHtml = html;
  }
}

function renderWorkspaceCard(ws: WorkspaceInfo, copy: UiCopy, platform: ChromePlatform): string {
  const meta = PRODUCTS[ws.id];
  const kbd = shortcutBadgeText(platform, ws.shortcutIndex || meta.shortcutIndex);
  const stateClass =
    ws.state === "running"
      ? "ws-state-pill-running"
      : ws.state === "starting"
        ? "ws-state-pill-starting"
        : ws.state === "error"
          ? "ws-state-pill-error"
          : "";
  const isRunning = ws.state === "running";
  const busyPrefix = `ws:${ws.id}:`;
  const cardBusy = state.busyKey?.startsWith(busyPrefix) ?? false;

  const manifestBadge = !ws.installed
    ? `<span class="ws-manifest-badge ws-manifest-idle">${escHtml(copy.notInstalled)}</span>`
    : ws.manifestValid
      ? `<span class="ws-manifest-badge">${escHtml(copy.manifestValid)}</span>`
      : `<span class="ws-manifest-badge ws-manifest-idle">${escHtml(copy.manifestPending)}</span>`;

  const primaryLabel = isRunning ? copy.openWorkspace : copy.startAndOpen;
  const secondaryFirstAction: WorkspaceAction = isRunning ? "restart" : "start";
  const secondaryFirstLabel = isRunning ? copy.restart : copy.start;

  return `
    <article class="ws-card" data-app="${escHtml(ws.id)}">
      <div class="ws-accent-bar" aria-hidden="true"></div>
      <div class="ws-body">
        <div class="ws-top-row">
          <div class="ws-identity">
            <div class="ws-kicker-row">
              <span class="ws-shortcut mono">${escHtml(kbd)}</span>
              ${manifestBadge}
            </div>
            <h3 class="ws-name">${escHtml(ws.name || meta.displayName)}</h3>
          </div>
          <span class="ws-state-pill ${stateClass}">
            <span class="state-dot state-dot-${escHtml(ws.state)}" aria-hidden="true"></span>
            ${escHtml(copy.stateLabel[ws.state])}
          </span>
        </div>
        <p class="ws-tagline">${escHtml(copy.taglines[ws.id])}</p>
        <div class="ws-specs">
          <div class="ws-spec-cell">
            <span class="ws-spec-key">${escHtml(copy.portLabel)}</span>
            <span class="ws-spec-val mono">:${escHtml(ws.port)}</span>
          </div>
          <div class="ws-spec-cell">
            <span class="ws-spec-key">${escHtml(copy.pidLabel)}</span>
            <span class="ws-spec-val mono">${ws.pid ? escHtml(ws.pid) : "—"}</span>
          </div>
          <div class="ws-spec-cell">
            <span class="ws-spec-key">${escHtml(copy.dbSizeLabel)}</span>
            <span class="ws-spec-val mono">${escHtml(formatBytes(ws.dbBytes))}</span>
          </div>
        </div>
        ${ws.lastError ? `<div class="ws-error-note mono">${escHtml(ws.lastError)}</div>` : ""}
      </div>
      <div class="ws-footer">
        <div class="ws-secondary-actions">
          <button
            type="button"
            class="btn btn-sm btn-surface"
            data-action="ws-op"
            data-ws="${escHtml(ws.id)}"
            data-op="${secondaryFirstAction}"
            ${!ws.installed || cardBusy ? "disabled" : ""}
          >${escHtml(secondaryFirstLabel)}</button>
          <button
            type="button"
            class="btn btn-sm btn-surface"
            data-action="ws-op"
            data-ws="${escHtml(ws.id)}"
            data-op="stop"
            ${ws.state === "stopped" || cardBusy ? "disabled" : ""}
          >${escHtml(copy.stop)}</button>
          <button
            type="button"
            class="btn btn-sm btn-surface"
            data-action="open-logs"
            data-ws="${escHtml(ws.id)}"
          >${escHtml(copy.logs)}</button>
        </div>
        <button
          type="button"
          class="btn btn-sm btn-primary"
          data-action="activate"
          data-target="${escHtml(ws.id)}"
          ${!ws.installed || cardBusy ? "disabled" : ""}
        >${escHtml(primaryLabel)}</button>
      </div>
    </article>
  `;
}

function renderContractSpecCard(workspaces: WorkspaceInfo[], copy: UiCopy): string {
  const validCount = workspaces.filter((w) => w.manifestValid).length;
  return `
    <article class="ws-card ws-card-contract" data-app="suite">
      <div class="ws-accent-bar" aria-hidden="true"></div>
      <div class="ws-body">
        <div class="ws-identity">
          <span class="editorial-kicker">${escHtml(copy.contractCardKicker)}</span>
          <h3 class="ws-name">${escHtml(copy.contractCardTitle)}</h3>
        </div>
        <p class="ws-tagline">${escHtml(copy.contractCardBody)}</p>
        <div class="contract-tags">
          <span class="contract-tag mono">${escHtml(copy.contractLoopback)}</span>
          <span class="contract-tag mono">${escHtml(copy.contractIsolatedDb)}</span>
          <span class="contract-tag mono">${escHtml(copy.contractCsp)}</span>
          <span class="contract-tag">${escHtml(copy.contractManifests(validCount, workspaces.length))}</span>
        </div>
      </div>
    </article>
  `;
}

function flattenTodayItems(today: TodayOverview | null): TodayItem[] {
  if (!today) return [];
  const all: TodayItem[] = [];
  for (const src of today.sources) {
    for (const item of src.items) {
      all.push(item);
    }
  }
  return all;
}

function renderHubView(status: DesktopStatus | null, copy: UiCopy): string {
  const edition = status?.edition ?? "suite";
  const platform = currentPlatform();
  const lang = currentLang();
  const themePref = currentThemePref();
  const workspaces = orderedWorkspaces(status);
  const runningCount = workspaces.filter((w) => w.state === "running").length;
  const isStartingAll = state.busyKey === "start-all";
  const autoStart = status?.prefs.autoStart ?? false;

  const errorBannerHtml = state.error
    ? `
      <section class="error-banner" role="alert">
        <div class="error-banner-text">
          <span class="error-banner-title">${escHtml(copy.errorTitle)}</span>
          <span class="error-banner-detail mono">${escHtml(state.error)}</span>
        </div>
        <button type="button" class="btn btn-sm btn-surface" data-action="retry-all">${escHtml(copy.errorRetry)}</button>
      </section>
    `
    : "";

  const versionMeta = status
    ? `v${status.version} · ${status.os}/${status.arch}`
    : "BELL Local Daemon";

  const primaryStandaloneWs =
    edition !== "suite"
      ? (workspaces.find((w) => w.id === edition) ?? workspaces[0] ?? null)
      : null;
  const primaryStandaloneState = primaryStandaloneWs?.state ?? "stopped";
  const standaloneBusy =
    edition !== "suite" && (state.busyKey?.startsWith(`ws:${edition}:`) ?? false);

  const heroActionsTopHtml =
    edition === "suite"
      ? `
          <span class="running-pill">
            <span class="state-dot ${runningCount > 0 ? "state-dot-running" : "state-dot-stopped"}" aria-hidden="true"></span>
            ${escHtml(copy.runningSummary(runningCount, workspaces.length))}
          </span>
          <button
            type="button"
            class="btn btn-primary"
            data-action="start-all"
            ${isStartingAll ? "disabled" : ""}
          >
            ${escHtml(isStartingAll ? copy.startingAll : copy.startAll)}
          </button>
        `
      : `
          <span class="running-pill">
            <span class="state-dot state-dot-${escHtml(primaryStandaloneState)}" aria-hidden="true"></span>
            ${escHtml(copy.standaloneHeroState[primaryStandaloneState])}
          </span>
          <button
            type="button"
            class="btn btn-primary"
            data-action="activate"
            data-target="${escHtml(edition)}"
            ${standaloneBusy ? "disabled" : ""}
          >
            ${escHtml(
              primaryStandaloneState === "running"
                ? copy.standaloneOpenWorkspace
                : copy.standaloneStartWorkspace,
            )}
          </button>
        `;

  const heroHtml = `
    <section class="hero-strip" data-app="${escHtml(edition)}">
      <div class="hero-copy">
        <span class="editorial-kicker">${escHtml(edition === "suite" ? copy.heroKickerSuite : copy.heroKickerStandalone)}</span>
        <div class="hero-title-row">
          <h1 class="hero-title">${escHtml(PRODUCTS[edition].displayName)}</h1>
          <span class="hero-meta-pill mono">${escHtml(versionMeta)}</span>
        </div>
        <p class="hero-subtitle">${escHtml(edition === "suite" ? copy.heroSubtitleSuite : copy.heroSubtitleStandalone)}</p>
      </div>
      <div class="hero-controls">
        <div class="hero-actions-top">
          ${heroActionsTopHtml}
        </div>
        <div class="hero-actions-bottom">
          <button
            type="button"
            class="toggle-chip"
            data-action="toggle-autostart"
            aria-pressed="${autoStart ? "true" : "false"}"
          >
            <span>${autoStart ? "✓" : "○"}</span>
            <span>${escHtml(copy.autoStart)}</span>
          </button>
          <div class="seg-group" role="group" aria-label="Language">
            <button type="button" class="seg-btn" data-action="set-lang" data-lang="zh-tw" aria-pressed="${lang === "zh-tw" ? "true" : "false"}">繁中</button>
            <button type="button" class="seg-btn" data-action="set-lang" data-lang="en" aria-pressed="${lang === "en" ? "true" : "false"}">EN</button>
          </div>
          <div class="seg-group" role="group" aria-label="Theme">
            <button type="button" class="seg-btn" data-action="set-theme" data-theme-pref="light" aria-pressed="${themePref === "light" ? "true" : "false"}">${escHtml(copy.themeLight)}</button>
            <button type="button" class="seg-btn" data-action="set-theme" data-theme-pref="dark" aria-pressed="${themePref === "dark" ? "true" : "false"}">${escHtml(copy.themeDark)}</button>
            <button type="button" class="seg-btn" data-action="set-theme" data-theme-pref="system" aria-pressed="${themePref === "system" ? "true" : "false"}">${escHtml(copy.themeSystem)}</button>
          </div>
        </div>
      </div>
    </section>
  `;

  const rt = status?.runtime;
  const isBundled = rt?.kind === "bundled";
  const cliEntries: Array<[string, boolean]> = [
    ["claude", Boolean(rt?.claudeCli)],
    ["codex", Boolean(rt?.codexCli)],
    ["gh", Boolean(rt?.ghCli)],
    ["git", Boolean(rt?.gitCli)],
  ];

  const runtimeHtml = `
    <section class="runtime-strip" aria-label="${escHtml(copy.runtimeTitle)}">
      <div class="runtime-left">
        <span class="runtime-label">${escHtml(copy.runtimeTitle)}</span>
        <span class="runtime-engine-pill ${isBundled ? "" : "runtime-engine-pill-system"}">
          <span>${isBundled ? "●" : "○"}</span>
          <span>${escHtml(isBundled ? copy.zeroNodeReady : copy.systemNode)}</span>
        </span>
        ${rt?.path ? `<span class="runtime-path mono" title="${escHtml(rt.path)}">${escHtml(rt.path)}</span>` : ""}
      </div>
      <div class="cli-badges">
        ${cliEntries
          .map(
            ([name, ok]) => `
            <span class="cli-pill mono ${ok ? "cli-pill-ready" : ""}" title="${escHtml(name)}: ${escHtml(ok ? copy.cliReady : copy.cliMissing)}">
              <span>${ok ? "✓" : "—"}</span>
              <span>${escHtml(name)}</span>
            </span>
          `,
          )
          .join("")}
      </div>
    </section>
  `;

  let workspacesGridSection = "";
  if (edition === "suite") {
    workspacesGridSection = `
      <div class="section-head">
        <h2 class="section-title">${escHtml(copy.workspacesSectionTitle)}</h2>
        <span class="section-sub">${escHtml(copy.workspacesSectionSub)}</span>
      </div>
      <section class="ws-grid">
        ${workspaces.map((ws) => renderWorkspaceCard(ws, copy, platform)).join("")}
        ${renderContractSpecCard(workspaces, copy)}
      </section>
    `;
  } else {
    const primary = workspaces.find((w) => w.id === edition) ?? workspaces[0];
    const siblings = workspaces.filter((w) => w.id !== primary?.id);
    workspacesGridSection = `
      ${
        primary
          ? `
            <div class="section-head">
              <h2 class="section-title">${escHtml(PRODUCTS[edition].displayName)}</h2>
              <span class="section-sub">${escHtml(copy.workspacesSectionSub)}</span>
            </div>
            <section class="ws-grid-standalone-primary">
              ${renderWorkspaceCard(primary, copy, platform)}
            </section>
          `
          : ""
      }
      ${
        siblings.length > 0
          ? `
            <div class="section-head">
              <h2 class="section-title">${escHtml(copy.siblingSectionTitle)}</h2>
            </div>
            <section class="ws-grid ws-grid-siblings">
              ${siblings.map((ws) => renderWorkspaceCard(ws, copy, platform)).join("")}
            </section>
          `
          : ""
      }
    `;
  }

  // Today Feed Panel
  const allItems = flattenTodayItems(state.today);
  const filteredItems =
    state.todayFilter === "all"
      ? allItems
      : allItems.filter((it) => it.state === state.todayFilter);
  const filterStates: Array<"all" | TodayItemState> = [
    "all",
    "due",
    "needs-you",
    "waiting",
    "new",
  ];

  const sourceChipsHtml = (state.today?.sources ?? [])
    .map((src) => {
      const itemCount = src.items.length;
      const ws = workspaces.find((w) => w.id === src.appId);
      const dotState = ws ? ws.state : src.status === "ready" ? "running" : "stopped";
      const chipBadge =
        src.status === "unsupported" && ws?.state === "running" && itemCount === 0
          ? copy.todayAggregatorBadge
          : String(itemCount);
      return `
        <span class="source-chip" data-app="${escHtml(src.appId)}">
          <span class="state-dot state-dot-${escHtml(dotState)}" aria-hidden="true"></span>
          <span>${escHtml(PRODUCTS[src.appId]?.shortLabel ?? src.appId)}</span>
          <span class="mono">${escHtml(chipBadge)}</span>
        </span>
      `;
    })
    .join("");

  const todayItemsHtml =
    filteredItems.length === 0
      ? `
        <div class="empty-box">
          <div class="empty-box-title">${escHtml(copy.todayEmptyTitle)}</div>
          <div class="empty-box-body">${escHtml(copy.todayEmptyBody)}</div>
        </div>
      `
      : filteredItems
          .map((item) => {
            const stateClass =
              item.state === "due"
                ? "item-state-due"
                : item.state === "needs-you"
                  ? "item-state-needs-you"
                  : item.state === "new"
                    ? "item-state-new"
                    : "";
            return `
              <div class="today-item" data-app="${escHtml(item.sourceApp)}">
                <div class="today-item-main">
                  <div class="today-item-badges">
                    <span class="app-chip">${escHtml(PRODUCTS[item.sourceApp]?.shortLabel ?? item.sourceApp)}</span>
                    <span class="item-state-badge ${stateClass}">${escHtml(copy.todayStateLabel[item.state] ?? item.state)}</span>
                    <code class="kbd-hint">${escHtml(item.href)}</code>
                  </div>
                  <div class="today-item-title">${escHtml(item.title)}</div>
                  <div class="today-item-meta">
                    ${item.detail ? `<span>${escHtml(item.detail)}</span>` : ""}
                    <span>${escHtml(formatShortTime(item.updatedAt, lang))}</span>
                  </div>
                </div>
                <button
                  type="button"
                  class="btn btn-sm btn-surface"
                  data-action="jump-today"
                  data-target="${escHtml(item.sourceApp)}"
                  data-route="${escHtml(item.href)}"
                >${escHtml(copy.todayJump)}</button>
              </div>
            `;
          })
          .join("");

  // Local SQLite Vault & Sync Quota Panel
  const vault = status?.vault;
  const usedBytes = vault?.totalBytes ?? 0;
  const quotaBytes = vault?.quotaBytes ?? 5 * 1024 * 1024 * 1024;
  const ratioPct = Math.min(100, Math.max(usedBytes > 0 ? 1.2 : 0, (usedBytes / Math.max(1, quotaBytes)) * 100));
  const backupSummary =
    vault && vault.backupsCount > 0 && vault.lastBackupAt
      ? copy.vaultBackupsSummary(vault.backupsCount, formatShortTime(vault.lastBackupAt, lang))
      : copy.vaultNeverBackedUp;
  const isBackingUp = state.busyKey === "backup-vault";

  const dbRowsHtml =
    !vault || vault.databases.length === 0
      ? `<div class="empty-box"><div class="empty-box-body">${escHtml(copy.vaultDbEmpty)}</div></div>`
      : vault.databases
          .map((db) => {
            const fileName = db.path.split(/[\\/]/).pop() || `${db.appId}.db`;
            const isJson = fileName.toLowerCase().endsWith(".json");
            const dotClass = db.healthy ? "state-dot-running" : "state-dot-error";
            const appLabel = PRODUCTS[db.appId]?.shortLabel ?? db.appId;
            const integrityText = isJson
              ? db.healthy
                ? copy.vaultJsonOk
                : copy.vaultJsonFail
              : db.healthy
                ? copy.vaultIntegrityOk
                : copy.vaultIntegrityFail;
            const countText = isJson
              ? copy.vaultJsonEntries(db.tables)
              : copy.vaultTables(db.tables);
            return `
              <div class="vault-db-row" data-app="${escHtml(db.appId)}">
                <div class="vault-db-left">
                  <span class="state-dot ${dotClass}" aria-hidden="true"></span>
                  <span class="app-chip">${escHtml(appLabel)}</span>
                  <span class="vault-db-name mono" title="${escHtml(db.path)}">${escHtml(fileName)}</span>
                </div>
                <div class="vault-db-right">
                  <span class="${db.healthy ? "integrity-ok" : "integrity-fail"}">
                    ${escHtml(integrityText)}
                  </span>
                  <span>${escHtml(countText)}</span>
                  <span class="mono">${escHtml(formatBytes(db.bytes))}</span>
                </div>
              </div>
            `;
          })
          .join("");

  const lowerSplitHtml = `
    <section class="lower-split">
      <div class="panel">
        <div class="panel-header">
          <div class="panel-heading-group">
            <div class="panel-title-row">
              <h2 class="panel-title">${escHtml(copy.todayTitle)}</h2>
              <span class="count-pill mono">${escHtml(state.today?.totalItems ?? allItems.length)}</span>
            </div>
            <p class="panel-sub">${escHtml(copy.todaySub)}</p>
          </div>
          <button type="button" class="btn btn-sm btn-surface" data-action="refresh-today">${escHtml(copy.todayRefresh)}</button>
        </div>
        <div class="today-toolbar">
          <div class="filter-pills" role="group" aria-label="Filter Today items">
            ${filterStates
              .map((f) => {
                const label = f === "all" ? copy.todayFilterAll : copy.todayStateLabel[f];
                return `<button type="button" class="filter-pill" data-action="filter-today" data-filter="${escHtml(f)}" aria-pressed="${state.todayFilter === f ? "true" : "false"}">${escHtml(label)}</button>`;
              })
              .join("")}
          </div>
          <div class="source-chips">${sourceChipsHtml}</div>
        </div>
        <div class="today-list">${todayItemsHtml}</div>
      </div>

      <div class="panel">
        <div class="panel-header">
          <div class="panel-heading-group">
            <h2 class="panel-title">${escHtml(copy.vaultTitle)}</h2>
            <p class="panel-sub">${escHtml(copy.vaultSub)}</p>
          </div>
        </div>
        <div class="vault-body">
          <div class="vault-path-well">
            <div class="vault-path-text">
              <span class="vault-path-label">${escHtml(copy.vaultRootLabel)}</span>
              <span class="vault-path-value mono" title="${escHtml(vault?.rootDir ?? "—")}">${escHtml(vault?.rootDir ?? "—")}</span>
            </div>
            ${
              vault?.rootDir
                ? `<button type="button" class="btn btn-sm btn-surface" data-action="copy-vault-path">${escHtml(state.copiedKey === "vault-path" ? copy.logsCopied : copy.vaultCopyPath)}</button>`
                : ""
            }
          </div>
          <div class="vault-quota-box">
            <div class="vault-quota-row">
              <span class="vault-quota-main mono">${escHtml(copy.vaultSyncCap(formatVaultMb(usedBytes), formatQuotaGb(quotaBytes)))}</span>
              <span class="vault-quota-sub">${escHtml(backupSummary)}</span>
            </div>
            <svg class="vault-meter-svg" viewBox="0 0 100 6" preserveAspectRatio="none" aria-hidden="true">
              <rect class="vault-meter-track" x="0" y="0" width="100" height="6" rx="3" />
              <rect class="vault-meter-fill" x="0" y="0" width="${ratioPct.toFixed(2)}" height="6" rx="3" />
            </svg>
          </div>
          <div class="vault-db-list">${dbRowsHtml}</div>
        </div>
        <div class="panel-footer">
          <button
            type="button"
            class="btn btn-primary"
            data-action="backup-vault"
            ${isBackingUp ? "disabled" : ""}
          >${escHtml(isBackingUp ? copy.vaultBackingUp : copy.vaultBackupNow)}</button>
        </div>
      </div>
    </section>
  `;

  return `
    ${errorBannerHtml}
    ${heroHtml}
    ${runtimeHtml}
    ${workspacesGridSection}
    ${lowerSplitHtml}
  `;
}

function renderActiveWorkspaceStage(target: AppId, copy: UiCopy): string {
  const workspaces = orderedWorkspaces(state.status);
  const ws = workspaces.find((w) => w.id === target) ?? {
    id: target,
    name: PRODUCTS[target].displayName,
    shortcutIndex: PRODUCTS[target].shortcutIndex,
    accentLight: PRODUCTS[target].accentLight,
    accentDark: PRODUCTS[target].accentDark,
    port: PRODUCTS[target].defaultPort,
    url: `http://127.0.0.1:${PRODUCTS[target].defaultPort}`,
    state: "stopped" as const,
    pid: null,
    manifestValid: false,
    installed: true,
    dbBytes: 0,
    restartCount: 0,
    lastError: null,
  };

  const routes = WORKSPACE_ROUTES[target];
  const activeRoute = sanitizeRoute(state.activeRoutes[target]) || routes[0] || "#/";
  const launchUrl = buildSafeWorkspaceLaunchUrl(ws.url, ws.port, activeRoute);
  const isRunning = ws.state === "running";
  const dbInfo = state.status?.vault.databases.find((d) => d.appId === target);
  const logsLines =
    state.stageLogs?.id === target ? state.stageLogs.lines : [];

  return `
    <section class="stage-card" data-app="${escHtml(target)}">
      <div class="ws-accent-bar" aria-hidden="true"></div>
      <div class="stage-header">
        <div class="hero-copy">
          <span class="editorial-kicker">${escHtml(copy.stageKicker)}</span>
          <div class="hero-title-row">
            <h1 class="hero-title">${escHtml(ws.name)}</h1>
            <span class="ws-state-pill ${isRunning ? "ws-state-pill-running" : ""}">
              <span class="state-dot state-dot-${escHtml(ws.state)}" aria-hidden="true"></span>
              ${escHtml(copy.stateLabel[ws.state])} · :${escHtml(ws.port)}
            </span>
            <span class="ws-manifest-badge ${ws.manifestValid ? "" : "ws-manifest-idle"}">
              ${escHtml(ws.manifestValid ? copy.manifestValid : copy.manifestPending)}
            </span>
          </div>
          <p class="hero-subtitle">${escHtml(copy.taglines[target])}</p>
        </div>
        <div class="stage-actions">
          <a
            class="btn btn-primary"
            href="${escHtml(launchUrl)}"
            target="_blank"
            rel="noopener noreferrer"
          >${escHtml(copy.stageLaunch(`http://127.0.0.1:${ws.port}/${activeRoute}`))}</a>
          <button
            type="button"
            class="btn btn-surface"
            data-action="ws-op"
            data-ws="${escHtml(target)}"
            data-op="${isRunning ? "restart" : "start"}"
          >${escHtml(isRunning ? copy.restart : copy.start)}</button>
          <button
            type="button"
            class="btn btn-surface"
            data-action="ws-op"
            data-ws="${escHtml(target)}"
            data-op="stop"
            ${ws.state === "stopped" ? "disabled" : ""}
          >${escHtml(copy.stop)}</button>
          <button
            type="button"
            class="btn btn-surface"
            data-action="activate"
            data-target="hub"
          >${escHtml(copy.stageBackHub)}</button>
        </div>
      </div>

      <div class="stage-body">
        <div class="stage-routes-box">
          <h2 class="stage-subhead">${escHtml(copy.stageRoutesTitle)}</h2>
          <div class="route-chips">
            ${routes
              .map(
                (r) => `
                <button
                  type="button"
                  class="route-chip mono"
                  data-action="select-stage-route"
                  data-ws="${escHtml(target)}"
                  data-route="${escHtml(r)}"
                  aria-pressed="${activeRoute === r ? "true" : "false"}"
                >${escHtml(r)}</button>
              `,
              )
              .join("")}
          </div>
          <p class="stage-note">${escHtml(copy.stageIsolationNote)}</p>
        </div>

        <h2 class="stage-subhead">${escHtml(copy.stageHealthTitle)}</h2>
        <div class="stage-health-grid">
          <div class="stage-metric">
            <span class="ws-spec-key">LOOPBACK ORIGIN</span>
            <span class="ws-spec-val mono">http://127.0.0.1:${escHtml(ws.port)}</span>
          </div>
          <div class="stage-metric">
            <span class="ws-spec-key">${escHtml(copy.pidLabel)} / ${escHtml(copy.restartsLabel)}</span>
            <span class="ws-spec-val mono">${ws.pid ? escHtml(ws.pid) : "—"} · ${escHtml(ws.restartCount)}</span>
          </div>
          <div class="stage-metric">
            <span class="ws-spec-key">${escHtml(copy.dbSizeLabel)}</span>
            <span class="ws-spec-val mono">${escHtml(formatBytes(ws.dbBytes))}</span>
          </div>
          <div class="stage-metric">
            <span class="ws-spec-key">INTEGRITY</span>
            <span class="ws-spec-val">${escHtml(
              dbInfo
                ? dbInfo.path.toLowerCase().endsWith(".json")
                  ? dbInfo.healthy
                    ? copy.vaultJsonOk
                    : copy.vaultJsonFail
                  : dbInfo.healthy
                    ? copy.vaultIntegrityOk
                    : copy.vaultIntegrityFail
                : "—",
            )}</span>
          </div>
        </div>

        <div class="log-console-box">
          <div class="log-console-bar">
            <span class="stage-subhead">${escHtml(copy.stageConsoleTitle)}</span>
            <div class="modal-actions">
              <button type="button" class="btn btn-sm btn-surface" data-action="refresh-stage-logs" data-ws="${escHtml(target)}">${escHtml(copy.logsRefresh)}</button>
              <button type="button" class="btn btn-sm btn-surface" data-action="copy-stage-logs">${escHtml(state.copiedKey === "stage-logs" ? copy.logsCopied : copy.logsCopy)}</button>
            </div>
          </div>
          <pre class="log-pre mono">${logsLines.length > 0 ? escHtml(logsLines.join("\n")) : escHtml(copy.logsEmpty)}</pre>
        </div>
      </div>
    </section>
  `;
}

function renderMainStage(copy: UiCopy): void {
  const activeTarget = state.status?.activeTarget ?? "hub";
  if (isTauri && activeTarget !== "hub") {
    mainMount.hidden = true;
    if (lastMainHtml !== "") {
      mainMount.innerHTML = "";
      lastMainHtml = "";
    }
    return;
  }

  mainMount.hidden = false;
  const html =
    activeTarget === "hub"
      ? renderHubView(state.status, copy)
      : renderActiveWorkspaceStage(activeTarget, copy);

  if (html !== lastMainHtml) {
    const prevScroll = window.scrollY;
    mainMount.innerHTML = html;
    lastMainHtml = html;
    window.scrollTo(0, prevScroll);
  }
}

function renderLogsModal(copy: UiCopy): void {
  const appId = state.logsModalApp;
  const activeTarget = state.status?.activeTarget ?? "hub";
  if (!appId || (isTauri && activeTarget !== "hub")) {
    if (lastModalHtml !== "") {
      modalMount.innerHTML = "";
      lastModalHtml = "";
    }
    return;
  }

  const meta = PRODUCTS[appId];
  const wsState = state.status?.workspaces.find((w) => w.id === appId)?.state ?? "stopped";
  const lines = state.logsModalData?.id === appId ? state.logsModalData.lines : [];
  const html = `
    <div class="modal-backdrop" data-action="close-logs-backdrop">
      <div class="modal-card" data-app="${escHtml(appId)}" role="dialog" aria-modal="true" aria-label="${escHtml(copy.logsModalTitle(meta.displayName))}">
        <div class="ws-accent-bar" aria-hidden="true"></div>
        <div class="modal-header">
          <h2 class="modal-title">
            <span class="state-dot state-dot-${escHtml(wsState)}" aria-hidden="true"></span>
            <span>${escHtml(copy.logsModalTitle(meta.displayName))}</span>
          </h2>
          <div class="modal-actions">
            <button type="button" class="btn btn-sm btn-surface" data-action="refresh-modal-logs" data-ws="${escHtml(appId)}">${escHtml(copy.logsRefresh)}</button>
            <button type="button" class="btn btn-sm btn-surface" data-action="copy-modal-logs">${escHtml(state.copiedKey === "modal-logs" ? copy.logsCopied : copy.logsCopy)}</button>
            <button type="button" class="btn btn-sm btn-primary" data-action="close-logs">${escHtml(copy.logsClose)}</button>
          </div>
        </div>
        <pre class="log-pre mono">${lines.length > 0 ? escHtml(lines.join("\n")) : escHtml(copy.logsEmpty)}</pre>
      </div>
    </div>
  `;

  if (html !== lastModalHtml) {
    modalMount.innerHTML = html;
    lastModalHtml = html;
  }
}

function renderToast(): void {
  if (!state.toast) {
    if (lastToastHtml !== "") {
      toastMount.innerHTML = "";
      lastToastHtml = "";
    }
    return;
  }
  const html = `
    <div class="toast-card ${state.toast.kind === "error" ? "toast-card-error" : ""}">
      <div>
        <div class="toast-title">${escHtml(state.toast.title)}</div>
        <div class="toast-detail mono">${escHtml(state.toast.detail)}</div>
      </div>
      <button type="button" class="btn btn-sm btn-surface" data-action="dismiss-toast" aria-label="Close">✕</button>
    </div>
  `;
  if (html !== lastToastHtml) {
    toastMount.innerHTML = html;
    lastToastHtml = html;
  }
}

function render(): void {
  syncDocumentAttributes();
  const copy = getCopy(currentLang());
  renderChromeBar(copy);
  renderMainStage(copy);
  renderLogsModal(copy);
  renderToast();
}

async function refreshStageLogsIfNeeded(): Promise<void> {
  if (isTauri) return;
  const target = state.status?.activeTarget ?? "hub";
  if (target === "hub") return;
  try {
    state.stageLogs = await getWorkspaceLogs(target);
    render();
  } catch {
    // Non-fatal in preview
  }
}

async function loadInitial(): Promise<void> {
  try {
    const [status, today] = await Promise.all([
      getDesktopStatus(),
      getTodayOverview().catch(() => null),
    ]);
    state.status = status;
    if (today) state.today = today;
    state.error = null;
    render();
    await refreshStageLogsIfNeeded();
  } catch (err) {
    state.error = err instanceof Error ? err.message : String(err);
    render();
  }
}

async function handleActivate(target: TargetId, route?: string): Promise<void> {
  if (target !== "hub" && route) {
    state.activeRoutes[target] = sanitizeRoute(route);
  }
  state.busyKey = target === "hub" ? "hub" : `ws:${target}:activate`;
  render();
  try {
    let next = await activateTarget(target, route);
    if (target !== "hub") {
      const ws = next.workspaces.find((w) => w.id === target);
      if (ws && ws.installed && (ws.state === "stopped" || ws.state === "error")) {
        next = await workspaceAction(target, "start");
      }
    }
    state.status = next;
    state.error = null;
    state.busyKey = null;
    render();
    await refreshStageLogsIfNeeded();
  } catch (err) {
    state.busyKey = null;
    render();
    showToast(
      {
        kind: "error",
        title: getCopy(currentLang()).errorTitle,
        detail: err instanceof Error ? err.message : String(err),
      },
      5000,
    );
  }
}

async function handleWorkspaceOp(id: AppId, op: WorkspaceAction): Promise<void> {
  state.busyKey = `ws:${id}:${op}`;
  render();
  try {
    state.status = await workspaceAction(id, op);
    state.busyKey = null;
    render();
    if (op !== "stop") {
      getTodayOverview()
        .then((t) => {
          state.today = t;
          render();
        })
        .catch(() => {});
    }
    await refreshStageLogsIfNeeded();
  } catch (err) {
    state.busyKey = null;
    render();
    showToast(
      {
        kind: "error",
        title: `${PRODUCTS[id].displayName}`,
        detail: err instanceof Error ? err.message : String(err),
      },
      5000,
    );
  }
}

// Centralized delegated click handler
appRoot.addEventListener("click", (ev) => {
  const targetEl = (ev.target as HTMLElement | null)?.closest<HTMLElement>("[data-action]");
  if (!targetEl) return;
  const action = targetEl.dataset.action;

  if (action === "close-logs-backdrop" && ev.target !== targetEl) {
    return;
  }

  switch (action) {
    case "activate": {
      const t = targetEl.dataset.target as TargetId | undefined;
      if (t) void handleActivate(t);
      break;
    }
    case "jump-today": {
      const t = targetEl.dataset.target as TargetId | undefined;
      const r = targetEl.dataset.route;
      if (t) void handleActivate(t, r);
      break;
    }
    case "select-stage-route": {
      const ws = targetEl.dataset.ws as AppId | undefined;
      const r = targetEl.dataset.route;
      if (ws && r) void handleActivate(ws, r);
      break;
    }
    case "ws-op": {
      const ws = targetEl.dataset.ws as AppId | undefined;
      const op = targetEl.dataset.op as WorkspaceAction | undefined;
      if (ws && op) void handleWorkspaceOp(ws, op);
      break;
    }
    case "start-all": {
      state.busyKey = "start-all";
      render();
      void startAllWorkspaces()
        .then(async (st) => {
          state.status = st;
          state.busyKey = null;
          render();
          const t = await getTodayOverview().catch(() => null);
          if (t) {
            state.today = t;
            render();
          }
        })
        .catch((err) => {
          state.busyKey = null;
          render();
          showToast({
            kind: "error",
            title: getCopy(currentLang()).errorTitle,
            detail: err instanceof Error ? err.message : String(err),
          });
        });
      break;
    }
    case "set-lang": {
      const nextLang = normalizeLang(targetEl.dataset.lang);
      state.langOverride = nextLang;
      render();
      void savePrefs({ lang: nextLang })
        .then((st) => {
          state.status = st;
          state.langOverride = null;
          render();
        })
        .catch(() => {});
      break;
    }
    case "set-theme": {
      const raw = targetEl.dataset.themePref;
      if (raw === "light" || raw === "dark" || raw === "system") {
        state.themeOverride = raw;
        render();
        void savePrefs({ theme: raw })
          .then((st) => {
            state.status = st;
            state.themeOverride = null;
            render();
          })
          .catch(() => {});
      }
      break;
    }
    case "toggle-autostart": {
      const nextVal = !(state.status?.prefs.autoStart ?? false);
      if (state.status) state.status.prefs.autoStart = nextVal;
      render();
      void savePrefs({ autoStart: nextVal })
        .then((st) => {
          state.status = st;
          render();
        })
        .catch(() => {});
      break;
    }
    case "filter-today": {
      const f = targetEl.dataset.filter as "all" | TodayItemState | undefined;
      if (f) {
        state.todayFilter = f;
        render();
      }
      break;
    }
    case "refresh-today": {
      void getTodayOverview()
        .then((t) => {
          state.today = t;
          render();
        })
        .catch(() => {});
      break;
    }
    case "backup-vault": {
      state.busyKey = "backup-vault";
      render();
      void backupVault()
        .then(async (res) => {
          const count = Array.isArray(res.files) ? res.files.length : Number(res.files) || 0;
          const copy = getCopy(currentLang());
          state.busyKey = null;
          showToast({
            kind: "ok",
            title: copy.backupToastTitle,
            detail: copy.backupToastDetail(res.backupDir, count, formatBytes(res.totalBytes)),
          });
          const next = await getDesktopStatus().catch(() => null);
          if (next) state.status = next;
          render();
        })
        .catch((err) => {
          state.busyKey = null;
          render();
          showToast({
            kind: "error",
            title: getCopy(currentLang()).errorTitle,
            detail: err instanceof Error ? err.message : String(err),
          });
        });
      break;
    }
    case "copy-vault-path": {
      const rootDir = state.status?.vault.rootDir;
      if (rootDir) void copyText(rootDir, "vault-path");
      break;
    }
    case "open-logs":
    case "refresh-modal-logs": {
      const ws = targetEl.dataset.ws as AppId | undefined;
      if (ws) {
        state.logsModalApp = ws;
        render();
        void getWorkspaceLogs(ws)
          .then((logs) => {
            if (state.logsModalApp === ws) {
              state.logsModalData = logs;
              render();
            }
          })
          .catch((err) => {
            state.logsModalData = {
              id: ws,
              lines: [err instanceof Error ? err.message : String(err)],
            };
            render();
          });
      }
      break;
    }
    case "copy-modal-logs": {
      const text = (state.logsModalData?.lines ?? []).join("\n");
      void copyText(text, "modal-logs");
      break;
    }
    case "close-logs":
    case "close-logs-backdrop": {
      state.logsModalApp = null;
      state.logsModalData = null;
      render();
      break;
    }
    case "refresh-stage-logs": {
      void refreshStageLogsIfNeeded();
      break;
    }
    case "copy-stage-logs": {
      const text = (state.stageLogs?.lines ?? []).join("\n");
      void copyText(text, "stage-logs");
      break;
    }
    case "dismiss-toast": {
      state.toast = null;
      renderToast();
      break;
    }
    case "retry-all": {
      void loadInitial();
      break;
    }
    case "win": {
      const w = targetEl.dataset.win as WindowAction | undefined;
      if (w) void windowAction(w);
      break;
    }
    default:
      break;
  }
});

appRoot.addEventListener("change", (ev) => {
  const selectEl = (ev.target as HTMLElement | null)?.closest<HTMLSelectElement>(
    "[data-action='suite-select']",
  );
  if (!selectEl) return;
  const val = selectEl.value as TargetId;
  selectEl.selectedIndex = 0;
  if (val) {
    void handleActivate(val);
  }
});

// Keyboard shortcuts: Cmd/Ctrl + 0..5 & Escape
window.addEventListener("keydown", (ev) => {
  if (ev.key === "Escape" && state.logsModalApp) {
    state.logsModalApp = null;
    state.logsModalData = null;
    render();
    return;
  }
  if (!(ev.metaKey || ev.ctrlKey) || ev.altKey || ev.shiftKey) return;
  const tag = (ev.target as HTMLElement | null)?.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA") return;

  if (ev.key === "0") {
    ev.preventDefault();
    void handleActivate("hub");
    return;
  }
  const idx = Number(ev.key);
  if (idx >= 1 && idx <= 5) {
    const appId = WORKSPACE_ORDER[idx - 1];
    if (appId) {
      ev.preventDefault();
      void handleActivate(appId);
    }
  }
});

darkMedia.addEventListener("change", () => {
  if (currentThemePref() === "system") {
    render();
  }
});

async function pollNow(): Promise<void> {
  try {
    const st = await getDesktopStatus();
    state.status = st;
    state.error = null;
    render();
    if ((st.activeTarget ?? "hub") === "hub") {
      const t = await getTodayOverview().catch(() => null);
      if (t) {
        state.today = t;
        render();
      }
    } else {
      await refreshStageLogsIfNeeded();
    }
  } catch {
    // Keep current UI state on transient poll error
  }
}

window.addEventListener("bell:sync", () => {
  void pollNow();
});

// Background status & Today feed polling
window.setInterval(() => {
  if (document.hidden || state.busyKey) return;
  void getDesktopStatus()
    .then((st) => {
      state.status = st;
      state.error = null;
      render();
    })
    .catch(() => {});
}, 3500);

window.setInterval(() => {
  if (document.hidden || (state.status?.activeTarget ?? "hub") !== "hub") return;
  void getTodayOverview()
    .then((t) => {
      state.today = t;
      render();
    })
    .catch(() => {});
}, 15000);

render();
void loadInitial();
