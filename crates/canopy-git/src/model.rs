use std::path::PathBuf;

/// State of one side (index or worktree) of a changed file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Change {
    Unmodified,
    Modified,
    TypeChanged,
    Added,
    Deleted,
    Renamed,
    Copied,
    Unmerged,
}

impl Change {
    pub fn from_code(c: char) -> Self {
        match c {
            'M' => Change::Modified,
            'T' => Change::TypeChanged,
            'A' => Change::Added,
            'D' => Change::Deleted,
            'R' => Change::Renamed,
            'C' => Change::Copied,
            'U' => Change::Unmerged,
            _ => Change::Unmodified,
        }
    }

    pub fn code(self) -> char {
        match self {
            Change::Unmodified => ' ',
            Change::Modified => 'M',
            Change::TypeChanged => 'T',
            Change::Added => 'A',
            Change::Deleted => 'D',
            Change::Renamed => 'R',
            Change::Copied => 'C',
            Change::Unmerged => 'U',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FileKind {
    Tracked,
    Untracked,
    Ignored,
    Conflicted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FileStatus {
    pub path: String,
    /// Original path for renames/copies.
    pub orig_path: Option<String>,
    pub index: Change,
    pub worktree: Change,
    pub kind: FileKind,
}

impl FileStatus {
    pub fn is_staged(&self) -> bool {
        self.kind == FileKind::Tracked && self.index != Change::Unmodified
    }

    pub fn is_unstaged(&self) -> bool {
        match self.kind {
            FileKind::Untracked | FileKind::Conflicted => true,
            FileKind::Tracked => self.worktree != Change::Unmodified,
            FileKind::Ignored => false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BranchInfo {
    /// `None` when HEAD is detached.
    pub head: Option<String>,
    pub oid: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Status {
    pub branch: BranchInfo,
    pub files: Vec<FileStatus>,
}

impl Status {
    pub fn staged(&self) -> impl Iterator<Item = &FileStatus> {
        self.files.iter().filter(|f| f.is_staged())
    }
    pub fn unstaged(&self) -> impl Iterator<Item = &FileStatus> {
        self.files.iter().filter(|f| f.is_unstaged())
    }
    pub fn conflicted(&self) -> impl Iterator<Item = &FileStatus> {
        self.files.iter().filter(|f| f.kind == FileKind::Conflicted)
    }
    pub fn is_clean(&self) -> bool {
        self.files.iter().all(|f| f.kind == FileKind::Ignored)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Commit {
    pub oid: String,
    pub short: String,
    pub parents: Vec<String>,
    pub author: String,
    pub email: String,
    /// Unix timestamp (author date).
    pub time: i64,
    pub refs: Vec<String>,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Branch {
    pub name: String,
    pub is_remote: bool,
    pub is_head: bool,
    pub oid: String,
    pub upstream: Option<String>,
    /// Raw track info like `ahead 1, behind 2` or `gone`.
    pub track: Option<String>,
    pub subject: String,
    pub time: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stash {
    pub index: usize,
    pub name: String,
    pub message: String,
    pub time: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Remote {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tag {
    pub name: String,
    pub oid: String,
    pub subject: String,
    pub time: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReflogEntry {
    pub oid: String,
    pub selector: String,
    pub subject: String,
    pub time: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
    /// `\ No newline at end of file`
    NoNewline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Hunk {
    pub header: String,
    pub old_start: u32,
    pub old_len: u32,
    pub new_start: u32,
    pub new_len: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FileDiff {
    pub old_path: String,
    pub new_path: String,
    /// Raw header lines (`diff --git`, `index`, `---`, `+++`, mode lines...).
    pub header: Vec<String>,
    pub hunks: Vec<Hunk>,
    pub binary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Repo {
    pub root: PathBuf,
    pub git_dir: PathBuf,
}

/// In-progress multi-step operation, detected from `.git` state files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RepoState {
    Clean,
    Merging,
    Rebasing,
    CherryPicking,
    Reverting,
    Bisecting,
}
