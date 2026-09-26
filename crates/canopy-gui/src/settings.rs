//! Settings (kept in desktop.json next to the recent repos), update checks,
//! and what the app found on this machine.

use serde::{Deserialize, Serialize};

use crate::recent;
use crate::Res;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// system, light or dark.
    pub theme: String,
    /// Show the git command after each action (teach mode).
    pub show_commands: bool,
    /// Seconds between automatic refreshes; 0 turns them off.
    pub refresh_secs: u32,
    /// What Pull does: default (your git config), merge or rebase.
    pub pull_mode: String,
    /// Look for a new version once a day.
    pub check_updates: bool,
    /// Unix time of the last update check.
    pub last_update_check: i64,
    /// "Fix with AI": auto (the first installed), claude, codex, off, or a
    /// command line (`{prompt}` marks where the prompt goes).
    pub ai_assistant: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "system".into(),
            show_commands: true,
            refresh_secs: 5,
            pull_mode: "default".into(),
            check_updates: true,
            last_update_check: 0,
            ai_assistant: "auto".into(),
        }
    }
}

impl Settings {
    /// Unknown values fall back to the defaults.
    fn sanitized(mut self) -> Self {
        let d = Settings::default();
        if !["system", "light", "dark"].contains(&self.theme.as_str()) {
            self.theme = d.theme;
        }
        if !["default", "merge", "rebase"].contains(&self.pull_mode.as_str()) {
            self.pull_mode = d.pull_mode;
        }
        self.refresh_secs = self.refresh_secs.min(3600);
        if self.ai_assistant.trim().is_empty() {
            self.ai_assistant = "auto".into();
        }
        self
    }
}

pub fn load() -> Settings {
    let Some(file) = recent::store_path() else { return Settings::default() };
    serde_json::from_value::<Settings>(recent::read_settings(&file)).unwrap_or_default().sanitized()
}

#[tauri::command]
pub fn get_settings() -> Settings {
    load()
}

#[tauri::command]
pub fn set_settings(settings: Settings) -> Res<Settings> {
    let settings = settings.sanitized();
    let file = recent::store_path().ok_or("no home folder to save settings in")?;
    let value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
    recent::write_settings(&file, value).map_err(|e| e.to_string())?;
    Ok(settings)
}

#[tauri::command]
pub fn clear_recent() -> Res<()> {
    let Some(file) = recent::store_path() else { return Ok(()) };
    recent::clear(&file).map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ updates

/// How this copy was installed, which decides how to update it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Install {
    Homebrew,
    Winget,
    AppImage,
    Deb,
    /// Built from source or copied by hand.
    Manual,
}

pub fn detect_install() -> Install {
    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
    if cfg!(target_os = "macos") {
        let cask = ["/opt/homebrew/Caskroom/canopy-desktop", "/usr/local/Caskroom/canopy-desktop"];
        if cask.iter().any(|p| std::path::Path::new(p).exists()) {
            return Install::Homebrew;
        }
    } else if cfg!(windows) {
        if exe.contains("WinGet") {
            return Install::Winget;
        }
    } else if std::env::var_os("APPIMAGE").is_some() {
        return Install::AppImage;
    } else if exe.starts_with("/usr/bin/") {
        return Install::Deb;
    }
    Install::Manual
}

/// On Windows the per-user installer puts the app in the same place whether
/// you ran it or winget did, so ask winget.
async fn refine_install(install: Install) -> Install {
    if !cfg!(windows) || install != Install::Manual {
        return install;
    }
    let out = tokio::process::Command::new("winget")
        .args(["list", "--id", "shankars.canopy-desktop", "--exact", "--accept-source-agreements"])
        .stdin(std::process::Stdio::null())
        .output()
        .await;
    match out {
        Ok(o) if o.status.success() && String::from_utf8_lossy(&o.stdout).contains("shankars.canopy-desktop") => {
            Install::Winget
        }
        _ => install,
    }
}

/// The command that updates this install, if there is one.
pub fn update_command(install: Install) -> Option<&'static str> {
    match install {
        Install::Homebrew => Some("brew upgrade --cask canopy-desktop"),
        Install::Winget => Some("winget upgrade shankars.canopy-desktop"),
        _ => None,
    }
}

/// `1.2.3` (or `v1.2.3`) as numbers; None if it isn't a version.
pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v');
    let core = v.split(['-', '+']).next()?;
    let mut it = core.split('.').map(|p| p.parse::<u64>().ok());
    Some((it.next()??, it.next().unwrap_or(Some(0))?, it.next().unwrap_or(Some(0))?))
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: Option<String>,
    pub newer: bool,
    pub url: String,
    pub install: Install,
    pub command: Option<String>,
    /// Why the check didn't work, if it didn't.
    pub error: Option<String>,
}

