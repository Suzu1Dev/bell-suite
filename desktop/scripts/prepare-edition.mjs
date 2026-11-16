#!/usr/bin/env node
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const DESKTOP_DIR = path.resolve(SCRIPT_DIR, "..");
const SUITE_ROOT = path.resolve(DESKTOP_DIR, "..");
const EDITIONS_DIR = path.join(DESKTOP_DIR, "editions");
const ICONS_ROOT = path.join(DESKTOP_DIR, "icons");
const SIDECARS_DIR = path.join(DESKTOP_DIR, "dist-sidecars");
const TAURI_CRATE_DIR = path.join(DESKTOP_DIR, "crates", "bell-tauri");
const TAURI_CONF_PATH = path.join(TAURI_CRATE_DIR, "tauri.conf.json");
const TAURI_ICONS_DIR = path.join(TAURI_CRATE_DIR, "icons");

const SUITE_PKG = JSON.parse(readFileSync(path.join(SUITE_ROOT, "package.json"), "utf8"));
const UI_PKG = JSON.parse(readFileSync(path.join(DESKTOP_DIR, "ui", "package.json"), "utf8"));
const DESKTOP_VERSION = UI_PKG.version || SUITE_PKG.version;

export const DEFAULT_CSP =
  "default-src 'self' tauri: http://tauri.localhost https://tauri.localhost; script-src 'self' tauri: http://tauri.localhost https://tauri.localhost; style-src 'self' 'unsafe-inline' tauri: http://tauri.localhost https://tauri.localhost; img-src 'self' tauri: http://tauri.localhost https://tauri.localhost asset: http://asset.localhost https://asset.localhost data: blob:; font-src 'self' tauri: http://tauri.localhost https://tauri.localhost data:; connect-src 'self' tauri: http://tauri.localhost https://tauri.localhost ipc: ipc://localhost http://ipc.localhost https://ipc.localhost http://127.0.0.1:*; object-src 'none'; base-uri 'self'; form-action 'self'";

export const EDITION_SPECS = {
  suite: {
    id: "suite",
    productName: "BELL Suite",
    identifier: "com.bell1dev.suite",
    defaultPort: 4600,
    accentLight: "#0f8a7d",
    accentDark: "#43cabc",
  },
  personal: {
    id: "personal",
    productName: "BELL Personal",
    identifier: "com.bell1dev.personal",
    defaultPort: 4601,
    accentLight: "#a4532c",
    accentDark: "#e08a5c",
  },
  explain: {
    id: "explain",
    productName: "BELL Explain",
    identifier: "com.bell1dev.explain",
    defaultPort: 4602,
    accentLight: "#5b45e0",
    accentDark: "#a799ff",
  },
  pr: {
    id: "pr",
    productName: "BELL PR",
    identifier: "com.bell1dev.pr",
    defaultPort: 4603,
    accentLight: "#1b6b4c",
    accentDark: "#46c289",
  },
  reader: {
    id: "reader",
    productName: "BELL Web Reader",
    identifier: "com.bell1dev.reader",
    defaultPort: 4604,
    accentLight: "#0b6bcb",
    accentDark: "#38aef8",
  },
  campaign: {
    id: "campaign",
    productName: "BELL Campaign",
    identifier: "com.bell1dev.campaign",
    defaultPort: 4605,
    accentLight: "#9e2b5c",
    accentDark: "#e25b91",
  },
};

const ICON_FILES = ["32x32.png", "128x128.png", "128x128@2x.png", "512x512.png", "icon.ico", "icon.icns"];

function stageResourcesForEdition(spec) {
  const stagedRoot = path.join(EDITIONS_DIR, ".staged", spec.id);
  const stagedSidecars = path.join(stagedRoot, "dist-sidecars");
  rmSync(stagedRoot, { recursive: true, force: true });
  mkdirSync(path.join(stagedSidecars, "bin"), { recursive: true });
  mkdirSync(path.join(stagedSidecars, "apps"), { recursive: true });

  const binSrc = path.join(SIDECARS_DIR, "bin");
  if (existsSync(binSrc)) {
    cpSync(binSrc, path.join(stagedSidecars, "bin"), { recursive: true });
  } else {
    writeFileSync(path.join(stagedSidecars, "bin", ".keep"), "");
  }

  if (spec.id === "suite") {
    const appsSrc = path.join(SIDECARS_DIR, "apps");
    if (!existsSync(appsSrc)) {
      throw new Error(`Missing ${appsSrc}. Run bundle-sidecars.mjs before --stage-resources.`);
    }
    cpSync(appsSrc, path.join(stagedSidecars, "apps"), { recursive: true });
  } else {
    const appSrc = path.join(SIDECARS_DIR, "apps", spec.id);
    if (!existsSync(appSrc)) {
      throw new Error(`Missing ${appSrc}. Run bundle-sidecars.mjs before --stage-resources.`);
    }
    cpSync(appSrc, path.join(stagedSidecars, "apps", spec.id), { recursive: true });
  }

  writeFileSync(
    path.join(stagedSidecars, "edition.json"),
    JSON.stringify({ edition: spec.id, version: DESKTOP_VERSION }, null, 2) + "\n",
  );

  return `../../editions/.staged/${spec.id}/dist-sidecars`;
}

