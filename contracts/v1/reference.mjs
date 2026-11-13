// BELL Suite contract v1 — dependency-free reference validator.
// Each product keeps its own typed copy (zod) of these rules; this file is the
// executable definition the fixtures are checked against (`node check.mjs`).
// Contract version: suite-contract v1 (2026-11-08; rev. d: handoff attempts, see README §5 and the revision log).

export const APP_IDS = ["explain", "personal", "pr", "reader", "campaign"];
export const ITEM_STATES = ["due", "new", "waiting", "needs-you"];
export const SOURCE_STATUSES = ["ready", "stale", "unavailable", "unconfigured", "unsupported"];
export const CAPABILITY_KINDS = ["accept-handoff", "today-feed", "object-route"];

export const LIMITS = {
  title: 120, detail: 80, id: 128, href: 512, items: 5, counts: 8, countMax: 100000,
  capabilities: 16, displayName: 40, summary: 40, links: 5,
  handoffText: 2000, handoffTitle: 120, back: 512,
  fetchTimeoutMs: 3000, fetchMaxBytes: 64 * 1024,
  cacheFreshMs: 60_000, activePollMs: 5 * 60_000, staleDropMs: 30 * 60_000,
};

const ISO_UTC = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,3})?Z$/;
const OBJECT_ID = /^[A-Za-z0-9._:-]{1,128}$/;
const OBJECT_TYPE = /^[a-z][a-z0-9-]{0,31}$/;
const COUNT_KEY = /^[a-z][A-Za-z0-9]{0,31}$/;
const CAPABILITY_ID = /^[a-z][a-z0-9-]*(?:\.[a-z][a-z0-9-]*)+$/;
const HREF = /^#\/[A-Za-z0-9/_\-.~%:]*(?:\?[A-Za-z0-9_\-.~%=&+]*)?$/;
const ROUTE_TEMPLATE = /^#\/[A-Za-z0-9/_\-.~{}]*$/;
const API_PATH = /^\/api\/[A-Za-z0-9/_\-.]+$/;
// C0, DEL, C1, and the bidi embedding/override/isolate controls.
const UNSAFE_TEXT = /[\u0000-\u001f\u007f-\u009f\u202a-\u202e\u2066-\u2069]/;
const UNSAFE_HANDOFF = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f\u202a-\u202e\u2066-\u2069]/g; // keeps \t \n

const length = (text) => [...text].length;
const isObject = (value) => value !== null && typeof value === "object" && !Array.isArray(value);
const fail = (path, message) => { throw new Error(`${path}: ${message}`); };
function onlyKeys(value, path, allowed) {
  for (const key of Object.keys(value)) if (!allowed.includes(key)) fail(path, `unknown key ${key}`);
}
function plain(value, path, max, { optional = false } = {}) {
  if (value === undefined && optional) return;
  if (typeof value !== "string" || !value.trim()) fail(path, "must be non-empty text");
  if (length(value) > max) fail(path, `longer than ${max}`);
  if (UNSAFE_TEXT.test(value)) fail(path, "control or bidi character");
}
function iso(value, path, { optional = false } = {}) {
  if (value === undefined && optional) return;
  if (typeof value !== "string" || !ISO_UTC.test(value) || Number.isNaN(Date.parse(value))) fail(path, "must be ISO UTC");
}

/** A relative hash route inside the sibling's own origin. Same rule for item href and handoff back. */
export function validHref(value, max = LIMITS.href) {
  if (typeof value !== "string" || value.length > max || !HREF.test(value)) return false;
  if (value.includes("//")) return false;
  let decoded;
  try { decoded = decodeURIComponent(value.replace(/\+/g, " ")); } catch { return false; }
  return !UNSAFE_TEXT.test(decoded) && !decoded.includes("\\") && !decoded.slice(2).startsWith("/");
}

/**
 * The routes each app may be linked at (item href and handoff back), matched
 * exactly; `{id}` is one OBJECT_ID segment, so no `%`, `?`, `/`, `.` or `..`
 * can get in. PR has none in v1 (its adapter adds `#/runs/{id}/{tab}`).
 */
