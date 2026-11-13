#!/usr/bin/env node
var major = Number(process.versions.node.split(".")[0]);
if (major < 20) {
  console.error(
    "BELL Suite requires Node.js 20 or later (22+ recommended for sibling SQLite apps); found " +
      process.versions.node +
      ".",
  );
  process.exit(1);
}

import { spawn, spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, readFileSync, symlinkSync, unlinkSync } from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { APP_IDS, validateManifest } from "../contracts/v1/reference.mjs";

const SUITE_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const PKG = JSON.parse(readFileSync(path.join(SUITE_ROOT, "package.json"), "utf8"));

export const SUITE_APPS = [
  {
    id: "personal",
    name: "BELL Personal",
    bin: "bell-personal",
    binRel: "bin/bell-personal.mjs",
    repo: "https://github.com/Suzu1Dev/BELL-Personal.git",
    offset: 1,
    color: "\x1b[33m",
    candidates: ["../bell-personal", "../BELL-Personal", "worktrees/personal-bot-suite"],
  },
  {
    id: "explain",
    name: "BELL Explain",
    bin: "bell-explain",
    binRel: "bin/bell-explain.mjs",
    repo: "https://github.com/Suzu1Dev/BELL-Explain.git",
    offset: 2,
    color: "\x1b[36m",
    candidates: ["../bell-explain", "../BELL-Explain", "worktrees/explain-launch"],
  },
  {
    id: "pr",
    name: "BELL PR",
    bin: "bell-pr",
    binRel: "bin/bellpr.mjs",
    repo: "https://github.com/Suzu1Dev/bell-pr.git",
    offset: 3,
    color: "\x1b[35m",
    candidates: ["../bell-pr", "../BELL-PR"],
  },
  {
    id: "reader",
    name: "BELL Web Reader",
    bin: "bell-reader",
    binRel: "bin/bell-reader.mjs",
    repo: "https://github.com/Suzu1Dev/bell-reader.git",
    offset: 4,
    color: "\x1b[32m",
    candidates: ["../bell-reader", "../BELL-Reader"],
  },
  {
    id: "campaign",
    name: "BELL Campaign",
    bin: "bell-campaign",
    binRel: "bin/bell-campaign.mjs",
    repo: "https://github.com/Suzu1Dev/bell-campaign.git",
    offset: 5,
    color: "\x1b[34m",
    candidates: ["../bell-campaign", "../BELL-Campaign"],
  },
];

const RESET = "\x1b[0m";

function commandOnPath(cmd, env = process.env) {
  const probe = process.platform === "win32" ? "where" : "which";
  const res = spawnSync(probe, [cmd], { env, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] });
  if (res.status !== 0) return null;
  const first = res.stdout.trim().split(/\r?\n/)[0];
  return first || null;
}

export function discoverApp(spec, env = process.env) {
  const envOverride = env[`BELL_SUITE_APP_${spec.id.toUpperCase()}`]?.trim();
  const dirs = [
    ...(envOverride ? [path.resolve(envOverride)] : []),
    ...spec.candidates.map((rel) => path.resolve(SUITE_ROOT, rel)),
    path.join(os.homedir(), ".local", "share", spec.bin, "app"),
  ];
  // Prefer checkouts that already ship the standalone CLI binary
  for (const dir of dirs) {
    if (existsSync(path.join(dir, "package.json")) && existsSync(path.join(dir, spec.binRel))) {
      return { kind: "checkout", dir, binPath: path.join(dir, spec.binRel) };
    }
  }
  for (const dir of dirs) {
    if (existsSync(path.join(dir, "package.json"))) {
      return { kind: "checkout", dir, binPath: path.join(dir, spec.binRel) };
    }
  }
  const globalBin = commandOnPath(spec.bin, env);
  if (globalBin) return { kind: "global", binPath: globalBin };
  return null;
}

