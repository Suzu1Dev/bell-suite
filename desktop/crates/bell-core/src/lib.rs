pub mod contract;
pub mod edition;
pub mod supervisor;
pub mod vault;

pub use contract::{
    aggregate_today, aggregate_today_with_tokens, allowed_route, browser_origin, build_suite_env,
    validate_feed, validate_manifest, valid_href, Capability, ContractError, SuiteManifest,
    TodayFeed, TodayItem, TodayOverview, TodaySource,
};
pub use edition::{is_valid_target, AppId, Edition};
pub use supervisor::{
    find_executable, DesktopStatus, RuntimeInfo, Supervisor, SupervisorConfig, SupervisorError,
    WorkspaceLogs, WorkspaceState, WorkspaceStatus,
};
pub use vault::{
    default_vault_root, ensure_pr_api_token_at_root, read_pr_api_token_at_root, DesktopPrefs,
    PrefsPatch, Vault, VaultBackupResult, VaultDatabase, VaultError, VaultStatus,
};

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::net::TcpListener;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn edition_and_brand_tokens_match_contract() {
        assert_eq!(Edition::Suite.display_name(), "BELL Suite");
        assert_eq!(Edition::Suite.bundle_id(), "com.bell1dev.suite");
        assert_eq!(Edition::Suite.default_port(), 4600);
        assert_eq!(Edition::Suite.accent_light(), "#0f8a7d");
        assert_eq!(Edition::Suite.accent_dark(), "#43cabc");

        let expected = [
            (
                AppId::Personal,
                "BELL Personal",
                "com.bell1dev.personal",
                1,
                4601,
                "#a4532c",
                "#e08a5c",
            ),
            (
                AppId::Explain,
                "BELL Explain",
                "com.bell1dev.explain",
                2,
                4602,
                "#5b45e0",
                "#a799ff",
            ),
            (
                AppId::Pr,
                "BELL PR",
                "com.bell1dev.pr",
                3,
                4603,
                "#1b6b4c",
                "#46c289",
            ),
            (
                AppId::Reader,
                "BELL Web Reader",
                "com.bell1dev.reader",
                4,
                4604,
                "#0b6bcb",
                "#38aef8",
            ),
            (
                AppId::Campaign,
                "BELL Campaign",
                "com.bell1dev.campaign",
                5,
                4605,
                "#9e2b5c",
                "#e25b91",
            ),
        ];
        for (id, name, bundle, shortcut, port, light, dark) in expected {
            assert_eq!(id.display_name(), name);
            assert_eq!(id.bundle_id(), bundle);
            assert_eq!(id.shortcut_index(), shortcut);
            assert_eq!(id.default_port(), port);
            assert_eq!(id.accent_light(), light);
            assert_eq!(id.accent_dark(), dark);
        }
    }

    #[test]
    fn manifest_and_feed_validation_enforces_contract_v1() {
        let valid = json!({
            "schemaVersion": 1,
            "appId": "explain",
            "displayName": "BELL Explain",
            "summary": "Interactive learning workspace",
            "capabilities": [
                {
                    "id": "explain.handoff",
                    "kind": "accept-handoff",
                    "route": "#/explain",
                    "accepts": ["text"],
                    "maxChars": 2000
                },
                {
                    "id": "explain.today",
                    "kind": "today-feed",
                    "api": "/api/suite/today"
                },
                {
                    "id": "explain.note",
                    "kind": "object-route",
                    "route": "#/note/{id}"
                }
            ],
            "links": {
                "personal": "http://127.0.0.1:4601",
                "pr": "http://127.0.0.1:4603"
            }
        });

        let parsed =
            validate_manifest(&valid, &["http://127.0.0.1:4602".to_string()]).expect("valid");
        assert_eq!(parsed.app_id, AppId::Explain);
        assert_eq!(parsed.capabilities.len(), 3);

        // Reject linking to own origin
        let own_origin_link = json!({
            "schemaVersion": 1,
            "appId": "explain",
            "displayName": "BELL Explain",
            "summary": "Learning",
            "capabilities": [],
            "links": {
                "personal": "http://127.0.0.1:4602"
            }
        });
        assert!(validate_manifest(&own_origin_link, &["http://127.0.0.1:4602".into()]).is_err());

        // Reject non-loopback HTTP link
        let bad_remote = json!({
            "schemaVersion": 1,
            "appId": "explain",
            "displayName": "BELL Explain",
            "summary": "Learning",
            "capabilities": [],
            "links": {
                "personal": "http://evil.example.com:4601"
            }
        });
        assert!(validate_manifest(&bad_remote, &["http://127.0.0.1:4602".into()]).is_err());

        // Accept matching MagicDNS tailnet HTTPS link
        let tailnet_manifest = json!({
            "schemaVersion": 1,
            "appId": "explain",
            "displayName": "BELL Explain",
            "summary": "Learning",
            "capabilities": [],
            "links": {
                "personal": "https://personal.tail99.ts.net"
            }
        });
        assert!(
            validate_manifest(
                &tailnet_manifest,
                &["https://explain.tail99.ts.net".into()]
            )
            .is_ok()
        );

        // Validate Today feed
        let feed_json = json!({
            "schemaVersion": 1,
            "appId": "explain",
            "generatedAt": "2026-11-16T01:30:00Z",
            "counts": { "dueReviews": 2 },
            "items": [
                {
                    "sourceApp": "explain",
                    "type": "review",
                    "id": "note-1",
                    "title": "Rust Ownership & Lifetimes",
                    "href": "#/note/note-1",
                    "state": "due",
                    "updatedAt": "2026-11-16T01:00:00Z",
                    "detail": "Due today"
                }
            ],
            "truncated": false
        });
        let feed = validate_feed(&feed_json).expect("valid feed");
        assert_eq!(feed.items.len(), 1);
        assert_eq!(feed.counts.get("dueReviews"), Some(&2));

        // Reject disallowed route in feed item
        let bad_feed = json!({
            "schemaVersion": 1,
            "appId": "explain",
            "generatedAt": "2026-11-16T01:30:00Z",
            "counts": {},
            "items": [
                {
                    "sourceApp": "explain",
                    "type": "review",
                    "id": "note-1",
                    "title": "Bad Route",
                    "href": "#/admin/secret",
                    "state": "due",
                    "updatedAt": "2026-11-16T01:00:00Z"
                }
            ],
            "truncated": false
        });
        assert!(validate_feed(&bad_feed).is_err());
    }

    #[test]
    fn build_suite_env_matches_cli_and_sets_vault_dirs() {
        let tmp = TempDir::new().unwrap();
        let mut ports = BTreeMap::new();
        for app in AppId::ALL {
            ports.insert(app, app.default_port());
        }

        let personal_env = build_suite_env(AppId::Personal, &ports, "127.0.0.1", tmp.path());
        assert_eq!(personal_env.get("HOST").unwrap(), "127.0.0.1");
        assert_eq!(personal_env.get("PORT").unwrap(), "4601");
        assert_eq!(personal_env.get("BELLPERSONAL_PORT").unwrap(), "4601");
        assert_eq!(personal_env.get("BELL_NO_OPEN").unwrap(), "1");
        assert_eq!(personal_env.get("BELLPR_NO_OPEN").unwrap(), "1");
        assert_eq!(
            personal_env.get("BELLPERSONAL_SUITE_LINKS").unwrap(),
            "explain=http://127.0.0.1:4602,pr=http://127.0.0.1:4603,reader=http://127.0.0.1:4604,campaign=http://127.0.0.1:4605"
        );
        assert_eq!(
            personal_env.get("BELLPERSONAL_SUITE_FETCH").unwrap(),
            "explain=http://127.0.0.1:4602,pr=http://127.0.0.1:4603,reader=http://127.0.0.1:4604"
        );
        assert_eq!(
            personal_env.get("BELLPERSONAL_DATA_DIR").unwrap(),
            &tmp.path().join("personal").to_string_lossy().into_owned()
        );
        assert_eq!(
            personal_env.get("BELL_DATA_DIR").unwrap(),
            &tmp.path().join("explain").to_string_lossy().into_owned()
        );
        assert_eq!(
            personal_env.get("BELLPR_DATA_DIR").unwrap(),
            &tmp.path().join("pr").to_string_lossy().into_owned()
        );
        assert_eq!(
            personal_env.get("BELL_READER_DATA_DIR").unwrap(),
            &tmp.path().join("reader").to_string_lossy().into_owned()
        );
        assert_eq!(
            personal_env.get("BELL_CAMPAIGN_DATA_DIR").unwrap(),
            &tmp.path().join("campaign").to_string_lossy().into_owned()
        );

        let reader_env = build_suite_env(AppId::Reader, &ports, "localhost", tmp.path());
        assert_eq!(
            reader_env.get("BELL_READER_SUITE_FETCH").unwrap(),
            "explain=http://127.0.0.1:4602,pr=http://127.0.0.1:4603"
        );
    }

    #[test]
    fn vault_prefs_scan_and_hot_backup_under_active_wal_writes() {
        let tmp = TempDir::new().unwrap();
        let vault = Vault::open(tmp.path().to_path_buf()).unwrap();

        let p0 = vault.load_prefs().unwrap();
        assert_eq!(p0.theme, "system");
        assert_eq!(p0.lang, "zh-tw");
        assert!(p0.auto_start);

        let p1 = vault
            .update_prefs(PrefsPatch {
                theme: Some("dark".into()),
                lang: Some("en".into()),
                active_target: Some("explain".into()),
                auto_start: Some(false),
            })
            .unwrap();
        assert_eq!(p1.theme, "dark");
        assert_eq!(p1.lang, "en");
        assert_eq!(p1.active_target, "explain");
        assert!(!p1.auto_start);
        assert!(vault
            .update_prefs(PrefsPatch {
                theme: Some("neon".into()),
                ..Default::default()
            })
            .is_err());

        // Create a WAL-mode SQLite DB inside explain/nested/notes.sqlite
        let nested_dir = tmp.path().join("explain").join("nested");
        std::fs::create_dir_all(&nested_dir).unwrap();
        let db_path = nested_dir.join("notes.sqlite");

        let writer = Connection::open(&db_path).unwrap();
        writer
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 CREATE TABLE notes (id INTEGER PRIMARY KEY, title TEXT NOT NULL);
                 CREATE TABLE reviews (id INTEGER PRIMARY KEY, note_id INTEGER NOT NULL);
                 INSERT INTO notes (id, title) VALUES (1, 'Committed A'), (2, 'Committed B');",
            )
            .unwrap();

        // Begin an uncommitted transaction in WAL while running scan + hot backup!
        writer
            .execute_batch(
                "BEGIN IMMEDIATE;
                 INSERT INTO notes (id, title) VALUES (3, 'Uncommitted C');",
            )
            .unwrap();

        let scan = vault.scan();
        assert_eq!(scan.databases.len(), 1);
        assert_eq!(scan.databases[0].app_id, AppId::Explain);
        assert!(scan.databases[0].healthy);
        assert_eq!(scan.databases[0].tables, 2);
        assert!(scan.databases[0].bytes > 0);

        let backup = vault.backup_vault().unwrap();
        assert_eq!(backup.files, vec!["explain/nested/notes.sqlite".to_string()]);
        assert!(backup.total_bytes > 0);

        // Verify the backed-up DB has committed rows 1..2, omits uncommitted row 3, and passes integrity_check!
        let snap_path = std::path::Path::new(&backup.backup_dir)
            .join("explain")
            .join("nested")
            .join("notes.sqlite");
        let snap_conn = Connection::open(&snap_path).unwrap();
        let integrity: String = snap_conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap();
        assert_eq!(integrity, "ok");
        let count: i64 = snap_conn
            .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);

        // Commit row 3 and run a second backup in the same second to verify collision-free backup dir naming
        writer.execute_batch("COMMIT;").unwrap();
        let backup2 = vault.backup_vault().unwrap();
        assert_ne!(backup.backup_dir, backup2.backup_dir);
        let snap_conn2 = Connection::open(
            std::path::Path::new(&backup2.backup_dir)
                .join("explain")
                .join("nested")
                .join("notes.sqlite"),
        )
        .unwrap();
        let count2: i64 = snap_conn2
            .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count2, 3);

        let scan_after = vault.scan();
        assert_eq!(scan_after.backups_count, 2);
        assert!(scan_after.last_backup_at.is_some());
    }

    #[tokio::test]
    async fn supervisor_lifecycle_logs_today_and_port_fallback() {
        let tmp = TempDir::new().unwrap();
        let vault_dir = tmp.path().join("vault");
        let sidecars_dir = tmp.path().join("sidecars");
        let explain_app_dir = sidecars_dir.join("apps").join("explain");
        std::fs::create_dir_all(&explain_app_dir).unwrap();

        let server_js = r##"
const http = require("node:http");
const port = Number(process.env.PORT || 4602);
const host = process.env.HOST || "127.0.0.1";
console.log("booting mock explain on " + host + ":" + port);
console.error("mock explain diagnostic line");

const server = http.createServer((req, res) => {
  if (req.url === "/api/suite/manifest") {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({
      schemaVersion: 1,
      appId: "explain",
      displayName: "BELL Explain",
      summary: "Mock explain workspace",
      capabilities: [
        { id: "explain.today", kind: "today-feed", api: "/api/suite/today" }
      ],
      links: {}
    }));
    return;
  }
  if (req.url === "/api/suite/today") {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({
      schemaVersion: 1,
      appId: "explain",
      generatedAt: "2026-11-16T01:30:00Z",
      counts: { dueReviews: 1 },
      items: [
        {
          sourceApp: "explain",
          type: "review",
          id: "note-42",
          title: "Async Tokio Supervision",
          href: "#/note/note-42",
          state: "due",
          updatedAt: "2026-11-16T01:00:00Z",
          detail: "Due now"
        }
      ],
      truncated: false
    }));
    return;
  }
  res.writeHead(200);
  res.end("ok");
});

