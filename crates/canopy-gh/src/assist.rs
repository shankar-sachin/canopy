//! "Fix with AI": when a CI run fails, write a prompt describing the failure
//! and open an AI coding assistant (Claude Code, Codex, or your own command)
//! in a new terminal tab or window, with the prompt already sent.
//!
//! The prompt goes in `.git/canopy/fix-ci.md` (inside `.git`, so it's never
//! committed); the assistant is started with a short message pointing at it.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::{Job, Run};

/// Which assistant to open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Assistant {
    Claude,
    Codex,
    /// A command line; `{prompt}` is replaced by the prompt (quoted), or the
    /// prompt is added at the end.
    Custom(String),
}

impl Assistant {
    /// From a setting: "claude", "codex", or a custom command. None = off.
    pub fn from_setting(s: &str) -> Option<Assistant> {
        match s.trim() {
            // "custom" is the setting before a command has been typed.
            "" | "off" | "none" | "custom" | "auto" => None,
            "claude" => Some(Assistant::Claude),
            "codex" => Some(Assistant::Codex),
            other => Some(Assistant::Custom(other.to_string())),
        }
    }

    pub fn name(&self) -> String {
        match self {
            Assistant::Claude => "Claude Code".into(),
            Assistant::Codex => "Codex".into(),
            Assistant::Custom(c) => c.split_whitespace().next().unwrap_or("your assistant").to_string(),
        }
    }

    /// The program and arguments to run, with `prompt` as its first message.
    pub fn argv(&self, prompt: &str) -> Vec<String> {
        match self {
            Assistant::Claude => vec!["claude".into(), prompt.into()],
            Assistant::Codex => vec!["codex".into(), prompt.into()],
            Assistant::Custom(cmd) => {
                let mut parts = split_args(cmd);
                if let Some(i) = parts.iter().position(|p| p.contains("{prompt}")) {
                    parts[i] = parts[i].replace("{prompt}", prompt);
                } else {
                    parts.push(prompt.into());
                }
                parts
            }
        }
    }
}