const RELEASES: &str = "https://github.com/shankar-sachin/canopy/releases";

/// Ask GitHub for the latest release (with curl, which macOS, Windows 10+
/// and nearly every Linux have; no extra HTTP library).
#[tauri::command]
pub async fn check_update() -> UpdateInfo {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let install = refine_install(detect_install()).await;
    let mut info = UpdateInfo {
        current: current.clone(),
        latest: None,
        newer: false,
        url: format!("{RELEASES}/latest"),
        install,
        command: update_command(install).map(String::from),
        error: None,
    };
    let out = tokio::process::Command::new("curl")
        .args(["-fsSL", "--max-time", "10", "-H", "User-Agent: canopy-desktop"])
        .arg("https://api.github.com/repos/shankar-sachin/canopy/releases/latest")
        .stdin(std::process::Stdio::null())
        .output()
        .await;
    let body = match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        Ok(o) => {
            info.error = Some(format!("GitHub didn't answer ({})", String::from_utf8_lossy(&o.stderr).trim()));
            return info;
        }
        Err(e) => {
            info.error = Some(format!("couldn't run curl: {e}"));
            return info;
        }
    };
    let tag =
        serde_json::from_str::<serde_json::Value>(&body).ok().and_then(|v| v["tag_name"].as_str().map(String::from));
    match tag {
        Some(tag) => {
            info.newer = matches!((parse_version(&tag), parse_version(&current)), (Some(l), Some(c)) if l > c);
            info.url = format!("{RELEASES}/tag/{tag}");
            info.latest = Some(tag.trim_start_matches('v').to_string());
        }
        None => info.error = Some("couldn't read the latest release".into()),
    }
    if let Some(file) = recent::store_path() {
        let mut s = load();
        s.last_update_check =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        if let Ok(v) = serde_json::to_value(&s) {
            let _ = recent::write_settings(&file, v);
        }
    }
    info
}

// ------------------------------------------------------------------ environment

#[derive(Debug, Clone, Serialize)]
pub struct Environment {
    pub version: String,
    pub git: Option<String>,
    pub gh: Option<String>,
    /// The GitHub account gh is logged in as.
    pub gh_user: Option<String>,
    pub install: Install,
    pub config_file: Option<String>,
}

async fn first_line(program: &str, args: &[&str]) -> Option<String> {
    let o = tokio::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .ok()
        .filter(|o| o.status.success())?;
    String::from_utf8_lossy(&o.stdout).lines().next().map(|l| l.trim().to_string())
}

#[tauri::command]
pub async fn environment() -> Environment {
    let (git, gh, gh_user) = tokio::join!(
        first_line("git", &["--version"]),
        first_line("gh", &["--version"]),
        first_line("gh", &["api", "user", "--jq", ".login"]),
    );
    Environment {
        version: env!("CARGO_PKG_VERSION").into(),
        git,
        gh,
        gh_user,
        install: refine_install(detect_install()).await,
        config_file: recent::store_path().map(|p| p.display().to_string()),
    }
}

// ------------------------------------------------------------------ profile

/// Who's using Canopy: the GitHub account (from gh) and the git name.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Profile {
    pub login: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    /// `git config user.name`, for the placeholder's initials.
    pub git_name: Option<String>,
}

#[tauri::command]
pub async fn profile() -> Profile {
    let (gh, git_name) = tokio::join!(
        first_line("gh", &["api", "user", "--jq", "[.login, (.name // \"\"), .avatar_url] | @tsv"]),
        first_line("git", &["config", "--global", "user.name"]),
    );
    let mut p = Profile { git_name: git_name.filter(|n| !n.is_empty()), ..Default::default() };
    if let Some(line) = gh {
        let mut parts = line.split('\t').map(|s| s.trim().to_string()).map(|s| Some(s).filter(|s| !s.is_empty()));
        p.login = parts.next().flatten();
        p.name = parts.next().flatten();
        // Only GitHub's own avatar host (the page's CSP allows just that).
        p.avatar_url = parts.next().flatten().filter(|u| u.starts_with("https://avatars.githubusercontent.com/"));
    }
    p
}

// ------------------------------------------------------------------ account

/// The GitHub connection, from `gh auth status` (the token itself is never
/// read or shown).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct GhAuth {
    pub host: Option<String>,
    pub account: Option<String>,
    /// Where gh keeps the token: keyring, file, GH_TOKEN, ...
    pub storage: Option<String>,
    /// ssh or https: how git talks to GitHub.
    pub protocol: Option<String>,
    pub scopes: Vec<String>,
}

