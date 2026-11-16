# BELL Desktop (`v1.0.3` — macOS & Windows)

Native macOS Universal (`aarch64` + `x86_64`) and Windows (`x86_64`) desktop application and supervisor for **BELL Suite** and all 5 local-first workspaces, built with a **100% Pure-Rust** desktop workspace (`bell-core`, `bell-hub`, `bell-daemon`, `bell-tauri` — zero TypeScript in `desktop/`), standard Tauri v2 Single-Webview `WebviewWindow` architecture (zero `unstable` feature, zero top chrome bar, built-in `SuiteSwitcher` + `⌘0`–`⌘5` / `Ctrl+0`–`Ctrl+5` navigation, Linear-grade dual-theme design system), and zero-`node_modules` bundled Node 22 LTS sidecars.

## Editions

Every edition shares the same Pure-Rust core and `bell-hub` bundle, configured via `desktop/scripts/prepare-edition.mjs`:

| Edition | Product Name | Bundle Identifier | Shortcut | Default Port | Accent (Light / Dark) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `suite` | `BELL Suite` | `com.bell1dev.suite` | `⌘0` / `Ctrl+0` | `4600` (daemon) | `#0f8a7d` / `#43cabc` |
| `personal` | `BELL Personal` | `com.bell1dev.personal` | `⌘1` / `Ctrl+1` | `4601` | `#a4532c` / `#e08a5c` |
| `explain` | `BELL Explain` | `com.bell1dev.explain` | `⌘2` / `Ctrl+2` | `4602` | `#5b45e0` / `#a799ff` |
| `pr` | `BELL PR` | `com.bell1dev.pr` | `⌘3` / `Ctrl+3` | `4603` | `#1b6b4c` / `#46c289` |
| `reader` | `BELL Web Reader` | `com.bell1dev.reader` | `⌘4` / `Ctrl+4` | `4604` | `#0b6bcb` / `#38aef8` |
| `campaign` | `BELL Campaign` | `com.bell1dev.campaign` | `⌘5` / `Ctrl+5` | `4605` | `#9e2b5c` / `#e25b91` |

## Directory Structure

```text
desktop/
├── CONTRACT.md                  # Shared Rust <-> Hub <-> Packaging interface contract
├── Cargo.toml                   # Pure-Rust workspace (bell-core, bell-hub, bell-daemon, bell-tauri)
├── crates/
│   ├── bell-core/               # Process supervisor, port allocation, Today feed, atomic SQLite vault backups
│   ├── bell-hub/                # Pure-Rust Linear-grade Suite Hub asset emitter (`build-hub-dist` -> hub-dist/)
│   ├── bell-daemon/             # Loopback HTTP control plane (127.0.0.1:4600) serving hub-dist + REST API
│   └── bell-tauri/              # Native Tauri v2 Single-Webview (`WebviewWindow`) app + menu/tray + Shortcuts
├── hub-dist/                    # Generated static Hub assets (`index.html`, `hub.css`, `hub.js`)
├── icons/                       # Generated squircle PNG/ICO/ICNS icons for all 6 editions
├── editions/                    # Tauri v2 configs (tauri.<edition>.conf.json) + macOS entitlements plists
├── dist-sidecars/               # Gitignored self-contained CJS workspace bundles + embedded Node runtime
└── scripts/
    ├── generate-icons.mjs       # Pure Node RGBA/ICO/ICNS icon generator (zero external dependencies)
    ├── bundle-sidecars.mjs      # Bundles all 5 backends into dist-sidecars/apps/<id>/server.cjs + smoke test
    └── prepare-edition.mjs      # Generates & optionally stages/applies per-edition Tauri v2 configs
```

## Build & Packaging Commands

### 1. Generate Icons (all 6 editions)

```bash
node desktop/scripts/generate-icons.mjs
```

### 2. Build & Test Pure-Rust Hub (`bell-hub -> hub-dist`)

```bash
cargo test --manifest-path desktop/Cargo.toml -p bell-core -p bell-hub -p bell-daemon
cargo run --manifest-path desktop/Cargo.toml -p bell-hub --bin build-hub-dist
```

### 3. Bundle Zero-`node_modules` Sidecars & Smoke-Verify

```bash
node desktop/scripts/bundle-sidecars.mjs --verify
```

Data-directory environment variables respected by each bundled sidecar:
- `personal`: `BELLPERSONAL_DATA_DIR` (also accepts `BELL_DATA_DIR`)
- `explain`: `BELL_DATA_DIR` / `BELL_EXPLAIN_DATA_DIR` / `BELL_DATA_PATH`
- `pr`: `BELLPR_DATA_DIR` (also accepts `BELLPR_HOME` / `BELL_DATA_DIR`)
- `reader`: `BELL_READER_DATA_DIR` (also accepts `BELL_DATA_DIR`)
- `campaign`: `BELL_CAMPAIGN_DATA_DIR` (also accepts `BELL_DATA_DIR`)

### 4. Prepare Edition Configs

```bash
# Generate desktop/editions/tauri.<edition>.conf.json for all 6 editions
node desktop/scripts/prepare-edition.mjs all

# Stage single-app sidecar resources + apply icons & config to crates/bell-tauri/tauri.conf.json
node desktop/scripts/prepare-edition.mjs explain --stage-resources --apply
```

## Security & CI Verification Guarantees

- **Zero-TypeScript Desktop Gate**: CI fails immediately if any `.ts` or `.tsx` file exists inside `desktop/`.
- **Single-Webview Full-Frame Isolation**: Standard stable `WebviewWindow` navigates directly between the embedded Hub and `http://127.0.0.1:<port>` workspaces (built-in `SuiteSwitcher` + native `⌘0`–`⌘5` / `Ctrl+0`–`Ctrl+5`), preserving every app's strict CSP (`frame-ancestors 'none'`) without iframes or multi-webview clipping.
- **Physical macOS `WindowServer` Verification**: `.github/workflows/desktop-release.yml` verifies deep bundle code signatures (`codesign --verify --deep --strict`), runs live `WKWebView` + sidecar self-tests (`BELL_DESKTOP_SELFTEST_OUT`), and queries macOS `WindowServer` via CoreGraphics (`CGWindowListCopyWindowInfo(.optionOnScreenOnly)`) to prove a physical onscreen `NSWindow` (`>= 800x550`) renders before packaging `.dmg` / `.app.tar.gz`.
- **Zero-Install Embedded Runtime & Atomic Vault**: Embeds SHA-256 verified Node.js `v22.22.1` LTS (`bell-runtime` Universal Mach-O on macOS, `bell-runtime.exe` on Windows x64 with `CREATE_NO_WINDOW`) and snapshots SQLite databases atomically.
