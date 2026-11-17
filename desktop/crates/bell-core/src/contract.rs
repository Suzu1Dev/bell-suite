use crate::edition::AppId;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::time::Duration;
use thiserror::Error;
use url::Url;

pub const MAX_FETCH_BYTES: usize = 64 * 1024;
pub const TODAY_TIMEOUT: Duration = Duration::from_millis(2500);

#[derive(Debug, Error)]
#[error("{path}: {message}")]
pub struct ContractError {
    pub path: String,
    pub message: String,
}

impl ContractError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capability {
    pub id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_chars: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuiteManifest {
    pub schema_version: u64,
    pub app_id: AppId,
    pub display_name: String,
    pub summary: String,
    pub capabilities: Vec<Capability>,
    pub links: BTreeMap<AppId, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodayItem {
    pub source_app: AppId,
    #[serde(rename = "type")]
    pub item_type: String,
    pub id: String,
    pub title: String,
    pub href: String,
    pub state: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodayFeed {
    pub schema_version: u64,
    pub app_id: AppId,
    pub generated_at: String,
    pub counts: BTreeMap<String, u64>,
    pub items: Vec<TodayItem>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodaySource {
    pub app_id: AppId,
    pub status: String,
    pub counts: BTreeMap<String, u64>,
    pub items: Vec<TodayItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodayOverview {
    pub generated_at: String,
    pub total_items: usize,
    pub sources: Vec<TodaySource>,
}

fn has_unsafe_text(s: &str) -> bool {
    s.chars().any(|c| {
        matches!(
            c as u32,
            0x0000..=0x001f | 0x007f..=0x009f | 0x202a..=0x202e | 0x2066..=0x2069
        )
    })
}

fn only_keys(
    obj: &serde_json::Map<String, Value>,
    path: &str,
    allowed: &[&str],
) -> Result<(), ContractError> {
    for key in obj.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(ContractError::new(path, format!("unknown key {key}")));
        }
    }
    Ok(())
}

fn plain_text(val: Option<&Value>, path: &str, max: usize) -> Result<String, ContractError> {
    let s = val
        .and_then(Value::as_str)
        .ok_or_else(|| ContractError::new(path, "must be non-empty text"))?;
    if s.trim().is_empty() {
        return Err(ContractError::new(path, "must be non-empty text"));
    }
    if s.chars().count() > max {
        return Err(ContractError::new(path, format!("longer than {max}")));
    }
    if has_unsafe_text(s) {
        return Err(ContractError::new(path, "control or bidi character"));
    }
    Ok(s.to_string())
}

fn is_iso_utc(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 20 || b.len() > 24 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T'
        || b[13] != b':' || b[16] != b':' || *b.last().unwrap() != b'Z'
    {
        return false;
    }
    let digits_at = |range: std::ops::Range<usize>| b[range].iter().all(|c| c.is_ascii_digit());
    if !digits_at(0..4)
        || !digits_at(5..7)
        || !digits_at(8..10)
        || !digits_at(11..13)
        || !digits_at(14..16)
        || !digits_at(17..19)
    {
        return false;
    }
    if b.len() > 20 && (b[19] != b'.' || b.len() == 21 || !digits_at(20..b.len() - 1)) {
        return false;
    }
    DateTime::parse_from_rfc3339(s).is_ok()
}

fn iso_field(val: Option<&Value>, path: &str) -> Result<String, ContractError> {
    let s = val
        .and_then(Value::as_str)
        .ok_or_else(|| ContractError::new(path, "must be ISO UTC"))?;
    if !is_iso_utc(s) {
        return Err(ContractError::new(path, "must be ISO UTC"));
    }
    Ok(s.to_string())
}

fn is_object_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
}

fn is_object_type(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 32
        && b[0].is_ascii_lowercase()
        && b[1..]
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

fn is_count_key(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 32
        && b[0].is_ascii_lowercase()
        && b[1..].iter().all(|c| c.is_ascii_alphanumeric())
}

fn is_capability_id(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() >= 2
        && parts.iter().all(|part| {
            let b = part.as_bytes();
            !b.is_empty()
                && b[0].is_ascii_lowercase()
                && b[1..]
                    .iter()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        })
}

fn is_route_template(s: &str) -> bool {
    s.starts_with("#/")
        && !s.contains("//")
        && s[2..].bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'/' | b'_' | b'-' | b'.' | b'~' | b'{' | b'}')
        })
}

