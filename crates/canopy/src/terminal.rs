//! Terminal setup/teardown, including a panic hook that restores the screen.

use std::io::{stdout, Write};

use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::execute;
use ratatui::DefaultTerminal;

pub fn init() -> DefaultTerminal {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        hook(info);
    }));
    let term = ratatui::init();
    let _ = execute!(stdout(), EnableMouseCapture);
    term
}

/// Leave raw mode / alternate screen (safe to call repeatedly).
pub fn restore() {
    let _ = execute!(stdout(), DisableMouseCapture);
    ratatui::restore();
    let _ = stdout().flush();
}

/// Re-enter the TUI after [`restore`].
pub fn enter() {
    let _ = ratatui::crossterm::terminal::enable_raw_mode();
    let _ = execute!(stdout(), ratatui::crossterm::terminal::EnterAlternateScreen, EnableMouseCapture);
}

/// A shell command for the platform: `sh -c` on macOS/Linux, `cmd /C` on
/// Windows. Used for custom commands.
pub fn shell_command(cmd: &str) -> tokio::process::Command {
    if cfg!(windows) {
        let mut c = tokio::process::Command::new("cmd");
        c.arg("/C").arg(cmd);
        c
    } else {
        let mut c = tokio::process::Command::new("sh");
        c.arg("-c").arg(cmd);
        c
    }
}

/// Open a URL in the default browser (never blocks the UI).
pub fn open_url(url: &str) -> bool {
    let mut cmd = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(windows) {
        // `start` is a cmd built-in; the empty string is the window title.
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    } else {
        std::process::Command::new("xdg-open")
    };
    cmd.arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .is_ok()
}

/// Copy text to the clipboard: `pbcopy`/`clip`/`wl-copy`/`xclip` if present,
/// otherwise the OSC 52 escape sequence (works over SSH in most terminals).
pub fn copy_to_clipboard(text: &str) -> bool {
    use std::process::{Command, Stdio};
    // Windows: clip. macOS, Wayland, X11: pbcopy, wl-copy, xclip.
    // OSC 52 below covers everything else.
    let tools: &[(&str, &[&str])] = if cfg!(windows) {
        &[("clip", &[])]
    } else {
        &[("pbcopy", &[]), ("wl-copy", &[]), ("xclip", &["-selection", "clipboard"])]
    };
    for &(cmd, args) in tools {
        if let Ok(mut child) = Command::new(cmd).args(args).stdin(Stdio::piped()).spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if child.wait().map(|s| s.success()).unwrap_or(false) {
                return true;
            }
        }
    }
    let seq = format!("\x1b]52;c;{}\x07", base64(text.as_bytes()));
    stdout().write_all(seq.as_bytes()).and_then(|_| stdout().flush()).is_ok()
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64() {
        assert_eq!(super::base64(b"abc"), "YWJj");
        assert_eq!(super::base64(b"ab"), "YWI=");
        assert_eq!(super::base64(b"a"), "YQ==");
    }
}
