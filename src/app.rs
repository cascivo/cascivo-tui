//! The gitview app: plain state, a pure `view`, and `update` for messages.
//!
//! All git access goes through the [`GitRepo`] trait, so tests drive the app with
//! [`FakeGit`](crate::git::fake::FakeGit) and get the same screen every time.

use cascivo_tui::prelude::*;
use cascivo_tui::widgets::{Col, Table};

use crate::fuzzy;
use crate::git::{Branch, Commit, CommitDetail, FileStatus, GitError, GitRepo};

/// How many commits the Log tab loads.
pub const LOG_LIMIT: usize = 200;
/// How long a toast stays up.
pub const TOAST_MS: u64 = 2000;

/// Every key the app understands, for the help modal.
pub const HELP: &[(&str, &str)] = &[
    ("1 2 3", "switch tab"),
    ("← →", "previous / next tab"),
    ("j k ↑ ↓", "move selection"),
    ("/", "fuzzy filter the list"),
    ("enter", "keep filter, back to list"),
    ("esc", "clear filter / close help"),
    ("?", "this help"),
    ("r", "refresh from git"),
    ("t", "next theme"),
    ("q", "quit"),
];

/// The three views.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Log,
    Status,
    Branches,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Log, Tab::Status, Tab::Branches];
    pub const TITLES: [&'static str; 3] = ["Log", "Status", "Branches"];

    pub fn index(self) -> usize {
        self as usize
    }

    /// Focus id of the tab's list.
    pub fn id(self) -> &'static str {
        match self {
            Tab::Log => "commits",
            Tab::Status => "files",
            Tab::Branches => "branches",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Tab(usize),
    NextTab,
    PrevTab,
    /// Select a row of the visible (filtered) list.
    Select(usize),
    Up,
    Down,
    FilterOpen,
    Query(String),
    FilterDone,
    FilterClear,
    Help,
    CloseHelp,
    Refresh,
    Theme,
    Expire,
    Quit,
}

pub struct Gitview<R> {
    repo: R,
    pub tab: Tab,
    pub head: String,
    pub commits: Vec<Commit>,
    pub files: Vec<FileStatus>,
    pub branches: Vec<Branch>,
    /// Detail of the selected commit on the Log tab.
    pub detail: Option<CommitDetail>,
    /// Selected row per tab, an index into the visible (filtered) rows.
    pub selected: [usize; 3],
    pub query: String,
    /// The filter input has focus.
    pub filtering: bool,
    pub help: bool,
    /// Index into [`Theme::names`].
    pub theme: usize,
    pub toasts: Vec<(Kind, String)>,
    cursor: Cursor,
    scroll: [Scroll; 3],
}

impl<R: GitRepo> Gitview<R> {
    /// A new app over `repo`; data loads in [`App::init`].
    pub fn new(repo: R) -> Self {
        let start = Theme::default().name();
        Self {
            repo,
            tab: Tab::Log,
            head: String::new(),
            commits: Vec::new(),
            files: Vec::new(),
            branches: Vec::new(),
            detail: None,
            selected: [0; 3],
            query: String::new(),
            filtering: false,
            help: false,
            theme: Theme::names().iter().position(|n| *n == start).unwrap_or(0),
            toasts: Vec::new(),
            cursor: Cursor::default(),
            scroll: Default::default(),
        }
    }

    /// Starts on the named theme; unknown names keep the default.
    #[must_use]
    pub fn with_theme(mut self, name: &str) -> Self {
        if let Some(i) = Theme::names().iter().position(|n| *n == name) {
            self.theme = i;
        }
        self
    }

    pub fn theme_name(&self) -> &'static str {
        Theme::names()[self.theme]
    }

    fn load(&mut self) -> Result<(), GitError> {
        self.head = self.repo.head()?.to_string();
        self.commits = self.repo.log(LOG_LIMIT)?;
        self.files = self.repo.status()?;
        self.branches = self.repo.branches()?;
        for tab in Tab::ALL {
            let len = self.visible(tab).len();
            let sel = &mut self.selected[tab.index()];
            *sel = (*sel).min(len.saturating_sub(1));
        }
        self.load_detail();
        Ok(())
    }

    fn load_detail(&mut self) {
        let hash = self
            .visible(Tab::Log)
            .get(self.selected[0])
            .map(|&i| self.commits[i].hash.clone());
        self.detail = hash.and_then(|h| self.repo.show(&h).ok());
    }

    /// Text each row of `tab` is fuzzy-matched against.
    fn keys_for(&self, tab: Tab) -> Vec<String> {
        match tab {
            Tab::Log => self
                .commits
                .iter()
                .map(|c| format!("{} {} {}", c.short, c.subject, c.author))
                .collect(),
            Tab::Status => self.files.iter().map(|f| f.path.clone()).collect(),
            Tab::Branches => self.branches.iter().map(|b| b.name.clone()).collect(),
        }
    }

    fn rows_len(&self, tab: Tab) -> usize {
        match tab {
            Tab::Log => self.commits.len(),
            Tab::Status => self.files.len(),
            Tab::Branches => self.branches.len(),
        }
    }

    /// Indices of the rows of `tab` that pass the filter (the filter applies to the current
    /// tab only), best match first.
    pub fn visible(&self, tab: Tab) -> Vec<usize> {
        let query = if tab == self.tab {
            self.query.as_str()
        } else {
            ""
        };
        let keys = self.keys_for(tab);
        fuzzy::filter(query, keys.iter().map(String::as_str))
    }

    fn select(&mut self, i: usize) {
        self.selected[self.tab.index()] = i;
        if self.tab == Tab::Log {
            self.load_detail();
        }
    }

    fn notify(&mut self, kind: Kind, msg: String) -> Cmd<Msg> {
        self.toasts.push((kind, msg));
        Cmd::tick(TOAST_MS, Msg::Expire)
    }

    fn set_tab(&mut self, i: usize) {
        self.tab = Tab::ALL[i.min(Tab::ALL.len() - 1)];
        self.query.clear();
        self.filtering = false;
    }

    // ------------------------------------------------------------ view parts

    /// The list is focusable unless the filter input holds focus. Dropping the id is what
    /// moves focus to the input (see FEEDBACK.md: there is no `Cmd` to focus an id).
    fn focusable(&self, t: Table<Msg>) -> Node<Msg> {
        let t = t.scroll(&self.scroll[self.tab.index()]).class("grow");
        if self.filtering {
            t.into()
        } else {
            t.id(self.tab.id())
        }
    }

    fn filter_bar(&self) -> Node<Msg> {
        let field = if self.filtering {
            input(&self.query, "type to filter", Msg::Query)
                .on_submit(Msg::FilterDone)
                .cursor(&self.cursor)
                .class("grow")
                .id("filter")
        } else if self.query.is_empty() {
            text("press / to filter").class("dim").into()
        } else {
            text(format!("{}  (esc clears)", self.query))
                .class("text-accent")
                .into()
        };
        row(
            "h-1 px-1 gap-1",
            [text("/").class("text-accent bold").into(), field],
        )
    }

    fn table_or_empty(&self, rows: Vec<Vec<String>>, empty: &str, t: Table<Msg>) -> Node<Msg> {
        if rows.is_empty() {
            let msg = if self.query.is_empty() {
                empty
            } else {
                "no matches"
            };
            text(msg).muted().into()
        } else {
            self.focusable(t)
        }
    }

    fn log_view(&self) -> Node<Msg> {
        let rows: Vec<Vec<String>> = self
            .visible(Tab::Log)
            .into_iter()
            .map(|i| {
                let c = &self.commits[i];
                vec![
                    c.short.clone(),
                    c.date.chars().take(10).collect(),
                    c.subject.clone(),
                ]
            })
            .collect();
        let t = table(
            &["Commit", "Date", "Subject"],
            rows.clone(),
            &[Col::Fixed(7), Col::Fixed(10), Col::Grow(1)],
            self.selected[0],
            Msg::Select,
        );
        let list = self.table_or_empty(rows, "no commits", t);
        row(
            "grow gap-1",
            [
                card("Commits", "grow-3", [list]),
                card("Detail", "grow-2 px-1", [self.detail_view()]),
            ],
        )
    }

    fn detail_view(&self) -> Node<Msg> {
        let Some(d) = &self.detail else {
            return text("no commit selected").muted().into();
        };
        let c = &d.commit;
        let mut kids = vec![
            heading(c.subject.clone()),
            text(format!("{}\n{}\n{}", c.short, c.author, c.date))
                .muted()
                .into(),
        ];
        if !c.body.trim().is_empty() {
            kids.push(text(c.body.trim()).into());
        }
        // Text height is its line count, not its wrapped height; `grow` gives the
        // last block the rest of the pane so a wrapped stat is not cut (FEEDBACK.md).
        kids.push(text(d.stat.trim()).class("grow text-success").into());
        col("gap-1", kids)
    }

    fn status_view(&self) -> Node<Msg> {
        let rows: Vec<Vec<String>> = self
            .visible(Tab::Status)
            .into_iter()
            .map(|i| {
                let f = &self.files[i];
                let path = match &f.orig_path {
                    Some(o) => format!("{o} → {}", f.path),
                    None => f.path.clone(),
                };
                vec![format!("{}{}", f.index, f.worktree), path]
            })
            .collect();
        let t = table(
            &["St", "Path"],
            rows.clone(),
            &[Col::Fixed(2), Col::Grow(1)],
            self.selected[1],
            Msg::Select,
        );
        card(
            "Changes",
            "grow",
            [self.table_or_empty(rows, "working tree clean", t)],
        )
    }

    fn branches_view(&self) -> Node<Msg> {
        let rows: Vec<Vec<String>> = self
            .visible(Tab::Branches)
            .into_iter()
            .map(|i| {
                let b = &self.branches[i];
                vec![
                    if b.current { "*" } else { "" }.to_owned(),
                    b.name.clone(),
                    b.upstream.clone().unwrap_or_default(),
                    b.short.clone(),
                    b.subject.clone(),
                ]
            })
            .collect();
        let t = table(
            &["", "Branch", "Upstream", "Tip", "Subject"],
            rows.clone(),
            &[
                Col::Fixed(1),
                Col::Grow(2),
                Col::Grow(2),
                Col::Fixed(7),
                Col::Grow(3),
            ],
            self.selected[2],
            Msg::Select,
        );
        card(
            "Branches",
            "grow",
            [self.table_or_empty(rows, "no branches", t)],
        )
    }

    fn help_view(&self) -> Node<Msg> {
        let lines: Vec<Node<Msg>> = HELP
            .iter()
            .map(|(k, d)| {
                row(
                    "gap-2",
                    [
                        text(*k).class("w-9 text-accent bold").into(),
                        text(*d).into(),
                    ],
                )
            })
            .collect();
        col("", lines)
    }
}