fn is_api_path(s: &str) -> bool {
    s.starts_with("/api/")
        && s.len() > 5
        && !s.contains("//")
        && !s.contains("..")
        && s[5..]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'_' | b'-' | b'.'))
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' => {
                if i + 2 >= bytes.len() {
                    return None;
                }
                let hi = (bytes[i + 1] as char).to_digit(16)?;
                let lo = (bytes[i + 2] as char).to_digit(16)?;
                out.push(((hi << 4) | lo) as u8);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

pub fn valid_href(value: &str, max: usize) -> bool {
    if value.len() > max || !value.starts_with("#/") || value.contains("//") {
        return false;
    }
    let rest = &value[2..];
    let (path_part, query_part) = match rest.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (rest, None),
    };
    if !path_part.bytes().all(|b| {
        b.is_ascii_alphanumeric()
            || matches!(b, b'/' | b'_' | b'-' | b'.' | b'~' | b'%' | b':')
    }) {
        return false;
    }
    if let Some(q) = query_part {
        if !q.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(b, b'_' | b'-' | b'.' | b'~' | b'%' | b'=' | b'&' | b'+')
        }) {
            return false;
        }
    }
    let Some(decoded) = percent_decode(value) else {
        return false;
    };
    !has_unsafe_text(&decoded) && !decoded.contains('\\') && !decoded[2..].starts_with('/')
}

fn app_routes(app: AppId) -> &'static [&'static str] {
    match app {
        AppId::Explain => &[
            "#/",
            "#/library",
            "#/graph",
            "#/favorites",
            "#/repositories",
            "#/review",
            "#/note/{id}",
        ],
        AppId::Personal => &[
            "#/today",
            "#/tasks",
            "#/tasks/{id}",
            "#/events",
            "#/events/{id}",
            "#/ideas",
            "#/ideas/{id}",
            "#/inbox",
            "#/inbox/{id}",
        ],
        AppId::Pr => &[
            "#/",
            "#/home",
            "#/demo",
            "#/repos",
            "#/repos/{id}",
            "#/discover",
            "#/team",
            "#/group",
            "#/learning",
            "#/settings",
            "#/runs/{id}",
        ],
        AppId::Reader => &["#/", "#/library", "#/topics", "#/page/{id}"],
        AppId::Campaign => &[
            "#/",
            "#/schedule",
            "#/tasks",
            "#/partners",
            "#/forms",
            "#/files",
            "#/meetings",
            "#/inspirations",
            "#/notes",
            "#/bot",
        ],
    }
}

pub fn allowed_route(app: AppId, value: &str) -> bool {
    if !valid_href(value, 512) {
        return false;
    }
    app_routes(app).iter().any(|route| {
        if let Some(at) = route.find("{id}") {
            let prefix = &route[..at];
            if !value.starts_with(prefix) {
                return false;
            }
            let id = &value[at..];
            is_object_id(id) && id != "." && id != ".."
        } else {
            value == *route
        }
    })
}

fn is_loopback_hostname(hostname: &str) -> bool {
    let bare = hostname.trim_start_matches('[').trim_end_matches(']');
    if bare == "::1" || bare.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let parts: Vec<&str> = bare.split('.').collect();
    parts.len() == 4
        && parts[0] == "127"
        && parts.iter().all(|p| !p.is_empty() && p.len() <= 3 && p.parse::<u8>().is_ok())
}

fn tailnet_of(hostname: &str) -> Option<String> {
    let lower = hostname.to_ascii_lowercase();
    let labels: Vec<&str> = lower.split('.').collect();
    if labels.len() >= 4
        && labels[labels.len() - 1] == "net"
        && labels[labels.len() - 2] == "ts"
    {
        Some(labels[labels.len() - 3..].join("."))
    } else {
        None
    }
}

