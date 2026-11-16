use axum::{
    body::Bytes,
    extract::{Path as AxumPath, Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bell_core::{
    AppId, Edition, PrefsPatch, Supervisor, SupervisorConfig, SupervisorError, WorkspaceState,
};
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

async fn handle_open_workspace(
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
        Some(dir) if dir.is_dir() => {
            let index = dir.join("index.html");
            let svc = ServeDir::new(dir).not_found_service(ServeFile::new(index));
            api.fallback_service(svc)
        }
        _ => api.fallback(|| async {
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
        candidates.push(cwd.join("desktop").join("ui").join("dist"));
        candidates.push(cwd.join("ui").join("dist"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(p) = exe.parent() {
            candidates.push(p.join("ui").join("dist"));
            candidates.push(p.join("..").join("Resources").join("ui").join("dist"));
        }
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("ui")
            .join("dist"),
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
}
