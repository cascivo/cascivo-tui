use gitview::git::{CliGit, GitRepo, Head};
use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args([
            "-c",
            "user.name=T",
            "-c",
            "user.email=t@e.x",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?} failed");
}

#[test]
fn cli_against_real_repo() {
    let dir = std::env::temp_dir().join(format!("gitview-it-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    std::fs::write(dir.join("a.txt"), "x").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first", "-m", "body text"]);
    std::fs::write(dir.join("b.txt"), "y").unwrap();

    let repo = CliGit::new(&dir);
    let log = repo.log(5).unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].subject, "first");
    assert_eq!(log[0].body, "body text");
    assert!(repo.show(&log[0].hash).unwrap().stat.contains("a.txt"));
    assert_eq!(repo.status().unwrap()[0].path, "b.txt");
    assert_eq!(repo.branches().unwrap()[0].name, "main");
    assert_eq!(repo.head().unwrap(), Head::Branch("main".into()));
    std::fs::remove_dir_all(&dir).ok();
}