fn exact_origin(value: &str, path: &str) -> Result<Url, ContractError> {
    let lower = value.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("http://")
        .or_else(|| lower.strip_prefix("https://"))
        .ok_or_else(|| ContractError::new(path, "not an exact origin"))?;
    if rest.is_empty()
        || value.ends_with(':')
        || value.ends_with(":0")
        || rest
            .chars()
            .any(|c| matches!(c, '/' | '?' | '#' | '\\' | '@' | '*') || c.is_whitespace())
    {
        return Err(ContractError::new(path, "not an exact origin"));
    }
    let url = Url::parse(value).map_err(|_| ContractError::new(path, "not an exact origin"))?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || url.port() == Some(0)
        || url.host_str().is_none()
    {
        return Err(ContractError::new(path, "not an exact origin"));
    }
    Ok(url)
}

pub fn browser_origin(
    value: &str,
    own_origins: &[String],
    path: &str,
) -> Result<String, ContractError> {
    let url = exact_origin(value, path)?;
    let host = url.host_str().unwrap_or_default();
    let normalized = url.origin().ascii_serialization();
    if is_loopback_hostname(host) {
        return Ok(normalized);
    }
    if url.scheme() != "https" {
        return Err(ContractError::new(path, "non-loopback origin must be https"));
    }
    let Some(tailnet) = tailnet_of(host) else {
        return Err(ContractError::new(
            path,
            "only this app's own tailnet is accepted",
        ));
    };
    let own_tailnets: HashSet<String> = own_origins
        .iter()
        .filter_map(|orig| Url::parse(orig).ok()?.host_str().and_then(tailnet_of))
        .collect();
    if !own_tailnets.contains(&tailnet) {
        return Err(ContractError::new(
            path,
            "only this app's own tailnet is accepted",
        ));
    }
    Ok(normalized)
}

