use crate::i18n::{
    app_badge_name, app_summary, copy_for, deep_routes, esc, format_shortcut, sanitize_hash_route,
    today_state_label, ws_state_label, Lang,
};
use crate::icons;
use bell_core::{AppId, DesktopStatus, TodayOverview, WorkspaceState};
use std::fmt::Write as _;

#[derive(Debug, Clone)]
pub enum PaletteExec {
    OpenWs(AppId),
    JumpRoute(AppId, String),
    SetView(&'static str),
    StartAll,
    VaultBackup,
    ToggleTheme,
    ToggleLang,
}

#[derive(Debug, Clone)]
pub struct PaletteEntry {
    pub group: &'static str,
    pub app: Option<AppId>,
    pub icon_html: &'static str,
    pub label: String,
    pub sub: String,
    pub kbd: Option<String>,
    pub search_blob: String,
    pub exec: PaletteExec,
}

pub fn build_palette_entries(
    status: &DesktopStatus,
    today: Option<&TodayOverview>,
    lang: Lang,
) -> Vec<PaletteEntry> {
    let c = copy_for(lang);
    let mut entries = Vec::with_capacity(36);

    // 1. Workspaces (⌘1..⌘5)
    for app in AppId::ALL {
        let ws = status.workspaces.iter().find(|w| w.id == app);
        let state = ws.map(|w| w.state).unwrap_or(WorkspaceState::Stopped);
        let port = ws.map(|w| w.port).unwrap_or_else(|| app.default_port());
        let label = app.display_name().to_string();
        let sub = format!(
            "{} · :{} · {}",
            ws_state_label(state, lang),
            port,
            app_summary(app, lang)
        );
        let idx_str = app.shortcut_index().to_string();
        let kbd = Some(format_shortcut(&status.os, &idx_str));
        let search_blob = format!(
            "{} {} {} {}",
            label.to_lowercase(),
            app.as_str(),
            sub.to_lowercase(),
            app.shortcut_index()
        );
        entries.push(PaletteEntry {
            group: c.palette_group_ws,
            app: Some(app),
            icon_html: icons::app_icon(app),
            label,
            sub,
            kbd,
            search_blob,
            exec: PaletteExec::OpenWs(app),
        });
    }

    // 2. Live Today Items
    if let Some(ov) = today {
        for src in &ov.sources {
            for item in &src.items {
                let clean_href = sanitize_hash_route(&item.href).to_string();
                let label = item.title.clone();
                let st_lbl = today_state_label(&item.state, lang);
                let sub = match &item.detail {
                    Some(d) if !d.is_empty() => {
                        format!("{} · {} · {}", app_badge_name(item.source_app), st_lbl, d)
                    }
                    _ => format!(
                        "{} · {} · {}",
                        app_badge_name(item.source_app),
                        st_lbl,
                        clean_href
                    ),
                };
                let search_blob = format!(
                    "{} {} {} {}",
                    label.to_lowercase(),
                    sub.to_lowercase(),
                    item.source_app.as_str(),
                    clean_href.to_lowercase()
                );
                entries.push(PaletteEntry {
                    group: c.palette_group_today,
                    app: Some(item.source_app),
                    icon_html: icons::inbox(),
                    label,
                    sub,
                    kbd: Some(clean_href.clone()),
                    exec: PaletteExec::JumpRoute(item.source_app, clean_href),
                    search_blob,
                });
            }
        }
    }

    // 3. Deep Routes
    for app in AppId::ALL {
        for &(route, route_label) in deep_routes(app, lang) {
            let label = format!("{} · {}", app.display_name(), route_label);
            let sub = route.to_string();
            let search_blob = format!(
                "{} {} {} route",
                label.to_lowercase(),
                route.to_lowercase(),
                app.as_str()
            );
            entries.push(PaletteEntry {
                group: c.palette_group_routes,
                app: Some(app),
                icon_html: icons::app_icon(app),
                label,
                sub,
                kbd: Some(route.to_string()),
                search_blob,
                exec: PaletteExec::JumpRoute(app, route.to_string()),
            });
        }
    }

    // 4. Hub Views & System Actions
    let view_cmds: [(&str, &str, &'static str); 5] = [
        ("overview", c.nav_overview, icons::overview()),
        ("today", c.nav_today, icons::inbox()),
        ("workspaces", c.nav_workspaces, icons::layers()),
        ("vault", c.nav_vault, icons::database()),
        ("logs", c.nav_logs, icons::terminal()),
    ];
    for (view_id, view_name, ico) in view_cmds {
        let label = format!("{} · {}", c.hub_name, view_name);
        let search_blob = format!("{} {} view hub", label.to_lowercase(), view_id);
        entries.push(PaletteEntry {
            group: c.palette_group_actions,
            app: None,
            icon_html: ico,
            label,
            sub: c.palette_sub_switch_view.to_string(),
            kbd: None,
            search_blob,
            exec: PaletteExec::SetView(view_id),
        });
    }

    let sys_cmds: [(&str, &str, &'static str, PaletteExec); 4] = [
        (
            c.palette_act_start_all,
            "start all workspaces boot",
            icons::bolt(),
            PaletteExec::StartAll,
        ),
        (
            c.palette_act_snapshot,
            "vault sqlite backup snapshot",
            icons::database(),
            PaletteExec::VaultBackup,
        ),
        (
            c.palette_act_theme,
            "theme dark light mode appearance",
            icons::overview(),
            PaletteExec::ToggleTheme,
        ),
        (
            c.palette_act_lang,
            "language zh-tw en english chinese",
            icons::layers(),
            PaletteExec::ToggleLang,
        ),
    ];

    for (title, keywords, ico, exec) in sys_cmds {
        entries.push(PaletteEntry {
            group: c.palette_group_actions,
            app: None,
            icon_html: ico,
            label: title.to_string(),
            sub: c.palette_sub_sys_cmd.to_string(),
            kbd: None,
            search_blob: format!("{} {}", title.to_lowercase(), keywords),
            exec,
        });
    }

    entries
}

pub fn filter_palette<'a>(entries: &'a [PaletteEntry], query: &str) -> Vec<&'a PaletteEntry> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        // Default curated compact view when query is empty:
        // all 5 workspaces + Today items + first route of each app + system commands (skipping 20 secondary routes until user types)
        let mut out = Vec::with_capacity(20);
        for e in entries {
            let include = match &e.exec {
                PaletteExec::JumpRoute(_, r) => {
                    matches!(r.as_str(), "#/today" | "#/review" | "#/discover" | "#/library")
                }
                _ => true,
            };
            if include {
                out.push(e);
            }
        }
        return out;
    }

