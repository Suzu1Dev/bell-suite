#!/usr/bin/env node
import { spawn, spawnSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import http from "node:http";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { validateManifest } from "../../contracts/v1/reference.mjs";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const DESKTOP_DIR = path.resolve(SCRIPT_DIR, "..");
const SUITE_ROOT = path.resolve(DESKTOP_DIR, "..");
const OUT_ROOT = path.join(DESKTOP_DIR, "dist-sidecars");

export const WORKSPACE_SPECS = [
  {
    id: "personal",
    defaultPort: 4601,
    verifyPort: 15601,
    dataEnv: "BELLPERSONAL_DATA_DIR",
    candidates: ["worktrees/personal-bot-suite", "../bell-personal", "../BELL-Personal"],
  },
  {
    id: "explain",
    defaultPort: 4602,
    verifyPort: 15602,
    dataEnv: "BELL_DATA_DIR",
    candidates: ["worktrees/explain-launch", "../bell-explain", "../BELL-Explain"],
  },
  {
    id: "pr",
    defaultPort: 4603,
    verifyPort: 15603,
    dataEnv: "BELLPR_DATA_DIR",
    candidates: ["../bell-pr", "../BELL-PR"],
  },
  {
    id: "reader",
    defaultPort: 4604,
    verifyPort: 15604,
    dataEnv: "BELL_READER_DATA_DIR",
    candidates: ["../bell-reader", "../BELL-Reader"],
  },
  {
    id: "campaign",
    defaultPort: 4605,
    verifyPort: 15605,
    dataEnv: "BELL_CAMPAIGN_DATA_DIR",
    candidates: ["../bell-campaign", "../BELL-Campaign"],
  },
];

function resolveAppDir(spec) {
  const override = process.env[`BELL_SUITE_APP_${spec.id.toUpperCase()}`]?.trim();
  const dirs = [
    ...(override ? [path.resolve(override)] : []),
    ...spec.candidates.map((rel) => path.resolve(SUITE_ROOT, rel)),
  ];
  for (const dir of dirs) {
    if (existsSync(path.join(dir, "package.json"))) return dir;
  }
  throw new Error(
    `Cannot locate checkout for "${spec.id}". Checked: ${dirs.join(", ")}`,
  );
}

function loadEsbuild(appDirs) {
  const candidates = [
    process.env.BELL_ESBUILD_PATH,
    path.resolve(SUITE_ROOT, "../bell-pr/node_modules/esbuild"),
    ...Object.values(appDirs).map((d) => path.join(d, "node_modules", "esbuild")),
  ].filter(Boolean);

  const req = createRequire(import.meta.url);
  for (const candidate of candidates) {
    if (existsSync(candidate)) {
      return req(candidate);
    }
  }
  return req("esbuild");
}

function ensureFrontendDist(id, appDir) {
  const indexHtml = path.join(appDir, "dist", "index.html");
  if (existsSync(indexHtml)) return;
  console.log(`[${id}] dist/index.html missing in ${appDir}; running npm run build...`);
  const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm";
  const res = spawnSync(npmCmd, ["run", "build"], { cwd: appDir, stdio: "inherit" });
  if (res.status !== 0 || !existsSync(indexHtml)) {
    throw new Error(`[${id}] Failed to build frontend dist/ in ${appDir}`);
  }
}

const CLI_PRELUDE = `
function __bellParseCliArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--port" && argv[i + 1]) out.port = argv[++i];
    else if (a.startsWith("--port=")) out.port = a.slice(7);
    else if (a === "--host" && argv[i + 1]) out.host = argv[++i];
    else if (a.startsWith("--host=")) out.host = a.slice(7);
    else if (a === "--data-dir" && argv[i + 1]) out.dataDir = argv[++i];
    else if (a.startsWith("--data-dir=")) out.dataDir = a.slice(11);
  }
  return out;
}
`;

function buildEntrySource(spec) {
  if (spec.id === "pr") {
    return `
import { mkdirSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { run } from "./server/cli.ts";
process.env.BELLPR_NO_OPEN ||= "1";
const prDataDir = path.resolve(
  process.env.BELLPR_DATA_DIR ||
    process.env.BELLPR_HOME ||
    process.env.BELL_DATA_DIR ||
    path.join(os.homedir(), ".bellpr"),
);
mkdirSync(prDataDir, { recursive: true, mode: 0o700 });
process.env.BELLPR_DATA_DIR = prDataDir;
const args = process.argv.slice(2);
if (args.length > 0) {
  run(args);
} else {
  const port = process.env.BELLPR_PORT || process.env.PORT || "${spec.defaultPort}";
  const host = process.env.BELLPR_HOST || process.env.HOST || "127.0.0.1";
  run(["start", "--port", port, "--host", host, "--data-dir", prDataDir, "--no-open", "--force"]);
}
`;
  }

  if (spec.id === "personal") {
    return `
import { mkdirSync } from "node:fs";
import os from "node:os";
import path from "node:path";
${CLI_PRELUDE}
const cli = __bellParseCliArgs(process.argv.slice(2));
process.env.BELLPERSONAL_HOST ||= cli.host || process.env.HOST || "127.0.0.1";
process.env.BELLPERSONAL_PORT ||= cli.port || process.env.PORT || "${spec.defaultPort}";
process.env.BELLPERSONAL_FRONTEND_PORT ||= process.env.BELLPERSONAL_PORT;
const dataDir = path.resolve(
  cli.dataDir ||
    process.env.BELLPERSONAL_DATA_DIR ||
    process.env.BELL_DATA_DIR ||
    path.join(os.homedir(), ".bell-personal", "data"),
);
mkdirSync(dataDir, { recursive: true, mode: 0o700 });
process.env.BELLPERSONAL_DATA_DIR = dataDir;
process.env.BELLPERSONAL_SECRETS_DIR ||= path.join(dataDir, ".secrets");
process.argv[1] = path.join(__dirname, "_server", "server.cjs");
require("./server/index.ts");
`;
  }

  if (spec.id === "explain") {
    return `
import { mkdirSync } from "node:fs";
import os from "node:os";
import path from "node:path";
${CLI_PRELUDE}
const cli = __bellParseCliArgs(process.argv.slice(2));
process.env.BELL_HOST ||= cli.host || process.env.HOST || "127.0.0.1";
process.env.BELL_PORT ||= cli.port || process.env.PORT || "${spec.defaultPort}";
if (!process.env.BELL_DATA_PATH) {
  const dataDir = path.resolve(
    cli.dataDir ||
      process.env.BELL_EXPLAIN_DATA_DIR ||
      process.env.BELL_DATA_DIR ||
      path.join(os.homedir(), ".bell-explain", "data"),
  );
  mkdirSync(dataDir, { recursive: true });
  process.env.BELL_DATA_PATH = path.join(dataDir, "bell.sqlite");
} else {
  mkdirSync(path.dirname(path.resolve(process.env.BELL_DATA_PATH)), { recursive: true });
}
require("./server/index.ts");
`;
  }

  if (spec.id === "reader") {
    return `
import { mkdirSync } from "node:fs";
import os from "node:os";
import path from "node:path";
${CLI_PRELUDE}
const cli = __bellParseCliArgs(process.argv.slice(2));
process.env.BELL_READER_HOST ||= cli.host || process.env.HOST || "127.0.0.1";
process.env.BELL_READER_PORT ||= cli.port || process.env.PORT || "${spec.defaultPort}";
const dataDir = path.resolve(
  cli.dataDir ||
    process.env.BELL_READER_DATA_DIR ||
    process.env.BELL_DATA_DIR ||
    path.join(os.homedir(), ".bell-reader", "data"),
);
mkdirSync(dataDir, { recursive: true });
process.env.BELL_READER_DATA_DIR = dataDir;
require("./server/index.ts");
`;
  }

  // campaign
  return `
import { mkdirSync } from "node:fs";
import os from "node:os";
import path from "node:path";
${CLI_PRELUDE}
const cli = __bellParseCliArgs(process.argv.slice(2));
process.env.BELL_CAMPAIGN_HOST ||= cli.host || process.env.HOST || "127.0.0.1";
process.env.BELL_CAMPAIGN_PORT ||= cli.port || process.env.PORT || "${spec.defaultPort}";
const dataDir = path.resolve(
  cli.dataDir ||
    process.env.BELL_CAMPAIGN_DATA_DIR ||
    process.env.BELL_DATA_DIR ||
    path.join(os.homedir(), ".bell-campaign", "data"),
);
mkdirSync(dataDir, { recursive: true });
process.env.BELL_CAMPAIGN_DATA_DIR = dataDir;
require("./server/index.ts");
`;
}

function createSidecarPlugin(spec, appDir) {
  return {
    name: `bell-sidecar-${spec.id}`,
    setup(build) {
      // Optional heavy native ML stack in BELL Explain: keep zero-node_modules bundle clean
      build.onResolve(
        { filter: /^(@huggingface\/transformers|onnxruntime-node|onnxruntime-web|sharp|canvas|@napi-rs\/canvas)$/ },
        (args) => ({ path: args.path, namespace: "bell-optional-native" }),
      );
      build.onLoad({ filter: /.*/, namespace: "bell-optional-native" }, (args) => ({
        contents: `
const { createRequire } = require("node:module");
module.exports = new Proxy({}, {
  get(_target, prop) {
    try {
      const mod = createRequire(__filename)(${JSON.stringify(args.path)});
      return mod[prop];
    } catch {
      if (prop === "__esModule") return false;
      if (prop === "then") return undefined;
      return () => {
        const err = new Error("桌面內建執行環境未包含選用原生模組 (" + ${JSON.stringify(args.path)} + ")，請改用 API 向量或內建關鍵字檢索。");
        err.status = 503;
        throw err;
      };
    }
  }
});
`,
        loader: "js",
      }));

      if (spec.id === "explain") {
        // Point pdfjs static asset lookup, embedded pdf.worker, & forked document worker to bundled assets
        const appTsPath = path.join(appDir, "server", "app.ts");
        const parserTsPath = path.join(appDir, "server", "document-parser.ts");
        const workerTsPath = path.join(appDir, "server", "document-worker.ts");
        const documentsTsPath = path.join(appDir, "server", "documents.ts");

        build.onLoad({ filter: /server[\\/](app|document-parser|document-worker|documents)\.ts$/ }, (args) => {
          let src = readFileSync(args.path, "utf8");
          if (path.resolve(args.path) === appTsPath) {
            src = src.replace(
              'dirname(createRequire(import.meta.url).resolve("pdfjs-dist/package.json"))',
              'fileURLToPath(new URL("../pdfjs-dist", import.meta.url))',
            );
          } else if (path.resolve(args.path) === parserTsPath) {
            src = src
              .replace(
                'new URL("./document-worker.ts", import.meta.url)',
                'new URL("../document-worker.cjs", import.meta.url)',
              )
              .replace(
                '["--import", "tsx", "--max-old-space-size=512", "--disable-proto=throw"]',
                '["--max-old-space-size=512", "--disable-proto=throw"]',
              );
          } else if (path.resolve(args.path) === workerTsPath) {
            const macPdfKitHelper = `
import { execFileSync as __bellExecFileSync } from "node:child_process";
function __bellRenderPdfPageDarwin(pdfPath: string, pageNum: number, width: number, height: number, password?: string): { mimeType: string; data: string } {
  const jxa = [
    'ObjC.import("Foundation");ObjC.import("AppKit");ObjC.import("Quartz");',
    'const env=$.NSProcessInfo.processInfo.environment;',
    'const p=ObjC.unwrap(env.objectForKey("BELL_PDF_PATH"));',
    'const pwObj=env.objectForKey("BELL_PDF_PWD");const pw=pwObj.isNil()?"":ObjC.unwrap(pwObj);',
    'const pgNum=Number(ObjC.unwrap(env.objectForKey("BELL_PDF_PAGE")));',
    'const w=Number(ObjC.unwrap(env.objectForKey("BELL_PDF_W")));',
    'const h=Number(ObjC.unwrap(env.objectForKey("BELL_PDF_H")));',
    'const doc=$.PDFDocument.alloc.initWithURL($.NSURL.fileURLWithPath(p));',
    'if(doc.isNil())throw new Error("PDF open failed");',
    'if(doc.isLocked&&(!pw||!doc.unlockWithPassword(pw)))throw new Error("PDF locked");',
    'const pg=doc.pageAtIndex(pgNum-1);if(pg.isNil())throw new Error("Page missing");',
    'const img=pg.thumbnailOfSizeForBox($.NSMakeSize(w,h),$.kPDFDisplayBoxMediaBox);',
    'const rep=$.NSBitmapImageRep.imageRepWithData(img.TIFFRepresentation);',
    'let out=rep.representationUsingTypeProperties($.NSBitmapImageFileTypePNG,$({}));let mime="image/png";',
    'if(out.length>900000){out=rep.representationUsingTypeProperties($.NSBitmapImageFileTypeJPEG,$({NSImageCompressionFactor:0.85}));mime="image/jpeg";}',
    'JSON.stringify({mimeType:mime,data:ObjC.unwrap(out.base64EncodedStringWithOptions(0))});',
  ].join("");
  const raw = __bellExecFileSync("/usr/bin/osascript", ["-l", "JavaScript", "-e", jxa], {
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
    timeout: 20_000,
    env: {
      PATH: "/usr/bin:/bin",
      BELL_PDF_PATH: pdfPath,
      BELL_PDF_PAGE: String(pageNum),
      BELL_PDF_W: String(width),
      BELL_PDF_H: String(height),
      ...(password ? { BELL_PDF_PWD: password } : {}),
    },
  });
  return JSON.parse(raw.trim());
}
`;
            src = macPdfKitHelper + src
              .replace(
                'const pdfjs = await import("pdfjs-dist/legacy/build/pdf.mjs");',
                '(globalThis as Record<string, unknown>).pdfjsWorker ||= await import("pdfjs-dist/legacy/build/pdf.worker.mjs");\n  const pdfjs = await import("pdfjs-dist/legacy/build/pdf.mjs");',
              )
              .replace(
                'new URL("../node_modules/pdfjs-dist/", import.meta.url)',
                'new URL("../pdfjs-dist/", import.meta.url)',
              )
              .replace(
                `const { canvas, context } = factory.create(width, height);
        await page.render({ canvasContext: context as never, canvas: canvas as never, viewport }).promise;
        let data = canvas.toBuffer("image/png"), mimeType = "image/png";
        // Photos and scans compress poorly as PNG.
        if (data.length > 900_000) { data = canvas.toBuffer("image/jpeg", 85); mimeType = "image/jpeg"; }
        images.push({ page: number, mimeType, data: data.toString("base64"), width, height });`,
                `try {
          const { canvas, context } = factory.create(width, height);
          await page.render({ canvasContext: context as never, canvas: canvas as never, viewport }).promise;
          let data = canvas.toBuffer("image/png"), mimeType = "image/png";
          if (data.length > 900_000) { data = canvas.toBuffer("image/jpeg", 85); mimeType = "image/jpeg"; }
          images.push({ page: number, mimeType, data: data.toString("base64"), width, height });
        } catch (canvasErr) {
          if (process.platform === "darwin") {
            const rendered = __bellRenderPdfPageDarwin(path, number, width, height, request.password);
            images.push({ page: number, mimeType: rendered.mimeType, data: rendered.data, width, height });
          } else {
            throw canvasErr;
          }
        }`,
              );
          } else if (path.resolve(args.path) === documentsTsPath) {
            // shortcut: Windows zero-node_modules bundle omits @napi-rs/canvas (~30 MB), fall back to extracted PDF text layer unless 100% scanned
            src = src.replace(
              `images = await this.renderer(join(this.directoryFor(id), "original.pdf"),
          visual
            ? { required: imageOnly, candidates: visual.map(item => item.page), limit: DOCUMENT_MAX_VISUAL_PAGES, ranked: true }
            : { required: imageOnly, candidates: [...chosen].filter(page => !transcribed(page)), limit: DOCUMENT_MAX_VISUAL_PAGES },
          signal, { password: options.password });`,
              `images = await this.renderer(join(this.directoryFor(id), "original.pdf"),
          visual
            ? { required: imageOnly, candidates: visual.map(item => item.page), limit: DOCUMENT_MAX_VISUAL_PAGES, ranked: true }
            : { required: imageOnly, candidates: [...chosen].filter(page => !transcribed(page)), limit: DOCUMENT_MAX_VISUAL_PAGES },
          signal, { password: options.password }).catch(err => {
            if (selected.blocks.length > 0 && err instanceof AppError && err.status === 422) {
              notes.push("目前執行環境未載入頁面影像繪製元件，本次改依 PDF 文字層進行解說。");
              return [];
            }
            throw err;
          });`,
            );
          }
          return { contents: src, loader: "ts", resolveDir: path.dirname(args.path) };
        });
      }
    },
  };
}

const CJS_SHIM_BANNER = [
  'const __bell_import_meta_dir = require("node:path").join(__dirname, "_server");',
  'const __bell_import_meta_file = require("node:path").join(__bell_import_meta_dir, "server.cjs");',
  'const __bell_import_meta_url = require("node:url").pathToFileURL(__bell_import_meta_file).href;',
].join("\n");

export async function bundleAllSidecars({ onlyIds } = {}) {
  const specs = onlyIds
    ? WORKSPACE_SPECS.filter((s) => onlyIds.includes(s.id))
    : WORKSPACE_SPECS;

  const appDirs = {};
  for (const spec of specs) {
    appDirs[spec.id] = resolveAppDir(spec);
  }
  const esbuild = loadEsbuild(appDirs);

  mkdirSync(path.join(OUT_ROOT, "bin"), { recursive: true });
  const keepFile = path.join(OUT_ROOT, "bin", ".keep");
  if (!existsSync(keepFile)) writeFileSync(keepFile, "");
  mkdirSync(path.join(OUT_ROOT, "apps"), { recursive: true });

  for (const spec of specs) {
    const appDir = appDirs[spec.id];
    ensureFrontendDist(spec.id, appDir);

    const pkg = JSON.parse(readFileSync(path.join(appDir, "package.json"), "utf8"));
    const outDir = path.join(OUT_ROOT, "apps", spec.id);
    rmSync(outDir, { recursive: true, force: true });
    mkdirSync(outDir, { recursive: true });

    const define = {
      "import.meta.url": "__bell_import_meta_url",
      "import.meta.dirname": "__bell_import_meta_dir",
      "import.meta.filename": "__bell_import_meta_file",
    };
    if (spec.id === "pr") {
      const buildDate = pkg.bellpr?.releaseDate ?? new Date().toISOString().slice(0, 10);
      define.__BELLPR_VERSION__ = JSON.stringify(pkg.version);
      define.__BELLPR_BUILD_DATE__ = JSON.stringify(buildDate);
    }

    const plugin = createSidecarPlugin(spec, appDir);
    await esbuild.build({
      absWorkingDir: appDir,
      stdin: {
        contents: buildEntrySource(spec),
        resolveDir: appDir,
        sourcefile: "sidecar-entry.ts",
        loader: "ts",
      },
      outfile: path.join(outDir, "server.cjs"),
      bundle: true,
      platform: "node",
      target: "node22",
      format: "cjs",
      minifyWhitespace: true,
      minifySyntax: true,
      keepNames: true,
      banner: { js: CJS_SHIM_BANNER },
      define,
      external: ["node:*"],
      plugins: [plugin],
      logLevel: "warning",
    });

    if (spec.id === "explain") {
      await esbuild.build({
        absWorkingDir: appDir,
        entryPoints: ["server/document-worker.ts"],
        outfile: path.join(outDir, "document-worker.cjs"),
        bundle: true,
        platform: "node",
        target: "node22",
        format: "cjs",
        minifyWhitespace: true,
        minifySyntax: true,
        keepNames: true,
        banner: { js: CJS_SHIM_BANNER },
        define,
        external: ["node:*"],
        plugins: [plugin],
        logLevel: "warning",
      });

      const pdfjsSrc = path.join(appDir, "node_modules", "pdfjs-dist");
      const pdfjsOut = path.join(outDir, "pdfjs-dist");
      mkdirSync(pdfjsOut, { recursive: true });
      for (const folder of ["cmaps", "standard_fonts", "wasm"]) {
        const folderSrc = path.join(pdfjsSrc, folder);
        if (existsSync(folderSrc)) {
          cpSync(folderSrc, path.join(pdfjsOut, folder), { recursive: true });
        }
      }
      const deployReadme = path.join(appDir, "deployment", "README.md");
      if (existsSync(deployReadme)) {
        mkdirSync(path.join(outDir, "deployment"), { recursive: true });
        cpSync(deployReadme, path.join(outDir, "deployment", "README.md"));
      }
    }

    if (spec.id === "personal") {
      const tokensCss = path.join(appDir, "src", "styles", "tokens.css");
      if (existsSync(tokensCss)) {
        mkdirSync(path.join(outDir, "src", "styles"), { recursive: true });
        cpSync(tokensCss, path.join(outDir, "src", "styles", "tokens.css"));
      }
    }

    // Copy frontend dist/
    cpSync(path.join(appDir, "dist"), path.join(outDir, "dist"), {
      recursive: true,
      filter: (src) => !src.endsWith(".map") && path.basename(src) !== ".vite",
    });

    // Copy vendor/ when present (bell-pr and bell-explain ponytail rules)
    if (existsSync(path.join(appDir, "vendor"))) {
      cpSync(path.join(appDir, "vendor"), path.join(outDir, "vendor"), { recursive: true });
    }

    // Minimal metadata package.json (type commonjs so server.cjs runs cleanly anywhere)
    writeFileSync(
      path.join(outDir, "package.json"),
      JSON.stringify(
        {
          name: pkg.name,
          version: pkg.version,
          license: pkg.license || "Apache-2.0",
          private: true,
        },
        null,
        2,
      ) + "\n",
    );

    const bundleSizeKb = (readFileSync(path.join(outDir, "server.cjs")).length / 1024).toFixed(1);
    console.log(`[${spec.id}] Bundled dist-sidecars/apps/${spec.id}/server.cjs (${bundleSizeKb} kB, v${pkg.version})`);
  }
}

function httpGet(port, urlPath, timeoutMs = 1500) {
  return new Promise((resolve) => {
    const req = http.get(
      {
        host: "127.0.0.1",
        port,
        path: urlPath,
        headers: { Host: `127.0.0.1:${port}`, Accept: "application/json,text/html" },
        timeout: timeoutMs,
      },
      (res) => {
        let body = "";
        res.setEncoding("utf8");
        res.on("data", (c) => {
          if (body.length < 65536) body += c;
        });
        res.on("end", () => resolve({ status: res.statusCode ?? 0, body }));
      },
    );
    req.on("error", () => resolve({ status: 0, body: "" }));
    req.on("timeout", () => {
      req.destroy();
      resolve({ status: 0, body: "" });
    });
  });
}

const SAMPLE_VERIFY_PDF = Buffer.from(`%PDF-1.4
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >> endobj
4 0 obj << /Length 44 >> stream
BT /F1 18 Tf 72 720 Td (Hello BELL Explain) Tj ET
endstream endobj
5 0 obj << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> endobj
xref
0 6
0000000000 65535 f 
0000000009 00000 n 
0000000058 00000 n 
0000000115 00000 n 
0000000229 00000 n 
0000000322 00000 n 
trailer << /Size 6 /Root 1 0 R >>
startxref
390
%%EOF`, "utf8");

function httpPostPdf(port, urlPath, pdfBytes, timeoutMs = 8000) {
  return new Promise((resolve) => {
    const req = http.request(
      {
        method: "POST",
        host: "127.0.0.1",
        port,
        path: urlPath,
        headers: {
          Host: `127.0.0.1:${port}`,
          Origin: `http://127.0.0.1:${port}`,
          "Content-Type": "application/octet-stream",
          "Content-Length": String(pdfBytes.byteLength),
          "X-Document-Name": encodeURIComponent("verify-sample.pdf"),
          Accept: "application/json",
        },
        timeout: timeoutMs,
      },
      (res) => {
        let body = "";
        res.setEncoding("utf8");
        res.on("data", (c) => {
          if (body.length < 65536) body += c;
        });
        res.on("end", () => resolve({ status: res.statusCode ?? 0, body }));
      },
    );
    req.on("error", () => resolve({ status: 0, body: "" }));
    req.on("timeout", () => {
      req.destroy();
      resolve({ status: 0, body: "" });
    });
    req.end(pdfBytes);
  });
}

export async function verifySidecars() {
  const tmpRoot = mkdtempSync(path.join(os.tmpdir(), "bell-sidecar-verify-"));
  const children = [];
  console.log(`\nVerifying all 5 sidecars from cwd=/tmp (isolated data root: ${tmpRoot})...`);

  try {
    for (const spec of WORKSPACE_SPECS) {
      const scriptPath = path.join(OUT_ROOT, "apps", spec.id, "server.cjs");
      const dataDir = path.join(tmpRoot, spec.id);
      mkdirSync(dataDir, { recursive: true, mode: 0o700 });

      const env = {
        ...process.env,
        HOST: "127.0.0.1",
        PORT: String(spec.verifyPort),
        BELL_DATA_DIR: dataDir,
        [spec.dataEnv]: dataDir,
        BELL_NO_OPEN: "1",
        BELLPR_NO_OPEN: "1",
      };

      let stderrLog = "";
      const child = spawn(process.execPath, [scriptPath], {
        cwd: os.tmpdir(),
        env,
        stdio: ["ignore", "pipe", "pipe"],
      });
      child.stdout.setEncoding("utf8");
      child.stderr.setEncoding("utf8");
      child.stderr.on("data", (chunk) => {
        if (stderrLog.length < 8192) stderrLog += chunk;
      });
      child.stdout.on("data", (chunk) => {
        if (stderrLog.length < 8192) stderrLog += chunk;
      });
      children.push({ spec, child, getLogs: () => stderrLog });
    }

    const deadline = Date.now() + 8000;
    await Promise.all(
      children.map(async ({ spec, child, getLogs }) => {
        const origin = `http://127.0.0.1:${spec.verifyPort}`;
        while (Date.now() < deadline) {
          if (child.exitCode !== null) {
            throw new Error(
              `[${spec.id}] exited prematurely with code ${child.exitCode}:\n${getLogs()}`,
            );
          }
          const manifestRes = await httpGet(spec.verifyPort, "/api/suite/manifest", 1000);
          if (manifestRes.status === 200) {
            const parsed = JSON.parse(manifestRes.body);
            validateManifest(parsed, { ownOrigins: [origin] });
            const indexRes = await httpGet(spec.verifyPort, "/", 1000);
            if (indexRes.status !== 200 || !indexRes.body.toLowerCase().includes("<html")) {
              throw new Error(
                `[${spec.id}] static frontend / returned HTTP ${indexRes.status}`,
              );
            }
            if (spec.id === "explain") {
              const pdfRes = await httpPostPdf(spec.verifyPort, "/api/documents", SAMPLE_VERIFY_PDF);
              if (pdfRes.status !== 201) {
                throw new Error(
                  `[explain] PDF upload/parse smoke check failed (HTTP ${pdfRes.status}): ${pdfRes.body}`,
                );
              }
              const pdfDoc = JSON.parse(pdfRes.body);
              if (pdfDoc.format !== "pdf" || Number(pdfDoc.extractedChars ?? 0) < 10) {
                throw new Error(`[explain] Unexpected parsed PDF payload: ${pdfRes.body}`);
              }
            }
            console.log(
              `  [PASS] ${spec.id.padEnd(9)} ${origin}/api/suite/manifest -> 200 (appId=${parsed.appId}, frontend=200${spec.id === "explain" ? ", pdf=201" : ""})`,
            );
            return;
          }
          if (manifestRes.status === 401 || manifestRes.status === 403) {
            const healthRes = await httpGet(spec.verifyPort, "/api/health", 1000);
            if (healthRes.status === 200) {
              const indexRes = await httpGet(spec.verifyPort, "/", 1000);
              if (indexRes.status !== 200 || !indexRes.body.toLowerCase().includes("<html")) {
                throw new Error(
                  `[${spec.id}] static frontend / returned HTTP ${indexRes.status}`,
                );
              }
              if (spec.id === "pr") {
                const tokenPath = path.join(tmpRoot, "pr", "api-token");
                if (!existsSync(tokenPath)) {
                  throw new Error(`[pr] expected api-token inside isolated data dir ${tokenPath}`);
                }
              }
              console.log(`  [PASS] ${spec.id.padEnd(9)} ${origin}/api/health -> 200 (auth-gated manifest, frontend=200, vault-isolated=ok)`);
              return;
            }
          }
          await new Promise((r) => setTimeout(r, 150));
        }
        throw new Error(
          `[${spec.id}] timed out waiting for HTTP 200 on port ${spec.verifyPort}.\nLogs:\n${getLogs()}`,
        );
      }),
    );
    console.log("All 5 sidecars verified cleanly with zero node_modules.\n");
  } finally {
    for (const { child } of children) {
      if (child.exitCode === null) {
        try {
          child.kill("SIGTERM");
        } catch {}
      }
    }
    await Promise.all(
      children.map(
        ({ child }) =>
          new Promise((resolve) => {
            if (child.exitCode !== null) return resolve();
            const timer = setTimeout(() => {
              try {
                child.kill("SIGKILL");
              } catch {}
              resolve();
            }, 2500);
            child.once("exit", () => {
              clearTimeout(timer);
              resolve();
            });
          }),
      ),
    );
    rmSync(tmpRoot, { recursive: true, force: true });
  }
}

async function main() {
  const args = process.argv.slice(2);
  const verify = args.includes("--verify");
  const skipBundle = args.includes("--verify-only");
  const onlyArg = args.find((a) => a.startsWith("--only="))?.slice(7);
  const onlyIds = onlyArg ? onlyArg.split(",").map((s) => s.trim()).filter(Boolean) : undefined;

  if (!skipBundle) {
    await bundleAllSidecars({ onlyIds });
  }
  if (verify || skipBundle) {
    await verifySidecars();
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((err) => {
    console.error(`bundle-sidecars failed: ${err instanceof Error ? err.stack || err.message : String(err)}`);
    process.exit(1);
  });
}