pub fn parse_auth_status(text: &str) -> GhAuth {
    let mut a = GhAuth::default();
    for line in text.lines() {
        let l = line.trim();
        if !line.starts_with(' ') && !l.is_empty() && a.host.is_none() {
            a.host = Some(l.to_string());
        } else if let Some(rest) = l.split_once("Logged in to ").map(|x| x.1) {
            // "github.com account shankar-sachin (keyring)"
            if let Some((_, acct)) = rest.split_once(" account ") {
                let mut it = acct.splitn(2, ' ');
                a.account = it.next().map(String::from);
                a.storage = it.next().map(|s| s.trim_matches(|c| c == '(' || c == ')').to_string());
            }
        } else if let Some(p) = l.strip_prefix("- Git operations protocol: ") {
            a.protocol = Some(p.to_string());
        } else if let Some(sc) = l.strip_prefix("- Token scopes: ") {
            a.scopes =
                sc.split(',').map(|s| s.trim().trim_matches('\'').to_string()).filter(|s| !s.is_empty()).collect();
        }
    }
    a
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Account {
    #[serde(flatten)]
    pub profile: Profile,
    pub auth: GhAuth,
    pub html_url: Option<String>,
    pub git_email: Option<String>,
    pub git: Option<String>,
    pub gh: Option<String>,
}

#[tauri::command]
pub async fn account() -> Account {
    let status = async {
        let o = tokio::process::Command::new("gh")
            .args(["auth", "status", "--active"])
            .stdin(std::process::Stdio::null())
            .output()
            .await
            .ok()?;
        // gh prints the status on stderr in older versions, stdout in newer.
        let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&o.stderr));
        Some(parse_auth_status(&text))
    };
    let (profile, auth, url, email, git, gh) = tokio::join!(
        profile(),
        status,
        first_line("gh", &["api", "user", "--jq", ".html_url"]),
        first_line("git", &["config", "--global", "user.email"]),
        first_line("git", &["--version"]),
        first_line("gh", &["--version"]),
    );
    Account {
        profile,
        auth: auth.unwrap_or_default(),
        html_url: url.filter(|u| u.starts_with("https://github.com/")),
        git_email: email.filter(|e| !e.is_empty()),
        git,
        gh,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_status() {
        let text = "github.com\n  ✓ Logged in to github.com account ada (keyring)\n  - Active account: true\n  \
                    - Git operations protocol: ssh\n  - Token: gho_************************************\n  \
                    - Token scopes: 'gist', 'read:org', 'repo', 'workflow'\n";
        let a = parse_auth_status(text);
        assert_eq!(a.host.as_deref(), Some("github.com"));
        assert_eq!(a.account.as_deref(), Some("ada"));
        assert_eq!(a.storage.as_deref(), Some("keyring"));
        assert_eq!(a.protocol.as_deref(), Some("ssh"));
        assert_eq!(a.scopes, ["gist", "read:org", "repo", "workflow"]);
        assert_eq!(
            parse_auth_status("You are not logged into any GitHub hosts."),
            GhAuth { host: Some("You are not logged into any GitHub hosts.".into()), ..Default::default() }
        );
    }

    #[test]
    fn versions() {
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.10"), Some((1, 10, 0)));
        assert_eq!(parse_version("2.0.0-beta.1"), Some((2, 0, 0)));
        assert_eq!(parse_version("nightly"), None);
        assert!(parse_version("1.10.0") > parse_version("1.9.9"));
    }

    #[test]
    fn settings_fall_back_to_defaults() {
        let s: Settings =
            serde_json::from_str(r#"{"theme":"neon","refresh_secs":99999,"show_commands":false}"#).unwrap();
        let s = s.sanitized();
        assert_eq!(s.theme, "system");
        assert_eq!(s.refresh_secs, 3600);
        assert!(!s.show_commands);
        assert_eq!(s.pull_mode, "default");
        // Missing file or junk: defaults.
        assert_eq!(
            serde_json::from_value::<Settings>(serde_json::Value::Null).unwrap_or_default(),
            Settings::default()
        );
    }

    #[test]
    fn settings_and_recents_share_the_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let file = dir.path().join("desktop.json");
        recent::add(&file, "/a").unwrap();
        recent::write_settings(&file, serde_json::json!({"theme": "dark"})).unwrap();
        recent::add(&file, "/b").unwrap();
        assert_eq!(recent::read_settings(&file)["theme"], "dark");
        assert_eq!(recent::list(&file).len(), 2);
        recent::clear(&file).unwrap();
        assert!(recent::list(&file).is_empty());
        assert_eq!(recent::read_settings(&file)["theme"], "dark");
    }

    #[test]
    fn update_commands() {
        assert_eq!(update_command(Install::Homebrew), Some("brew upgrade --cask canopy-desktop"));
        assert_eq!(update_command(Install::Winget), Some("winget upgrade shankars.canopy-desktop"));
        assert_eq!(update_command(Install::Manual), None);
    }
}
