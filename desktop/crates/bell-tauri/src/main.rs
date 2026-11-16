#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use bell_core::{
    AppId, DesktopStatus, Edition, PrefsPatch, Supervisor, SupervisorConfig, TodayOverview,
    VaultBackupResult, WorkspaceLogs, WorkspaceState,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuBuilder, MenuItem, MenuItemBuilder, Submenu, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, LogicalPosition, LogicalSize, Manager, RunEvent, State, WebviewUrl, WindowEvent,
    Wry,
};
use url::Url;

const CHROME_BAR_HEIGHT: f64 = 44.0;
const DEFAULT_WIDTH: f64 = 1360.0;
const DEFAULT_HEIGHT: f64 = 860.0;
const MIN_WIDTH: f64 = 900.0;
const MIN_HEIGHT: f64 = 620.0;

struct AppState {
    supervisor: Arc<Supervisor>,
    webview_ports: Mutex<BTreeMap<AppId, u16>>,
}

fn window_logical_size(window: &tauri::Window) -> (f64, f64) {
    let scale = window.scale_factor().unwrap_or(1.0);
    if let Ok(phys) = window.inner_size() {
        let logical = phys.to_logical::<f64>(scale);
        (logical.width.max(320.0), logical.height.max(200.0))
    } else {
        (DEFAULT_WIDTH, DEFAULT_HEIGHT)
    }
}

fn show_and_focus_main(app: &AppHandle) {
    if let Some(window) = app.get_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn pr_token_init_script(token: &str) -> String {
    format!("try {{ sessionStorage.setItem('bellpr-api-token', {token:?}); }} catch {{}}")
}

fn resize_webview_bounds(app: &AppHandle) {
    let Some(window) = app.get_window("main") else {
        return;
    };
    let Some(chrome) = app.get_webview("chrome") else {
        return;
    };
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };

    let (width, height) = window_logical_size(&window);
    let status = state.supervisor.status();

    let active_running = status
        .active_target
        .parse::<AppId>()
        .ok()
        .and_then(|target_id| {
            status
                .workspaces
                .iter()
                .find(|w| w.id == target_id && w.state == WorkspaceState::Running)
                .map(|_| target_id)
        });

    if let Some(target_id) = active_running {
        let _ = chrome.set_position(LogicalPosition::new(0.0, 0.0));
        let _ = chrome.set_size(LogicalSize::new(width, CHROME_BAR_HEIGHT));
        if let Some(wv) = app.get_webview(&format!("ws-{}", target_id.as_str())) {
            let ws_height = (height - CHROME_BAR_HEIGHT).max(100.0);
            let _ = wv.set_position(LogicalPosition::new(0.0, CHROME_BAR_HEIGHT));
            let _ = wv.set_size(LogicalSize::new(width, ws_height));
        }
    } else {
        let _ = chrome.set_position(LogicalPosition::new(0.0, 0.0));
        let _ = chrome.set_size(LogicalSize::new(width, height));
    }
}

fn sync_webview_layout(app: &AppHandle, route: Option<&str>) {
    let Some(window) = app.get_window("main") else {
        return;
    };
    let Some(chrome) = app.get_webview("chrome") else {
        return;
    };
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };

    let (width, height) = window_logical_size(&window);
    let status = state.supervisor.status();

    let active_running = status
        .active_target
        .parse::<AppId>()
        .ok()
        .and_then(|target_id| {
            status
                .workspaces
                .iter()
                .find(|w| w.id == target_id && w.state == WorkspaceState::Running)
                .map(|w| (target_id, w.port))
        });

    if let Some((target_id, port)) = active_running {
        for id in AppId::ALL {
            if id != target_id {
                if let Some(wv) = app.get_webview(&format!("ws-{}", id.as_str())) {
                    let _ = wv.hide();
                }
            }
        }

        let _ = chrome.set_position(LogicalPosition::new(0.0, 0.0));
        let _ = chrome.set_size(LogicalSize::new(width, CHROME_BAR_HEIGHT));

        let ws_height = (height - CHROME_BAR_HEIGHT).max(100.0);
        let hash_suffix = match route {
            Some(r) if !r.trim().is_empty() => {
                let trimmed = r.trim();
                if trimmed.starts_with('#') {
                    trimmed.to_string()
                } else {
                    format!("#{trimmed}")
                }
            }
            _ => String::new(),
        };
        let full_url_str = format!("http://127.0.0.1:{port}/{hash_suffix}");
        let label = format!("ws-{}", target_id.as_str());

        if let Some(wv) = app.get_webview(&label) {
            let prev_port = state
                .webview_ports
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&target_id)
                .copied();

            if prev_port != Some(port) {
                if let Ok(parsed) = Url::parse(&full_url_str) {
                    let _ = wv.navigate(parsed);
                    state
                        .webview_ports
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(target_id, port);
                }
                if target_id == AppId::Pr {
                    if let Some(tok) = state.supervisor.pr_api_token() {
                        let _ = wv.eval(pr_token_init_script(&tok));
                    }
                }
            } else if !hash_suffix.is_empty() {
                if let Ok(json_hash) = serde_json::to_string(&hash_suffix) {
                    let _ = wv.eval(format!(
                        "if (window.location.hash !== {json_hash}) {{ window.location.hash = {json_hash}; }}"
                    ));
                }
            }

            let _ = wv.set_position(LogicalPosition::new(0.0, CHROME_BAR_HEIGHT));
            let _ = wv.set_size(LogicalSize::new(width, ws_height));
            let _ = wv.show();
        } else if let Ok(parsed) = Url::parse(&full_url_str) {
            let mut builder =
                tauri::webview::WebviewBuilder::new(&label, WebviewUrl::External(parsed));
            if target_id == AppId::Pr {
                if let Some(tok) = state.supervisor.pr_api_token() {
                    builder = builder.initialization_script(pr_token_init_script(&tok));
                }
            }
            if window
                .add_child(
                    builder,
                    LogicalPosition::new(0.0, CHROME_BAR_HEIGHT),
                    LogicalSize::new(width, ws_height),
                )
                .is_ok()
            {
                state
                    .webview_ports
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(target_id, port);
            }
        }
    } else {
        for id in AppId::ALL {
            if let Some(wv) = app.get_webview(&format!("ws-{}", id.as_str())) {
                let _ = wv.hide();
            }
        }
        let _ = chrome.set_position(LogicalPosition::new(0.0, 0.0));
        let _ = chrome.set_size(LogicalSize::new(width, height));
    }

    let _ = chrome.eval("window.dispatchEvent(new CustomEvent('bell:sync'))");
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
    let status = state
        .supervisor
        .activate(&target)
        .map_err(|e| e.to_string())?;
    sync_webview_layout(&app, route.as_deref());
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
    sync_webview_layout(&app, None);
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
    sync_webview_layout(&app, None);
    Ok(status)
}