/// Which of the known assistants are installed (on PATH).
pub fn installed() -> Vec<Assistant> {
    let found = |p: &str| {
        Command::new(p)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    [Assistant::Claude, Assistant::Codex].into_iter().filter(|a| found(&a.argv("")[0])).collect()
}

/// The first installed assistant, for a default setting of "auto".
pub fn pick(setting: &str) -> Option<Assistant> {
    if setting.trim() == "auto" {
        installed().into_iter().next()
    } else {
        Assistant::from_setting(setting)
    }
}

/// Split a command line on spaces, keeping "quoted parts" together.
pub fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for c in s.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(c),
            (None, c) if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            (_, c) => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Everything the prompt says about the failure.
#[derive(Debug, Clone, Default)]
pub struct Failure {
    pub repo: String,
    pub branch: String,
    pub run: Option<Run>,
    pub jobs: Vec<Job>,
    /// `gh run view` text: summary and annotations.
    pub summary: String,
    /// The failed steps' log.
    pub log: String,
}

impl Failure {
    /// Whether GitHub couldn't run the workflow at all (a broken workflow
    /// file): no jobs ran, or the run says so.
    pub fn workflow_invalid(&self) -> bool {
        let c = self.run.as_ref().map(|r| r.conclusion.as_str()).unwrap_or("");
        c == "startup_failure" || self.summary.contains("workflow file") || (self.jobs.is_empty() && c == "failure")
    }

    /// One line for a banner or toast.
    pub fn headline(&self) -> String {
        if self.workflow_invalid() {
            return "GitHub couldn't run the workflow: the workflow file has a problem.".into();
        }
        let failed = self.failed_steps();
        match failed.first() {
            Some((job, step)) => format!("CI failed in {job} → {step}."),
            None => "CI failed on your latest commit.".into(),
        }
    }

    fn failed_steps(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for j in &self.jobs {
            if j.conclusion == "failure" {
                let step =
                    j.steps.iter().find(|s| s.conclusion == "failure").map(|s| s.name.clone()).unwrap_or_default();
                out.push((j.name.clone(), step));
            }
        }
        out
    }

    /// The full prompt, as Markdown.
    pub fn prompt(&self) -> String {
        let mut p = String::new();
        let run = self.run.as_ref();
        p.push_str("# Fix a failing CI run\n\n");
        p.push_str(&format!(
            "canopy found an error in CI for the latest commit on branch `{}` of `{}`.\n\n",
            self.branch, self.repo
        ));
        if let Some(r) = run {
            let sha = r.head_sha.get(..7).unwrap_or(&r.head_sha);
            p.push_str(&format!(
                "- Workflow: **{}** (run #{}, {})\n- Commit: `{sha}` {}\n- Result: {}\n- Run: {}\n\n",
                r.workflow_name,
                r.number,
                r.event,
                r.display_title,
                if r.conclusion.is_empty() { &r.status } else { &r.conclusion },
                r.url
            ));
        }
        if self.workflow_invalid() {
            p.push_str(
                "GitHub couldn't run the workflow, which usually means a workflow file under `.github/workflows/` is invalid \
                 (YAML syntax, an unknown key, a bad expression). The details GitHub gave are below.\n\n",
            );
        }
        let failed = self.failed_steps();
        if !failed.is_empty() {
            p.push_str("## What failed\n\n");
            for (job, step) in &failed {
                p.push_str(&format!("- Job **{job}**, step **{step}**\n"));
            }
            p.push('\n');
        }
        if !self.summary.trim().is_empty() {
            p.push_str("## GitHub's summary\n\n```\n");
            p.push_str(self.summary.trim());
            p.push_str("\n```\n\n");
        }
        let log = clean_log(&self.log);
        if !log.trim().is_empty() {
            p.push_str("## End of the failed log\n\n```\n");
            p.push_str(&tail(log.trim(), 200));
            p.push_str("\n```\n\n");
        }
        p.push_str(
            "## What to do\n\n\
             1. Find the cause. Read the relevant files in this repository (and the workflow under `.github/workflows/` if the \
             failure is in CI setup rather than the code).\n\
             2. Fix it with the smallest change that makes sense.\n\
             3. Run the same checks locally when you can (tests, lint, build).\n\
             4. Explain what was wrong and what you changed.\n\
             5. Don't commit or push; I'll review the change first.\n",
        );
        p
    }
}

/// Collect what went wrong in `run`: jobs, GitHub's summary (which says why
/// a workflow file is invalid) and the failed steps' log.
pub async fn gather(gh: &crate::Gh, repo: &str, branch: &str, run: Run) -> Failure {
    let id = run.database_id;
    let (jobs, summary, log) = tokio::join!(gh.run_jobs(id), gh.run_summary(id), gh.run_failed_log(id));
    Failure {
        repo: repo.to_string(),
        branch: branch.to_string(),
        run: Some(run),
        jobs: jobs.unwrap_or_default(),
        summary: summary.unwrap_or_default(),
        log: log.unwrap_or_default(),
    }
}

/// `gh run view --log-failed` lines are `job<TAB>step<TAB>timestamp text`,
/// with terminal color codes. Keep just the text, and start a new section
/// when the step changes.
pub fn clean_log(log: &str) -> String {
    let mut out = String::new();
    let mut last_step = String::new();
    for line in log.lines() {
        let mut parts = line.splitn(3, '\t');
        let (_job, step, rest) = (parts.next(), parts.next(), parts.next());
        let (step, text) = match (step, rest) {
            (Some(step), Some(rest)) => {
                let rest = rest.trim_start_matches('\u{feff}');
                let text =
                    rest.split_once(' ').filter(|(t, _)| t.ends_with('Z') && t.contains('T')).map_or(rest, |(_, t)| t);
                (step.to_string(), text)
            }
            _ => (String::new(), line),
        };
        if !step.is_empty() && step != last_step {
            out.push_str(&format!("--- {step} ---\n"));
            last_step = step;
        }
        out.push_str(&strip_ansi(text));
        out.push('\n');
    }
    out
}

fn strip_ansi(s: &str) -> String {
    // gh writes the color codes out as text ("^[[1m"), not as escapes.
    let s = &strip_caret_codes(s);
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // ESC [ ... letter
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// Remove `^[[…m`-style color codes written as plain text.
fn strip_caret_codes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("^[[") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 3..];
        match after.find(|c: char| c.is_ascii_alphabetic()) {
            // A color code: digits and semicolons, then "m" (or any letter
            // after at least one digit).
            Some(j)
                if after[..j].chars().all(|c| c.is_ascii_digit() || c == ';')
                    && (j > 0 || after[j..].starts_with('m')) =>
            {
                rest = &after[j + 1..]
            }
            _ => {
                out.push_str("^[[");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// Save the prompt in `<git_dir>/canopy/fix-ci.md`; returns its path.
pub fn write_prompt(git_dir: &Path, prompt: &str) -> std::io::Result<PathBuf> {
    let dir = git_dir.join("canopy");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("fix-ci.md");
    std::fs::write(&path, prompt)?;
    Ok(path)
}

/// The short first message the assistant gets.
pub fn kickoff(prompt_file: &Path, repo_root: &Path) -> String {
    let shown = prompt_file.strip_prefix(repo_root).unwrap_or(prompt_file);
    format!(
        "canopy found an error in this repository's CI. The details are in {}. Read that file, then find and fix the problem as it describes. Don't commit or push.",
        shown.display()
    )
}

/// Where to open the assistant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenIn {
    /// A new tab in this terminal when it can make one, else a new window.
    Tab,
    /// A new terminal window.
    Window,
    /// Right here (the TUI steps aside while it runs).
    Here,
}

impl OpenIn {
    pub fn from_setting(s: &str) -> OpenIn {
        match s.trim() {
            "window" => OpenIn::Window,
            "here" => OpenIn::Here,
            _ => OpenIn::Tab,
        }
    }
}

/// What `launch` did, for the toast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launched {
    /// Opened in a new tab or window of this app.
    Opened(&'static str),
    /// Couldn't open a new tab or window: run `argv` here instead.
    RunHere(Vec<String>),
    /// macOS didn't let us control this terminal app (Automation permission).
    Blocked(&'static str),
}

/// What to do when macOS blocks canopy from controlling a terminal app.
pub fn blocked_help(app: &str) -> String {
    format!(
        "macOS didn't let canopy open {app}. Allow it in System Settings → Privacy & Security → Automation \
         (turn on {app} under canopy), then try again."
    )
}

/// How an AppleScript run went.
#[derive(Debug, PartialEq, Eq)]
enum Script {
    Ok,
    /// The user (or the system) denied the Automation permission.
    Denied,
    Failed,
}

/// Run AppleScript and wait for it (it returns as soon as the terminal has
/// the command), so a refusal isn't mistaken for success.
fn osascript(script: &str) -> Script {
    match Command::new("osascript").args(["-e", script]).stdin(Stdio::null()).output() {
        Ok(o) if o.status.success() => Script::Ok,
        Ok(o) => script_error(&String::from_utf8_lossy(&o.stderr)),
        Err(_) => Script::Failed,
    }
}

/// -1743 is errAEEventNotPermitted: Automation is off for this app pair.
fn script_error(stderr: &str) -> Script {
    if stderr.contains("-1743") || stderr.contains("Not authorized to send Apple events") {
        Script::Denied
    } else {
        Script::Failed
    }
}

fn sh_quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:@%+,".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

/// `cd <dir> && <argv>` for a POSIX shell.
pub fn shell_line(dir: &Path, argv: &[String]) -> String {
    let cmd: Vec<String> = argv.iter().map(|a| sh_quote(a)).collect();
    format!("cd {} && {}", sh_quote(&dir.display().to_string()), cmd.join(" "))
}

fn applescript_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn spawn(program: &str, args: &[String]) -> bool {
    Command::new(program).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok()
}

/// Open `argv` in `dir` in a new terminal tab or window. `inside_terminal` is
/// true for the TUI (it can use the terminal it runs in).
pub fn launch(dir: &Path, argv: &[String], open_in: OpenIn, inside_terminal: bool) -> Launched {
    if open_in == OpenIn::Here && inside_terminal {
        return Launched::RunHere(argv.to_vec());
    }
    let line = shell_line(dir, argv);
    let d = dir.display().to_string();
    let env = |k: &str| std::env::var_os(k).is_some();
    let tab = open_in == OpenIn::Tab && inside_terminal;

    if tab {
        if env("TMUX") {
            let mut a = vec!["new-window".to_string(), "-c".into(), d.clone(), "--".into()];
            a.extend(argv.iter().cloned());
            if spawn("tmux", &a) {
                return Launched::Opened("a new tmux window");
            }
        }
        if env("WT_SESSION") {
            let mut a = vec!["-w".to_string(), "0".into(), "new-tab".into(), "-d".into(), d.clone()];
            a.extend(argv.iter().cloned());
            if spawn("wt", &a) {
                return Launched::Opened("a new Windows Terminal tab");
            }
        }
        if env("KITTY_WINDOW_ID") {
            let mut a = vec!["@".to_string(), "launch".into(), "--type=tab".into(), format!("--cwd={d}")];
            a.extend(argv.iter().cloned());
            if Command::new("kitty")
                .args(&a)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
            {
                return Launched::Opened("a new kitty tab");
            }
        }
        if env("WEZTERM_PANE") {
            let mut a = vec!["cli".to_string(), "spawn".into(), "--cwd".into(), d.clone(), "--".into()];
            a.extend(argv.iter().cloned());
            if spawn("wezterm", &a) {
                return Launched::Opened("a new WezTerm tab");
            }
        }
        if std::env::var("TERM_PROGRAM").as_deref() == Ok("iTerm.app") {
            let script = format!(
                "tell application \"iTerm2\" to tell current window to create tab with default profile command {}",
                applescript_string(&format!("/bin/sh -lc {}", sh_quote(&format!("{line}; exec $SHELL"))))
            );
            match osascript(&script) {
                Script::Ok => return Launched::Opened("a new iTerm tab"),
                Script::Denied => return Launched::Blocked("iTerm"),
                Script::Failed => {}
            }
        }
    }

    // A new window.
    if cfg!(target_os = "macos") {
        let term = std::env::var("TERM_PROGRAM").unwrap_or_default();
        let app = if term == "iTerm.app" { "iTerm" } else { "Terminal" };
        let script = if term == "iTerm.app" {
            format!(
                "tell application \"iTerm2\" to create window with default profile command {}",
                applescript_string(&format!("/bin/sh -lc {}", sh_quote(&format!("{line}; exec $SHELL"))))
            )
        } else {
            format!("tell application \"Terminal\"\nactivate\ndo script {}\nend tell", applescript_string(&line))
        };
        match osascript(&script) {
            Script::Ok if app == "iTerm" => return Launched::Opened("a new iTerm window"),
            Script::Ok => return Launched::Opened("a new Terminal window"),
            Script::Denied => return Launched::Blocked(app),
            Script::Failed => {}
        }
    } else if cfg!(windows) {
        let mut a = vec!["-w".to_string(), "new".into(), "new-tab".into(), "-d".into(), d.clone()];
        a.extend(argv.iter().cloned());
        if spawn("wt", &a) {
            return Launched::Opened("a new Windows Terminal window");
        }
        let mut a =
            vec!["/C".to_string(), "start".into(), "".into(), "/D".into(), d.clone(), "cmd".into(), "/K".into()];
        a.extend(argv.iter().cloned());
        if spawn("cmd", &a) {
            return Launched::Opened("a new command window");
        }
    } else {
        let hold = format!("{line}; exec \"${{SHELL:-sh}}\"");
        for (prog, pre) in [
            ("x-terminal-emulator", vec!["-e"]),
            ("gnome-terminal", vec!["--"]),
            ("konsole", vec!["-e"]),
            ("kitty", vec![]),
            ("alacritty", vec!["-e"]),
            ("xterm", vec!["-e"]),
        ] {
            let mut a: Vec<String> = pre.iter().map(|s| s.to_string()).collect();
            a.extend(["sh".to_string(), "-c".into(), hold.clone()]);
            if spawn(prog, &a) {
                return Launched::Opened("a new terminal window");
            }
        }
    }
    if inside_terminal {
        Launched::RunHere(argv.to_vec())
    } else {
        Launched::Opened("nothing: no terminal app was found")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refused_apple_event_is_not_success() {
        let denied = "execution error: Not authorized to send Apple events to Terminal. (-1743)";
        assert_eq!(script_error(denied), Script::Denied);
        assert_eq!(script_error("execution error: Terminal got an error: some other problem (-1728)"), Script::Failed);
        assert!(blocked_help("Terminal").contains("Privacy & Security → Automation"));
    }

    fn failure(conclusion: &str, jobs: Vec<Job>, summary: &str) -> Failure {
        let run: Run = serde_json::from_value(serde_json::json!({
            "databaseId": 9, "number": 18, "displayTitle": "Add CSV parser", "workflowName": "CI",
            "headBranch": "feature/parser", "headSha": "abc1234def", "status": "completed",
            "conclusion": conclusion, "event": "push", "url": "https://github.com/o/r/actions/runs/9"
        }))
        .unwrap();
        Failure {
            repo: "o/r".into(),
            branch: "feature/parser".into(),
            run: Some(run),
            jobs,
            summary: summary.into(),
            log: "a\nb\nerror: boom".into(),
        }
    }

    fn job(name: &str, conclusion: &str, step: &str) -> Job {
        serde_json::from_value(serde_json::json!({
            "databaseId": 1, "name": name, "status": "completed", "conclusion": conclusion,
            "steps": [{"name": "Checkout", "status": "completed", "conclusion": "success", "number": 1},
                      {"name": step, "status": "completed", "conclusion": conclusion, "number": 2}]
        }))
        .unwrap()
    }

    #[test]
    fn prompt_for_a_failed_step() {
        let f =
            failure("failure", vec![job("test", "failure", "Run cargo test"), job("lint", "success", "Clippy")], "");
        assert!(!f.workflow_invalid());
        assert_eq!(f.headline(), "CI failed in test → Run cargo test.");
        let p = f.prompt();
        assert!(p.contains("branch `feature/parser` of `o/r`"));
        assert!(p.contains("Commit: `abc1234` Add CSV parser"));
        assert!(p.contains("Job **test**, step **Run cargo test**") && !p.contains("Job **lint**"));
        assert!(p.contains("error: boom") && p.contains("Don't commit or push"));
    }

    #[test]
    fn prompt_for_an_invalid_workflow() {
        let f = failure("startup_failure", vec![], "X .github/workflows/ci.yml: invalid workflow file");
        assert!(f.workflow_invalid());
        assert!(f.headline().starts_with("GitHub couldn't run the workflow"));
        assert!(f.prompt().contains("`.github/workflows/` is invalid"));
    }

    #[test]
    fn assistants_and_commands() {
        assert_eq!(Assistant::from_setting("off"), None);
        assert_eq!(Assistant::from_setting("custom"), None);
        assert_eq!(Assistant::from_setting("claude"), Some(Assistant::Claude));
        assert_eq!(Assistant::Claude.argv("hi"), ["claude", "hi"]);
        assert_eq!(Assistant::Codex.argv("hi"), ["codex", "hi"]);
        let custom = Assistant::from_setting("aider --message {prompt}").unwrap();
        assert_eq!(custom.argv("fix it"), ["aider", "--message", "fix it"]);
        assert_eq!(Assistant::Custom("gemini".into()).argv("x"), ["gemini", "x"]);
        assert_eq!(split_args(r#"my-ai --model "big one""#), ["my-ai", "--model", "big one"]);
    }

    #[test]
    fn logs_are_cleaned() {
        let log =
            "test\tRun cargo test\t\u{feff}2026-09-26T16:10:15.8330051Z \u{1b}[1m\u{1b}[92mCompiling\u{1b}[0m x\n\
                   test\tRun cargo test\t2026-09-26T16:10:16.0000000Z thread 'a' panicked at src/lib.rs:3";
        assert_eq!(clean_log(log), "--- Run cargo test ---\nCompiling x\nthread 'a' panicked at src/lib.rs:3\n");
        // gh writes codes out as text too.
        assert_eq!(strip_ansi("^[[1m^[[92m   Compiling^[[0m canopy (^[[x kept)"), "   Compiling canopy (^[[x kept)");
    }

    #[test]
    fn shell_lines_quote_everything() {
        let line = shell_line(Path::new("/code/my app"), &["claude".into(), "fix it's CI".into()]);
        assert_eq!(line, r#"cd '/code/my app' && claude 'fix it'\''s CI'"#);
    }

    #[test]
    fn prompt_file_lives_in_git_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let git_dir = tmp.path().join(".git");
        let path = write_prompt(&git_dir, "hello").unwrap();
        assert_eq!(path, git_dir.join("canopy/fix-ci.md"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello");
        let k = kickoff(&path, tmp.path());
        assert!(k.contains(".git/canopy/fix-ci.md") || k.contains(r".git\canopy\fix-ci.md"), "{k}");
    }

    /// Prints the prompt for a real failed run of this repository:
    /// `CANOPY_RUN=<id> cargo test -p canopy-gh real_prompt -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn real_prompt() {
        let id: u64 = std::env::var("CANOPY_RUN").unwrap().parse().unwrap();
        let gh = crate::Gh::new(env!("CARGO_MANIFEST_DIR"), None);
        let run = gh.run_list(None, 100).await.unwrap().into_iter().find(|r| r.database_id == id).expect("run");
        let f = gather(&gh, "shankar-sachin/canopy", &run.head_branch.clone(), run).await;
        println!("{}\n-----\n{}", f.headline(), f.prompt());
    }
}