export function parseOnlyList(raw) {
  if (!raw || !raw.trim()) return SUITE_APPS.map((a) => a.id);
  const items = raw
    .split(",")
    .map((s) => s.trim().toLowerCase())
    .filter(Boolean);
  for (const item of items) {
    if (!APP_IDS.includes(item)) {
      throw new Error(`Unknown Suite app "${item}". Expected a subset of: ${SUITE_APPS.map((a) => a.id).join(",")}`);
    }
  }
  return [...new Set(items)];
}

export function buildSuiteEnvForApp(appId, portsByApp, host = "127.0.0.1", baseEnv = process.env) {
  const connectHost = host === "0.0.0.0" || host === "::" ? "127.0.0.1" : host;
  const originHost = connectHost.includes(":") && !connectHost.startsWith("[") ? `[${connectHost}]` : connectHost;
  const fetchHost = connectHost === "localhost" ? "127.0.0.1" : originHost;

  const links = Object.entries(portsByApp)
    .filter(([id]) => id !== appId)
    .map(([id, port]) => `${id}=http://${originHost}:${port}`)
    .join(",");

  const personalFetch = ["explain", "pr", "reader"]
    .filter((id) => id !== appId && portsByApp[id])
    .map((id) => `${id}=http://${fetchHost}:${portsByApp[id]}`)
    .join(",");

  const readerFetch = ["explain", "pr"]
    .filter((id) => id !== appId && portsByApp[id])
    .map((id) => `${id}=http://${fetchHost}:${portsByApp[id]}`)
    .join(",");

  const port = String(portsByApp[appId]);
  const env = {
    ...baseEnv,
    HOST: host,
    PORT: port,
    BELL_EDITION: "suite",
    BELLPR_EDITION: "suite",
    BELLPERSONAL_EDITION: "suite",
    BELL_READER_EDITION: "suite",
    BELL_CAMPAIGN_EDITION: "suite",
    BELLPR_NO_OPEN: "1",
  };

  if (appId === "personal") {
    env.BELLPERSONAL_HOST = host;
    env.BELLPERSONAL_PORT = port;
    env.BELLPERSONAL_FRONTEND_PORT = port;
    env.BELLPERSONAL_SUITE_LINKS = links;
    if (personalFetch) env.BELLPERSONAL_SUITE_FETCH = personalFetch;
  } else if (appId === "explain") {
    env.BELL_HOST = host;
    env.BELL_PORT = port;
    env.BELL_SUITE_LINKS = links;
  } else if (appId === "pr") {
    env.BELLPR_HOST = host;
    env.BELLPR_PORT = port;
    env.BELLPR_SUITE_LINKS = links;
    env.BELL_SUITE_LINKS = links;
  } else if (appId === "reader") {
    env.BELL_READER_HOST = host;
    env.BELL_READER_PORT = port;
    env.BELL_READER_SUITE_LINKS = links;
    if (readerFetch) env.BELL_READER_SUITE_FETCH = readerFetch;
  } else if (appId === "campaign") {
    env.BELL_CAMPAIGN_HOST = host;
    env.BELL_CAMPAIGN_PORT = port;
    env.BELL_CAMPAIGN_SUITE_LINKS = links;
  }
  return env;
}

