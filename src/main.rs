use gitview::git::{CliGit, GitRepo};

fn main() {
    // Placeholder: the UI is built on top of `gitview::git`.
    let repo = CliGit::new(".");
    match repo.head() {
        Ok(head) => println!("HEAD: {head}"),
        Err(e) => eprintln!("gitview: {e}"),
    }
}
