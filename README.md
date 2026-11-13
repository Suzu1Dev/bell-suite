# BELL Suite

[繁體中文說明 (README.zh-TW.md)](./README.zh-TW.md)

**BELL Suite** is a local-first, privacy-preserving suite of five interconnected workspaces—each usable as an **independent standalone app** with zero configuration, or orchestrated together as a unified 5-app suite (`⌃1`–`⌃5` switcher, Today feed aggregation, and zero-write draft handoffs).

| Shortcut | Product | Standalone CLI | Default Suite Port | Role |
| :--- | :--- | :--- | :--- | :--- |
| `⌃1` | **BELL Personal** | `bell-personal` | `4601` | Today dashboard, tasks, calendar, ideas & inbox |
| `⌃2` | **BELL Explain** | `bell-explain` | `4602` | Interactive concept explanations, code walkthroughs & FSRS review |
| `⌃3` | **BELL PR** | `bell-pr` | `4603` | Local AI-assisted open-source contribution & code review workbench |
| `⌃4` | **BELL Web Reader** | `bell-reader` | `4604` | Clean web capture, paper-like ink annotations & hand-drawn sketches |
| `⌃5` | **BELL Campaign** | `bell-campaign` | `4605` | Event countdown timeline, team tasks, forms & handbook generator |

---

## Quick Start

### 1. Install via `curl | sh` (macOS / Linux / WSL2)

```sh
curl -fsSL https://raw.githubusercontent.com/Suzu1Dev/bell-suite/main/install.sh | sh
```

### 2. Install on Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/Suzu1Dev/bell-suite/main/install.ps1 | iex
```

### 3. Run via `npx` or Clone from GitHub

```sh
# Run directly with npx (once published)
npx bell-suite doctor

# Or clone from GitHub
git clone https://github.com/Suzu1Dev/bell-suite.git
cd bell-suite
./bin/bell-suite.mjs setup   # Clones & builds any missing BELL apps into ~/.local
./bin/bell-suite.mjs start   # Launches all 5 workspaces with unified Suite links
```

---

## CLI Usage (`bell-suite`)

```sh
bell-suite start                       # Start all discovered workspaces on ports 4601–4605
bell-suite start --base-port 5600      # Customize base port (5601–5605)
bell-suite start --only explain,reader # Launch only selected Suite apps
bell-suite start --no-open             # Headless mode (do not open browser)
bell-suite status                      # Probe ports & validate /api/suite/manifest
bell-suite doctor [--json]             # Check Node, Git, AI CLIs & workspace installations
bell-suite setup                       # Clone/update & build all 5 BELL apps from GitHub
```

---

## Architecture & Suite Contract v1

Every app in BELL Suite runs its own local process and SQLite database—apps **never** share databases or write directly to sibling storage:

1. **Manifest (`/api/suite/manifest`)**: Declares `appId`, `displayName`, `capabilities`, and validated loopback/tailnet `links` so every `⌃1`–`⌃5` switcher stays in sync.
2. **Today Feed (`/api/suite/today`)**: Data-minimized read-only summary (counts, titles, and `#/` object routes—never private bodies or secrets) aggregated by BELL Personal over loopback HTTP (`BELLPERSONAL_SUITE_FETCH`).
3. **Two-Way Draft Handoff (`#/<route>?draft=...&from=<app>&back=<hash>`)**: Browser-only URL navigation that opens a pre-filled draft in the receiving app without writing to disk until the user confirms:
   - **BELL Personal**: `#/tasks?draft=...`
   - **BELL Explain**: `#/explain?text=...&title=...`
   - **BELL PR**: `#/discover?draft=...&title=...`
   - **BELL Web Reader**: `#/read?text=...&title=...`
   - **BELL Campaign**: `#/tasks?draft=...&title=...`

Validate the executable contract specification and all 130+ test vectors:

```sh
npm test
# or: node contracts/v1/check.mjs
```

## License

[Apache-2.0](./LICENSE) · [Trademarks](./TRADEMARKS.md)