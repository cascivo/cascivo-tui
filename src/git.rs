//! Git domain layer: types, the [`GitRepo`] trait, a CLI-backed
//! implementation, and pure parsers (no I/O) for git's machine formats.

use std::fmt;
use std::path::PathBuf;
use std::process::Command;

pub mod fake;

/// Field and record separators used in `--format` strings.
const FS: char = '\x1f';
const RS: char = '\x1e';
const LOG_FORMAT: &str = "--format=%H%x1f%h%x1f%an%x1f%aI%x1f%s%x1f%b%x1e";
const BRANCH_FORMAT: &str =
    "--format=%(HEAD)%1f%(refname:short)%1f%(upstream:short)%1f%(objectname:short)%1f%(symref)%1f%(contents:subject)%1e";

// ---------------------------------------------------------------- types

/// A commit. `date` is ISO-8601 strict (`%aI`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub hash: String,
    pub short: String,
    pub author: String,
    pub date: String,
    pub subject: String,
    pub body: String,
}

/// A commit plus its `--stat` summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitDetail {
    pub commit: Commit,
    pub stat: String,
}

/// One entry of `git status --porcelain=v1`: `index`/`worktree` are the
/// two status letters (`' '` = unmodified, `'?'` = untracked).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStatus {
    pub path: String,
    /// Source path for renames/copies.
    pub orig_path: Option<String>,
    pub index: char,
    pub worktree: char,
}

impl FileStatus {
    pub fn is_untracked(&self) -> bool {
        self.index == '?'
    }
    pub fn is_staged(&self) -> bool {
        !matches!(self.index, ' ' | '?' | '!')
    }
    pub fn is_conflicted(&self) -> bool {
        matches!(
            (self.index, self.worktree),
            ('U', _) | (_, 'U') | ('A', 'A') | ('D', 'D')
        )
    }
}

/// A local or remote-tracking branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    pub current: bool,
    pub upstream: Option<String>,
    pub short: String,
    pub subject: String,
    pub remote: bool,
}

/// What `HEAD` points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Head {
    Branch(String),
    Detached(String),
}

impl fmt::Display for Head {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Head::Branch(b) => f.write_str(b),
            Head::Detached(h) => write!(f, "(detached at {h})"),
        }
    }
}

// ---------------------------------------------------------------- errors

#[derive(Debug)]
pub enum GitError {
    Io(std::io::Error),
    Failed { code: Option<i32>, stderr: String },
    Parse(String),
}

pub type Result<T> = std::result::Result<T, GitError>;

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::Io(e) => write!(f, "cannot run git: {e}"),
            GitError::Failed { stderr, .. } => write!(f, "git failed: {}", stderr.trim()),
            GitError::Parse(m) => write!(f, "unexpected git output: {m}"),
        }
    }
}

impl std::error::Error for GitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GitError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for GitError {
    fn from(e: std::io::Error) -> Self {
        GitError::Io(e)
    }
}

// ---------------------------------------------------------------- trait

/// Read-only repository access. Implemented by [`CliGit`] and
/// [`fake::FakeGit`].
pub trait GitRepo {
    /// Most recent `limit` commits reachable from HEAD, newest first.
    fn log(&self, limit: usize) -> Result<Vec<Commit>>;
    /// Full detail for one commit (any rev git understands).
    fn show(&self, rev: &str) -> Result<CommitDetail>;
    /// Working tree status.
    fn status(&self) -> Result<Vec<FileStatus>>;
    /// Local branches followed by remote-tracking branches.
    fn branches(&self) -> Result<Vec<Branch>>;
    /// Current HEAD.
    fn head(&self) -> Result<Head>;
}

// ---------------------------------------------------------------- CLI

/// [`GitRepo`] backed by the `git` executable.
#[derive(Debug, Clone)]
pub struct CliGit {
    dir: PathBuf,
}

impl CliGit {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn run(&self, args: &[&str]) -> Result<String> {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.dir)
            .args(args)
            .output()?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(GitError::Failed {
                code: out.status.code(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            })
        }
    }
}