export const ROUTES = {
  explain: ["#/", "#/library", "#/graph", "#/favorites", "#/repositories", "#/review", "#/note/{id}"],
  personal: ["#/today", "#/tasks", "#/tasks/{id}", "#/events", "#/events/{id}", "#/ideas", "#/ideas/{id}", "#/inbox", "#/inbox/{id}"],
  pr: ["#/", "#/home", "#/demo", "#/repos", "#/repos/{id}", "#/discover", "#/team", "#/group", "#/learning", "#/settings", "#/runs/{id}"],
  reader: ["#/", "#/library", "#/topics", "#/page/{id}"],
  campaign: ["#/", "#/schedule", "#/tasks", "#/partners", "#/forms", "#/files", "#/meetings", "#/inspirations", "#/notes", "#/bot"],
};

/** A relative hash route that is one of `app`'s allowed routes. */
export function allowedRoute(app, value, max = LIMITS.href) {
  if (!validHref(value, max)) return false;
  return (ROUTES[app] ?? []).some((route) => {
    const at = route.indexOf("{id}");
    if (at < 0) return value === route;
    const id = value.slice(at);
    return value.startsWith(route.slice(0, at)) && OBJECT_ID.test(id) && id !== "." && id !== "..";
  });
}

export function validateItem(item, path, appId) {
  if (!isObject(item)) fail(path, "must be an object");
  onlyKeys(item, path, ["sourceApp", "type", "id", "title", "href", "state", "updatedAt", "dueAt", "detail"]);
  if (item.sourceApp !== appId) fail(`${path}.sourceApp`, "must equal the feed appId");
  if (typeof item.type !== "string" || !OBJECT_TYPE.test(item.type)) fail(`${path}.type`, "invalid");
  if (typeof item.id !== "string" || !OBJECT_ID.test(item.id)) fail(`${path}.id`, "invalid");
  plain(item.title, `${path}.title`, LIMITS.title);
  if (!allowedRoute(appId, item.href)) fail(`${path}.href`, "not an allowed route of the source app");
  if (!ITEM_STATES.includes(item.state)) fail(`${path}.state`, "unknown state");
  iso(item.updatedAt, `${path}.updatedAt`);
  iso(item.dueAt, `${path}.dueAt`, { optional: true });
  plain(item.detail, `${path}.detail`, LIMITS.detail, { optional: true });
}

export function validateFeed(feed, path = "feed") {
  if (!isObject(feed)) fail(path, "must be an object");
  onlyKeys(feed, path, ["schemaVersion", "appId", "generatedAt", "counts", "items", "truncated"]);
  if (feed.schemaVersion !== 1) fail(`${path}.schemaVersion`, "unsupported version");
  if (!APP_IDS.includes(feed.appId)) fail(`${path}.appId`, "unknown app");
  iso(feed.generatedAt, `${path}.generatedAt`);
  if (!isObject(feed.counts)) fail(`${path}.counts`, "must be an object");
  const keys = Object.keys(feed.counts);
  if (keys.length > LIMITS.counts) fail(`${path}.counts`, "too many counts");
  for (const key of keys) {
    const value = feed.counts[key];
    if (!COUNT_KEY.test(key)) fail(`${path}.counts`, `invalid key ${key}`);
    if (!Number.isSafeInteger(value) || value < 0 || value > LIMITS.countMax) fail(`${path}.counts.${key}`, "must be a non-negative integer");
  }
  if (!Array.isArray(feed.items) || feed.items.length > LIMITS.items) fail(`${path}.items`, `at most ${LIMITS.items}`);
  feed.items.forEach((item, index) => validateItem(item, `${path}.items[${index}]`, feed.appId));
  if (typeof feed.truncated !== "boolean") fail(`${path}.truncated`, "must be boolean");
  return feed;
}

