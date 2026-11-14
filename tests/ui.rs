//! Headless UI tests: drive the app with FakeGit on a fixed 80x24 grid and compare the
//! screen with `tests/snapshots/*.txt`. Accept changes with `UPDATE_SNAPSHOTS=1 cargo test`.

use std::path::PathBuf;

use cascivo_tui::{Role, TestApp};
use gitview::app::{Gitview, TOAST_MS};
use gitview::git::fake::FakeGit;

const COLS: u16 = 80;
const ROWS: u16 = 24;

fn app() -> TestApp<Gitview<FakeGit>> {
    let app = Gitview::new(FakeGit::default()).with_theme("terminal");
    TestApp::new(app, COLS, ROWS).theme("terminal")
}

fn snapshot(name: &str, t: &mut TestApp<Gitview<FakeGit>>) {
    let screen = t.render().expect("view has no class errors") + "\n";
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(format!("{name}.txt"));
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &screen).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing {}; run UPDATE_SNAPSHOTS=1 cargo test",
            path.display()
        )
    });
    assert_eq!(screen, want, "snapshot {name} differs");
}

#[test]
fn log_tab() {
    let mut t = app();
    assert_eq!(t.focused(), Some("commits"));
    snapshot("log", &mut t);
}

#[test]
fn log_selection_moves_detail() {
    let mut t = app();
    t.press("j");
    let s = t.render().unwrap();
    assert!(s.contains("0000002") && s.lines().nth(3).unwrap().contains("Add types"));
    snapshot("log_second", &mut t);
}

#[test]
fn status_tab() {
    let mut t = app();
    t.press("2");
    assert_eq!(t.focused(), Some("files"));
    snapshot("status", &mut t);
}

#[test]
fn branches_tab() {
    let mut t = app();
    t.press("right").press("right");
    assert_eq!(t.focused(), Some("branches"));
    snapshot("branches", &mut t);
}

#[test]
fn help_modal() {
    let mut t = app();
    t.press("?");
    snapshot("help", &mut t);
    t.press("q"); // app keys are skipped while the modal is open
    assert!(!t.is_quit());
    t.press("esc");
    assert!(!t.app().help);
    assert_eq!(t.focused(), Some("commits"));
}

#[test]
fn filter_with_query() {
    let mut t = app();
    t.press("2").press("/");
    assert_eq!(t.focused(), Some("filter"));
    t.type_text("src");
    assert_eq!(t.app().query, "src");
    snapshot("filter", &mut t);
    t.press("enter");
    assert_eq!(t.focused(), Some("files"));
    snapshot("filter_kept", &mut t);
    t.press("esc");
    assert!(t.app().query.is_empty());
}

#[test]
fn filter_keys_type_instead_of_acting() {
    let mut t = app();
    t.press("/").type_text("q?t");
    assert!(!t.is_quit() && !t.app().help);
    assert_eq!(t.app().query, "q?t");
    t.press("esc");
    assert_eq!(t.focused(), Some("commits"));
}

#[test]
fn toast_on_refresh_expires() {
    let mut t = app();
    t.press("r");
    snapshot("toast", &mut t);
    t.advance(TOAST_MS);
    assert!(!t.render().unwrap().contains("refreshed"));
}

#[test]
fn theme_cycle() {
    let mut t = app();
    let before = t.cell(1, 0);
    t.press("t");
    let name = t.app().theme_name();
    assert_ne!(name, "terminal");
    let s = t.render().unwrap();
    assert!(s.contains(&format!("main · {name}")));
    let after = t.cell(1, 0);
    assert_eq!(after.symbol, before.symbol);
    assert_ne!(
        (after.fg, after.bg),
        (before.fg, before.bg),
        "colours follow the theme"
    );
    snapshot("theme", &mut t);
}

#[test]
fn selected_row_is_highlighted() {
    let mut t = app();
    // Row 4 is the first commit (tabs, filter, card top, header).
    assert_eq!(t.cell(3, 4).bg_role, Some(Role::Selection));
    assert_ne!(t.cell(3, 5).bg_role, Some(Role::Selection));
}

#[test]
fn quit() {
    let mut t = app();
    t.press("q");
    assert!(t.is_quit());
}
