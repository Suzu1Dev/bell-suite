use axum::{
    body::Bytes,
    extract::{Path as AxumPath, Query, Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bell_core::{
    AppId, Edition, PrefsPatch, Supervisor, SupervisorConfig, SupervisorError, WorkspaceState,
};
use bell_hub::{HubRenderModel, HubUiClientState};
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Clone)]
pub struct DaemonState {
    pub supervisor: Arc<Supervisor>,
    pub allowed_origins: Arc<Vec<String>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ActivateBody {
    target: String,
    #[allow(dead_code)]
    route: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PrefsBody {
    theme: Option<String>,
    lang: Option<String>,
    auto_start: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct HubRenderQuery {
    demo: Option<String>,
    edition: Option<String>,
    theme: Option<String>,
    lang: Option<String>,
}

fn parse_hub_ui_body(body: &[u8]) -> Result<HubUiClientState, serde_json::Error> {
    if body.is_empty() {
        return Ok(HubUiClientState::default());
    }
    let val: Value = serde_json::from_slice(body)?;
    if let Some(nested) = val.get("ui") {
        if nested.is_object() {
            return serde_json::from_value(nested.clone());
        }
    }
    serde_json::from_value(val)
}

#[derive(Debug, Clone)]
struct CliArgs {
    host: String,
    port: u16,
    base_port: u16,
    edition: Edition,
    vault_dir: Option<PathBuf>,
    sidecars_dir: Option<PathBuf>,
    ui_dir: Option<PathBuf>,
    no_autostart: bool,
    help: bool,
    version: bool,
}

fn parse_cli<I, S>(iter: I) -> Result<CliArgs, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let default_port = std::env::var("BELL_DESKTOP_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(4600);

    let mut out = CliArgs {
        host: "127.0.0.1".to_string(),
        port: default_port,
        base_port: 4600,
        edition: Edition::active(),
        vault_dir: None,
        sidecars_dir: None,
        ui_dir: None,
        no_autostart: false,
        help: false,
        version: false,
    };

    let mut args = iter.into_iter().map(Into::into);
    while let Some(arg) = args.next() {
        let take_val = |flag: &str, inline: Option<&str>, next: Option<String>| -> Result<String, String> {
            if let Some(v) = inline {
                Ok(v.to_string())
            } else {
                next.ok_or_else(|| format!("missing value for {flag}"))
            }
        };

        if arg == "-h" || arg == "--help" {
            out.help = true;
        } else if arg == "-v" || arg == "--version" {
            out.version = true;
        } else if arg == "--no-autostart" {
            out.no_autostart = true;
        } else if arg == "--port" || arg.starts_with("--port=") {
            let v = take_val("--port", arg.strip_prefix("--port="), args.next())?;
            out.port = v
                .parse::<u16>()
                .map_err(|_| format!("invalid --port '{v}'"))?;
        } else if arg == "--base-port" || arg.starts_with("--base-port=") {
            let v = take_val("--base-port", arg.strip_prefix("--base-port="), args.next())?;
            out.base_port = v
                .parse::<u16>()
                .map_err(|_| format!("invalid --base-port '{v}'"))?;
        } else if arg == "--host" || arg.starts_with("--host=") {
            out.host = take_val("--host", arg.strip_prefix("--host="), args.next())?;
        } else if arg == "--edition" || arg.starts_with("--edition=") {
            let v = take_val("--edition", arg.strip_prefix("--edition="), args.next())?;
            out.edition = v.parse::<Edition>()?;
        } else if arg == "--vault-dir" || arg.starts_with("--vault-dir=") {
            let v = take_val("--vault-dir", arg.strip_prefix("--vault-dir="), args.next())?;
            out.vault_dir = Some(PathBuf::from(v));
        } else if arg == "--sidecars-dir" || arg.starts_with("--sidecars-dir=") {
            let v = take_val(
                "--sidecars-dir",
                arg.strip_prefix("--sidecars-dir="),
                args.next(),
            )?;
            out.sidecars_dir = Some(PathBuf::from(v));
        } else if arg == "--ui-dir" || arg.starts_with("--ui-dir=") {
            let v = take_val("--ui-dir", arg.strip_prefix("--ui-dir="), args.next())?;
            out.ui_dir = Some(PathBuf::from(v));
        } else {
            return Err(format!("unknown option '{arg}'"));
        }
    }

    Ok(out)
}

fn print_help() {
    println!(
        "bell-daemon v{} — BELL Desktop Control Server

Usage:
  bell-daemon [options]

Options:
  --port <u16>           Control server port (default: 4600)
  --base-port <u16>      Workspace base port (default: 4600)
  --host <addr>          Bind address (default: 127.0.0.1)
  --edition <id>         suite|personal|explain|pr|reader|campaign
  --vault-dir <path>     Override SQLite vault directory
  --sidecars-dir <path>  Override dist-sidecars directory
  --ui-dir <path>        Override static UI dist directory
  --no-autostart         Do not auto-start workspaces on boot
  -v, --version          Print version
  -h, --help             Show this help",
        env!("CARGO_PKG_VERSION")
    );
}

pub fn build_allowed_origins(host: &str, port: u16) -> Vec<String> {
    let mut list = vec![
        format!("http://127.0.0.1:{port}"),
        format!("http://localhost:{port}"),
    ];
    let bracketed = if host.contains(':') && !host.starts_with('[') {
        format!("http://[{host}]:{port}")
    } else {
        format!("http://{host}:{port}")
    };
    if !list.contains(&bracketed) {
        list.push(bracketed);
    }
    list
}

async fn security_and_origin_middleware(
    State(state): State<DaemonState>,
    req: Request,
    next: Next,
) -> Response {
    if matches!(
        *req.method(),
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    ) {
        let origin_ok = req
            .headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok())
            .map(|origin| state.allowed_origins.iter().any(|allowed| allowed == origin))
            .unwrap_or(false);

        if !origin_ok {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "forbidden origin" })),
            )
                .into_response();
        }
    }

    let mut res = next.run(req).await;
    let headers = res.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self'; object-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'",
        ),
    );
    res
}