export function probeManifest(host, port, timeoutMs = 2000) {
  const connectHost = host === "0.0.0.0" || host === "::" ? "127.0.0.1" : host;
  const hostHeader = `${connectHost.includes(":") && !connectHost.startsWith("[") ? `[${connectHost}]` : connectHost}:${port}`;
  return new Promise((resolve) => {
    const req = http.get(
      {
        host: connectHost,
        port,
        path: "/api/suite/manifest",
        headers: { Host: hostHeader, Accept: "application/json" },
        timeout: timeoutMs,
      },
      (res) => {
        let body = "";
        res.setEncoding("utf8");
        res.on("data", (chunk) => {
          if (body.length < 65536) body += chunk;
        });
        res.on("end", () => {
          if (res.statusCode === 200) {
            try {
              const parsed = JSON.parse(body);
              const validated = validateManifest(parsed, { ownOrigins: [`http://${hostHeader}`] });
              resolve({ ok: true, status: 200, manifest: validated });
              return;
            } catch (err) {
              resolve({ ok: false, status: 200, error: err instanceof Error ? err.message : String(err) });
              return;
            }
          }
          if (res.statusCode === 401 || res.statusCode === 403 || res.statusCode === 404) {
            // Check /api/health for token-gated listeners (e.g. BELL PR)
            const healthReq = http.get(
              { host: connectHost, port, path: "/api/health", headers: { Host: hostHeader }, timeout: timeoutMs },
              (hRes) => {
                hRes.resume();
                resolve({
                  ok: hRes.statusCode === 200,
                  status: res.statusCode,
                  authGated: hRes.statusCode === 200,
                });
              },
            );
            healthReq.on("error", () => resolve({ ok: false, status: res.statusCode }));
            healthReq.on("timeout", () => {
              healthReq.destroy();
              resolve({ ok: false, status: res.statusCode });
            });
            return;
          }
          resolve({ ok: false, status: res.statusCode });
        });
      },
    );
    req.on("error", () => resolve({ ok: false, status: 0 }));
    req.on("timeout", () => {
      req.destroy();
      resolve({ ok: false, status: 0 });
    });
  });
}

function canOpenBrowser(env = process.env) {
  if (env.CI || env.BELL_NO_OPEN === "1" || !process.stdout.isTTY) return false;
  if (env.SSH_CONNECTION || env.SSH_CLIENT || env.SSH_TTY) return false;
  if (process.platform === "linux" && !env.DISPLAY && !env.WAYLAND_DISPLAY) return false;
  if (existsSync("/.dockerenv") || existsSync("/run/.containerenv")) return false;
  return true;
}

function openBrowser(url) {
  const [cmd, args] =
    process.platform === "darwin"
      ? ["open", [url]]
      : process.platform === "win32"
        ? ["cmd", ["/c", "start", '""', url]]
        : ["xdg-open", [url]];
  try {
    const child = spawn(cmd, args, { detached: true, stdio: "ignore", windowsHide: true });
    child.on("error", () => {});
    child.unref();
  } catch {
    // non-fatal
  }
}

function renderDashboardTable(rows) {
  const cols = [
    { key: "idx", title: "#", width: 3 },
    { key: "name", title: "Workspace", width: 17 },
    { key: "id", title: "App ID", width: 10 },
    { key: "url", title: "URL", width: 25 },
    { key: "state", title: "Status", width: 20 },
  ];
  for (const r of rows) {
    for (const c of cols) c.width = Math.max(c.width, String(r[c.key]).length);
  }
  const sep = "+-" + cols.map((c) => "-".repeat(c.width)).join("-+-") + "-+";
  const fmt = (obj) =>
    "| " + cols.map((c) => String(obj[c.key]).padEnd(c.width)).join(" | ") + " |";
  const header = fmt(Object.fromEntries(cols.map((c) => [c.key, c.title])));
  return [sep, header, sep, ...rows.map(fmt), sep].join("\n");
}

function printHelp() {
  console.log(`BELL Suite v${PKG.version} — Unified 5-App Local-First Orchestrator

Usage:
  bell-suite [start] [options]     Start BELL Suite workspaces together
  bell-suite status [options]      Probe running Suite workspaces & validate manifests
  bell-suite doctor [options]      Check runtime, AI CLIs, checkouts, and Suite health
  bell-suite setup [options]       Clone/update & build missing BELL Suite apps from GitHub

Options:
  --base-port <n>    Base port (default: 4600 -> personal:4601, explain:4602, pr:4603, reader:4604, campaign:4605)
  --host <host>      Bind host (default: 127.0.0.1)
  --only <apps>      Comma-separated subset of: personal,explain,pr,reader,campaign
  --no-open          Do not open the browser automatically on start
  --prefix <dir>     Install prefix for 'bell-suite setup' (default: ~/.local)
  --json             Output machine-readable JSON (for status / doctor)
  -v, --version      Print version
  -h, --help         Show this help
`);
}

