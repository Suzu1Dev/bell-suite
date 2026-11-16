# BELL Desktop (`v1.0.4` — macOS & Windows)

Native macOS **Apple Silicon (`aarch64`)**, **macOS Intel (`x86_64`)**, and **Windows (`x86_64`)** desktop application and supervisor for **BELL Suite** and all 5 local-first workspaces, built with a **100% Pure-Rust** desktop workspace (`bell-core`, `bell-hub`, `bell-daemon`, `bell-tauri` — zero TypeScript in `desktop/`), standard Tauri v2 Single-Webview `WebviewWindow` architecture, and single-archive (`apps.tar.gz`) minified Node 22 LTS sidecars.

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
│   ├── bell-core/               # Supervisor, apps.tar.gz cache unpacker, Today feed, atomic SQLite backups
│   ├── bell-hub/                # Pure-Rust Linear-grade Suite Hub asset emitter (`build-hub-dist` -> hub-dist/)
│   ├── bell-daemon/             # Loopback HTTP control plane (127.0.0.1:4600) serving hub-dist + REST API
│   └── bell-tauri/              # Native Tauri v2 Single-Webview (`WebviewWindow`) app + menu/tray + Shortcuts
├── hub-dist/                    # Generated static Hub assets (`index.html`, `hub.css`, `hub.js`)
├── icons/                       # Generated squircle PNG/ICO/ICNS icons for all 6 editions
├── editions/                    # Tauri v2 configs (tauri.<edition>.conf.json) + macOS entitlements plists
├── dist-sidecars/               # Gitignored minified CJS workspace bundles + embedded Node runtime
└── scripts/
    ├── generate-icons.mjs       # Pure Node RGBA/ICO/ICNS icon generator (zero external dependencies)
    ├── bundle-sidecars.mjs      # Minifies & bundles all 5 backends into dist-sidecars/apps/<id>/server.cjs
    └── prepare-edition.mjs      # Generates edition configs & packs staged apps/ into single apps.tar.gz
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

### 3. Bundle Minified Zero-`node_modules` Sidecars & Smoke-Verify

```bash
node desktop/scripts/bundle-sidecars.mjs --verify
```

### 4. Prepare Edition Configs & Single-Archive Resource Staging

```bash
# Generate canonical desktop/editions/tauri.<edition>.conf.json for all 6 editions
node desktop/scripts/prepare-edition.mjs all

# Pack single apps.tar.gz into editions/.staged/<id>/dist-sidecars + apply config & icons
node desktop/scripts/prepare-edition.mjs suite --stage-resources --apply
```

## Launch-Speed & Security Guarantees (`v1.0.4`)

- **Single `apps.tar.gz` Sidecar Bundle (`<= 6` Sealed Files)**: Packaged installers ship only `edition.json`, `apps.tar.gz`, and `bin/bell-runtime(.exe)` inside `dist-sidecars/` (eliminating 482 loose files from macOS `syspolicyd` / `XProtect` / `CodeResources` scanning) and unpack once into `<vault>/runtime-cache/<edition>-<version>/`.
- **Native Stripped `aarch64` & `x64` Binaries**: Separate native Apple Silicon (`_aarch64.dmg`) and Intel (`_x64.dmg`) builds with `strip -x` on `bell-runtime` and `[profile.release] strip = "symbols", lto = "thin"` on `bell-desktop`.
- **Millisecond CI Launch Gate**: `.github/workflows/desktop-release.yml` enforces Zero-TypeScript in `desktop/`, `<= 15` sealed resource files, `codesign --verify --deep --strict`, live `WKWebView` + sidecar self-tests, and a compiled Swift CoreGraphics `WindowServer` stopwatch (`COLD_WINDOW_OPEN_MS` + `WARM_WINDOW_OPEN_MS < 2000ms`).