pub fn validate_manifest(
    manifest: &Value,
    own_origins: &[String],
) -> Result<SuiteManifest, ContractError> {
    let path = "manifest";
    let obj = manifest
        .as_object()
        .ok_or_else(|| ContractError::new(path, "must be an object"))?;
    only_keys(
        obj,
        path,
        &[
            "schemaVersion",
            "appId",
            "displayName",
            "summary",
            "capabilities",
            "links",
        ],
    )?;

    let schema_version = obj
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .filter(|v| *v == 1)
        .ok_or_else(|| ContractError::new("manifest.schemaVersion", "unsupported version"))?;

    let app_id: AppId = obj
        .get("appId")
        .and_then(Value::as_str)
        .ok_or_else(|| ContractError::new("manifest.appId", "unknown app"))?
        .parse()
        .map_err(|_| ContractError::new("manifest.appId", "unknown app"))?;

    let display_name = plain_text(obj.get("displayName"), "manifest.displayName", 40)?;
    let summary = plain_text(obj.get("summary"), "manifest.summary", 40)?;

    let caps_arr = obj
        .get("capabilities")
        .and_then(Value::as_array)
        .filter(|a| a.len() <= 16)
        .ok_or_else(|| ContractError::new("manifest.capabilities", "invalid list"))?;

    let mut seen_ids = HashSet::new();
    let mut capabilities = Vec::with_capacity(caps_arr.len());
    for (idx, cap_val) in caps_arr.iter().enumerate() {
        let at = format!("manifest.capabilities[{idx}]");
        let cap_obj = cap_val
            .as_object()
            .ok_or_else(|| ContractError::new(&at, "must be an object"))?;
        only_keys(
            cap_obj,
            &at,
            &["id", "kind", "route", "api", "accepts", "maxChars"],
        )?;

        let id = cap_obj
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| is_capability_id(s))
            .ok_or_else(|| ContractError::new(format!("{at}.id"), "invalid or duplicate"))?;
        if !seen_ids.insert(id.to_string()) {
            return Err(ContractError::new(format!("{at}.id"), "invalid or duplicate"));
        }

        let kind = cap_obj
            .get("kind")
            .and_then(Value::as_str)
            .filter(|k| matches!(*k, "accept-handoff" | "today-feed" | "object-route"))
            .ok_or_else(|| ContractError::new(format!("{at}.kind"), "unknown kind"))?;

        let route = match cap_obj.get("route") {
            None => None,
            Some(v) => {
                let r = v
                    .as_str()
                    .filter(|s| is_route_template(s))
                    .ok_or_else(|| ContractError::new(format!("{at}.route"), "invalid"))?;
                Some(r.to_string())
            }
        };

        let api = match cap_obj.get("api") {
            None => None,
            Some(v) => {
                let a = v
                    .as_str()
                    .filter(|s| is_api_path(s))
                    .ok_or_else(|| ContractError::new(format!("{at}.api"), "invalid"))?;
                Some(a.to_string())
            }
        };

        let mut accepts = None;
        let mut max_chars = None;
        if kind == "accept-handoff" {
            if route.is_none() {
                return Err(ContractError::new(
                    &at,
                    "accept-handoff needs route and accepts:[\"text\"]",
                ));
            }
            let acc_arr = cap_obj
                .get("accepts")
                .and_then(Value::as_array)
                .filter(|a| !a.is_empty() && a.iter().all(|v| v.as_str() == Some("text")))
                .ok_or_else(|| {
                    ContractError::new(&at, "accept-handoff needs route and accepts:[\"text\"]")
                })?;
            accepts = Some(
                acc_arr
                    .iter()
                    .map(|v| v.as_str().unwrap_or("text").to_string())
                    .collect(),
            );
            let mc = cap_obj
                .get("maxChars")
                .and_then(Value::as_u64)
                .filter(|n| (1..=2000).contains(n))
                .ok_or_else(|| ContractError::new(format!("{at}.maxChars"), "1-2000"))?;
            max_chars = Some(mc);
        } else if cap_obj.contains_key("accepts") || cap_obj.contains_key("maxChars") {
            return Err(ContractError::new(
                &at,
                "accepts/maxChars only for accept-handoff",
            ));
        }

        if kind == "today-feed" && (api.is_none() || route.is_some()) {
            return Err(ContractError::new(&at, "today-feed needs api only"));
        }
        if kind == "object-route" && (route.is_none() || api.is_some()) {
            return Err(ContractError::new(&at, "object-route needs route only"));
        }

        capabilities.push(Capability {
            id: id.to_string(),
            kind: kind.to_string(),
            route,
            api,
            accepts,
            max_chars,
        });
    }

    let links_obj = obj
        .get("links")
        .and_then(Value::as_object)
        .filter(|m| m.len() <= 5)
        .ok_or_else(|| ContractError::new("manifest.links", "must be an object"))?;

    let normalized_own: HashSet<String> = own_origins
        .iter()
        .filter_map(|o| Url::parse(o).ok().map(|u| u.origin().ascii_serialization()))
        .collect();

    let mut links = BTreeMap::new();
    for (link_key, origin_val) in links_obj {
        let target_app: AppId = link_key
            .parse()
            .map_err(|_| ContractError::new("manifest.links", format!("invalid app {link_key}")))?;
        if target_app == app_id {
            return Err(ContractError::new(
                "manifest.links",
                format!("invalid app {link_key}"),
            ));
        }
        let origin_str = origin_val.as_str().ok_or_else(|| {
            ContractError::new(format!("manifest.links.{link_key}"), "not an exact origin")
        })?;
        let validated_origin = browser_origin(
            origin_str,
            own_origins,
            &format!("manifest.links.{link_key}"),
        )?;
        if normalized_own.contains(&validated_origin) {
            return Err(ContractError::new(
                format!("manifest.links.{link_key}"),
                "must not link to own origin",
            ));
        }
        links.insert(target_app, validated_origin);
    }

    Ok(SuiteManifest {
        schema_version,
        app_id,
        display_name,
        summary,
        capabilities,
        links,
    })
}