async function runStatusOrDoctor(mode, opts) {
  const basePort = Number(opts["base-port"] || process.env.BELL_SUITE_BASE_PORT || 4600);
  if (!Number.isInteger(basePort) || basePort < 1024 || basePort > 65520) {
    throw new Error("--base-port must be an integer between 1024 and 65520.");
  }
  const host = (opts.host || "127.0.0.1").trim();
  const selected = new Set(parseOnlyList(opts.only));

  const workspaceChecks = [];
  for (const spec of SUITE_APPS) {
    if (!selected.has(spec.id)) continue;
    const port = basePort + spec.offset;
    const found = discoverApp(spec);
    const probe = await probeManifest(host, port, 1500);
    workspaceChecks.push({
      appId: spec.id,
      name: spec.name,
      port,
      url: `http://${host}:${port}`,
      installed: Boolean(found),
      source: found ? (found.kind === "checkout" ? found.dir : found.binPath) : null,
      running: probe.ok,
      manifestValid: Boolean(probe.manifest),
      authGated: Boolean(probe.authGated),
      error: probe.error ?? null,
    });
  }

  const tools = {
    node: process.versions.node,
    nodeOk: Number(process.versions.node.split(".")[0]) >= 20,
    sqliteBuiltin: Number(process.versions.node.split(".")[0]) >= 22,
    npm: Boolean(commandOnPath("npm")),
    git: Boolean(commandOnPath("git")),
    claude: Boolean(commandOnPath("claude")),
    codex: Boolean(commandOnPath("codex")),
    gh: Boolean(commandOnPath("gh")),
  };

  const payload = {
    suiteVersion: PKG.version,
    mode,
    basePort,
    host,
    tools,
    workspaces: workspaceChecks,
  };

  if (opts.json) {
    console.log(JSON.stringify(payload, null, 2));
    return;
  }

  console.log(`BELL Suite v${PKG.version} (${mode})`);
  if (mode === "doctor") {
    console.log(
      `Runtime: Node ${tools.node} (${tools.sqliteBuiltin ? "node:sqlite ready" : "upgrade to Node 22+ for SQLite apps"}) | git:${tools.git ? "ok" : "missing"} | npm:${tools.npm ? "ok" : "missing"}`,
    );
    console.log(
      `Optional CLIs: claude:${tools.claude ? "found" : "not found"} | codex:${tools.codex ? "found" : "not found"} | gh:${tools.gh ? "found" : "not found"}`,
    );
  }
  const rows = workspaceChecks.map((w, idx) => ({
    idx: idx + 1,
    name: w.name,
    id: w.appId,
    url: w.url,
    state: w.running
      ? w.manifestValid
        ? "READY (manifest v1)"
        : "READY (health ok)"
      : w.installed
        ? "STOPPED (installed)"
        : "NOT INSTALLED",
  }));
  console.log(renderDashboardTable(rows));
}

async function runSetup(opts) {
  const prefix = path.resolve(opts.prefix || path.join(os.homedir(), ".local"));
  const selected = new Set(parseOnlyList(opts.only));
  if (!commandOnPath("git") || !commandOnPath("npm")) {
    throw new Error("Both 'git' and 'npm' are required for 'bell-suite setup'.");
  }
  mkdirSync(path.join(prefix, "bin"), { recursive: true });
  for (const spec of SUITE_APPS) {
    if (!selected.has(spec.id)) continue;
    const targetDir = path.join(prefix, "share", spec.bin, "app");
    mkdirSync(path.dirname(targetDir), { recursive: true });
    if (existsSync(path.join(targetDir, ".git"))) {
      console.log(`[${spec.id}] Updating ${targetDir}...`);
      spawnSync("git", ["-C", targetDir, "pull", "--ff-only"], { stdio: "inherit" });
    } else {
      console.log(`[${spec.id}] Cloning ${spec.repo} -> ${targetDir}...`);
      const clone = spawnSync("git", ["clone", "--depth", "1", spec.repo, targetDir], { stdio: "inherit" });
      if (clone.status !== 0) throw new Error(`Failed to clone ${spec.repo}`);
    }
    console.log(`[${spec.id}] Installing dependencies & building...`);
    const inst = spawnSync("npm", ["install", "--no-audit", "--no-fund"], { cwd: targetDir, stdio: "inherit" });
    if (inst.status !== 0) throw new Error(`npm install failed for ${spec.name}`);
    const bld = spawnSync("npm", ["run", "build"], { cwd: targetDir, stdio: "inherit" });
    if (bld.status !== 0) throw new Error(`npm run build failed for ${spec.name}`);
    const binSource = path.join(targetDir, spec.binRel);
    if (existsSync(binSource)) {
      chmodSync(binSource, 0o755);
      const linkPath = path.join(prefix, "bin", spec.bin);
      try {
        unlinkSync(linkPath);
      } catch {}
      symlinkSync(binSource, linkPath);
      console.log(`[${spec.id}] Linked ${linkPath}`);
    }
  }
  console.log(`\nBELL Suite setup complete. Launch everything with: bell-suite start`);
}