fn map_supervisor_err(err: SupervisorError) -> Response {
    let code = match err {
        SupervisorError::InvalidTarget(_) | SupervisorError::InvalidAction(_) => {
            StatusCode::BAD_REQUEST
        }
        SupervisorError::Vault(bell_core::VaultError::InvalidPref(_)) => StatusCode::BAD_REQUEST,
        SupervisorError::NotInstalled(_) => StatusCode::NOT_FOUND,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (code, Json(json!({ "error": err.to_string() }))).into_response()
}

async fn handle_health(State(state): State<DaemonState>) -> impl IntoResponse {
    Json(json!({
        "ok": true,
        "edition": state.supervisor.edition(),
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn handle_status(State(state): State<DaemonState>) -> impl IntoResponse {
    Json(state.supervisor.status())
}

async fn handle_today(State(state): State<DaemonState>) -> impl IntoResponse {
    Json(state.supervisor.today().await)
}

async fn handle_activate(
    State(state): State<DaemonState>,
    body: Bytes,
) -> Response {
    let parsed: ActivateBody = match serde_json::from_slice(&body) {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("invalid json: {e}") })),
            )
                .into_response()
        }
    };
    match state.supervisor.activate(&parsed.target) {
        Ok(st) => Json(st).into_response(),
        Err(e) => map_supervisor_err(e),
    }
}

async fn handle_workspace_action(
    State(state): State<DaemonState>,
    AxumPath((id_raw, action)): AxumPath<(String, String)>,
) -> Response {
    let Ok(app_id) = id_raw.parse::<AppId>() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("unknown workspace '{id_raw}'") })),
        )
            .into_response();
    };
    match state.supervisor.workspace_action(app_id, &action).await {
        Ok(st) => Json(st).into_response(),
        Err(e) => map_supervisor_err(e),
    }
}

async fn handle_start_all(State(state): State<DaemonState>) -> Response {
    match state.supervisor.start_all().await {
        Ok(st) => Json(st).into_response(),
        Err(e) => map_supervisor_err(e),
    }
}

