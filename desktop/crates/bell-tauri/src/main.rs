#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use bell_core::{
    AppId, DesktopStatus, Edition, PrefsPatch, Supervisor, SupervisorConfig, TodayOverview,
    VaultBackupResult, WorkspaceLogs, WorkspaceState,
};
use bell_hub::{HubRenderModel, HubUiClientState, HubViewPatch};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::{
    menu::{Menu, MenuBuilder, MenuItem, MenuItemBuilder, Submenu, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::NewWindowResponse,
    AppHandle, Manager, RunEvent, State, Theme, WebviewUrl, Wry,
};
use url::Url;

const DEFAULT_WIDTH: f64 = 1360.0;
const DEFAULT_HEIGHT: f64 = 860.0;
const MIN_WIDTH: f64 = 900.0;
const MIN_HEIGHT: f64 = 620.0;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopRenderReport {
    #[serde(default)]
    chrome_height: u32,
    #[serde(default)]
    main_height: u32,
    #[serde(default)]
    text_length: usize,
    #[serde(default)]
    theme: String,
}

struct SelftestCoordinator {
    out_path: PathBuf,
    dom_report: Mutex<Option<DesktopRenderReport>>,
    autostart_done: AtomicBool,
    finished: AtomicBool,
}

struct AppState {
    supervisor: Arc<Supervisor>,
    hub_url: Mutex<Url>,
    initial_pr_port: u16,
    selftest: Option<Arc<SelftestCoordinator>>,
}

fn default_hub_url() -> Url {
    if cfg!(target_os = "windows") {
        Url::parse("http://tauri.localhost/index.html").expect("valid windows hub url")
    } else {
        Url::parse("tauri://localhost/index.html").expect("valid unix hub url")
    }
}

fn parse_tauri_theme(theme_pref: &str) -> Option<Theme> {
    if theme_pref.eq_ignore_ascii_case("dark") {
        Some(Theme::Dark)
    } else if theme_pref.eq_ignore_ascii_case("light") {
        Some(Theme::Light)
    } else {
        None
    }
}

fn build_pr_origin_guarded_init_script(token: &str, pr_port: u16) -> String {
    let origin_127 = format!("http://127.0.0.1:{pr_port}");
    let origin_local = format!("http://localhost:{pr_port}");
    format!(
        "(function(){{if(window.location&&(window.location.origin==={origin_127:?}||window.location.origin==={origin_local:?})){{try{{sessionStorage.setItem('bellpr-api-token',{token:?});}}catch(_){{}}}}}})();"
    )
}

fn sanitize_hash_route(route: Option<&str>) -> String {
    let Some(raw) = route else {
        return String::new();
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_control) {
        return String::new();
    }
    if trimmed.starts_with('#') {
        trimmed.to_string()
    } else if trimmed.starts_with('/') {
        format!("#{trimmed}")
    } else {
        format!("#/{trimmed}")
    }
}

fn show_and_focus_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn open_external_browser(url_str: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("/usr/bin/open")
            .arg(url_str)
            .spawn();
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", url_str])
            .creation_flags(0x0800_0000)
            .spawn();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(url_str)
            .spawn();
    }
}

fn notify_hub_sync(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let is_remote_workspace = window
            .url()
            .ok()
            .and_then(|u| u.host_str().map(|h| h == "127.0.0.1" || h == "localhost"))
            .unwrap_or(false);
        if !is_remote_workspace {
            let _ = window.eval("window.dispatchEvent(new CustomEvent('bell:sync'))");
        }
    }
}

fn navigate_to_hub(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let (hub_url, title) = if let Some(state) = app.try_state::<AppState>() {
        let url = state
            .hub_url
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        (url, state.supervisor.edition().display_name().to_string())
    } else {
        (default_hub_url(), Edition::active().display_name().to_string())
    };
    let _ = window.set_title(&title);
    let _ = window.navigate(hub_url);
}