function prefixStream(stream, tag, color, target) {
  let buffer = "";
  stream.setEncoding("utf8");
  stream.on("data", (chunk) => {
    buffer += chunk;
    const lines = buffer.split(/\r?\n/);
    buffer = lines.pop() ?? "";
    for (const line of lines) {
      if (line.trim()) target.write(`${color}[${tag}]${RESET} ${line}\n`);
    }
  });
  stream.on("end", () => {
    if (buffer.trim()) target.write(`${color}[${tag}]${RESET} ${buffer}\n`);
  });
}

function resolveSpawnCommand(spec, found, port, host) {
  if (found.kind === "checkout") {
    const { dir, binPath } = found;
    if (existsSync(binPath)) {
      const args = [binPath, "start", "--port", String(port), "--host", host, "--no-open"];
      if (spec.id === "pr") args.push("--force");
      return { cmd: process.execPath, args, cwd: dir };
    }
    // Fallback if checkout does not have bin/<cmd>.mjs yet
    if (!existsSync(path.join(dir, "dist", "index.html"))) {
      console.log(`${spec.color}[${spec.id}]${RESET} Building frontend dist/...`);
      spawnSync("npm", ["run", "build"], { cwd: dir, stdio: "inherit" });
    }
    const tsxBin = path.join(dir, "node_modules", ".bin", process.platform === "win32" ? "tsx.cmd" : "tsx");
    if (existsSync(tsxBin) && existsSync(path.join(dir, "server", "index.ts"))) {
      return { cmd: tsxBin, args: ["server/index.ts"], cwd: dir };
    }
    return { cmd: "npm", args: ["run", "start"], cwd: dir };
  }
  const args = ["start", "--port", String(port), "--host", host, "--no-open"];
  if (spec.id === "pr") args.push("--force");
  return { cmd: found.binPath, args, cwd: process.cwd() };
}

async function waitForReady(host, port, child, timeoutMs = 25000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) return { ok: false, exited: child.exitCode };
    const probe = await probeManifest(host, port, 1200);
    if (probe.ok) return probe;
    await new Promise((r) => setTimeout(r, 250));
  }
  return { ok: false, timeout: true };
}