async fn handle_save_prefs(
    State(state): State<DaemonState>,
    body: Bytes,
) -> Response {
    let parsed: PrefsBody = if body.is_empty() {
        PrefsBody::default()
    } else {
        match serde_json::from_slice(&body) {
            Ok(b) => b,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("invalid json: {e}") })),
                )
                    .into_response()
            }
        }
    };
    let patch = PrefsPatch {
        theme: parsed.theme,
        lang: parsed.lang,
        active_target: None,
        auto_start: parsed.auto_start,
    };
    match state.supervisor.save_prefs(patch) {
        Ok(st) => Json(st).into_response(),
        Err(e) => map_supervisor_err(e),
    }
}

async fn handle_vault_backup(State(state): State<DaemonState>) -> Response {
    match state.supervisor.backup_vault() {
        Ok(res) => Json(res).into_response(),
        Err(e) => map_supervisor_err(e),
    }
}

async fn handle_logs(
    State(state): State<DaemonState>,
    AxumPath(id_raw): AxumPath<String>,
) -> Response {
    let Ok(app_id) = id_raw.parse::<AppId>() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("unknown workspace '{id_raw}'") })),
        )
            .into_response();
    };
    Json(state.supervisor.logs(app_id)).into_response()
}

async fn handle_hub_render(
    State(state): State<DaemonState>,
    Query(query): Query<HubRenderQuery>,
    body: Bytes,
) -> Response {
    let ui = match parse_hub_ui_body(&body) {
        Ok(u) => u,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("invalid json: {e}") })),
            )
                .into_response()
        }
    };

    let is_demo = query
        .demo
        .as_deref()
        .is_some_and(|d| d == "1" || d.eq_ignore_ascii_case("true"));

    if is_demo {
        let ed = query
            .edition
            .as_deref()
            .and_then(|s| s.parse::<Edition>().ok())
            .unwrap_or_else(|| state.supervisor.edition());
        let theme = query.theme.as_deref().unwrap_or("dark");
        let lang = query.lang.as_deref().unwrap_or("zh-tw");
        let (demo_status, demo_today, demo_logs) = bell_hub::build_demo_status(ed, theme, lang);
        let patch = bell_hub::render_hub_patch(&HubRenderModel {
            status: &demo_status,
            today: Some(&demo_today),
            logs: Some(&demo_logs),
            ui: &ui,
        });
        return Json(patch).into_response();
    }

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

    let patch = bell_hub::render_hub_patch(&HubRenderModel {
        status: &status,
        today: today_val.as_ref(),
        logs: logs_val.as_ref(),
        ui: &ui,
    });
    Json(patch).into_response()
}

async fn handle_in_memory_hub_css() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/css; charset=utf-8"),
        )],
        bell_hub::HUB_CSS,
    )
}

async fn handle_in_memory_hub_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/javascript; charset=utf-8"),
        )],
        bell_hub::HUB_BOOT_JS,
    )
}

async fn handle_in_memory_hub_index() -> impl IntoResponse {
    Html(bell_hub::render_initial_html())
}

async fn handle_in_memory_favicon_svg() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("image/svg+xml; charset=utf-8"),
        )],
        bell_hub::FAVICON_SVG,
    )
}

async fn handle_in_memory_favicon_ico() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, HeaderValue::from_static("image/x-icon"))],
        bell_hub::build_favicon_ico_bytes(),
    )
}

