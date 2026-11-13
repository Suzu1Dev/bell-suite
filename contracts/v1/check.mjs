// Runs every fixture through reference.mjs. `node contracts/v1/check.mjs` exits 0 only when
// each valid case passes and each invalid case is refused.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  allowedRoute, handoffUrl, isHandoffAttempt, parseFetch, parseHandoff, parseLinks,
  validateFeed, validateManifest, validateSources,
} from "./reference.mjs";

const fixture = (name) => JSON.parse(readFileSync(new URL(`./fixtures/${name}`, import.meta.url), "utf8"));
const clone = (value) => structuredClone(value);
let passed = 0;
function check(name, run) {
  try { run(); passed += 1; } catch (error) {
    console.error(`FAIL ${name}\n  ${error.message}`);
    process.exitCode = 1;
  }
}

const today = fixture("today.explain.valid.json");
const manifests = {
  explain: fixture("manifest.explain.valid.json"),
  personal: fixture("manifest.personal.valid.json"),
  reader: fixture("manifest.reader.valid.json"),
};
const sources = fixture("sources.personal.valid.json");
const config = fixture("config.cases.json");
const handoff = fixture("handoff.cases.json");
const ownOrigins = config.ownOrigins;

check("valid: today feed", () => validateFeed(today));
check("valid: empty today feed", () => validateFeed(fixture("today.empty.valid.json")));
check("valid: Explain manifest", () => validateManifest(manifests.explain, { ownOrigins: [] }));
check("valid: Reader manifest", () => validateManifest(manifests.reader, { ownOrigins: [] }));
check("valid: Reader <-> Explain handoff round trip", () => {
  const toExplain = handoffUrl({ target: "explain", origin: "http://127.0.0.1:21101", text: "微任務佇列", title: "Event Loop", from: "reader", back: "#/page/p-1" });
  assert.deepEqual(
    parseHandoff(toExplain.slice(toExplain.indexOf("#")), { target: "explain", links: { reader: "http://127.0.0.1:21104" } }),
    { from: "reader", text: "微任務佇列", title: "Event Loop", truncated: false, backUrl: "http://127.0.0.1:21104/#/page/p-1" },
  );
  const toReader = handoffUrl({ target: "reader", origin: "http://127.0.0.1:21104", text: "閱讀摘錄", title: "標題", from: "explain", back: "#/note/n-1" });
  assert.deepEqual(
    parseHandoff(toReader.slice(toReader.indexOf("#")), { target: "reader", links: { explain: "http://127.0.0.1:21101" } }),
    { from: "explain", text: "閱讀摘錄", title: "標題", truncated: false, backUrl: "http://127.0.0.1:21101/#/note/n-1" },
  );
});
check("valid: PR routes and two-way PR & Campaign handoffs", () => {
  for (const r of ["#/", "#/home", "#/demo", "#/repos", "#/repos/bell-pr", "#/discover", "#/team", "#/group", "#/learning", "#/settings", "#/runs/run-42"]) {
    assert.equal(allowedRoute("pr", r), true, r);
  }
  assert.equal(allowedRoute("pr", "#/runs/.."), false);
  assert.equal(allowedRoute("pr", "#/unknown"), false);
  const toPr = handoffUrl({ target: "pr", origin: "http://127.0.0.1:4603", text: "Fix parser edge case", title: "Parser issue", from: "explain", back: "#/note/n-1" });
  assert.match(toPr, /^http:\/\/127\.0\.0\.1:4603\/#\/discover\?/);
  assert.deepEqual(
    parseHandoff(toPr.slice(toPr.indexOf("#")), { target: "pr", links: { explain: "http://127.0.0.1:4602" } }),
    { from: "explain", text: "Fix parser edge case", title: "Parser issue", truncated: false, backUrl: "http://127.0.0.1:4602/#/note/n-1" },
  );
  const fromPr = handoffUrl({ target: "explain", origin: "http://127.0.0.1:4602", text: "Why does rebase conflict here?", title: "Git Rebase", from: "pr", back: "#/runs/run-42" });
  assert.deepEqual(
    parseHandoff(fromPr.slice(fromPr.indexOf("#")), { target: "explain", links: { pr: "http://127.0.0.1:4603" } }),
    { from: "pr", text: "Why does rebase conflict here?", title: "Git Rebase", truncated: false, backUrl: "http://127.0.0.1:4603/#/runs/run-42" },
  );
  const toCampaign = handoffUrl({ target: "campaign", origin: "http://127.0.0.1:4605", text: "聯絡主講人確認投影片", title: "議程組待辦", from: "personal", back: "#/today" });
  assert.match(toCampaign, /^http:\/\/127\.0\.0\.1:4605\/#\/tasks\?/);
  assert.deepEqual(
    parseHandoff(toCampaign.slice(toCampaign.indexOf("#")), { target: "campaign", links: { personal: "http://127.0.0.1:4601" } }),
    { from: "personal", text: "聯絡主講人確認投影片", title: "議程組待辦", truncated: false, backUrl: "http://127.0.0.1:4601/#/today" },
  );
});
check("valid: Personal manifest", () => validateManifest(manifests.personal, { ownOrigins }));
check("invalid: tailnet link without an own tailnet origin", () =>
  assert.throws(() => validateManifest(manifests.personal, { ownOrigins: [] })));
check("valid: Personal sources", () => validateSources(sources));

for (const entry of fixture("invalid.json")) {
  check(`invalid: ${entry.case}`, () => {
    const base = clone(entry.kind === "feed" ? today : entry.kind === "manifest" ? manifests.explain : sources);
    Object.assign(base, entry.patch ?? {});
    if (entry.item) Object.assign(base.items[0], entry.item);
    if (entry.repeatTitle) base.items[0].title = base.items[0].title.repeat(entry.repeatTitle);
    if (entry.sixItems) base.items = Array.from({ length: 6 }, (_, index) => ({ ...today.items[0], id: `item-${index}` }));
    const validate = { feed: validateFeed, manifest: (value) => validateManifest(value, { ownOrigins }), sources: validateSources }[entry.kind];
    assert.throws(() => validate(base));
  });
}

for (const entry of config.links) {
  check(`links: ${entry.value || "(empty)"}`, () => {
    const run = () => parseLinks(entry.value, { self: entry.self, ownOrigins, variable: "BELLPERSONAL_SUITE_LINKS" });
    if (entry.ok) assert.deepEqual(run(), entry.ok);
    else assert.throws(run, undefined, entry.error);
  });
}
for (const entry of config.fetch) {
  check(`fetch: ${entry.value}`, () => {
    const run = () => parseFetch(entry.value);
    if (entry.ok) assert.deepEqual(run(), entry.ok);
    else assert.throws(run, undefined, entry.error);
  });
}

for (const entry of handoff.cases) {
  check(`handoff: ${entry.case}`, () => {
    const hash = entry.repeatDraft ? `#/tasks?draft=${"字".repeat(entry.repeatDraft)}&from=explain` : entry.hash;
    const result = parseHandoff(hash, { target: entry.target, links: handoff.links[entry.target] });
    // rev. d: every case that is not null is an attempt, so the receiver clears the older draft first.
    assert.equal(isHandoffAttempt(hash, entry.target), entry.expect !== null, "isHandoffAttempt agrees with the case");
    if (entry.expect === null) assert.equal(result, undefined);
    else if (entry.expect.textLength) {
      assert.equal([...result.text].length, entry.expect.textLength);
      assert.equal(result.truncated, entry.expect.truncated);
      assert.equal(result.from, entry.expect.from);
    } else assert.deepEqual(result, entry.expect);
  });
}

for (const text of ["a+b & c=d #e", "第一行\n第二行\t縮排", "emoji 📚 與 100% 完成", "?draft=x&from=pr"]) {
  check(`round trip: ${JSON.stringify(text)}`, () => {
    const url = handoffUrl({ target: "personal", origin: "http://127.0.0.1:21103", text, from: "explain", back: "#/note/abc" });
    const hash = url.slice(url.indexOf("#"));
    assert.deepEqual(parseHandoff(hash, { target: "personal", links: handoff.links.personal }),
      { from: "explain", text, truncated: false, backUrl: "http://127.0.0.1:21101/#/note/abc" });
  });
}

console.log(`${passed} checks passed${process.exitCode ? ", some failed" : ""}`);