export function validateManifest(manifest, { ownOrigins = [] } = {}) {
  const path = "manifest";
  if (!isObject(manifest)) fail(path, "must be an object");
  onlyKeys(manifest, path, ["schemaVersion", "appId", "displayName", "summary", "capabilities", "links"]);
  if (manifest.schemaVersion !== 1) fail(`${path}.schemaVersion`, "unsupported version");
  if (!APP_IDS.includes(manifest.appId)) fail(`${path}.appId`, "unknown app");
  plain(manifest.displayName, `${path}.displayName`, LIMITS.displayName);
  plain(manifest.summary, `${path}.summary`, LIMITS.summary);
  if (!Array.isArray(manifest.capabilities) || manifest.capabilities.length > LIMITS.capabilities) fail(`${path}.capabilities`, "invalid list");
  const ids = new Set();
  manifest.capabilities.forEach((capability, index) => {
    const at = `${path}.capabilities[${index}]`;
    if (!isObject(capability)) fail(at, "must be an object");
    onlyKeys(capability, at, ["id", "kind", "route", "api", "accepts", "maxChars"]);
    if (typeof capability.id !== "string" || !CAPABILITY_ID.test(capability.id) || ids.has(capability.id)) fail(`${at}.id`, "invalid or duplicate");
    ids.add(capability.id);
    if (!CAPABILITY_KINDS.includes(capability.kind)) fail(`${at}.kind`, "unknown kind");
    if (capability.route !== undefined && (typeof capability.route !== "string" || !ROUTE_TEMPLATE.test(capability.route) || capability.route.includes("//"))) fail(`${at}.route`, "invalid");
    if (capability.api !== undefined && (typeof capability.api !== "string" || !API_PATH.test(capability.api) || capability.api.includes("//") || capability.api.includes(".."))) fail(`${at}.api`, "invalid");
    if (capability.kind === "accept-handoff") {
      if (!capability.route || !Array.isArray(capability.accepts) || !capability.accepts.length || capability.accepts.some((value) => value !== "text")) fail(at, "accept-handoff needs route and accepts:[\"text\"]");
      if (!Number.isSafeInteger(capability.maxChars) || capability.maxChars < 1 || capability.maxChars > LIMITS.handoffText) fail(`${at}.maxChars`, "1–2000");
    } else if (capability.accepts !== undefined || capability.maxChars !== undefined) fail(at, "accepts/maxChars only for accept-handoff");
    if (capability.kind === "today-feed" && (!capability.api || capability.route)) fail(at, "today-feed needs api only");
    if (capability.kind === "object-route" && (!capability.route || capability.api)) fail(at, "object-route needs route only");
  });
  if (!isObject(manifest.links)) fail(`${path}.links`, "must be an object");
  for (const [appId, origin] of Object.entries(manifest.links)) {
    if (!APP_IDS.includes(appId) || appId === manifest.appId) fail(`${path}.links`, `invalid app ${appId}`);
    browserOrigin(origin, ownOrigins, `${path}.links.${appId}`);
  }
  return manifest;
}

export function validateSources(response) {
  const path = "sources";
  if (!isObject(response)) fail(path, "must be an object");
  onlyKeys(response, path, ["schemaVersion", "sources"]);
  if (response.schemaVersion !== 1) fail(`${path}.schemaVersion`, "unsupported version");
  if (!Array.isArray(response.sources)) fail(`${path}.sources`, "must be a list");
  const seen = new Set();
  response.sources.forEach((source, index) => {
    const at = `${path}.sources[${index}]`;
    if (!isObject(source)) fail(at, "must be an object");
    onlyKeys(source, at, ["appId", "status", "lastSuccessAt", "feed"]);
    if (!["explain", "pr", "reader"].includes(source.appId) || seen.has(source.appId)) fail(`${at}.appId`, "invalid or duplicate");
    seen.add(source.appId);
    if (!SOURCE_STATUSES.includes(source.status)) fail(`${at}.status`, "unknown status");
    iso(source.lastSuccessAt, `${at}.lastSuccessAt`, { optional: true });
    const hasFeed = source.feed !== undefined;
    if (source.status === "ready" || source.status === "stale") {
      if (!hasFeed || source.lastSuccessAt === undefined) fail(at, `${source.status} needs feed and lastSuccessAt`);
      validateFeed(source.feed, `${at}.feed`);
      if (source.feed.appId !== source.appId) fail(`${at}.feed.appId`, "must equal appId");
    } else if (hasFeed) fail(at, `${source.status} carries no feed`);
    if ((source.status === "unconfigured" || source.status === "unsupported") && source.lastSuccessAt !== undefined) fail(at, `${source.status} has no lastSuccessAt`);
  });
  return response;
}