impl GitRepo for CliGit {
    fn log(&self, limit: usize) -> Result<Vec<Commit>> {
        let n = format!("-n{limit}");
        parse_log(&self.run(&["log", &n, LOG_FORMAT])?)
    }

    fn show(&self, rev: &str) -> Result<CommitDetail> {
        let meta = self.run(&["show", "-s", LOG_FORMAT, rev, "--"])?;
        let commit = parse_log(&meta)?
            .into_iter()
            .next()
            .ok_or_else(|| GitError::Parse(format!("no commit for {rev}")))?;
        let stat = self.run(&["show", "--stat", "--format=", rev, "--"])?;
        Ok(CommitDetail {
            commit,
            stat: stat.trim_matches('\n').to_string(),
        })
    }

    fn status(&self) -> Result<Vec<FileStatus>> {
        parse_status(&self.run(&["status", "--porcelain=v1", "-z"])?)
    }

    fn branches(&self) -> Result<Vec<Branch>> {
        let local = self.run(&["for-each-ref", BRANCH_FORMAT, "refs/heads"])?;
        let remote = self.run(&["for-each-ref", BRANCH_FORMAT, "refs/remotes"])?;
        let mut all = parse_branches(&local, false)?;
        all.extend(parse_branches(&remote, true)?);
        Ok(all)
    }

