# BELL Desktop Shared Interface Contract

Single source of truth for the Rust workspace (`crates/bell-core`, `crates/bell-daemon`, `crates/bell-tauri`), Desktop Shell UI (`ui/`), and Packaging/CI (`scripts/`, `.github/workflows/desktop-release.yml`).

## 1. Products & Ground-Truth Brand Tokens

| Edition / ID | AppId (`Option`) | Display Name | Bundle Identifier | Shortcut | Default Port | Light Accent | Dark Accent |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `suite` | `None` (all 5) | `BELL Suite` | `com.bell1dev.suite` | `⌘0` / `Ctrl+0` (Hub) | `4600` (daemon) | `#0f8a7d` | `#43cabc` |
| `personal` | `Some("personal")` | `BELL Personal` | `com.bell1dev.personal` | `⌘1` / `Ctrl+1` | `4601` | `#a4532c` | `#e08a5c` |
| `explain` | `Some("explain")` | `BELL Explain` | `com.bell1dev.explain` | `⌘2` / `Ctrl+2` | `4602` | `#5b45e0` | `#a799ff` |
| `pr` | `Some("pr")` | `BELL PR` | `com.bell1dev.pr` | `⌘3` / `Ctrl+3` | `4603` | `#1b6b4c` | `#46c289` |
| `reader` | `Some("reader")` | `BELL Web Reader` | `com.bell1dev.reader` | `⌘4` / `Ctrl+4` | `4604` | `#0b6bcb` | `#38aef8` |
| `campaign` | `Some("campaign")` | `BELL Campaign` | `com.bell1dev.campaign` | `⌘5` / `Ctrl+5` | `4605` | `#9e2b5c` | `#e25b91` |

## 2. Sidecar & Resource Layout (`desktop/dist-sidecars`)

Produced by `node desktop/scripts/bundle-sidecars.mjs`:

```text
desktop/dist-sidecars/
├── bin/
│   └── bell-runtime(.exe)          # Optional embedded Node 22 LTS runtime (populated in CI / release; falls back to system `node` in dev)
└── apps/
    ├── personal/
    │   ├── server.cjs              # Self-contained esbuild CJS server bundle (zero node_modules needed)
    │   ├── package.json            # Minimal metadata { name, version }
    │   └── dist/                   # Built Vite frontend static assets
    ├── explain/
    │   ├── server.cjs
    │   ├── package.json
    │   └── dist/
    ├── pr/
    │   ├── server.cjs
    │   ├── package.json
    │   ├── vendor/                 # ponytail rules.md + provenance.json
    │   └── dist/
    ├── reader/
    │   ├── server.cjs
    │   ├── package.json
    │   └── dist/
    └── campaign/
        ├── server.cjs
        ├── package.json
        └── dist/
```

