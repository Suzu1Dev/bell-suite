use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppId {
    Personal,
    Explain,
    Pr,
    Reader,
    Campaign,
}

impl AppId {
    pub const ALL: [AppId; 5] = [
        AppId::Personal,
        AppId::Explain,
        AppId::Pr,
        AppId::Reader,
        AppId::Campaign,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            AppId::Personal => "personal",
            AppId::Explain => "explain",
            AppId::Pr => "pr",
            AppId::Reader => "reader",
            AppId::Campaign => "campaign",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            AppId::Personal => "BELL Personal",
            AppId::Explain => "BELL Explain",
            AppId::Pr => "BELL PR",
            AppId::Reader => "BELL Web Reader",
            AppId::Campaign => "BELL Campaign",
        }
    }

    pub const fn bundle_id(self) -> &'static str {
        match self {
            AppId::Personal => "com.bell1dev.personal",
            AppId::Explain => "com.bell1dev.explain",
            AppId::Pr => "com.bell1dev.pr",
            AppId::Reader => "com.bell1dev.reader",
            AppId::Campaign => "com.bell1dev.campaign",
        }
    }

    pub const fn shortcut_index(self) -> u8 {
        match self {
            AppId::Personal => 1,
            AppId::Explain => 2,
            AppId::Pr => 3,
            AppId::Reader => 4,
            AppId::Campaign => 5,
        }
    }

    pub const fn port_offset(self) -> u16 {
        self.shortcut_index() as u16
    }

    pub const fn default_port(self) -> u16 {
        4600 + self.port_offset()
    }

    pub const fn accent_light(self) -> &'static str {
        match self {
            AppId::Personal => "#a4532c",
            AppId::Explain => "#5b45e0",
            AppId::Pr => "#1b6b4c",
            AppId::Reader => "#0b6bcb",
            AppId::Campaign => "#9e2b5c",
        }
    }

    pub const fn accent_dark(self) -> &'static str {
        match self {
            AppId::Personal => "#e08a5c",
            AppId::Explain => "#a799ff",
            AppId::Pr => "#46c289",
            AppId::Reader => "#38aef8",
            AppId::Campaign => "#e25b91",
        }
    }
}

impl fmt::Display for AppId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for AppId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "personal" => Ok(AppId::Personal),
            "explain" => Ok(AppId::Explain),
            "pr" => Ok(AppId::Pr),
            "reader" => Ok(AppId::Reader),
            "campaign" => Ok(AppId::Campaign),
            other => Err(format!("unknown appId '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edition {
    Suite,
    Personal,
    Explain,
    Pr,
    Reader,
    Campaign,
}

impl Edition {
    pub const fn as_str(self) -> &'static str {
        match self {
            Edition::Suite => "suite",
            Edition::Personal => "personal",
            Edition::Explain => "explain",
            Edition::Pr => "pr",
            Edition::Reader => "reader",
            Edition::Campaign => "campaign",
        }
    }

    pub const fn primary_app(self) -> Option<AppId> {
        match self {
            Edition::Suite => None,
            Edition::Personal => Some(AppId::Personal),
            Edition::Explain => Some(AppId::Explain),
            Edition::Pr => Some(AppId::Pr),
            Edition::Reader => Some(AppId::Reader),
            Edition::Campaign => Some(AppId::Campaign),
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self.primary_app() {
            None => "BELL Suite",
            Some(app) => app.display_name(),
        }
    }

    pub const fn bundle_id(self) -> &'static str {
        match self.primary_app() {
            None => "com.bell1dev.suite",
            Some(app) => app.bundle_id(),
        }
    }

    pub const fn shortcut_index(self) -> u8 {
        match self.primary_app() {
            None => 0,
            Some(app) => app.shortcut_index(),
        }
    }

    pub const fn default_port(self) -> u16 {
        match self.primary_app() {
            None => 4600,
            Some(app) => app.default_port(),
        }
    }

    pub const fn accent_light(self) -> &'static str {
        match self.primary_app() {
            None => "#0f8a7d",
            Some(app) => app.accent_light(),
        }
    }

    pub const fn accent_dark(self) -> &'static str {
        match self.primary_app() {
            None => "#43cabc",
            Some(app) => app.accent_dark(),
        }
    }

    pub fn active() -> Self {
        if let Ok(raw) = std::env::var("BELL_DESKTOP_EDITION") {
            if let Ok(ed) = raw.parse::<Edition>() {
                return ed;
            }
        }
        if let Some(raw) = option_env!("BELL_DESKTOP_EDITION") {
            if let Ok(ed) = raw.parse::<Edition>() {
                return ed;
            }
        }
        Edition::Suite
    }
}

impl fmt::Display for Edition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Edition {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "suite" => Ok(Edition::Suite),
            "personal" => Ok(Edition::Personal),
            "explain" => Ok(Edition::Explain),
            "pr" => Ok(Edition::Pr),
            "reader" => Ok(Edition::Reader),
            "campaign" => Ok(Edition::Campaign),
            other => Err(format!("unknown edition '{other}'")),
        }
    }
}

pub fn is_valid_target(target: &str) -> bool {
    target == "hub" || target.parse::<AppId>().is_ok()
}