async function runStart(opts) {
  const basePort = Number(opts["base-port"] || process.env.BELL_SUITE_BASE_PORT || 4600);
  if (!Number.isInteger(basePort) || basePort < 1024 || basePort > 65520) {
    throw new Error("--base-port must be an integer between 1024 and 65520.");
  }
  const host = (opts.host || "127.0.0.1").trim();
  const selectedIds = parseOnlyList(opts.only);
  const selectedSpecs = SUITE_APPS.filter((a) => selectedIds.includes(a.id));

  const discovered = new Map();
  const portsByApp = {};
  for (const spec of selectedSpecs) {
    const found = discoverApp(spec);
    if (found) {
      discovered.set(spec.id, found);
      portsByApp[spec.id] = basePort + spec.offset;
    }
  }

  if (discovered.size === 0) {
    console.error(
      "No BELL Suite apps were found locally. Run `bell-suite setup` to install them from GitHub, or place sibling checkouts beside bell-suite.",
    );
    process.exit(1);
  }

  const children = [];
  let shuttingDown = false;
  const shutdown = (signal) => {
    if (shuttingDown) return;
    shuttingDown = true;
    console.log(`\nStopping ${children.length} BELL Suite workspace(s) (${signal})...`);
    for (const child of children) {
      if (child.exitCode === null) {
        try {
          child.kill("SIGTERM");
        } catch {}
      }
    }
    setTimeout(() => {
      for (const child of children) {
        if (child.exitCode === null) {
          try {
            child.kill("SIGKILL");
          } catch {}
        }
      }
      process.exit(0);
    }, 5000).unref();
  };

  process.on("SIGINT", () => shutdown("SIGINT"));
  process.on("SIGTERM", () => shutdown("SIGTERM"));

  const statusByApp = new Map();
  for (const spec of selectedSpecs) {
    const found = discovered.get(spec.id);
    if (!found) {
      statusByApp.set(spec.id, { state: "MISSING (run setup)", url: "-" });
      continue;
    }
    const port = portsByApp[spec.id];
    const env = buildSuiteEnvForApp(spec.id, portsByApp, host);
    const { cmd, args, cwd } = resolveSpawnCommand(spec, found, port, host);
    const child = spawn(cmd, args, {
      cwd,
      env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    children.push(child);
    prefixStream(child.stdout, spec.id, spec.color, process.stdout);
    prefixStream(child.stderr, spec.id, spec.color, process.stderr);
    statusByApp.set(spec.id, { child, port, url: `http://${host}:${port}` });
  }

  await Promise.all(
    selectedSpecs.map(async (spec) => {
      const entry = statusByApp.get(spec.id);
      if (!entry?.child) return;
      const probe = await waitForReady(host, entry.port, entry.child);
      entry.state = probe.ok
        ? probe.manifest
          ? "READY (manifest v1)"
          : "READY (health ok)"
        : entry.child.exitCode !== null
          ? `EXITED (${entry.child.exitCode})`
          : "STARTING / TIMEOUT";
    }),
  );

  const rows = selectedSpecs.map((spec, idx) => {
    const entry = statusByApp.get(spec.id);
    return {
      idx: `⌃${spec.offset}`,
      name: spec.name,
      id: spec.id,
      url: entry?.url ?? "-",
      state: entry?.state ?? "MISSING",
    };
  });

  console.log(`\nBELL Suite v${PKG.version} — Unified Local Workspaces\n` + renderDashboardTable(rows) + "\n");

  const firstReady = selectedSpecs.map((s) => statusByApp.get(s.id)).find((e) => e?.state?.startsWith("READY"));
  if (firstReady && !opts["no-open"] && canOpenBrowser()) {
    openBrowser(firstReady.url);
  }

  await Promise.all(
    children.map(
      (child) =>
        new Promise((resolve) => {
          if (child.exitCode !== null) resolve();
          else child.once("exit", () => resolve());
        }),
    ),
  );
}

export async function runCli(argv = process.argv.slice(2)) {
  const { values, positionals } = parseArgs({
    args: argv,
    allowPositionals: true,
    strict: true,
    options: {
      "base-port": { type: "string" },
      host: { type: "string" },
      only: { type: "string" },
      "no-open": { type: "boolean", default: false },
      prefix: { type: "string" },
      json: { type: "boolean", default: false },
      version: { type: "boolean", short: "v", default: false },
      help: { type: "boolean", short: "h", default: false },
    },
  });

  if (values.version) {
    console.log(PKG.version);
    return;
  }
  if (values.help) {
    printHelp();
    return;
  }

  const command = positionals[0] ?? "start";
  if (command === "start") {
    await runStart(values);
  } else if (command === "status" || command === "doctor") {
    await runStatusOrDoctor(command, values);
  } else if (command === "setup") {
    await runSetup(values);
  } else {
    throw new Error(`Unknown command "${command}". Run 'bell-suite --help' for usage.`);
  }
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  runCli().catch((err) => {
    console.error(`bell-suite: ${err instanceof Error ? err.message : String(err)}`);
    process.exit(1);
  });
}