Runtime resolution order in `bell-core`:
1. `BELL_DESKTOP_SIDECARS_DIR` env var (if set), else `<resource_dir>/dist-sidecars`, else `<repo>/desktop/dist-sidecars`.
2. Runtime binary: `BELL_DESKTOP_RUNTIME_BIN` env var, else `<sidecars_dir>/bin/bell-runtime(.exe)`, else `node` on `PATH`.
3. If `dist-sidecars/apps/<id>/server.cjs` exists, spawn `<runtime> server.cjs` with `cwd = <sidecars_dir>/apps/<id>` and `build_suite_env(...)` (plus CLI args `start --port <port> --host 127.0.0.1 --no-open` + `--force` for `pr` when `server.cjs` is `bell-pr`'s CLI).
   - **Important**: `bundle-sidecars.mjs` ensures every app's `server.cjs` accepts uniform env vars (`HOST=127.0.0.1`, `PORT=<port>`, plus app-specific `BELL*_HOST`, `BELL*_PORT`, `BELL*_DATA_DIR`, `BELL*_SUITE_LINKS`, `BELL*_SUITE_FETCH`, `BELL_NO_OPEN=1`, `BELLPR_NO_OPEN=1`) and starts listening immediately when run as `<runtime> server.cjs` (no extra CLI subcommand required, or standard `start --port <port> --host 127.0.0.1 --no-open`). Specifically: `bundle-sidecars.mjs` wraps `pr` so `<runtime> server.cjs` runs `start --port $PORT --host $HOST --no-open --force` automatically!
4. Fallback if `dist-sidecars` is absent: developer checkout discovery (`../worktrees/personal-bot-suite`, `../worktrees/explain-launch`, `../../bell-pr`, `../../bell-reader`, `../../bell-campaign`).

## 3. JSON Data Shapes (`camelCase` via `#[serde(rename_all = "camelCase")]`)

### `DesktopStatus` (`GET /api/desktop/status` / Tauri `desktop_status`)
```json
{
  "edition": "suite",
  "version": "1.0.1",
  "os": "macos",
  "arch": "aarch64",
  "activeTarget": "hub",
  "runtime": {
    "kind": "bundled",
    "path": "/.../bell-runtime",
    "claudeCli": true,
    "codexCli": true,
    "ghCli": true,
    "gitCli": true
  },
  "prefs": {
    "theme": "system",
    "lang": "zh-tw",
    "activeTarget": "hub",
    "autoStart": true
  },
  "workspaces": [
    {
      "id": "personal",
      "name": "BELL Personal",
      "shortcutIndex": 1,
      "accentLight": "#a4532c",
      "accentDark": "#e08a5c",
      "port": 4601,
      "url": "http://127.0.0.1:4601",
      "state": "running",
      "pid": 12345,
      "manifestValid": true,
      "installed": true,
      "dbBytes": 131072,
      "restartCount": 0,
      "lastError": null
    }
  ],
  "vault": {
    "rootDir": "/.../BELL",
    "totalBytes": 524288,
    "quotaBytes": 5368709120,
    "backupsCount": 1,
    "lastBackupAt": "2026-11-16T01:30:00Z",
    "databases": [
      {
        "appId": "personal",
        "path": "/.../personal.db",
        "bytes": 131072,
        "healthy": true,
        "tables": 8
      }
    ]
  }
}
```
Note: `WorkspaceState` is `"stopped" | "starting" | "running" | "error"`. `activeTarget` is `"hub" | "personal" | "explain" | "pr" | "reader" | "campaign"`.

### `TodayOverview` (`GET /api/desktop/today` / Tauri `desktop_today`)
```json
{
  "generatedAt": "2026-11-16T01:30:00Z",
  "totalItems": 3,
  "sources": [
    {
      "appId": "explain",
      "status": "ready",
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
      ]
    }
  ]
}
```

### Action Endpoints (`bell-daemon` HTTP & `bell-tauri` Commands)

| Action | HTTP (`bell-daemon`) | Tauri v2 Command (`invoke`) | Payload / Result |
| :--- | :--- | :--- | :--- |
| Get full status | `GET /api/desktop/status` | `desktop_status()` | `-> DesktopStatus` |
| Get Today feed | `GET /api/desktop/today` | `desktop_today()` | `-> TodayOverview` |
| Switch active view / child webview | `POST /api/desktop/activate` `{ target, route? }` | `desktop_activate({ target, route })` | `-> DesktopStatus` |
| Start / Stop / Restart workspace | `POST /api/desktop/workspaces/:id/:action` (`start`\|`stop`\|`restart`) | `desktop_workspace_action({ id, action })` | `-> DesktopStatus` |
| Start all workspaces | `POST /api/desktop/workspaces/start-all` | `desktop_start_all()` | `-> DesktopStatus` |
| Update preferences | `POST /api/desktop/prefs` `{ theme?, lang?, autoStart? }` | `desktop_save_prefs({ theme, lang, autoStart })` | `-> DesktopStatus` |
| Atomic SQLite backup | `POST /api/desktop/vault/backup` `{}` | `desktop_vault_backup()` | `-> { backupDir, files, totalBytes, createdAt }` |
| Read workspace logs | `GET /api/desktop/logs/:id` | `desktop_logs({ id })` | `-> { id, lines: string[] }` |
| Window controls (Tauri) | N/A (no-op in browser) | `desktop_window_action({ action })` (`minimize`\|`toggleMaximize`\|`close`) | `-> ()` |

Security in `bell-daemon`:
- Binds strictly to `127.0.0.1` by default.
- Every `POST` request checks `Origin` matches the daemon's own `http://127.0.0.1:<port>` or `http://localhost:<port>` (returns `403` JSON `{"error":"forbidden origin"}` otherwise).