async fn handle_open_workspace(
    State(state): State<DaemonState>,
    AxumPath(id_raw): AxumPath<String>,
    headers: HeaderMap,
) -> Response {
    if let Some(sfs) = headers.get("sec-fetch-site") {
        let allowed = sfs
            .to_str()
            .ok()
            .map(|v| {
                let norm = v.trim().to_ascii_lowercase();
                norm == "same-origin" || norm == "none"
            })
            .unwrap_or(false);
        if !allowed {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "forbidden cross-site navigation" })),
            )
                .into_response();
        }
    }

    let Ok(app_id) = id_raw.parse::<AppId>() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("unknown workspace '{id_raw}'") })),
        )
            .into_response();
    };
    let status = state.supervisor.status();
    let Some(ws) = status
        .workspaces
        .iter()
        .find(|w| w.id == app_id && w.state == WorkspaceState::Running)
    else {
        return (
            StatusCode::CONFLICT,
            Json(json!({ "error": format!("workspace '{app_id}' is not running") })),
        )
            .into_response();
    };

    let location = if app_id == AppId::Pr {
        if let Some(tok) = state.supervisor.pr_api_token() {
            format!("http://127.0.0.1:{}/#bell_token={tok}", ws.port)
        } else {
            format!("http://127.0.0.1:{}/#/", ws.port)
        }
    } else {
        format!("http://127.0.0.1:{}/#/", ws.port)
    };

    let Ok(loc_val) = HeaderValue::from_str(&location) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    (
        StatusCode::FOUND,
        [
            (header::LOCATION, loc_val),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
            (
                header::REFERRER_POLICY,
                HeaderValue::from_static("no-referrer"),
            ),
        ],
    )
        .into_response()
}

pub fn build_router(state: DaemonState, ui_dir: Option<&Path>) -> Router {
    let api = Router::new()
        .route("/api/health", get(handle_health))
        .route("/api/desktop/status", get(handle_status))
        .route("/api/desktop/today", get(handle_today))
        .route("/api/desktop/hub-render", post(handle_hub_render))
        .route("/api/desktop/open/{id}", get(handle_open_workspace))
        .route("/api/desktop/activate", post(handle_activate))
        .route("/api/desktop/workspaces/start-all", post(handle_start_all))
        .route(
            "/api/desktop/workspaces/{id}/{action}",
            post(handle_workspace_action),
        )
        .route("/api/desktop/prefs", post(handle_save_prefs))
        .route("/api/desktop/vault/backup", post(handle_vault_backup))
        .route("/api/desktop/logs/{id}", get(handle_logs));

    let router = match ui_dir {
        Some(dir) if dir.is_dir() && dir.join("index.html").is_file() => {
            let mut api = if dir.join("hub.css").is_file() {
                api
            } else {
                api.route("/hub.css", get(handle_in_memory_hub_css))
            };
            if !dir.join("favicon.svg").is_file() {
                api = api.route("/favicon.svg", get(handle_in_memory_favicon_svg));
            }
            if !dir.join("favicon.ico").is_file() {
                api = api.route("/favicon.ico", get(handle_in_memory_favicon_ico));
            }
            let index = dir.join("index.html");
            let svc = ServeDir::new(dir).not_found_service(ServeFile::new(index));
            api.fallback_service(svc)
        }
        _ => api
            .route("/", get(handle_in_memory_hub_index))
            .route("/index.html", get(handle_in_memory_hub_index))
            .route("/hub.css", get(handle_in_memory_hub_css))
            .route("/hub.js", get(handle_in_memory_hub_js))
            .route("/favicon.svg", get(handle_in_memory_favicon_svg))
            .route("/favicon.ico", get(handle_in_memory_favicon_ico))
            .fallback(|| async {
                (
                    StatusCode::NOT_FOUND,
                    Json::<Value>(json!({ "error": "not found" })),
                )
            }),
    };

    router
        .layer(middleware::from_fn_with_state(
            state.clone(),
            security_and_origin_middleware,
        ))
        .with_state(state)
}

