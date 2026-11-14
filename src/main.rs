//! `gitview [DIR]`: browse the git repository in DIR (default: the current directory).

use cascivo_tui::{run_with, RunOptions, TuiError};
use gitview::app::Gitview;
use gitview::git::CliGit;

fn main() -> Result<(), TuiError> {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let mut app = Gitview::new(CliGit::new(dir));
    // Keep the app's theme index in step with the theme the runtime starts on.
    if let Ok(name) = std::env::var("CASCIVO_THEME") {
        app = app.with_theme(&name);
    }
    let theme = Some(app.theme_name().to_owned());
    run_with(
        app,
        RunOptions {
            theme,
            ..Default::default()
        },
    )
}