export function buildTauriConfig(
  spec,
  {
    resourceSource = "../../dist-sidecars",
    baseConfig = null,
    hasDeveloperIdCert = Boolean(process.env.APPLE_CERTIFICATE && process.env.APPLE_CERTIFICATE.trim()),
  } = {},
) {
  const baseWindows = Array.isArray(baseConfig?.app?.windows) && baseConfig.app.windows.length > 0
    ? baseConfig.app.windows[0]
    : {};

  return {
    $schema: "https://schema.tauri.app/config/2",
    ...(baseConfig ?? {}),
    productName: spec.productName,
    mainBinaryName: "bell-desktop",
    version: DESKTOP_VERSION,
    identifier: spec.identifier,
    build: {
      frontendDist: "../../ui/dist",
      devUrl: "http://127.0.0.1:5173",
      ...(baseConfig?.build ?? {}),
    },
    app: {
      ...(baseConfig?.app ?? {}),
      withGlobalTauri: true,
      windows: [
        {
          label: "main",
          create: false,
          ...baseWindows,
          title: spec.productName,
          width: 1360,
          height: 860,
          minWidth: 900,
          minHeight: 620,
          resizable: true,
          fullscreen: false,
        },
      ],
      security: {
        ...(baseConfig?.app?.security ?? {}),
        csp: DEFAULT_CSP,
      },
    },
    bundle: {
      ...(baseConfig?.bundle ?? {}),
      active: true,
      targets: ["dmg", "app", "msi", "nsis"],
      icon: [
        "icons/32x32.png",
        "icons/128x128.png",
        "icons/128x128@2x.png",
        "icons/icon.icns",
        "icons/icon.ico",
      ],
      resources: {
        [resourceSource]: "dist-sidecars",
      },
      macOS: {
        ...(baseConfig?.bundle?.macOS ?? {}),
        minimumSystemVersion: "12.0",
        // Ad-hoc signed macOS builds (`APPLE_SIGNING_IDENTITY="-"`) must NOT set CS_RUNTIME
        // (`hardenedRuntime: true`), otherwise WKWebView WebContent/GPU XPC fails to attach
        // and leaves TaoView's uninitialized black OpenGL layer visible.
        hardenedRuntime: hasDeveloperIdCert,
        entitlements: hasDeveloperIdCert ? "../../editions/entitlements.macos.plist" : null,
      },
      windows: {
        ...(baseConfig?.bundle?.windows ?? {}),
        webviewInstallMode: {
          type: "downloadBootstrapper",
        },
        wix: {
          language: "en-US",
          ...(baseConfig?.bundle?.windows?.wix ?? {}),
        },
        nsis: {
          installMode: "both",
          ...(baseConfig?.bundle?.windows?.nsis ?? {}),
        },
      },
    },
  };
}

function syncEditionIconsToTauri(editionId) {
  const srcDir = path.join(ICONS_ROOT, editionId);
  if (!existsSync(srcDir)) {
    throw new Error(`Icons missing for edition "${editionId}" at ${srcDir}. Run generate-icons.mjs first.`);
  }
  mkdirSync(TAURI_ICONS_DIR, { recursive: true });
  for (const file of ICON_FILES) {
    const src = path.join(srcDir, file);
    if (existsSync(src)) {
      copyFileSync(src, path.join(TAURI_ICONS_DIR, file));
    }
  }
}

function printUsage() {
  console.log(`Usage: node desktop/scripts/prepare-edition.mjs <suite|personal|explain|pr|reader|campaign|all> [--apply] [--stage-resources]`);
}

function main() {
  const args = process.argv.slice(2);
  if (args.includes("-h") || args.includes("--help")) {
    printUsage();
    return;
  }

  const apply = args.includes("--apply");
  const stageResources = args.includes("--stage-resources");
  const positional = args.filter((a) => !a.startsWith("-"));
  const target = (positional[0] || "all").toLowerCase();

  if (target !== "all" && !(target in EDITION_SPECS)) {
    throw new Error(
      `Unknown edition "${target}". Expected one of: ${Object.keys(EDITION_SPECS).join(", ")}, all`,
    );
  }
  if (apply && target === "all") {
    throw new Error(`--apply requires a single edition (${Object.keys(EDITION_SPECS).join("|")}).`);
  }

  mkdirSync(EDITIONS_DIR, { recursive: true });

  // Read existing crates/bell-tauri/tauri.conf.json as merge base only when --apply is passed
  let baseConfig = null;
  if (apply && existsSync(TAURI_CONF_PATH)) {
    try {
      baseConfig = JSON.parse(readFileSync(TAURI_CONF_PATH, "utf8"));
    } catch {
      baseConfig = null;
    }
  }

  const targets = target === "all" ? Object.values(EDITION_SPECS) : [EDITION_SPECS[target]];
  for (const spec of targets) {
    // Canonical edition configs always reference ../../dist-sidecars (never polluted with .staged paths)
    const canonicalConf = buildTauriConfig(spec, { resourceSource: "../../dist-sidecars" });
    const outPath = path.join(EDITIONS_DIR, `tauri.${spec.id}.conf.json`);
    writeFileSync(outPath, JSON.stringify(canonicalConf, null, 2) + "\n");
    console.log(`Generated ${path.relative(SUITE_ROOT, outPath)} (${spec.productName} / ${spec.identifier})`);

    const stagedSource = stageResources ? stageResourcesForEdition(spec) : "../../dist-sidecars";
    if (apply) {
      const appliedConf = buildTauriConfig(spec, {
        resourceSource: stagedSource,
        baseConfig,
      });
      syncEditionIconsToTauri(spec.id);
      writeFileSync(TAURI_CONF_PATH, JSON.stringify(appliedConf, null, 2) + "\n");
      console.log(`Applied ${spec.id} icons & config to ${path.relative(SUITE_ROOT, TAURI_CONF_PATH)}`);
    }
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (err) {
    console.error(`prepare-edition: ${err instanceof Error ? err.message : String(err)}`);
    process.exit(1);
  }
}