    fn head(&self) -> Result<Head> {
        let abbrev = self.run(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        let short = self.run(&["rev-parse", "--short", "HEAD"])?;
        Ok(parse_head(&abbrev, &short))
    }
}

// ---------------------------------------------------------------- parsers

/// Parse `git log/show --format=%H\x1f%h\x1f%an\x1f%aI\x1f%s\x1f%b\x1e`.
pub fn parse_log(out: &str) -> Result<Vec<Commit>> {
    out.split(RS)
        .map(|r| r.trim_start_matches('\n'))
        .filter(|r| !r.trim().is_empty())
        .map(|rec| {
            let f: Vec<&str> = rec.splitn(6, FS).collect();
            if f.len() != 6 {
                return Err(GitError::Parse(format!(
                    "commit record has {} fields",
                    f.len()
                )));
            }
            Ok(Commit {
                hash: f[0].into(),
                short: f[1].into(),
                author: f[2].into(),
                date: f[3].into(),
                subject: f[4].into(),
                body: f[5].trim_end().into(),
            })
        })
        .collect()
}

/// Parse `git status --porcelain=v1 -z`.
pub fn parse_status(out: &str) -> Result<Vec<FileStatus>> {
    let mut parts = out.split('\0').filter(|p| !p.is_empty());
    let mut files = Vec::new();
    while let Some(entry) = parts.next() {
        let mut chars = entry.chars();
        let (Some(index), Some(worktree), Some(' ')) = (chars.next(), chars.next(), chars.next())
        else {
            return Err(GitError::Parse(format!("bad status entry {entry:?}")));
        };
        let path = chars.as_str().to_string();
        // Renames/copies are followed by the source path.
        let orig_path = if matches!(index, 'R' | 'C') || matches!(worktree, 'R' | 'C') {
            Some(
                parts
                    .next()
                    .ok_or_else(|| GitError::Parse("rename without source".into()))?
                    .to_string(),
            )
        } else {
            None
        };
        files.push(FileStatus {
            path,
            orig_path,
            index,
            worktree,
        });
    }
    Ok(files)
}

/// Parse `git for-each-ref` output produced with `BRANCH_FORMAT`.
pub fn parse_branches(out: &str, remote: bool) -> Result<Vec<Branch>> {
    out.split(RS)
        .map(|r| r.trim_start_matches('\n'))
        .filter(|r| !r.trim().is_empty())
        .filter_map(|rec| {
            let f: Vec<&str> = rec.splitn(6, FS).collect();
            if f.len() != 6 {
                return Some(Err(GitError::Parse(format!(
                    "branch record has {} fields",
                    f.len()
                ))));
            }
            // `origin/HEAD` is a symbolic ref, not a real branch.
            if !f[4].is_empty() {
                return None;
            }
            Some(Ok(Branch {
                name: f[1].into(),
                current: f[0] == "*",
                upstream: (!f[2].is_empty()).then(|| f[2].to_string()),
                short: f[3].into(),
                subject: f[5].trim_end().into(),
                remote,
            }))
        })
        .collect()
}

/// `abbrev` is `rev-parse --abbrev-ref HEAD` (prints `HEAD` when detached).
pub fn parse_head(abbrev: &str, short: &str) -> Head {
    match abbrev.trim() {
        "HEAD" => Head::Detached(short.trim().to_string()),
        name => Head::Branch(name.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(h: &str, subj: &str, body: &str) -> String {
        format!(
            "{h}\x1f{}\x1fAda\x1f2026-01-02T03:04:05+00:00\x1f{subj}\x1f{body}\x1e\n",
            &h[..3]
        )
    }

    #[test]
    fn log_empty() {
        assert!(parse_log("").unwrap().is_empty());
        assert!(parse_log("\n").unwrap().is_empty());
    }

    #[test]
    fn log_multiple_and_multiline_body() {
        let out = rec("aaa111", "one", "line1\n\nline3\n") + &rec("bbb222", "two", "");
        let c = parse_log(&out).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].short, "aaa");
        assert_eq!(c[0].body, "line1\n\nline3");
        assert_eq!(c[1].hash, "bbb222");
        assert_eq!(c[1].body, "");
    }

    #[test]
    fn log_bad_record() {
        assert!(matches!(parse_log("a\x1fb\x1e"), Err(GitError::Parse(_))));
    }

    #[test]
    fn status_empty() {
        assert!(parse_status("").unwrap().is_empty());
    }

    #[test]
    fn status_kinds() {
        let s = parse_status(" M a.rs\0A  b.rs\0?? c.rs\0UU d.rs\0").unwrap();
        assert_eq!(s.len(), 4);
        assert_eq!((s[0].index, s[0].worktree), (' ', 'M'));
        assert!(!s[0].is_staged());
        assert!(s[1].is_staged());
        assert!(s[2].is_untracked() && !s[2].is_staged());
        assert!(s[3].is_conflicted());
    }

    #[test]
    fn status_rename_consumes_source() {
        let s = parse_status("R  new name.rs\0old.rs\0 M z.rs\0").unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].path, "new name.rs");
        assert_eq!(s[0].orig_path.as_deref(), Some("old.rs"));
        assert_eq!(s[1].path, "z.rs");
        assert_eq!(s[1].orig_path, None);
    }

    #[test]
    fn status_rename_missing_source_and_garbage() {
        assert!(parse_status("R  new.rs\0").is_err());
        assert!(parse_status("x\0").is_err());
    }

    #[test]
    fn branches_local_with_upstream() {
        let out = "*\x1fmain\x1forigin/main\x1fabc1234\x1f\x1fInit\x1e\n \x1ffeat\x1f\x1fdef5678\x1f\x1fWork\x1e\n";
        let b = parse_branches(out, false).unwrap();
        assert_eq!(b.len(), 2);
        assert!(b[0].current && !b[0].remote);
        assert_eq!(b[0].upstream.as_deref(), Some("origin/main"));
        assert!(!b[1].current);
        assert_eq!(b[1].upstream, None);
    }

    #[test]
    fn branches_remote_skips_head_alias_and_empty() {
        let out = " \x1forigin\x1f\x1fabc\x1frefs/remotes/origin/main\x1fx\x1e\n \x1forigin/main\x1f\x1fabc\x1f\x1fx\x1e\n";
        let b = parse_branches(out, true).unwrap();
        assert_eq!(b.len(), 1);
        assert!(b[0].remote);
        assert!(parse_branches("", false).unwrap().is_empty());
    }

    #[test]
    fn head_branch_and_detached() {
        assert_eq!(parse_head("main\n", "abc\n"), Head::Branch("main".into()));
        let d = parse_head("HEAD\n", "abc1234\n");
        assert_eq!(d, Head::Detached("abc1234".into()));
        assert_eq!(d.to_string(), "(detached at abc1234)");
    }
}