impl<R: GitRepo> App for Gitview<R> {
    type Msg = Msg;

    fn init(&mut self) -> Cmd<Msg> {
        match self.load() {
            Ok(()) => Cmd::none(),
            Err(e) => self.notify(Kind::Danger, e.to_string()),
        }
    }

    fn update(&mut self, msg: Msg) -> Cmd<Msg> {
        let n = Tab::ALL.len();
        match msg {
            Msg::Tab(i) => self.set_tab(i),
            Msg::NextTab => self.set_tab((self.tab.index() + 1) % n),
            Msg::PrevTab => self.set_tab((self.tab.index() + n - 1) % n),
            Msg::Select(i) => self.select(i),
            Msg::Down => {
                let len = self.visible(self.tab).len();
                let sel = self.selected[self.tab.index()];
                self.select((sel + 1).min(len.saturating_sub(1)));
            }
            Msg::Up => self.select(self.selected[self.tab.index()].saturating_sub(1)),
            Msg::FilterOpen => {
                self.filtering = true;
                self.cursor = Cursor::default();
            }
            Msg::Query(q) => {
                self.query = q;
                self.select(0);
            }
            Msg::FilterDone => self.filtering = false,
            Msg::FilterClear => {
                self.filtering = false;
                self.query.clear();
                self.select(0);
            }
            Msg::Help => self.help = true,
            Msg::CloseHelp => self.help = false,
            Msg::Refresh => {
                return match self.load() {
                    Ok(()) => {
                        let msg = format!(
                            "refreshed: {} commits, {} changes",
                            self.commits.len(),
                            self.files.len()
                        );
                        self.notify(Kind::Success, msg)
                    }
                    Err(e) => self.notify(Kind::Danger, e.to_string()),
                };
            }
            Msg::Theme => {
                self.theme = (self.theme + 1) % Theme::names().len();
                return Cmd::SetTheme(self.theme_name().to_owned());
            }
            Msg::Expire => {
                if !self.toasts.is_empty() {
                    self.toasts.remove(0);
                }
            }
            Msg::Quit => return Cmd::Quit,
        }
        Cmd::none()
    }

