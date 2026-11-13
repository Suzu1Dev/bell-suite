import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import http from "node:http";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { buildSuiteEnvForApp, parseOnlyList, probeManifest } from "../bin/bell-suite.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const BIN = path.join(ROOT, "bin", "bell-suite.mjs");
const PKG_VERSION = JSON.parse(readFileSync(path.join(ROOT, "package.json"), "utf8")).version;

test("bell-suite --version and --help work cleanly", () => {
  const ver = execFileSync(process.execPath, [BIN, "--version"], { encoding: "utf8" }).trim();
  assert.equal(ver, PKG_VERSION);

  const help = execFileSync(process.execPath, [BIN, "--help"], { encoding: "utf8" });
  assert.match(help, /bell-suite status/);
  assert.match(help, /bell-suite doctor/);
  assert.match(help, /personal,explain,pr,reader,campaign/);
});

test("buildSuiteEnvForApp computes full 5-app SUITE_LINKS and fetch targets without self-links", () => {
  const ports = { personal: 4601, explain: 4602, pr: 4603, reader: 4604, campaign: 4605 };

  const personalEnv = buildSuiteEnvForApp("personal", ports, "127.0.0.1", {});
  assert.equal(
    personalEnv.BELLPERSONAL_SUITE_LINKS,
    "explain=http://127.0.0.1:4602,pr=http://127.0.0.1:4603,reader=http://127.0.0.1:4604,campaign=http://127.0.0.1:4605",
  );
  assert.equal(
    personalEnv.BELLPERSONAL_SUITE_FETCH,
    "explain=http://127.0.0.1:4602,pr=http://127.0.0.1:4603,reader=http://127.0.0.1:4604",
  );
  assert.equal(personalEnv.BELLPERSONAL_PORT, "4601");

  const explainEnv = buildSuiteEnvForApp("explain", ports, "127.0.0.1", {});
  assert.equal(
    explainEnv.BELL_SUITE_LINKS,
    "personal=http://127.0.0.1:4601,pr=http://127.0.0.1:4603,reader=http://127.0.0.1:4604,campaign=http://127.0.0.1:4605",
  );

  const prEnv = buildSuiteEnvForApp("pr", ports, "127.0.0.1", {});
  assert.equal(
    prEnv.BELLPR_SUITE_LINKS,
    "personal=http://127.0.0.1:4601,explain=http://127.0.0.1:4602,reader=http://127.0.0.1:4604,campaign=http://127.0.0.1:4605",
  );
  assert.equal(prEnv.BELLPR_EDITION, "suite");

  const readerEnv = buildSuiteEnvForApp("reader", ports, "127.0.0.1", {});
  assert.equal(
    readerEnv.BELL_READER_SUITE_LINKS,
    "personal=http://127.0.0.1:4601,explain=http://127.0.0.1:4602,pr=http://127.0.0.1:4603,campaign=http://127.0.0.1:4605",
  );

  const campaignEnv = buildSuiteEnvForApp("campaign", ports, "127.0.0.1", {});
  assert.equal(
    campaignEnv.BELL_CAMPAIGN_SUITE_LINKS,
    "personal=http://127.0.0.1:4601,explain=http://127.0.0.1:4602,pr=http://127.0.0.1:4603,reader=http://127.0.0.1:4604",
  );

  assert.deepEqual(parseOnlyList("explain,reader"), ["explain", "reader"]);
  assert.throws(() => parseOnlyList("unknown-app"), /Unknown Suite app/);
});

test("probeManifest validates live Suite v1 manifest via contracts/v1/reference.mjs", async (t) => {
  const srv = http.createServer((req, res) => {
    if (req.url === "/api/suite/manifest") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(
        JSON.stringify({
          schemaVersion: 1,
          appId: "explain",
          displayName: "BELL Explain",
          summary: "理解與複習",
          capabilities: [{ id: "review.today", kind: "today-feed", api: "/api/suite/today" }],
          links: { personal: "http://127.0.0.1:4601" },
        }),
      );
      return;
    }
    res.writeHead(404).end();
  });
  await new Promise((resolve) => srv.listen(0, "127.0.0.1", resolve));
  t.after(() => srv.close());
  const addr = srv.address();
  const port = typeof addr === "object" && addr ? addr.port : 0;

  const ok = await probeManifest("127.0.0.1", port);
  assert.equal(ok.ok, true);
  assert.equal(ok.manifest?.appId, "explain");
});

test("bell-suite doctor --json reports runtime and discovered workspaces", () => {
  const out = execFileSync(process.execPath, [BIN, "doctor", "--base-port", "49900", "--json"], {
    encoding: "utf8",
  });
  const data = JSON.parse(out);
  assert.equal(data.suiteVersion, PKG_VERSION);
  assert.equal(data.tools.nodeOk, true);
  assert.equal(data.workspaces.length, 5);
});
