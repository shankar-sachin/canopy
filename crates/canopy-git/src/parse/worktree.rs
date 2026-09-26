//! `git worktree list --porcelain`

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    /// `None` for a bare repository entry.
    pub head: Option<String>,
    /// Short branch name; `None` when detached or bare.
    pub branch: Option<String>,
    pub bare: bool,
    /// Reason if locked (may be empty).
    pub locked: Option<String>,
    /// Reason if git considers it prunable (its directory is gone).
    pub prunable: Option<String>,
}

pub fn parse(out: &str) -> Vec<Worktree> {
    let mut out_list = Vec::new();
    for block in out.split("\n\n") {
        let mut wt: Option<Worktree> = None;
        for line in block.lines() {
            let (key, val) = line.split_once(' ').unwrap_or((line, ""));
            if key == "worktree" {
                wt = Some(Worktree {
                    path: PathBuf::from(val),
                    head: None,
                    branch: None,
                    bare: false,
                    locked: None,
                    prunable: None,
                });
                continue;
            }
            let Some(w) = wt.as_mut() else { continue };
            match key {
                "HEAD" => w.head = Some(val.to_string()),
                "branch" => w.branch = Some(val.strip_prefix("refs/heads/").unwrap_or(val).to_string()),
                "bare" => w.bare = true,
                "locked" => w.locked = Some(val.to_string()),
                "prunable" => w.prunable = Some(val.to_string()),
                _ => {}
            }
        }
        if let Some(w) = wt {
            out_list.push(w);
        }
    }
    out_list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_kinds() {
        let out = "worktree /code/app\nHEAD aaa\nbranch refs/heads/main\n\n\
                   worktree /code/app-fix\nHEAD bbb\nbranch refs/heads/fix/login\nlocked on a usb drive\n\n\
                   worktree /code/app-detached\nHEAD ccc\ndetached\nprunable gitdir file points to non-existent location\n\n";
        let w = parse(out);
        assert_eq!(w.len(), 3);
        assert_eq!(w[0].branch.as_deref(), Some("main"));
        assert_eq!(w[1].branch.as_deref(), Some("fix/login"));
        assert_eq!(w[1].locked.as_deref(), Some("on a usb drive"));
        assert_eq!(w[2].branch, None);
        assert!(w[2].prunable.is_some());
        assert_eq!(w[2].path, PathBuf::from("/code/app-detached"));
    }

    #[test]
    fn bare_entry() {
        let w = parse("worktree /srv/repo.git\nbare\n\n");
        assert!(w[0].bare && w[0].head.is_none());
    }
}