fn discover_default_ui_dir() -> Option<PathBuf> {
    if let Ok(raw) = std::env::var("BELL_DESKTOP_UI_DIR") {
        let p = PathBuf::from(raw.trim());
        if p.is_dir() {
            return Some(p);
        }
    }
    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("desktop").join("hub-dist"));
        candidates.push(cwd.join("hub-dist"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(p) = exe.parent() {
            candidates.push(p.join("hub-dist"));
            candidates.push(p.join("..").join("Resources").join("hub-dist"));
        }
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("hub-dist"),
    );
    candidates.into_iter().find(|p| p.is_dir())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[tokio::main]
async fn main() {
    let cli = match parse_cli(std::env::args().skip(1)) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("bell-daemon: {err}");
            std::process::exit(2);
        }
    };

    if cli.help {
        print_help();
        return;
    }
    if cli.version {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let supervisor = match Supervisor::new(SupervisorConfig {
        edition: cli.edition,
        base_port: cli.base_port,
        vault_dir: cli.vault_dir,
        sidecars_dir: cli.sidecars_dir,
        resource_dir: None,
        allow_dev_fallback: true,
        ..Default::default()
    }) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("bell-daemon: failed to initialize supervisor: {err}");
            std::process::exit(1);
        }
    };

    let bind_str = if cli.host.contains(':') && !cli.host.starts_with('[') {
        format!("[{}]:{}", cli.host, cli.port)
    } else {
        format!("{}:{}", cli.host, cli.port)
    };
    let addr: SocketAddr = match bind_str.parse() {
        Ok(a) => a,
        Err(err) => {
            eprintln!("bell-daemon: invalid bind address '{bind_str}': {err}");
            std::process::exit(2);
        }
    };

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(err) => {
            eprintln!("bell-daemon: failed to bind {addr}: {err}");
            std::process::exit(1);
        }
    };
    let local_port = listener.local_addr().map(|a| a.port()).unwrap_or(cli.port);

    if !cli.no_autostart && supervisor.status().prefs.auto_start {
        let sup_bg = supervisor.clone();
        tokio::spawn(async move {
            let _ = sup_bg.start_all().await;
        });
    }

    let ui_dir = cli.ui_dir.or_else(discover_default_ui_dir);
    let state = DaemonState {
        supervisor: supervisor.clone(),
        allowed_origins: Arc::new(build_allowed_origins(&cli.host, local_port)),
    };
    let app = build_router(state, ui_dir.as_deref());

    println!(
        "BELL Desktop Daemon ({}) listening on http://{}:{}",
        cli.edition, cli.host, local_port
    );

    if let Err(err) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        eprintln!("bell-daemon: server error: {err}");
    }

    supervisor.shutdown_all().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request as HttpRequest;
    use std::time::Duration;
    use tempfile::TempDir;
    use tower::ServiceExt;

    fn test_router(port: u16) -> (Router, TempDir) {
        let tmp = TempDir::new().unwrap();
        let ui_dir = tmp.path().join("ui");
        std::fs::create_dir_all(&ui_dir).unwrap();
        std::fs::write(ui_dir.join("index.html"), "<!doctype html><title>BELL</title>").unwrap();

        let sup = Supervisor::new(SupervisorConfig {
            edition: Edition::Suite,
            base_port: 4600,
            vault_dir: Some(tmp.path().join("vault")),
            sidecars_dir: Some(tmp.path().join("sidecars")),
            resource_dir: None,
            allow_dev_fallback: false,
            ready_timeout: Duration::from_secs(3),
        })
        .unwrap();

        let state = DaemonState {
            supervisor: sup,
            allowed_origins: Arc::new(build_allowed_origins("127.0.0.1", port)),
        };
        (build_router(state, Some(&ui_dir)), tmp)
    }

    #[tokio::test]
    async fn enforces_strict_origin_check_on_all_post_routes() {
        let (app, _tmp) = test_router(4600);

        // Missing Origin -> 403
        let res_missing = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/api/desktop/activate")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"target":"hub"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_missing.status(), StatusCode::FORBIDDEN);
        let body_missing: Value = serde_json::from_slice(
            &to_bytes(res_missing.into_body(), 4096).await.unwrap(),
        )
        .unwrap();
        assert_eq!(body_missing["error"], "forbidden origin");

        // Cross-origin -> 403
        let res_evil = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/api/desktop/prefs")
                    .header("origin", "https://attacker.example")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"theme":"dark"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_evil.status(), StatusCode::FORBIDDEN);

        // Wrong port -> 403
        let res_wrong_port = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/api/desktop/vault/backup")
                    .header("origin", "http://127.0.0.1:9999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_wrong_port.status(), StatusCode::FORBIDDEN);

        // Allowed loopback origins -> 200
        for origin in ["http://127.0.0.1:4600", "http://localhost:4600"] {
            let res_ok = app
                .clone()
                .oneshot(
                    HttpRequest::builder()
                        .method("POST")
                        .uri("/api/desktop/activate")
                        .header("origin", origin)
                        .header("content-type", "application/json")
                        .body(Body::from(r#"{"target":"explain"}"#))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(res_ok.status(), StatusCode::OK);
            let csp = res_ok
                .headers()
                .get("content-security-policy")
                .unwrap()
                .to_str()
                .unwrap();
            assert!(csp.contains("default-src 'self'"));
            assert!(csp.contains("frame-ancestors 'none'"));
            let json_ok: Value = serde_json::from_slice(
                &to_bytes(res_ok.into_body(), 65536).await.unwrap(),
            )
            .unwrap();
            assert_eq!(json_ok["activeTarget"], "explain");
        }
    }

    #[tokio::test]
    async fn status_prefs_backup_logs_and_static_ui_routes_work() {
        let (app, _tmp) = test_router(4600);

        // GET /api/desktop/status
        let res_status = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/desktop/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_status.status(), StatusCode::OK);
        let status_json: Value =
            serde_json::from_slice(&to_bytes(res_status.into_body(), 65536).await.unwrap())
                .unwrap();
        assert_eq!(status_json["edition"], "suite");
        assert_eq!(status_json["workspaces"].as_array().unwrap().len(), 5);

        // POST /api/desktop/prefs
        let res_prefs = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/api/desktop/prefs")
                    .header("origin", "http://127.0.0.1:4600")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"theme":"light","lang":"en","autoStart":false}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_prefs.status(), StatusCode::OK);
        let prefs_json: Value =
            serde_json::from_slice(&to_bytes(res_prefs.into_body(), 65536).await.unwrap())
                .unwrap();
        assert_eq!(prefs_json["prefs"]["theme"], "light");
        assert_eq!(prefs_json["prefs"]["lang"], "en");
        assert_eq!(prefs_json["prefs"]["autoStart"], false);

        // POST /api/desktop/vault/backup
        let res_backup = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/api/desktop/vault/backup")
                    .header("origin", "http://127.0.0.1:4600")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_backup.status(), StatusCode::OK);
        let backup_json: Value =
            serde_json::from_slice(&to_bytes(res_backup.into_body(), 65536).await.unwrap())
                .unwrap();
        assert!(backup_json["backupDir"].as_str().is_some());

        // GET /api/desktop/logs/explain
        let res_logs = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/desktop/logs/explain")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_logs.status(), StatusCode::OK);
        let logs_json: Value =
            serde_json::from_slice(&to_bytes(res_logs.into_body(), 16384).await.unwrap())
                .unwrap();
        assert_eq!(logs_json["id"], "explain");

        // GET / serves UI index.html
        let res_ui = app
            .oneshot(HttpRequest::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res_ui.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn workspace_action_start_all_and_today_http_routes_work() {
        let tmp = TempDir::new().unwrap();
        let sidecars = tmp.path().join("sidecars");
        let pr_dir = sidecars.join("apps").join("pr");
        std::fs::create_dir_all(&pr_dir).unwrap();
        // Simulate auth-gated PR server where /api/suite/manifest returns 401 and /api/health returns 200
        std::fs::write(
            pr_dir.join("server.cjs"),
            r#"
const http = require("node:http");
const port = Number(process.env.PORT || 4603);
const srv = http.createServer((req, res) => {
  if (req.url === "/api/suite/manifest") { res.writeHead(401); res.end(); return; }
  if (req.url === "/api/health") { res.writeHead(200); res.end("ok"); return; }
  res.writeHead(404); res.end();
});
process.on("SIGTERM", () => srv.close(() => process.exit(0)));
srv.listen(port, "127.0.0.1");
"#,
        )
        .unwrap();

        let sup = Supervisor::new(SupervisorConfig {
            edition: Edition::Suite,
            base_port: 49200,
            vault_dir: Some(tmp.path().join("vault")),
            sidecars_dir: Some(sidecars),
            resource_dir: None,
            allow_dev_fallback: false,
            ready_timeout: Duration::from_secs(8),
        })
        .unwrap();

        let state = DaemonState {
            supervisor: sup.clone(),
            allowed_origins: Arc::new(build_allowed_origins("127.0.0.1", 4600)),
        };
        let app = build_router(state, None);

        let res_start = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/api/desktop/workspaces/start-all")
                    .header("origin", "http://127.0.0.1:4600")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_start.status(), StatusCode::OK);
        let st_json: Value =
            serde_json::from_slice(&to_bytes(res_start.into_body(), 65536).await.unwrap()).unwrap();
        let pr_ws = st_json["workspaces"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["id"] == "pr")
            .unwrap();
        assert_eq!(pr_ws["state"], "running");
        assert_eq!(pr_ws["manifestValid"], false);

        let res_today = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/desktop/today")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_today.status(), StatusCode::OK);

        // Enforce Sec-Fetch-Site guard on GET /api/desktop/open/{id}
        for blocked_site in ["cross-site", "same-site"] {
            let res_open_blocked = app
                .clone()
                .oneshot(
                    HttpRequest::builder()
                        .uri("/api/desktop/open/pr")
                        .header("sec-fetch-site", blocked_site)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(res_open_blocked.status(), StatusCode::FORBIDDEN);
        }

        for allowed_site in [Some("same-origin"), Some("none"), None] {
            let mut req_builder = HttpRequest::builder().uri("/api/desktop/open/pr");
            if let Some(val) = allowed_site {
                req_builder = req_builder.header("sec-fetch-site", val);
            }
            let res_open_ok = app
                .clone()
                .oneshot(req_builder.body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(res_open_ok.status(), StatusCode::FOUND);
        }

        let res_stop = app
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/api/desktop/workspaces/pr/stop")
                    .header("origin", "http://127.0.0.1:4600")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_stop.status(), StatusCode::OK);
        sup.shutdown_all().await;
    }

    #[tokio::test]
    async fn hub_render_live_demo_and_in_memory_shell_work() {
        let tmp = TempDir::new().unwrap();
        let sup = Supervisor::new(SupervisorConfig {
            edition: Edition::Suite,
            base_port: 49300,
            vault_dir: Some(tmp.path().join("vault")),
            sidecars_dir: Some(tmp.path().join("sidecars")),
            resource_dir: None,
            allow_dev_fallback: false,
            ready_timeout: Duration::from_secs(3),
        })
        .unwrap();

        let state = DaemonState {
            supervisor: sup,
            allowed_origins: Arc::new(build_allowed_origins("127.0.0.1", 4600)),
        };
        let app = build_router(state, None);

        // In-memory shell & CSS work even when hub-dist is absent on disk
        let res_shell = app
            .clone()
            .oneshot(HttpRequest::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res_shell.status(), StatusCode::OK);
        let shell_html = to_bytes(res_shell.into_body(), 262_144).await.unwrap();
        assert!(std::str::from_utf8(&shell_html).unwrap().contains("<!doctype html>"));

        let res_css = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri("/hub.css")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_css.status(), StatusCode::OK);

        let res_favicon = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri("/favicon.svg")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_favicon.status(), StatusCode::OK);
        assert!(res_favicon
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("image/svg+xml"));

        // Enforce Origin check on POST /api/desktop/hub-render
        let res_forbidden = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/api/desktop/hub-render")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_forbidden.status(), StatusCode::FORBIDDEN);

        // Live hub-render
        let res_live = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/api/desktop/hub-render")
                    .header("origin", "http://127.0.0.1:4600")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"view":"overview","paletteOpen":true}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_live.status(), StatusCode::OK);
        let live_patch: Value =
            serde_json::from_slice(&to_bytes(res_live.into_body(), 262_144).await.unwrap())
                .unwrap();
        assert!(live_patch["html"].as_str().is_some());
        assert!(live_patch["title"].as_str().is_some());

        // Deterministic ?demo=1 hub-render
        let res_demo = app
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/api/desktop/hub-render?demo=1&edition=explain&theme=light&lang=en")
                    .header("origin", "http://127.0.0.1:4600")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"ui":{"view":"today"}}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_demo.status(), StatusCode::OK);
        let demo_patch: Value =
            serde_json::from_slice(&to_bytes(res_demo.into_body(), 262_144).await.unwrap())
                .unwrap();
        assert!(demo_patch["html"].as_str().unwrap().contains("BELL Explain"));
    }
}