// ── Configuration ──────────────────────────────────────────────────────────

function loopbackHostname(hostname) {
  const bare = hostname.replace(/^\[|\]$/g, "");
  if (bare === "::1" || bare === "localhost") return true;
  const parts = bare.split(".");
  return parts.length === 4 && parts[0] === "127" && parts.every((part) => /^\d{1,3}$/.test(part) && Number(part) <= 255);
}
function loopbackIpLiteral(hostname) {
  return hostname !== "localhost" && loopbackHostname(hostname);
}
const tailnetOf = (hostname) => {
  const labels = hostname.toLowerCase().split(".");
  return labels.length >= 4 && labels.at(-1) === "net" && labels.at(-2) === "ts" ? labels.slice(-3).join(".") : undefined;
};
function exactOrigin(value, path) {
  if (typeof value !== "string" || !/^https?:\/\/[^/?#\\\s@*]+$/i.test(value) || value.endsWith(":")) fail(path, "not an exact origin");
  let url;
  try { url = new URL(value); } catch { fail(path, "not an exact origin"); }
  if (url.username || url.password || url.search || url.hash || url.pathname !== "/" || url.port === "0") fail(path, "not an exact origin");
  return url;
}

/**
 * Browser link origin: loopback (http or https), or https on a MagicDNS name in
 * the same tailnet as one of this app's own allowed origins. Any other https host is refused.
 */
export function browserOrigin(value, ownOrigins, path = "origin") {
  const url = exactOrigin(value, path);
  if (loopbackHostname(url.hostname)) return url.origin;
  if (url.protocol !== "https:") fail(path, "non-loopback origin must be https");
  const tailnet = tailnetOf(url.hostname);
  const own = new Set(ownOrigins.map((origin) => { try { return tailnetOf(new URL(origin).hostname); } catch { return undefined; } }).filter(Boolean));
  if (!tailnet || !own.has(tailnet)) fail(path, "only this app's own tailnet is accepted");
  return url.origin;
}

function entries(value, variable) {
  const text = (value ?? "").trim();
  if (!text) return [];
  const list = text.split(",").map((part) => part.trim());
  if (list.length > LIMITS.links) fail(variable, `at most ${LIMITS.links} entries`);
  const seen = new Set();
  return list.map((part) => {
    const match = /^([a-z]+)=(.+)$/.exec(part);
    if (!match) fail(variable, "entries are appId=origin");
    if (seen.has(match[1])) fail(variable, `duplicate ${match[1]}`);
    seen.add(match[1]);
    return [match[1], match[2]];
  });
}

/** BELL_SUITE_LINKS / BELLPERSONAL_SUITE_LINKS → { appId: origin }. Throws at boot. */
export function parseLinks(value, { self, ownOrigins = [], variable = "SUITE_LINKS" }) {
  const links = {};
  for (const [appId, origin] of entries(value, variable)) {
    if (!APP_IDS.includes(appId) || appId === self) fail(variable, `invalid app ${appId}`);
    links[appId] = browserOrigin(origin, ownOrigins, `${variable}.${appId}`);
  }
  return links;
}

/** BELLPERSONAL_SUITE_FETCH → { appId: origin }: http on an IP-literal loopback only (no DNS, not even localhost). */
export function parseFetch(value, { variable = "BELLPERSONAL_SUITE_FETCH" } = {}) {
  const targets = {};
  for (const [appId, origin] of entries(value, variable)) {
    if (!["explain", "pr", "reader"].includes(appId)) fail(variable, `invalid app ${appId}`);
    const url = exactOrigin(origin, `${variable}.${appId}`);
    if (url.protocol !== "http:" || !loopbackIpLiteral(url.hostname)) fail(`${variable}.${appId}`, "server reads only http on 127.0.0.0/8 or [::1]");
    targets[appId] = url.origin;
  }
  return targets;
}

// ── Handoff ────────────────────────────────────────────────────────────────

export const HANDOFF = {
  explain: { route: "/explain", textKey: "text", allowTitle: true },
  personal: { route: "/tasks", textKey: "draft", allowTitle: false },
  pr: { route: "/discover", textKey: "draft", allowTitle: true },
  reader: { route: "/read", textKey: "text", allowTitle: true },
  campaign: { route: "/tasks", textKey: "draft", allowTitle: true },
};
/**
 * The reserved handoff keys, on either target (rev. d). Any one of them on the target's
 * entry route makes a handoff attempt (isHandoffAttempt). D9: each may appear once only;
 * a parser that keeps the first copy and one that keeps the last would read different
 * text, senders or ways back, so a second copy of any of them refuses the whole handoff.
 */
export const SINGLE_HANDOFF_PARAMS = ["text", "title", "draft", "from", "back"];

function splitHandoffHash(hash) {
  const [path, query = ""] = String(hash).replace(/^#/, "").split("?", 2);
  return [path, query];
}

/**
 * Whether `hash` is a handoff attempt (rev. d): the target's entry route exactly
 * (`#/explain`, `#/tasks`) with any reserved key, even empty or under a percent-encoded
 * name. An attempt clears the address and the older draft first, then is checked;
 * parseHandoff returns undefined exactly when this is false. Other routes (for example
 * `#/note/{id}/{section}?from=personal`) and a query without reserved keys are not.
 */
export function isHandoffAttempt(hash, target) {
  const [path, query] = splitHandoffHash(hash);
  if (path !== HANDOFF[target].route || !query) return false;
  const params = new URLSearchParams(query);
  return SINGLE_HANDOFF_PARAMS.some((key) => params.has(key));
}

function cleanHandoff(value, max) {
  const text = value.replace(/\r\n?/g, "\n").replace(UNSAFE_HANDOFF, "").trim();
  const chars = [...text];
  return { text: chars.slice(0, max).join(""), truncated: chars.length > max };
}

/**
 * Reads a handoff from the target app's own hash. Returns undefined when the hash is
 * no handoff attempt (isHandoffAttempt); otherwise { rejected } — "duplicate" (a reserved
 * key twice), "missing" (no `text` / `draft` key for this target, e.g. only from/back/title),
 * "from" or "empty" — which the receiver reports as a recoverable error, or the draft.
 * `back` is dropped (draft kept) unless it is an allowed route of `from`.
 */
export function parseHandoff(hash, { target, links }) {
  const spec = HANDOFF[target];
  if (!isHandoffAttempt(hash, target)) return undefined;
  const params = new URLSearchParams(splitHandoffHash(hash)[1]);
  if (SINGLE_HANDOFF_PARAMS.some((key) => params.getAll(key).length > 1)) return { rejected: "duplicate" };
  if (!params.has(spec.textKey)) return { rejected: "missing" };
  const from = params.get("from");
  if (!from || !APP_IDS.includes(from) || from === target || !links[from]) return { rejected: "from" };
  const body = cleanHandoff(params.get(spec.textKey) ?? "", LIMITS.handoffText);
  if (!body.text) return { rejected: "empty" };
  const title = spec.allowTitle && params.get("title") ? cleanHandoff(params.get("title"), LIMITS.handoffTitle) : undefined;
  const back = params.get("back");
  const draft = { from, text: body.text, truncated: body.truncated };
  if (title?.text) draft.title = title.text;
  if (back && allowedRoute(from, back, LIMITS.back)) draft.backUrl = `${links[from]}/${back}`;
  return draft;
}

/** What the sending app writes; the receiver parses it with URLSearchParams ('+' is a space). */
export function handoffUrl({ target, origin, text, title, from, back }) {
  const spec = HANDOFF[target];
  const params = new URLSearchParams();
  params.set(spec.textKey, [...text].slice(0, LIMITS.handoffText).join(""));
  if (spec.allowTitle && title) params.set("title", [...title].slice(0, LIMITS.handoffTitle).join(""));
  params.set("from", from);
  if (back && allowedRoute(from, back, LIMITS.back)) params.set("back", back);
  return `${origin}/#${spec.route}?${params.toString()}`;
}