#[tauri::command]
async fn desktop_save_prefs(
    state: State<'_, AppState>,
    theme: Option<String>,
    lang: Option<String>,
    auto_start: Option<bool>,
) -> Result<DesktopStatus, String> {
    state
        .supervisor
        .save_prefs(PrefsPatch {
            theme,
            lang,
            active_target: None,
            auto_start,
        })
        .map_err(|e| e.to_string())
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
        .get_window("main")
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

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItem::with_id(app, "show", "Show BELL", true, None::<&str>)?;
    let hub_item = MenuItem::with_id(app, "hub", "Suite Hub", true, None::<&str>)?;
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
                sync_webview_layout(app, None);
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
    let hub = MenuItemBuilder::with_id("nav:hub", "Suite Hub")
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

    let app = tauri::Builder::default()
        .on_menu_event(|app_handle, event| {
            if let Some(target) = event.id().as_ref().strip_prefix("nav:") {
                let app_bg = app_handle.clone();
                let target_owned = target.to_string();
                tauri::async_runtime::spawn(async move {
                    if let Some(st) = app_bg.try_state::<AppState>() {
                        if let Ok(next) = st.supervisor.activate(&target_owned) {
                            if let Ok(app_id) = target_owned.parse::<AppId>() {
                                let needs_start = next
                                    .workspaces
                                    .iter()
                                    .find(|w| w.id == app_id)
                                    .is_some_and(|w| {
                                        w.installed
                                            && matches!(
                                                w.state,
                                                WorkspaceState::Stopped | WorkspaceState::Error
                                            )
                                    });
                                if needs_start {
                                    let _ = st.supervisor.start_workspace(app_id).await;
                                }
                            }
                        }
                        sync_webview_layout(&app_bg, None);
                    }
                });
            }
        })
        .setup(move |app| {
            let resource_dir = app.path().resource_dir().ok();
            let supervisor = Supervisor::new(SupervisorConfig {
                edition,
                resource_dir,
                ..Default::default()
            })?;

            app.manage(AppState {
                supervisor: supervisor.clone(),
                webview_ports: Mutex::new(BTreeMap::new()),
            });

            setup_app_menu(app, edition.display_name())?;
            setup_tray(app)?;

            let mut win_builder = tauri::window::WindowBuilder::new(app, "main")
                .title(edition.display_name())
                .inner_size(DEFAULT_WIDTH, DEFAULT_HEIGHT)
                .min_inner_size(MIN_WIDTH, MIN_HEIGHT);

            #[cfg(target_os = "macos")]
            {
                win_builder = win_builder
                    .title_bar_style(tauri::TitleBarStyle::Overlay)
                    .hidden_title(true);
            }

            #[cfg(target_os = "windows")]
            {
                win_builder = win_builder.decorations(false);
            }

            let window = win_builder.build()?;

            #[cfg(target_os = "windows")]
            {
                let _ = window.hide_menu();
            }

            let chrome_builder = tauri::webview::WebviewBuilder::new(
                "chrome",
                WebviewUrl::App("index.html".into()),
            );
            window.add_child(
                chrome_builder,
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(DEFAULT_WIDTH, DEFAULT_HEIGHT),
            )?;

            let handle_for_resize = app.handle().clone();
            window.on_window_event(move |event| {
                if matches!(
                    event,
                    WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
                ) {
                    resize_webview_bounds(&handle_for_resize);
                }
            });

            if supervisor.status().prefs.auto_start {
                let sup_bg = supervisor.clone();
                let handle_bg = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let _ = sup_bg.start_all().await;
                    sync_webview_layout(&handle_bg, None);
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            desktop_status,
            desktop_today,
            desktop_activate,
            desktop_workspace_action,
            desktop_start_all,
            desktop_save_prefs,
            desktop_vault_backup,
            desktop_logs,
            desktop_window_action,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build BELL Desktop application");

    app.run(|app_handle, event| {
        if let RunEvent::Exit = event {
            if let Some(state) = app_handle.try_state::<AppState>() {
                let sup = state.supervisor.clone();
                tauri::async_runtime::block_on(async move {
                    sup.shutdown_all().await;
                });
            }
        }
    });
}