fn navigate_to_workspace(app: &AppHandle, app_id: AppId, route: Option<&str>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };

    let status = state.supervisor.status();
    let Some(ws) = status
        .workspaces
        .iter()
        .find(|w| w.id == app_id && w.state == WorkspaceState::Running)
    else {
        return;
    };

    let hash_suffix = sanitize_hash_route(route);
    let full_url_str = if app_id == AppId::Pr
        && ws.port != state.initial_pr_port
        && hash_suffix.is_empty()
    {
        if let Some(tok) = state.supervisor.pr_api_token() {
            format!("http://127.0.0.1:{}/#bell_token={tok}", ws.port)
        } else {
            format!("http://127.0.0.1:{}/", ws.port)
        }
    } else {
        format!("http://127.0.0.1:{}/{hash_suffix}", ws.port)
    };

    if let Ok(parsed) = Url::parse(&full_url_str) {
        let _ = window.set_title(app_id.display_name());
        let _ = window.navigate(parsed);
    }
}

fn try_complete_selftest(app: &AppHandle, force_timeout: bool) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let Some(selftest) = state.selftest.clone() else {
        return;
    };

    let dom_opt = selftest
        .dom_report
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let autostart_done = selftest.autostart_done.load(Ordering::SeqCst);

    if !force_timeout && (dom_opt.is_none() || !autostart_done) {
        return;
    }
    if selftest.finished.swap(true, Ordering::SeqCst) {
        return;
    }

    let status = state.supervisor.status();
    let installed_count = status.workspaces.iter().filter(|w| w.installed).count();
    let healthy_running_count = status
        .workspaces
        .iter()
        .filter(|w| w.installed && w.state == WorkspaceState::Running && w.manifest_valid)
        .count();

    let dom_ok = dom_opt.as_ref().is_some_and(|d| {
        d.text_length >= 10 && d.main_height >= 36 && d.chrome_height >= 36
    });
    let sidecars_ok = installed_count > 0 && healthy_running_count == installed_count;
    let all_ok = dom_ok && sidecars_ok && !force_timeout;

    let payload = serde_json::json!({
        "ok": all_ok,
        "timedOut": force_timeout,
        "edition": status.edition,
        "version": status.version,
        "os": status.os,
        "arch": status.arch,
        "domReady": dom_opt,
        "autostartDone": autostart_done,
        "installedWorkspaces": installed_count,
        "healthyRunningWorkspaces": healthy_running_count,
        "workspaces": status.workspaces,
    });

    let out_path = selftest.out_path.clone();
    let sup = state.supervisor.clone();
    let handle = app.clone();

    tauri::async_runtime::spawn(async move {
        if let Some(parent) = out_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(serialized) = serde_json::to_string_pretty(&payload) {
            let _ = std::fs::write(&out_path, format!("{serialized}\n"));
        }
        sup.shutdown_all().await;
        let code = if all_ok { 0 } else { 1 };
        handle.exit(code);
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        std::process::exit(code);
    });
}

#[tauri::command]
async fn desktop_hub_render(
    state: State<'_, AppState>,
    ui: Option<HubUiClientState>,
) -> Result<HubViewPatch, String> {
    let ui = ui.unwrap_or_default();
    let status = state.supervisor.status();
    let need_today = matches!(ui.view.as_str(), "" | "overview" | "today") || ui.palette_open;
    let today_val = if need_today {
        Some(state.supervisor.today().await)
    } else {
        None
    };
    let logs_val = if ui.view == "logs" {
        let logs_app = ui
            .logs_app
            .unwrap_or_else(|| status.edition.primary_app().unwrap_or(AppId::Personal));
        Some(state.supervisor.logs(logs_app))
    } else {
        None
    };

    Ok(bell_hub::render_hub_patch(&HubRenderModel {
        status: &status,
        today: today_val.as_ref(),
        logs: logs_val.as_ref(),
        ui: &ui,
    }))
}

#[tauri::command]
async fn desktop_status(state: State<'_, AppState>) -> Result<DesktopStatus, String> {
    Ok(state.supervisor.status())
}

#[tauri::command]
async fn desktop_today(state: State<'_, AppState>) -> Result<TodayOverview, String> {
    Ok(state.supervisor.today().await)
}

