//! Themes, the text editor, and settings shared with the terminal app.
//!
//! Themes are canopy-config palettes: the built-ins plus JSON files in
//! `~/.canopy/themes/`, which the terminal app reads too.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use canopy_config::settings::Shared;
use canopy_config::theme::{self, Palette, ThemeFile};
use serde::Serialize;
use tauri_plugin_dialog::DialogExt;

use crate::settings::{load, Settings};
use crate::Res;

#[derive(Debug, Clone, Serialize)]
pub struct ThemeInfo {
    pub name: String,
    pub builtin: bool,
    pub dark: bool,
    /// Every color, as "#rrggbb".
    pub colors: BTreeMap<String, String>,
    /// The base it starts from (custom themes).
    pub base: String,
    /// Where a custom theme is saved.
    pub path: Option<String>,
}

fn info(p: &Palette, builtin: bool, base: &str, path: Option<&Path>) -> ThemeInfo {
    ThemeInfo {
        name: p.name.clone(),
        builtin,
        dark: p.is_dark(),
        colors: p.colors.iter().map(|(k, v)| (k.clone(), theme::to_hex(*v))).collect(),
        base: base.to_string(),
        path: path.map(|p| p.display().to_string()),
    }
}

fn dir() -> Res<PathBuf> {
    theme::themes_dir().ok_or_else(|| "couldn't find the config folder".to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct Themes {
    pub themes: Vec<ThemeInfo>,
    pub fields: Vec<(&'static str, &'static str)>,
    pub folder: String,
}

#[tauri::command]
pub fn list_themes() -> Res<Themes> {
    let d = dir()?;
    let mut themes: Vec<ThemeInfo> = theme::builtin_names()
        .into_iter()
        .filter_map(Palette::builtin)
        .map(|p| info(&p, true, &p.name.clone(), None))
        .collect();
    for (path, t) in theme::list_custom(&d) {
        if let Ok(p) = t.resolve() {
            themes.push(info(&p, false, &t.base, Some(&path)));
        }
    }
    Ok(Themes {
        themes,
        fields: theme::FIELDS.iter().map(|f| (*f, theme::describe(f))).collect(),
        folder: d.display().to_string(),
    })
}

/// Save a theme into the themes folder (both apps pick it up).
#[tauri::command]
pub fn save_theme(theme: ThemeFile) -> Res<String> {
    theme.resolve()?;
    if theme::builtin_names().iter().any(|b| b.eq_ignore_ascii_case(theme.name.trim())) {
        return Err(format!("{:?} is a built-in theme; give yours another name", theme.name));
    }
    theme::save(&dir()?, &theme).map(|p| p.display().to_string()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_theme(name: String) -> Res<()> {
    let d = dir()?;
    let (path, _) = theme::list_custom(&d).into_iter().find(|(_, t)| t.name == name).ok_or("no such custom theme")?;
    std::fs::remove_file(path).map_err(|e| e.to_string())
}

async fn pick(app: &tauri::AppHandle, title: &str) -> Option<PathBuf> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title(title).add_filter("JSON", &["json"]).pick_file(move |p| {
        let _ = tx.send(p);
    });
    rx.await.ok().flatten()?.into_path().ok()
}

async fn save_as(app: &tauri::AppHandle, title: &str, name: &str) -> Option<PathBuf> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title(title).set_file_name(name).add_filter("JSON", &["json"]).save_file(move |p| {
        let _ = tx.send(p);
    });
    rx.await.ok().flatten()?.into_path().ok()
}

/// Import a theme file (from the file picker) into the themes folder.
#[tauri::command]
pub async fn import_theme(app: tauri::AppHandle) -> Res<Option<String>> {
    let Some(path) = pick(&app, "Import a Canopy theme").await else { return Ok(None) };
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let t = ThemeFile::parse(&text)?;
    theme::save(&dir()?, &t).map_err(|e| e.to_string())?;
    Ok(Some(t.name))
}

/// Export a theme (built-in or custom) to a file the user picks.
#[tauri::command]
pub async fn export_theme(name: String, app: tauri::AppHandle) -> Res<Option<String>> {
    let d = dir()?;
    let p = theme::find(&name, Some(&d)).ok_or("no such theme")?;
    let base =
        theme::list_custom(&d).into_iter().find(|(_, t)| t.name == name).map(|(_, t)| t.base).unwrap_or(name.clone());
    let file = p.to_file(&p.name, &base);
    let Some(path) = save_as(&app, "Export theme", &format!("{}.json", theme::slug(&name))).await else {
        return Ok(None);
    };
    std::fs::write(&path, file.to_json()).map_err(|e| e.to_string())?;
    Ok(Some(path.display().to_string()))
}

// ------------------------------------------------------------------ editor

#[derive(Debug, Clone, Serialize)]
pub struct Editor {
    pub id: &'static str,
    pub name: &'static str,
    /// The command Canopy runs, with the file added at the end.
    pub command: &'static str,
    /// Runs inside a terminal (opens a new terminal window).
    pub terminal: bool,
}

const EDITORS: [Editor; 9] = [
    Editor { id: "code", name: "Visual Studio Code", command: "code", terminal: false },
    Editor { id: "cursor", name: "Cursor", command: "cursor", terminal: false },
    Editor { id: "zed", name: "Zed", command: "zed", terminal: false },
    Editor { id: "subl", name: "Sublime Text", command: "subl", terminal: false },
    Editor { id: "nvim", name: "Neovim", command: "nvim", terminal: true },
    Editor { id: "vim", name: "Vim", command: "vim", terminal: true },
    Editor { id: "hx", name: "Helix", command: "hx", terminal: true },
    Editor { id: "nano", name: "nano", command: "nano", terminal: true },
    Editor { id: "emacs", name: "Emacs", command: "emacs", terminal: false },
];

/// Where `program` is. Apps opened from the Finder or a desktop menu get a
/// short PATH, so the usual install folders and the editors' own app bundles
/// are checked too.
fn locate(program: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut dirs: Vec<PathBuf> =
        std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/snap/bin"].map(PathBuf::from));
    if let Some(h) = &home {
        dirs.push(h.join(".local/bin"));
        dirs.push(h.join(".cargo/bin"));
    }
    let names = if cfg!(windows) {
        vec![format!("{program}.exe"), format!("{program}.cmd"), program.to_string()]
    } else {
        vec![program.to_string()]
    };
    if let Some(p) = dirs.iter().flat_map(|d| names.iter().map(move |n| d.join(n))).find(|p| p.is_file()) {
        return Some(p);
    }
    if cfg!(target_os = "macos") {
        let bundled = match program {
            "code" => "Visual Studio Code.app/Contents/Resources/app/bin/code",
            "cursor" => "Cursor.app/Contents/Resources/app/bin/cursor",
            "zed" => "Zed.app/Contents/MacOS/cli",
            "subl" => "Sublime Text.app/Contents/SharedSupport/bin/subl",
            _ => return None,
        };
        let apps = [Some(PathBuf::from("/Applications")), home.map(|h| h.join("Applications"))];
        return apps.into_iter().flatten().map(|a| a.join(bundled)).find(|p| p.is_file());
    }
    None
}

/// Editors found on this computer.
#[tauri::command]
pub fn detect_editors() -> Vec<Editor> {
    EDITORS.iter().filter(|e| locate(e.command).is_some()).cloned().collect()
}

/// Open `file` in the editor from Settings: "system" (the default app), an
/// editor id, or any command line.
pub fn open_with_editor(file: &Path, setting: &str) -> Res<String> {
    let setting = setting.trim();
    if setting.is_empty() || setting == "system" {
        let mut cmd = if cfg!(target_os = "macos") {
            let mut c = std::process::Command::new("open");
            c.arg("-t");
            c
        } else if cfg!(windows) {
            let mut c = std::process::Command::new("cmd");
            c.args(["/C", "start", "", "notepad"]);
            c
        } else {
            std::process::Command::new("xdg-open")
        };
        cmd.arg(file).spawn().map_err(|e| format!("Couldn't open an editor: {e}"))?;
        return Ok("your default editor".into());
    }
    let known = EDITORS.iter().find(|e| e.id == setting);
    let (command, terminal, name) = match known {
        Some(e) => (e.command.to_string(), e.terminal, e.name.to_string()),
        None => {
            let prog = canopy_gh::assist::split_args(setting).first().cloned().unwrap_or_default();
            let terminal = ["vi", "vim", "nvim", "nano", "hx", "helix", "micro", "emacs -nw"]
                .iter()
                .any(|t| setting == *t || prog == *t);
            (setting.to_string(), terminal, prog)
        }
    };
    let mut argv = canopy_gh::assist::split_args(&command);
    if argv.is_empty() {
        return Err("The editor command is empty".into());
    }
    argv.push(file.display().to_string());
    if terminal {
        let dir = file.parent().unwrap_or(Path::new("."));
        match canopy_gh::assist::launch(dir, &argv, canopy_gh::assist::OpenIn::Window, false) {
            canopy_gh::assist::Launched::Opened(w) if !w.starts_with("nothing") => Ok(format!("{name} in {w}")),
            _ => Err("Couldn't open a terminal for the editor".into()),
        }
    } else {
        // The full path, since this app's PATH may not have it.
        let program = locate(&argv[0]).unwrap_or_else(|| PathBuf::from(&argv[0]));
        std::process::Command::new(program)
            .args(&argv[1..])
            .spawn()
            .map(|_| name)
            .map_err(|e| format!("Couldn't run {}: {e}. Is it installed (on your PATH)?", argv[0]))
    }
}

/// Open a file (relative to the open repository, or a theme file) in the
/// editor from Settings.
#[tauri::command]
pub async fn open_in_editor(path: String, state: tauri::State<'_, crate::AppState>) -> Res<String> {
    let p = PathBuf::from(&path);
    let full = if p.is_absolute() { p } else { crate::current(&state).await?.repo.root.join(p) };
    let setting = load().editor;
    tauri::async_runtime::spawn_blocking(move || open_with_editor(&full, &setting)).await.map_err(|e| e.to_string())?
}

// ------------------------------------------------------------------ shared settings

/// Theme names differ a little: the desktop's "system" / "dark" are the
/// terminal app's "canopy".
fn theme_for_terminal(desktop: &str) -> String {
    match desktop {
        "system" | "dark" | "" => "canopy".into(),
        other => other.into(),
    }
}

fn theme_for_desktop(terminal: &str) -> String {
    match terminal {
        "canopy" => "dark".into(),
        other => other.into(),
    }
}

fn shared_from(s: &Settings) -> Shared {
    let name = theme_for_terminal(&s.theme);
    let theme_file = (!theme::builtin_names().contains(&name.as_str()))
        .then(|| theme::themes_dir().and_then(|d| theme::custom_file(&name, &d)))
        .flatten();
    Shared {
        kind: canopy_config::settings::KIND.into(),
        theme: Some(name),
        theme_file,
        editor: (s.editor != "system").then(|| editor_command(&s.editor)),
        ai: Some(s.ai_assistant.clone()),
        show_commands: Some(s.show_commands),
    }
}

/// An editor setting as a command line (the terminal app wants a command).
fn editor_command(setting: &str) -> String {
    let base = EDITORS.iter().find(|e| e.id == setting).map(|e| e.command).unwrap_or(setting);
    // GUI editors need to wait, so the terminal app knows when you're done.
    match base {
        "code" | "cursor" | "zed" | "subl" => format!("{base} --wait"),
        other => other.to_string(),
    }
}

fn apply(shared: &Shared) -> Res<Settings> {
    if let (Some(t), Ok(d)) = (&shared.theme_file, dir()) {
        theme::save(&d, t).map_err(|e| e.to_string())?;
    }
    let mut s = load();
    if let Some(t) = &shared.theme {
        s.theme = theme_for_desktop(t);
    }
    if let Some(e) = &shared.editor {
        let cmd = e.trim().trim_end_matches(" --wait").trim_end_matches(" -w");
        s.editor = EDITORS.iter().find(|x| x.command == cmd).map(|x| x.id.to_string()).unwrap_or_else(|| e.clone());
    }
    if let Some(a) = &shared.ai {
        s.ai_assistant = a.clone();
    }
    if let Some(b) = shared.show_commands {
        s.show_commands = b;
    }
    crate::settings::set_settings(s)
}

/// Where the terminal app reads its config (same rules as the app).
fn terminal_config() -> Option<PathBuf> {
    match std::env::var_os("CANOPY_CONFIG") {
        Some(p) => Some(PathBuf::from(p)),
        None => canopy_config::config_dir().map(|d| d.join("config.toml")),
    }
}

/// Take the terminal app's settings (from its config.toml).
#[tauri::command]
pub fn import_from_terminal() -> Res<Settings> {
    let path = terminal_config().ok_or("couldn't find the config folder")?;
    let text = std::fs::read_to_string(&path)
        .map_err(|_| format!("The terminal app has no config yet ({}).", path.display()))?;
    apply(&Shared::from_toml(&text)?)
}

/// Write these settings into the terminal app's config.toml (only those
/// keys change).
#[tauri::command]
pub fn send_to_terminal() -> Res<String> {
    let path = terminal_config().ok_or("couldn't find the config folder")?;
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let shared = shared_from(&load());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, shared.apply_to_toml(&text)).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub async fn export_settings(app: tauri::AppHandle) -> Res<Option<String>> {
    let Some(path) = save_as(&app, "Export settings", "canopy-settings.json").await else { return Ok(None) };
    std::fs::write(&path, shared_from(&load()).to_json()).map_err(|e| e.to_string())?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
pub async fn import_settings(app: tauri::AppHandle) -> Res<Option<Settings>> {
    let Some(path) = pick(&app, "Import settings").await else { return Ok(None) };
    let shared = canopy_config::settings::read(&path)?;
    apply(&shared).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_translate_between_apps() {
        assert_eq!(theme_for_terminal("system"), "canopy");
        assert_eq!(theme_for_terminal("dark"), "canopy");
        assert_eq!(theme_for_terminal("nord"), "nord");
        assert_eq!(theme_for_desktop("canopy"), "dark");
        assert_eq!(theme_for_desktop("Ocean"), "Ocean");
        assert_eq!(editor_command("code"), "code --wait");
        assert_eq!(editor_command("nvim"), "nvim");
        assert_eq!(editor_command("my-editor -x"), "my-editor -x");
    }
}