fn validate_item(val: &Value, path: &str, app_id: AppId) -> Result<TodayItem, ContractError> {
    let obj = val
        .as_object()
        .ok_or_else(|| ContractError::new(path, "must be an object"))?;
    only_keys(
        obj,
        path,
        &[
            "sourceApp",
            "type",
            "id",
            "title",
            "href",
            "state",
            "updatedAt",
            "dueAt",
            "detail",
        ],
    )?;

    let source_app: AppId = obj
        .get("sourceApp")
        .and_then(Value::as_str)
        .and_then(|s| s.parse().ok())
        .filter(|a| *a == app_id)
        .ok_or_else(|| {
            ContractError::new(format!("{path}.sourceApp"), "must equal the feed appId")
        })?;

    let item_type = obj
        .get("type")
        .and_then(Value::as_str)
        .filter(|s| is_object_type(s))
        .ok_or_else(|| ContractError::new(format!("{path}.type"), "invalid"))?
        .to_string();

    let id = obj
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| is_object_id(s))
        .ok_or_else(|| ContractError::new(format!("{path}.id"), "invalid"))?
        .to_string();

    let title = plain_text(obj.get("title"), &format!("{path}.title"), 120)?;

    let href = obj
        .get("href")
        .and_then(Value::as_str)
        .filter(|s| allowed_route(app_id, s))
        .ok_or_else(|| {
            ContractError::new(format!("{path}.href"), "not an allowed route of the source app")
        })?
        .to_string();

    let state = obj
        .get("state")
        .and_then(Value::as_str)
        .filter(|s| matches!(*s, "due" | "new" | "waiting" | "needs-you"))
        .ok_or_else(|| ContractError::new(format!("{path}.state"), "unknown state"))?
        .to_string();

    let updated_at = iso_field(obj.get("updatedAt"), &format!("{path}.updatedAt"))?;
    let due_at = match obj.get("dueAt") {
        None => None,
        Some(v) => Some(iso_field(Some(v), &format!("{path}.dueAt"))?),
    };
    let detail = match obj.get("detail") {
        None => None,
        Some(v) => Some(plain_text(Some(v), &format!("{path}.detail"), 80)?),
    };

    Ok(TodayItem {
        source_app,
        item_type,
        id,
        title,
        href,
        state,
        updated_at,
        due_at,
        detail,
    })
}

pub fn validate_feed(feed: &Value) -> Result<TodayFeed, ContractError> {
    let path = "feed";
    let obj = feed
        .as_object()
        .ok_or_else(|| ContractError::new(path, "must be an object"))?;
    only_keys(
        obj,
        path,
        &[
            "schemaVersion",
            "appId",
            "generatedAt",
            "counts",
            "items",
            "truncated",
        ],
    )?;

    let schema_version = obj
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .filter(|v| *v == 1)
        .ok_or_else(|| ContractError::new("feed.schemaVersion", "unsupported version"))?;

    let app_id: AppId = obj
        .get("appId")
        .and_then(Value::as_str)
        .ok_or_else(|| ContractError::new("feed.appId", "unknown app"))?
        .parse()
        .map_err(|_| ContractError::new("feed.appId", "unknown app"))?;

    let generated_at = iso_field(obj.get("generatedAt"), "feed.generatedAt")?;

    let counts_obj = obj
        .get("counts")
        .and_then(Value::as_object)
        .ok_or_else(|| ContractError::new("feed.counts", "must be an object"))?;
    if counts_obj.len() > 8 {
        return Err(ContractError::new("feed.counts", "too many counts"));
    }
    let mut counts = BTreeMap::new();
    for (k, v) in counts_obj {
        if !is_count_key(k) {
            return Err(ContractError::new("feed.counts", format!("invalid key {k}")));
        }
        let n = v
            .as_u64()
            .filter(|n| *n <= 100_000)
            .ok_or_else(|| {
                ContractError::new(format!("feed.counts.{k}"), "must be a non-negative integer")
            })?;
        counts.insert(k.clone(), n);
    }

    let items_arr = obj
        .get("items")
        .and_then(Value::as_array)
        .filter(|a| a.len() <= 5)
        .ok_or_else(|| ContractError::new("feed.items", "at most 5"))?;
    let mut items = Vec::with_capacity(items_arr.len());
    for (idx, item_val) in items_arr.iter().enumerate() {
        items.push(validate_item(
            item_val,
            &format!("feed.items[{idx}]"),
            app_id,
        )?);
    }

    let truncated = obj
        .get("truncated")
        .and_then(Value::as_bool)
        .ok_or_else(|| ContractError::new("feed.truncated", "must be boolean"))?;

    Ok(TodayFeed {
        schema_version,
        app_id,
        generated_at,
        counts,
        items,
        truncated,
    })
}