#[tauri::command]
async fn desktop_activate(
    app: AppHandle,
    state: State<'_, AppState>,
    target: String,
    route: Option<String>,
) -> Result<DesktopStatus, String> {
    let norm = target.trim().to_ascii_lowercase();
    if norm == "hub" {
        let status = state
            .supervisor
            .activate("hub")
            .map_err(|e| e.to_string())?;
        navigate_to_hub(&app);
        return Ok(status);
    }

    let app_id: AppId = norm.parse()?;
    let needs_start = state
        .supervisor
        .status()
        .workspaces
        .iter()
        .find(|w| w.id == app_id)
        .is_some_and(|w| {
            w.installed && matches!(w.state, WorkspaceState::Stopped | WorkspaceState::Error)
        });
    if needs_start {
        state
            .supervisor
            .start_workspace(app_id)
            .await
            .map_err(|e| e.to_string())?;
    }

    let status = state
        .supervisor
        .activate(app_id.as_str())
        .map_err(|e| e.to_string())?;
    navigate_to_workspace(&app, app_id, route.as_deref());
    Ok(status)
}

#[tauri::command]
async fn desktop_workspace_action(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    action: String,
) -> Result<DesktopStatus, String> {
    let app_id: AppId = id.parse()?;
    let status = state
        .supervisor
        .workspace_action(app_id, &action)
        .await
        .map_err(|e| e.to_string())?;
    notify_hub_sync(&app);
    Ok(status)
}

#[tauri::command]
async fn desktop_start_all(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DesktopStatus, String> {
    let status = state
        .supervisor
        .start_all()
        .await
        .map_err(|e| e.to_string())?;
    notify_hub_sync(&app);
    Ok(status)
}

#[tauri::command]
async fn desktop_save_prefs(
    app: AppHandle,
    state: State<'_, AppState>,
    theme: Option<String>,
    lang: Option<String>,
    auto_start: Option<bool>,
) -> Result<DesktopStatus, String> {
    let updated = state
        .supervisor
        .save_prefs(PrefsPatch {
            theme,
            lang,
            active_target: None,
            auto_start,
        })
        .map_err(|e| e.to_string())?;

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_theme(parse_tauri_theme(&updated.prefs.theme));
    }
    notify_hub_sync(&app);
    Ok(updated)
}

#[tauri::command]
async fn desktop_vault_backup(state: State<'_, AppState>) -> Result<VaultBackupResult, String> {
    state.supervisor.backup_vault().map_err(|e| e.to_string())
}

#[tauri::command]
async fn desktop_logs(state: State<'_, AppState>, id: String) -> Result<WorkspaceLogs, String> {
    let app_id: AppId = id.parse()?;
    Ok(state.supervisor.logs(app_id))
}

#[tauri::command]
async fn desktop_window_action(app: AppHandle, action: String) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    match action.as_str() {
        "minimize" => window.minimize().map_err(|e| e.to_string()),
        "toggleMaximize" => {
            if window.is_maximized().unwrap_or(false) {
                window.unmaximize().map_err(|e| e.to_string())
            } else {
                window.maximize().map_err(|e| e.to_string())
            }
        }
        "close" => window.close().map_err(|e| e.to_string()),
        other => Err(format!("unknown window action '{other}'")),
    }
}