process.on("SIGTERM", () => server.close(() => process.exit(0)));
server.listen(port, host, () => {
  console.log("listening:" + port);
});
"##;
        std::fs::write(explain_app_dir.join("server.cjs"), server_js).unwrap();

        // Occupy a dynamic port and set base_port = blocker_port - 2 so Explain's preferred port (base + 2) is busy!
        let blocker = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let blocked_port = blocker.local_addr().unwrap().port();
        let base_port = blocked_port.saturating_sub(2);

        let sup = Supervisor::new(SupervisorConfig {
            edition: Edition::Suite,
            base_port,
            vault_dir: Some(vault_dir),
            sidecars_dir: Some(sidecars_dir),
            resource_dir: None,
            allow_dev_fallback: false,
            ready_timeout: Duration::from_secs(10),
        })
        .unwrap();

        // Verify ensure_pr_api_token generates a valid 64-hex token before PR starts and is idempotent
        let generated_tok = sup.ensure_pr_api_token().unwrap();
        assert_eq!(generated_tok.len(), 64);
        assert!(generated_tok.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(sup.ensure_pr_api_token().unwrap(), generated_tok);

        let initial = sup.status();
        let explain_init = initial
            .workspaces
            .iter()
            .find(|w| w.id == AppId::Explain)
            .unwrap();
        assert!(explain_init.installed);
        assert_eq!(explain_init.state, WorkspaceState::Stopped);

        let started = sup.start_workspace(AppId::Explain).await.unwrap();
        let explain_run = started
            .workspaces
            .iter()
            .find(|w| w.id == AppId::Explain)
            .unwrap();
        assert_eq!(explain_run.state, WorkspaceState::Running);
        assert!(explain_run.manifest_valid);
        assert!(explain_run.pid.is_some());
        if base_port + 2 == blocked_port {
            assert_ne!(explain_run.port, blocked_port);
        }

        // Verify Today aggregation against live server
        let today = sup.today().await;
        assert_eq!(today.total_items, 1);
        assert_eq!(today.sources.len(), 1);
        assert_eq!(today.sources[0].app_id, AppId::Explain);
        assert_eq!(today.sources[0].status, "ready");
        assert_eq!(today.sources[0].items[0].id, "note-42");

        // Give async log reader a moment to flush stdout/stderr lines
        tokio::time::sleep(Duration::from_millis(80)).await;
        let logs = sup.logs(AppId::Explain);
        assert!(logs.lines.iter().any(|l| l.contains("booting mock explain")));
        assert!(logs
            .lines
            .iter()
            .any(|l| l.contains("[stderr] mock explain diagnostic line")));

        // Activate explain, then restart, then stop (stopping active resets activeTarget to "hub")
        let act = sup.activate("explain").unwrap();
        assert_eq!(act.active_target, "explain");

        let restarted = sup.restart_workspace(AppId::Explain).await.unwrap();
        let explain_restarted = restarted
            .workspaces
            .iter()
            .find(|w| w.id == AppId::Explain)
            .unwrap();
        assert_eq!(explain_restarted.state, WorkspaceState::Running);
        assert_eq!(explain_restarted.restart_count, 1);

        let stopped = sup.stop_workspace(AppId::Explain).await.unwrap();
        let explain_stopped = stopped
            .workspaces
            .iter()
            .find(|w| w.id == AppId::Explain)
            .unwrap();
        assert_eq!(explain_stopped.state, WorkspaceState::Stopped);
        assert!(explain_stopped.pid.is_none());
        assert_eq!(stopped.active_target, "hub");

        drop(blocker);
    }

    #[tokio::test]
    async fn pr_bearer_auth_json_vault_backup_and_start_all_prealloc_work() {
        let tmp = TempDir::new().unwrap();
        let vault_dir = tmp.path().join("vault");
        let sidecars_dir = tmp.path().join("sidecars");
        let pr_app_dir = sidecars_dir.join("apps").join("pr");
        let personal_app_dir = sidecars_dir.join("apps").join("personal");
        std::fs::create_dir_all(&pr_app_dir).unwrap();
        std::fs::create_dir_all(&personal_app_dir).unwrap();

        let token = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";

        // Mock PR sidecar writes 64-hex api-token, app-settings.json, discovery.json, plus ephemeral files
        // that MUST be excluded from vault scan & backup (api-token, bellpr.lock, children.json, state.json.tmp).
        let pr_server_js = format!(
            r##"
const fs = require("node:fs");
const path = require("node:path");
const http = require("node:http");
const port = Number(process.env.PORT || 4603);
const dataDir = process.env.BELLPR_DATA_DIR;
const tok = "{token}";

fs.mkdirSync(dataDir, {{ recursive: true }});
fs.writeFileSync(path.join(dataDir, "api-token"), tok + "\n", {{ mode: 0o600 }});
fs.writeFileSync(path.join(dataDir, "app-settings.json"), JSON.stringify({{ locale: "zh-TW" }}));
fs.writeFileSync(path.join(dataDir, "discovery.json"), JSON.stringify({{ latest: null }}));
fs.writeFileSync(path.join(dataDir, "bellpr.lock"), JSON.stringify({{ pid: process.pid }}));
fs.writeFileSync(path.join(dataDir, "children.json"), JSON.stringify([]));
fs.writeFileSync(path.join(dataDir, "state.json.tmp"), "partial");

const srv = http.createServer((req, res) => {{
  const auth = req.headers["authorization"] || "";
  if (req.url === "/api/health") {{
    res.writeHead(200);
    res.end("ok");
    return;
  }}
  if (auth !== "Bearer " + tok) {{
    res.writeHead(401, {{ "Content-Type": "application/json" }});
    res.end(JSON.stringify({{ error: "unauthorized" }}));
    return;
  }}
  if (req.url === "/api/suite/manifest") {{
    res.writeHead(200, {{ "Content-Type": "application/json" }});
    res.end(JSON.stringify({{
      schemaVersion: 1,
      appId: "pr",
      displayName: "BELL PR",
      summary: "Code review workspace",
      capabilities: [{{ id: "pr.today", kind: "today-feed", api: "/api/suite/today" }}],
      links: {{}}
    }}));
    return;
  }}
  if (req.url === "/api/suite/today") {{
    res.writeHead(200, {{ "Content-Type": "application/json" }});
    res.end(JSON.stringify({{
      schemaVersion: 1,
      appId: "pr",
      generatedAt: "2026-11-16T02:00:00Z",
      counts: {{ activeRuns: 1 }},
      items: [{{
        sourceApp: "pr",
        type: "run",
        id: "run-7",
        title: "Review PR #7",
        href: "#/runs/run-7",
        state: "needs-you",
        updatedAt: "2026-11-16T01:55:00Z"
      }}],
      truncated: false
    }}));
    return;
  }}
  res.writeHead(404);
  res.end();
}});
process.on("SIGTERM", () => srv.close(() => process.exit(0)));
srv.listen(port, "127.0.0.1");
"##
        );
        std::fs::write(pr_app_dir.join("server.cjs"), pr_server_js).unwrap();

        // Mock Personal sidecar echoes BELLPERSONAL_SUITE_LINKS into /links so we verify start_all port pre-allocation!
        let personal_server_js = r##"
const http = require("node:http");
const port = Number(process.env.PORT || 4601);
const srv = http.createServer((req, res) => {
  if (req.url === "/api/suite/manifest") {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({
      schemaVersion: 1,
      appId: "personal",
      displayName: "BELL Personal",
      summary: "Personal workspace",
      capabilities: [],
      links: {}
    }));
    return;
  }
  if (req.url === "/env-links") {
    res.writeHead(200, { "Content-Type": "text/plain" });
    res.end(process.env.BELLPERSONAL_SUITE_LINKS || "");
    return;
  }
  res.writeHead(200);
  res.end("ok");
});
process.on("SIGTERM", () => srv.close(() => process.exit(0)));
srv.listen(port, "127.0.0.1");
"##;
        std::fs::write(personal_app_dir.join("server.cjs"), personal_server_js).unwrap();

        // Block PR's preferred port (base_port + 3) so start_all pre-allocates an ephemeral port for PR!
        let blocker = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let blocked_pr_port = blocker.local_addr().unwrap().port();
        let base_port = blocked_pr_port.saturating_sub(3);

        let sup = Supervisor::new(SupervisorConfig {
            edition: Edition::Suite,
            base_port,
            vault_dir: Some(vault_dir),
            sidecars_dir: Some(sidecars_dir),
            resource_dir: None,
            allow_dev_fallback: false,
            ready_timeout: Duration::from_secs(10),
        })
        .unwrap();

        // Verify ensure_pr_api_token generates a valid 64-hex token before PR starts and is idempotent
        let generated_tok = sup.ensure_pr_api_token().unwrap();
        assert_eq!(generated_tok.len(), 64);
        assert!(generated_tok.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(sup.ensure_pr_api_token().unwrap(), generated_tok);

        let status = sup.start_all().await.unwrap();
        // Verify token is readable via pr_api_token() and NEVER leaked into DesktopStatus JSON!
        assert_eq!(sup.pr_api_token().as_deref(), Some(token));
        let serialized_status = serde_json::to_string(&status).unwrap();
        assert!(!serialized_status.contains(token));

        let pr_ws = status.workspaces.iter().find(|w| w.id == AppId::Pr).unwrap();
        let personal_ws = status
            .workspaces
            .iter()
            .find(|w| w.id == AppId::Personal)
            .unwrap();
        assert_eq!(pr_ws.state, WorkspaceState::Running);
        assert!(pr_ws.manifest_valid, "PR manifest must validate via bearer token");
        assert!(pr_ws.db_bytes > 0, "PR JSON stores must count toward dbBytes");

        if base_port + 3 == blocked_pr_port {
            assert_ne!(pr_ws.port, blocked_pr_port);
            // Verify Personal received PR's pre-allocated fallback port in BELLPERSONAL_SUITE_LINKS!
            let links_txt = reqwest::get(format!("http://127.0.0.1:{}/env-links", personal_ws.port))
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            assert!(
                links_txt.contains(&format!("pr=http://127.0.0.1:{}", pr_ws.port)),
                "expected pre-allocated PR port in Personal links, got: {links_txt}"
            );
        }

        // Verify TodayOverview includes PR as ready via bearer token
        let today = sup.today().await;
        let pr_today = today.sources.iter().find(|s| s.app_id == AppId::Pr).unwrap();
        assert_eq!(pr_today.status, "ready");
        assert_eq!(pr_today.items.len(), 1);
        assert_eq!(pr_today.items[0].id, "run-7");

        // Verify Vault scan & backup includes state.json, app-settings.json, discovery.json and excludes secrets/locks
        let pr_dbs: Vec<_> = status
            .vault
            .databases
            .iter()
            .filter(|d| d.app_id == AppId::Pr)
            .collect();
        assert_eq!(pr_dbs.len(), 3);
        assert!(pr_dbs.iter().all(|d| d.healthy && d.tables == 1));

        let backup = sup.backup_vault().unwrap();
        assert_eq!(
            backup.files,
            vec![
                "pr/app-settings.json".to_string(),
                "pr/discovery.json".to_string(),
                "pr/state.json".to_string(),
            ]
        );
        assert!(
            !std::path::Path::new(&backup.backup_dir)
                .join("pr")
                .join("api-token")
                .exists()
        );

        sup.shutdown_all().await;
        drop(blocker);
    }

    #[tokio::test]
    async fn apps_tar_gz_unpack_ready_cache_and_zero_sqlite_pr_token_work() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let tmp = TempDir::new().unwrap();
        let vault_dir = tmp.path().join("vault");
        let sidecars_dir = tmp.path().join("sidecars");
        std::fs::create_dir_all(&sidecars_dir).unwrap();

        // 1. Verify ensure_pr_api_token_at_root works with zero SQLite files on disk!
        let fast_tok = ensure_pr_api_token_at_root(&vault_dir).unwrap();
        assert_eq!(fast_tok.len(), 64);
        assert!(fast_tok.bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(!vault_dir.join("desktop-prefs.sqlite").exists());
        assert_eq!(ensure_pr_api_token_at_root(&vault_dir).unwrap(), fast_tok);

        // 2. Build a real apps.tar.gz containing apps/explain/server.cjs
        let server_code = br#"
const http = require("node:http");
const port = Number(process.env.PORT || 4602);
const srv = http.createServer((req, res) => {
  if (req.url === "/api/suite/manifest") {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({
      schemaVersion: 1,
      appId: "explain",
      displayName: "BELL Explain",
      summary: "Packed sidecar test",
      capabilities: [],
      links: {}
    }));
    return;
  }
  res.writeHead(200);
  res.end("ok");
});
process.on("SIGTERM", () => srv.close(() => process.exit(0)));
srv.listen(port, "127.0.0.1");
"#;
        let tar_gz_path = sidecars_dir.join("apps.tar.gz");
        {
            let file = std::fs::File::create(&tar_gz_path).unwrap();
            let enc = GzEncoder::new(file, Compression::fast());
            let mut tar = tar::Builder::new(enc);
            let mut header = tar::Header::new_gnu();
            header.set_size(server_code.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, "apps/explain/server.cjs", &server_code[..])
                .unwrap();
            tar.finish().unwrap();
        }

        let sup = Supervisor::new(SupervisorConfig {
            edition: Edition::Explain,
            base_port: 49400,
            vault_dir: Some(vault_dir.clone()),
            sidecars_dir: Some(sidecars_dir.clone()),
            resource_dir: None,
            allow_dev_fallback: false,
            ready_timeout: Duration::from_secs(10),
        })
        .unwrap();

        // Before unpacking apps.tar.gz, status() already reports Explain installed: true in 0ms!
        let cache_dir = vault_dir
            .join("runtime-cache")
            .join(format!("explain-{}", env!("CARGO_PKG_VERSION")));
        assert!(!cache_dir.exists());
        let pre_status = sup.status();
        let explain_pre = pre_status
            .workspaces
            .iter()
            .find(|w| w.id == AppId::Explain)
            .unwrap();
        assert!(explain_pre.installed);

        // start_all_staggered unpacks apps.tar.gz, writes .ready, and boots Explain cleanly
        let callbacks = std::sync::Arc::new(AtomicUsize::new(0));
        let cb_clone = callbacks.clone();
        let started = sup
            .start_all_staggered(move || {
                cb_clone.fetch_add(1, Ordering::SeqCst);
            })
            .await
            .unwrap();
        assert_eq!(callbacks.load(Ordering::SeqCst), 1);
        assert!(cache_dir.join(".ready").is_file());
        let explain_run = started
            .workspaces
            .iter()
            .find(|w| w.id == AppId::Explain)
            .unwrap();
        assert_eq!(explain_run.state, WorkspaceState::Running);
        assert!(explain_run.manifest_valid);
        sup.shutdown_all().await;

        // Corrupt apps.tar.gz on disk to prove warm boot reuses .ready cache without re-unpacking!
        std::fs::write(&tar_gz_path, b"not-a-gzip-anymore").unwrap();
        let sup2 = Supervisor::new(SupervisorConfig {
            edition: Edition::Explain,
            base_port: 49410,
            vault_dir: Some(vault_dir),
            sidecars_dir: Some(sidecars_dir),
            resource_dir: None,
            allow_dev_fallback: false,
            ready_timeout: Duration::from_secs(10),
        })
        .unwrap();
        sup2.ensure_sidecars_ready();
        let st2 = sup2.start_workspace(AppId::Explain).await.unwrap();
        let explain2 = st2
            .workspaces
            .iter()
            .find(|w| w.id == AppId::Explain)
            .unwrap();
        assert_eq!(explain2.state, WorkspaceState::Running);
        sup2.shutdown_all().await;
    }
}