pub async fn read_bounded_bytes(
    mut res: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    while let Some(chunk) = res.chunk().await.map_err(|e| e.to_string())? {
        if buf.len() + chunk.len() > max_bytes {
            return Err(format!("response exceeded {max_bytes} bytes"));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

async fn fetch_today_source(
    client: reqwest::Client,
    app_id: AppId,
    port: u16,
    bearer_token: Option<String>,
) -> TodaySource {
    let url = format!("http://127.0.0.1:{port}/api/suite/today");
    let unavailable = |status: &str| TodaySource {
        app_id,
        status: status.to_string(),
        counts: BTreeMap::new(),
        items: Vec::new(),
    };

    let mut req = client
        .get(&url)
        .header("Accept", "application/json")
        .timeout(TODAY_TIMEOUT);
    if let Some(tok) = bearer_token {
        req = req.bearer_auth(tok);
    }

    let Ok(res) = req.send().await else {
        return unavailable("unavailable");
    };

    let code = res.status().as_u16();
    if code == 404 || code == 501 {
        return unavailable("unsupported");
    }
    if code != 200 {
        return unavailable("unavailable");
    }

    let Ok(bytes) = read_bounded_bytes(res, MAX_FETCH_BYTES).await else {
        return unavailable("unavailable");
    };
    let Ok(val) = serde_json::from_slice::<Value>(&bytes) else {
        return unavailable("unavailable");
    };
    match validate_feed(&val) {
        Ok(feed) if feed.app_id == app_id => TodaySource {
            app_id,
            status: "ready".to_string(),
            counts: feed.counts,
            items: feed.items,
        },
        _ => unavailable("unavailable"),
    }
}

pub async fn aggregate_today(running_workspaces: &[(AppId, u16)]) -> TodayOverview {
    aggregate_today_with_tokens(running_workspaces, &BTreeMap::new()).await
}

pub async fn aggregate_today_with_tokens(
    running_workspaces: &[(AppId, u16)],
    tokens: &BTreeMap<AppId, String>,
) -> TodayOverview {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(TODAY_TIMEOUT)
        .build()
        .unwrap_or_default();

    let mut set = tokio::task::JoinSet::new();
    for &(app_id, port) in running_workspaces {
        let c = client.clone();
        let tok = tokens.get(&app_id).cloned();
        set.spawn(async move { fetch_today_source(c, app_id, port, tok).await });
    }

    let mut sources = Vec::with_capacity(running_workspaces.len());
    while let Some(Ok(source)) = set.join_next().await {
        sources.push(source);
    }
    sources.sort_by_key(|s| s.app_id);

    let total_items = sources.iter().map(|s| s.items.len()).sum();
    TodayOverview {
        generated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        total_items,
        sources,
    }
}

pub fn build_suite_env(
    app_id: AppId,
    ports_by_app: &BTreeMap<AppId, u16>,
    host: &str,
    vault_root: &Path,
) -> BTreeMap<String, String> {
    let connect_host = if host == "0.0.0.0" || host == "::" {
        "127.0.0.1"
    } else {
        host
    };
    let origin_host = if connect_host.contains(':') && !connect_host.starts_with('[') {
        format!("[{connect_host}]")
    } else {
        connect_host.to_string()
    };
    let fetch_host = if connect_host.eq_ignore_ascii_case("localhost") {
        "127.0.0.1".to_string()
    } else {
        origin_host.clone()
    };

    let links = ports_by_app
        .iter()
        .filter(|(&id, _)| id != app_id)
        .map(|(&id, &p)| format!("{}=http://{origin_host}:{p}", id.as_str()))
        .collect::<Vec<_>>()
        .join(",");

    let personal_fetch = [AppId::Explain, AppId::Pr, AppId::Reader]
        .into_iter()
        .filter(|&id| id != app_id)
        .filter_map(|id| ports_by_app.get(&id).map(|p| format!("{}=http://{fetch_host}:{p}", id.as_str())))
        .collect::<Vec<_>>()
        .join(",");

    let reader_fetch = [AppId::Explain, AppId::Pr]
        .into_iter()
        .filter(|&id| id != app_id)
        .filter_map(|id| ports_by_app.get(&id).map(|p| format!("{}=http://{fetch_host}:{p}", id.as_str())))
        .collect::<Vec<_>>()
        .join(",");

    let port = ports_by_app
        .get(&app_id)
        .copied()
        .unwrap_or_else(|| app_id.default_port())
        .to_string();

    let mut env = BTreeMap::new();
    env.insert("HOST".into(), host.into());
    env.insert("PORT".into(), port.clone());
    env.insert("BELL_EDITION".into(), "suite".into());
    env.insert("BELLPR_EDITION".into(), "suite".into());
    env.insert("BELLPERSONAL_EDITION".into(), "suite".into());
    env.insert("BELL_READER_EDITION".into(), "suite".into());
    env.insert("BELL_CAMPAIGN_EDITION".into(), "suite".into());
    env.insert("BELL_NO_OPEN".into(), "1".into());
    env.insert("BELLPR_NO_OPEN".into(), "1".into());

    // Populate per-app data directory env vars pointing inside <vault_root>/<app_id>
    let personal_dir = vault_root.join(AppId::Personal.as_str());
    let explain_dir = vault_root.join(AppId::Explain.as_str());
    let pr_dir = vault_root.join(AppId::Pr.as_str());
    let reader_dir = vault_root.join(AppId::Reader.as_str());
    let campaign_dir = vault_root.join(AppId::Campaign.as_str());

    env.insert(
        "BELLPERSONAL_DATA_DIR".into(),
        personal_dir.to_string_lossy().into_owned(),
    );
    env.insert(
        "BELL_DATA_DIR".into(),
        explain_dir.to_string_lossy().into_owned(),
    );
    env.insert(
        "BELL_DATA_PATH".into(),
        explain_dir.join("bell.sqlite").to_string_lossy().into_owned(),
    );
    env.insert(
        "BELLPR_DATA_DIR".into(),
        pr_dir.to_string_lossy().into_owned(),
    );
    env.insert(
        "BELL_READER_DATA_DIR".into(),
        reader_dir.to_string_lossy().into_owned(),
    );
    env.insert(
        "BELL_CAMPAIGN_DATA_DIR".into(),
        campaign_dir.to_string_lossy().into_owned(),
    );

    match app_id {
        AppId::Personal => {
            env.insert("BELLPERSONAL_HOST".into(), host.into());
            env.insert("BELLPERSONAL_PORT".into(), port.clone());
            env.insert("BELLPERSONAL_FRONTEND_PORT".into(), port);
            env.insert("BELLPERSONAL_SUITE_LINKS".into(), links);
            if !personal_fetch.is_empty() {
                env.insert("BELLPERSONAL_SUITE_FETCH".into(), personal_fetch);
            }
            let personal_secrets_dir = vault_root.join("personal-secrets");
            env.insert(
                "BELLPERSONAL_SECRETS_DIR".into(),
                personal_secrets_dir.to_string_lossy().into_owned(),
            );
            if let Some(key) = crate::vault::read_personal_master_key_at_root(vault_root) {
                env.insert("BELLPERSONAL_MASTER_KEY".into(), key);
            }
        }
        AppId::Explain => {
            env.insert("BELL_HOST".into(), host.into());
            env.insert("BELL_PORT".into(), port);
            env.insert("BELL_SUITE_LINKS".into(), links);
        }
        AppId::Pr => {
            env.insert("BELLPR_HOST".into(), host.into());
            env.insert("BELLPR_PORT".into(), port);
            env.insert("BELLPR_SUITE_LINKS".into(), links.clone());
            env.insert("BELL_SUITE_LINKS".into(), links);
        }
        AppId::Reader => {
            env.insert("BELL_READER_HOST".into(), host.into());
            env.insert("BELL_READER_PORT".into(), port);
            env.insert("BELL_READER_SUITE_LINKS".into(), links);
            if !reader_fetch.is_empty() {
                env.insert("BELL_READER_SUITE_FETCH".into(), reader_fetch);
            }
        }
        AppId::Campaign => {
            env.insert("BELL_CAMPAIGN_HOST".into(), host.into());
            env.insert("BELL_CAMPAIGN_PORT".into(), port);
            env.insert("BELL_CAMPAIGN_SUITE_LINKS".into(), links);
        }
    }

    env
}
