//! Deterministic in-memory [`GitRepo`] for tests and demos.

use super::*;

#[derive(Debug, Clone)]
pub struct FakeGit {
    pub commits: Vec<Commit>,
    pub files: Vec<FileStatus>,
    pub branches: Vec<Branch>,
    pub head: Head,
}

fn commit(n: u32, subject: &str, body: &str) -> Commit {
    let hash = format!("{n:07x}{:033x}", 0);
    Commit {
        short: hash[..7].to_string(),
        hash,
        author: "Ada Lovelace".into(),
        date: format!("2026-01-{n:02}T12:00:00+00:00"),
        subject: subject.into(),
        body: body.into(),
    }
}

impl Default for FakeGit {
    fn default() -> Self {
        let st = |i, w, p: &str, o: Option<&str>| FileStatus {
            path: p.into(),
            orig_path: o.map(Into::into),
            index: i,
            worktree: w,
        };
        let br = |name: &str, current, up: Option<&str>, n: u32, remote| Branch {
            name: name.into(),
            current,
            upstream: up.map(Into::into),
            short: format!("{n:07x}"),
            subject: format!("tip of {name}"),
            remote,
        };
        Self {
            commits: vec![
                commit(
                    3,
                    "Add parser",
                    "Parses porcelain output.\n\nIncludes renames.",
                ),
                commit(2, "Add types", ""),
                commit(1, "Initial commit", ""),
            ],
            files: vec![
                st(' ', 'M', "src/main.rs", None),
                st('A', ' ', "src/new.rs", None),
                st('R', ' ', "docs/b.md", Some("docs/a.md")),
                st('?', '?', "notes.txt", None),
            ],
            branches: vec![
                br("main", true, Some("origin/main"), 3, false),
                br("feature/ui", false, None, 2, false),
                br("origin/main", false, None, 3, true),
            ],
            head: Head::Branch("main".into()),
        }
    }
}

impl GitRepo for FakeGit {
    fn log(&self, limit: usize) -> Result<Vec<Commit>> {
        Ok(self.commits.iter().take(limit).cloned().collect())
    }

    fn show(&self, rev: &str) -> Result<CommitDetail> {
        let commit = self
            .commits
            .iter()
            .find(|c| c.hash.starts_with(rev) || c.short == rev)
            .cloned()
            .ok_or_else(|| GitError::Failed {
                code: Some(128),
                stderr: format!("unknown revision {rev}"),
            })?;
        Ok(CommitDetail {
            stat: " src/lib.rs | 2 +-\n 1 file changed, 1 insertion(+), 1 deletion(-)".into(),
            commit,
        })
    }

    fn status(&self) -> Result<Vec<FileStatus>> {
        Ok(self.files.clone())
    }

    fn branches(&self) -> Result<Vec<Branch>> {
        Ok(self.branches.clone())
    }

    fn head(&self) -> Result<Head> {
        Ok(self.head.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_is_deterministic_and_limits() {
        let g = FakeGit::default();
        assert_eq!(g.log(2).unwrap().len(), 2);
        let c = &g.log(10).unwrap()[0];
        assert_eq!(g.show(&c.short).unwrap().commit, *c);
        assert!(g.show("zzz").is_err());
    }
}