#[tauri::command]
async fn desktop_report_ready(
    app: AppHandle,
    state: State<'_, AppState>,
    report: DesktopRenderReport,
) -> Result<(), String> {
    if let Some(ref selftest) = state.selftest {
        *selftest
            .dom_report
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(report);
        try_complete_selftest(&app, false);
    }
    Ok(())
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItem::with_id(app, "show", "Show BELL", true, None::<&str>)?;
    let hub_item = MenuItem::with_id(app, "hub", "BELL Hub", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit BELL", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &hub_item, &quit_item])?;

    let mut tray_builder = TrayIconBuilder::with_id("bell-tray")
        .menu(&menu)
        .tooltip(Edition::active().display_name())
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_and_focus_main(app),
            "hub" => {
                if let Some(st) = app.try_state::<AppState>() {
                    let _ = st.supervisor.activate("hub");
                }
                navigate_to_hub(app);
                show_and_focus_main(app);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_and_focus_main(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray_builder = tray_builder.icon(icon);
    }
    tray_builder.build(app)?;
    Ok(())
}

fn build_workspaces_submenu(app: &tauri::App) -> Result<Submenu<Wry>, tauri::Error> {
    let hub = MenuItemBuilder::with_id("nav:hub", "BELL Hub")
        .accelerator("CmdOrCtrl+0")
        .build(app)?;
    let personal = MenuItemBuilder::with_id("nav:personal", "BELL Personal")
        .accelerator("CmdOrCtrl+1")
        .build(app)?;
    let explain = MenuItemBuilder::with_id("nav:explain", "BELL Explain")
        .accelerator("CmdOrCtrl+2")
        .build(app)?;
    let pr = MenuItemBuilder::with_id("nav:pr", "BELL PR")
        .accelerator("CmdOrCtrl+3")
        .build(app)?;
    let reader = MenuItemBuilder::with_id("nav:reader", "BELL Web Reader")
        .accelerator("CmdOrCtrl+4")
        .build(app)?;
    let campaign = MenuItemBuilder::with_id("nav:campaign", "BELL Campaign")
        .accelerator("CmdOrCtrl+5")
        .build(app)?;

    SubmenuBuilder::new(app, "Workspaces")
        .item(&hub)
        .separator()
        .item(&personal)
        .item(&explain)
        .item(&pr)
        .item(&reader)
        .item(&campaign)
        .build()
}

#[cfg(target_os = "macos")]
fn setup_app_menu(app: &tauri::App, title: &str) -> Result<(), Box<dyn std::error::Error>> {
    let app_submenu = SubmenuBuilder::new(app, title)
        .about(None)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let edit_submenu = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let workspaces_submenu = build_workspaces_submenu(app)?;

    let window_submenu = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .separator()
        .close_window()
        .build()?;

    let menu = MenuBuilder::new(app)
        .items(&[
            &app_submenu,
            &edit_submenu,
            &workspaces_submenu,
            &window_submenu,
        ])
        .build()?;
    app.set_menu(menu)?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn setup_app_menu(app: &tauri::App, _title: &str) -> Result<(), Box<dyn std::error::Error>> {
    let workspaces_submenu = build_workspaces_submenu(app)?;
    let menu = MenuBuilder::new(app).items(&[&workspaces_submenu]).build()?;
    app.set_menu(menu)?;
    Ok(())
}

fn main() {
    let edition = Edition::active();
    let selftest_out = std::env::var("BELL_DESKTOP_SELFTEST_OUT")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);

    let builder = tauri::Builder::default();

    #[cfg(target_os = "macos")]
    let builder = builder.on_web_content_process_terminate(|webview| {
        let _ = webview.reload();
    });

    let app = builder
        .on_menu_event(|app_handle, event| {
            if let Some(target) = event.id().as_ref().strip_prefix("nav:") {
                let app_bg = app_handle.clone();
                let target_owned = target.to_string();
                tauri::async_runtime::spawn(async move {
                    let Some(st) = app_bg.try_state::<AppState>() else {
                        return;
                    };
                    if target_owned == "hub" {
                        let _ = st.supervisor.activate("hub");
                        navigate_to_hub(&app_bg);
                        show_and_focus_main(&app_bg);
                        return;
                    }
                    let Ok(app_id) = target_owned.parse::<AppId>() else {
                        return;
                    };
                    let needs_start = st
                        .supervisor
                        .status()
                        .workspaces
                        .iter()
                        .find(|w| w.id == app_id)
                        .is_some_and(|w| {
                            w.installed
                                && matches!(w.state, WorkspaceState::Stopped | WorkspaceState::Error)
                        });
                    if needs_start {
                        let _ = st.supervisor.start_workspace(app_id).await;
                    }
                    let _ = st.supervisor.activate(app_id.as_str());
                    navigate_to_workspace(&app_bg, app_id, None);
                    show_and_focus_main(&app_bg);
                });
            }
        })
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Regular);

            let resource_dir = app.path().resource_dir().ok();
            let supervisor = Supervisor::new(SupervisorConfig {
                edition,
                resource_dir,
                ..Default::default()
            })?;

            // Always greet BELL Suite with BELL Hub on fresh app launch
            if edition == Edition::Suite {
                let _ = supervisor.activate("hub");
            }

            // Pre-allocate ports & PR bearer token BEFORE building WebviewWindow so
            // origin-guarded initialization_script always knows PR's port and secret.
            supervisor.preallocate_all_ports();
            let pr_token = supervisor.ensure_pr_api_token().unwrap_or_default();
            let initial_status = supervisor.status();
            let initial_pr_port = initial_status
                .workspaces
                .iter()
                .find(|w| w.id == AppId::Pr)
                .map(|w| w.port)
                .unwrap_or_else(|| AppId::Pr.default_port());

            let selftest = selftest_out.map(|out_path| {
                Arc::new(SelftestCoordinator {
                    out_path,
                    dom_report: Mutex::new(None),
                    autostart_done: AtomicBool::new(false),
                    finished: AtomicBool::new(false),
                })
            });

            setup_app_menu(app, edition.display_name())?;
            setup_tray(app)?;

            let pr_init_js = build_pr_origin_guarded_init_script(&pr_token, initial_pr_port);
            let nav_handle = app.handle().clone();
            let sup_for_new_win = supervisor.clone();

            let mut win_builder = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::App("index.html".into()),
            )
            .title(edition.display_name())
            .inner_size(DEFAULT_WIDTH, DEFAULT_HEIGHT)
            .min_inner_size(MIN_WIDTH, MIN_HEIGHT)
            .center()
            .visible(true)
            .focused(true)
            .theme(parse_tauri_theme(&initial_status.prefs.theme))
            .on_new_window(move |url, _features| {
                let is_workspace_url = (url.scheme() == "http" || url.scheme() == "https")
                    && matches!(url.host_str(), Some("127.0.0.1" | "localhost"))
                    && url.port().is_some_and(|p| {
                        sup_for_new_win
                            .status()
                            .workspaces
                            .iter()
                            .any(|w| w.port == p)
                    });
                if is_workspace_url {
                    let h = nav_handle.clone();
                    let target_url = url.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Some(win) = h.get_webview_window("main") {
                            let _ = win.navigate(target_url);
                        }
                    });
                } else if matches!(url.scheme(), "http" | "https") {
                    open_external_browser(url.as_str());
                }
                NewWindowResponse::Deny
            });

            if !pr_token.is_empty() {
                win_builder = win_builder.initialization_script(&pr_init_js);
            }

            let window = win_builder.build()?;

            #[cfg(target_os = "macos")]
            {
                let win_for_close = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = win_for_close.hide();
                    }
                });
            }

            #[cfg(target_os = "windows")]
            {
                let _ = window.hide_menu();
            }

            let captured_hub_url = window.url().unwrap_or_else(|_| default_hub_url());
            app.manage(AppState {
                supervisor: supervisor.clone(),
                hub_url: Mutex::new(captured_hub_url),
                initial_pr_port,
                selftest: selftest.clone(),
            });

            #[cfg(target_os = "macos")]
            {
                let _ = app.handle().show();
            }
            let _ = window.show();
            let _ = window.set_focus();

            let should_start = initial_status.prefs.auto_start || selftest.is_some();
            if should_start {
                let sup_bg = supervisor.clone();
                let handle_bg = app.handle().clone();
                let selftest_bg = selftest.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = sup_bg.start_all().await;
                    if let Some(primary) = edition.primary_app() {
                        if selftest_bg.is_none() {
                            navigate_to_workspace(&handle_bg, primary, None);
                        }
                    } else {
                        notify_hub_sync(&handle_bg);
                    }
                    if let Some(st) = selftest_bg {
                        st.autostart_done.store(true, Ordering::SeqCst);
                        try_complete_selftest(&handle_bg, false);
                    }
                });
            } else if let Some(ref st) = selftest {
                st.autostart_done.store(true, Ordering::SeqCst);
            }

            if selftest.is_some() {
                let watchdog_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(40)).await;
                    try_complete_selftest(&watchdog_handle, true);
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            desktop_hub_render,
            desktop_status,
            desktop_today,
            desktop_activate,
            desktop_workspace_action,
            desktop_start_all,
            desktop_save_prefs,
            desktop_vault_backup,
            desktop_logs,
            desktop_window_action,
            desktop_report_ready,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build BELL Desktop application");

    app.run(|app_handle, event| match event {
        RunEvent::Ready => {
            #[cfg(target_os = "macos")]
            {
                let _ = app_handle.show();
            }
            show_and_focus_main(app_handle);
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => {
            let _ = app_handle.show();
            show_and_focus_main(app_handle);
        }
        RunEvent::Exit => {
            if let Some(state) = app_handle.try_state::<AppState>() {
                let sup = state.supervisor.clone();
                tauri::async_runtime::block_on(async move {
                    sup.shutdown_all().await;
                });
            }
        }
        _ => {}
    });
}