    let terms: Vec<&str> = q.split_whitespace().collect();
    entries
        .iter()
        .filter(|e| terms.iter().all(|t| e.search_blob.contains(t)))
        .collect()
}

pub fn render_palette_modal(
    status: &DesktopStatus,
    today: Option<&TodayOverview>,
    query: &str,
    selected_index: usize,
    lang: Lang,
) -> String {
    let c = copy_for(lang);
    let all = build_palette_entries(status, today, lang);
    let filtered = filter_palette(&all, query);
    let active_idx = if filtered.is_empty() {
        0
    } else {
        selected_index.min(filtered.len() - 1)
    };

    let mut list_html = String::with_capacity(4096);
    if filtered.is_empty() {
        let _ = write!(
            list_html,
            r#"<div class="palette-empty">{}</div>"#,
            esc(c.palette_empty)
        );
    } else {
        let mut last_group = "";
        for (idx, item) in filtered.iter().enumerate() {
            if item.group != last_group {
                last_group = item.group;
                let _ = write!(
                    list_html,
                    r#"<div class="palette-group-head">{}</div>"#,
                    esc(last_group)
                );
            }
            let is_active = idx == active_idx;
            let active_id = if is_active {
                r#" id="palette-active-item""#
            } else {
                ""
            };
            let app_attr = match item.app {
                Some(a) => format!(r#" data-app="{}""#, a.as_str()),
                None => String::new(),
            };
            let exec_attrs = match &item.exec {
                PaletteExec::OpenWs(a) => {
                    format!(r#"data-exec-kind="open-ws" data-ws="{}""#, a.as_str())
                }
                PaletteExec::JumpRoute(a, r) => format!(
                    r#"data-exec-kind="jump-route" data-ws="{}" data-route="{}""#,
                    a.as_str(),
                    esc(r)
                ),
                PaletteExec::SetView(v) => {
                    format!(r#"data-exec-kind="set-view" data-view="{v}""#)
                }
                PaletteExec::StartAll => r#"data-exec-kind="start-all""#.to_string(),
                PaletteExec::VaultBackup => r#"data-exec-kind="vault-backup""#.to_string(),
                PaletteExec::ToggleTheme => r#"data-exec-kind="toggle-theme""#.to_string(),
                PaletteExec::ToggleLang => r#"data-exec-kind="toggle-lang""#.to_string(),
            };
            let tile_class = if item.app.is_some() {
                "app-tile app-tile-sm"
            } else {
                "cmd-ico"
            };
            let kbd_html = match &item.kbd {
                Some(k) => format!(r#"<kbd class="kbd">{}</kbd>"#, esc(k)),
                None => String::new(),
            };

            let _ = write!(
                list_html,
                r#"<button type="button" class="palette-item"{active_id}{app_attr} role="option" aria-selected="{}" data-action="palette-exec" data-palette-idx="{idx}" {exec_attrs}><span class="{tile_class}">{}</span><span class="palette-item-main"><bdi class="palette-item-title">{}</bdi><span class="palette-item-sub">{}</span></span>{kbd_html}</button>"#,
                if is_active { "true" } else { "false" },
                item.icon_html,
                esc(&item.label),
                esc(&item.sub),
            );
        }
    }

    format!(
        r#"<div class="palette-scrim" data-action="close-palette"><div class="palette-dialog" role="dialog" aria-modal="true" aria-label="Command Palette"><div class="palette-search-row"><span class="palette-search-ico">{search_ico}</span><input id="palette-input" class="palette-input" type="text" autocomplete="off" spellcheck="false" value="{q_val}" placeholder="{placeholder}" aria-label="{placeholder}" /><kbd class="kbd">ESC</kbd></div><div class="palette-list" role="listbox" data-palette-count="{count}">{list_html}</div></div></div>"#,
        search_ico = icons::search(),
        q_val = esc(query),
        placeholder = esc(c.palette_placeholder),
        count = filtered.len(),
        list_html = list_html,
    )
}