    fn view(&self) -> Node<Msg> {
        let body = match self.tab {
            Tab::Log => self.log_view(),
            Tab::Status => self.status_view(),
            Tab::Branches => self.branches_view(),
        };
        let shown = self.visible(self.tab).len();
        let total = self.rows_len(self.tab);
        let mut kids = vec![
            tabs(&Tab::TITLES, self.tab.index(), Msg::Tab),
            self.filter_bar(),
            col("grow px-1", [body]),
            statusbar(
                format!("{} · {}", self.head, self.theme_name()),
                format!("{shown}/{total}"),
            ),
            keyhints(&self.keys()),
        ];
        if self.help {
            kids.push(modal("Keys", self.help_view(), Msg::CloseHelp));
        }
        // Overlays last so they paint over the page.
        kids.push(toasts(&self.toasts));
        col("", kids)
    }

    fn keys(&self) -> Keymap<Msg> {
        Keymap::new()
            .bind("/", "filter", Msg::FilterOpen)
            .bind("?", "help", Msg::Help)
            .bind("r", "refresh", Msg::Refresh)
            .bind("t", "theme", Msg::Theme)
            .bind("q", "quit", Msg::Quit)
            .ambient("1", Msg::Tab(0))
            .ambient("2", Msg::Tab(1))
            .ambient("3", Msg::Tab(2))
            .ambient("right", Msg::NextTab)
            .ambient("left", Msg::PrevTab)
            .ambient("down", Msg::Down)
            .ambient("up", Msg::Up)
            .ambient("esc", Msg::FilterClear)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::fake::FakeGit;
    use crate::git::{Head, Result};

    fn app() -> Gitview<FakeGit> {
        let mut a = Gitview::new(FakeGit::default());
        assert_eq!(a.init(), Cmd::None);
        a
    }

    #[test]
    fn init_loads_everything_and_selects_first_commit() {
        let a = app();
        assert_eq!(a.head, "main");
        assert_eq!(
            (a.commits.len(), a.files.len(), a.branches.len()),
            (3, 4, 3)
        );
        assert_eq!(a.detail.as_ref().unwrap().commit.subject, "Add parser");
    }

    #[test]
    fn tabs_wrap_and_clear_filter() {
        let mut a = app();
        a.update(Msg::PrevTab);
        assert_eq!(a.tab, Tab::Branches);
        a.update(Msg::Query("x".into()));
        a.update(Msg::NextTab);
        assert_eq!((a.tab, a.query.as_str()), (Tab::Log, ""));
        a.update(Msg::Tab(9));
        assert_eq!(a.tab, Tab::Branches, "out of range clamps");
    }

    #[test]
    fn moving_clamps_and_updates_detail() {
        let mut a = app();
        for _ in 0..5 {
            a.update(Msg::Down);
        }
        assert_eq!(a.selected[0], 2);
        assert_eq!(a.detail.as_ref().unwrap().commit.subject, "Initial commit");
        a.update(Msg::Up);
        a.update(Msg::Select(0));
        a.update(Msg::Up);
        assert_eq!(a.selected[0], 0);
    }

    #[test]
    fn query_filters_current_tab_and_resets_selection() {
        let mut a = app();
        a.update(Msg::Down);
        a.update(Msg::FilterOpen);
        a.update(Msg::Query("types".into()));
        assert_eq!(a.visible(Tab::Log), vec![1]);
        assert_eq!(a.selected[0], 0);
        assert_eq!(a.detail.as_ref().unwrap().commit.subject, "Add types");
        assert_eq!(a.visible(Tab::Status).len(), 4, "other tabs unfiltered");
        a.update(Msg::FilterDone);
        assert!(!a.filtering && a.query == "types");
        a.update(Msg::FilterClear);
        assert!(a.query.is_empty());
        assert_eq!(a.visible(Tab::Log).len(), 3);
    }

    #[test]
    fn no_match_clears_detail() {
        let mut a = app();
        a.update(Msg::Query("zzz".into()));
        assert!(a.visible(Tab::Log).is_empty());
        assert!(a.detail.is_none());
        a.update(Msg::Down); // no panic on empty
    }

    #[test]
    fn theme_cycles_through_all_names() {
        let mut a = app().with_theme("warm");
        let Cmd::SetTheme(next) = a.update(Msg::Theme) else {
            panic!("expected SetTheme")
        };
        let names = Theme::names();
        let warm = names.iter().position(|n| *n == "warm").unwrap();
        assert_eq!(next, names[(warm + 1) % names.len()]);
        for _ in 1..names.len() {
            a.update(Msg::Theme);
        }
        assert_eq!(a.theme_name(), "warm");
    }

    #[test]
    fn refresh_toasts_and_expire_removes_oldest() {
        let mut a = app();
        assert_eq!(a.update(Msg::Refresh), Cmd::tick(TOAST_MS, Msg::Expire));
        a.update(Msg::Refresh);
        assert_eq!(a.toasts.len(), 2);
        assert_eq!(a.toasts[0].0, Kind::Success);
        a.update(Msg::Expire);
        a.update(Msg::Expire);
        a.update(Msg::Expire);
        assert!(a.toasts.is_empty());
    }

    #[test]
    fn help_and_quit() {
        let mut a = app();
        a.update(Msg::Help);
        assert!(a.help);
        a.update(Msg::CloseHelp);
        assert!(!a.help);
        assert_eq!(a.update(Msg::Quit), Cmd::Quit);
    }

    #[test]
    fn every_hinted_key_is_in_help() {
        for (chord, _) in app().keys().hints() {
            assert!(
                HELP.iter().any(|(k, _)| *k == chord),
                "{chord} missing from HELP"
            );
        }
    }

    struct Broken;
    impl GitRepo for Broken {
        fn log(&self, _: usize) -> Result<Vec<Commit>> {
            Err(GitError::Failed {
                code: Some(128),
                stderr: "not a git repository".into(),
            })
        }
        fn show(&self, _: &str) -> Result<CommitDetail> {
            unreachable!()
        }
        fn status(&self) -> Result<Vec<FileStatus>> {
            Ok(vec![])
        }
        fn branches(&self) -> Result<Vec<Branch>> {
            Ok(vec![])
        }
        fn head(&self) -> Result<Head> {
            Ok(Head::Detached("abc1234".into()))
        }
    }

    #[test]
    fn git_errors_become_danger_toasts() {
        let mut a = Gitview::new(Broken);
        assert_eq!(a.init(), Cmd::tick(TOAST_MS, Msg::Expire));
        assert_eq!(a.toasts[0].0, Kind::Danger);
        assert!(a.toasts[0].1.contains("not a git repository"));
    }
}
