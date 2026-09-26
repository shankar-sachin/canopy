//! `git submodule status --recursive`

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmoduleState {
    /// Checked out at the commit the superproject records.
    InSync,
    /// Not initialized / not cloned yet.
    Uninitialized,
    /// Checked out at a different commit than recorded.
    Modified,
    /// Merge conflict in the recorded commit.
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submodule {
    pub path: String,
    pub oid: String,
    pub state: SubmoduleState,
    /// `git describe` of the checked-out commit, e.g. `v1.2.0` or `heads/main`.
    pub describe: Option<String>,
}

pub fn parse(out: &str) -> Vec<Submodule> {
    out.lines()
        .filter_map(|line| {
            let mut chars = line.chars();
            let state = match chars.next()? {
                ' ' => SubmoduleState::InSync,
                '-' => SubmoduleState::Uninitialized,
                '+' => SubmoduleState::Modified,
                'U' => SubmoduleState::Conflict,
                _ => return None,
            };
            let rest = chars.as_str();
            let (oid, rest) = rest.split_once(' ')?;
            let (path, describe) = match rest.rsplit_once(" (") {
                Some((p, d)) if d.ends_with(')') => (p, Some(d.trim_end_matches(')').to_string())),
                _ => (rest, None),
            };
            Some(Submodule { path: path.to_string(), oid: oid.to_string(), state, describe })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_states() {
        let out = " aaa lib/core (v1.2.0)\n-bbb vendor/x\n+ccc lib/ui (heads/main)\nUddd lib/conflicted\n";
        let s = parse(out);
        assert_eq!(s.len(), 4);
        assert_eq!(s[0].state, SubmoduleState::InSync);
        assert_eq!(s[0].path, "lib/core");
        assert_eq!(s[0].describe.as_deref(), Some("v1.2.0"));
        assert_eq!(s[1].state, SubmoduleState::Uninitialized);
        assert_eq!(s[1].describe, None);
        assert_eq!(s[2].state, SubmoduleState::Modified);
        assert_eq!(s[3].state, SubmoduleState::Conflict);
    }
}
