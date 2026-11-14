# gitview

A small read-only git browser for the terminal, built with
[`cascivo-tui`](https://crates.io/crates/cascivo-tui). It shows the log, working-tree
status and branches of a repository, with fuzzy filtering, a help overlay and theme
cycling. It is a showcase: the code is short on purpose so you can copy its structure.

```text
 Log   Status   Branches
 / press / to filter
 ╭ Commits ───────────────────────────────────╮ ╭ Detail ─────────────────────╮
 │ Commit  Date       Subject                 │ │ Add parser                  │
 │ 0000003 2026-01-03 Add parser              │ │                             │
 │ 0000002 2026-01-02 Add types               │ │ 0000003                     │
 │ 0000001 2026-01-01 Initial commit          │ │ Ada Lovelace                │
 │                                            │ │ 2026-01-03T12:00:00+00:00   │
 │                                            │ │                             │
 │                                            │ │ Parses porcelain output.    │
 │                                            │ │                             │
 │                                            │ │ Includes renames.           │
 │                                            │ │                             │
 │                                            │ │ src/lib.rs | 2 +-           │
 │                                            │ │ 1 file changed, 1           │
 │                                            │ │ insertion(+), 1 deletion(-) │
 │                                            │ │                             │
 │                                            │ │                             │
 │                                            │ │                             │
 │                                            │ │                             │
 │                                            │ │                             │
 ╰────────────────────────────────────────────╯ ╰─────────────────────────────╯
 main · terminal                                                            3/3
/ filter · ? help · r refresh · t theme · q quit · tab focus
```

<sub>Screenshot is `tests/snapshots/log.txt`; `cargo test` checks it
(`UPDATE_SNAPSHOTS=1 cargo test` regenerates it).</sub>

## Keys

| Key | Action |
| --- | --- |
| `1` `2` `3` | switch tab (Log, Status, Branches) |
| `←` `→` | previous / next tab |
| `j` `k` `↑` `↓` | move selection |
| `/` | fuzzy filter the list |
| `enter` | keep filter, back to list |
| `esc` | clear filter / close help |
| `?` | help |
| `r` | refresh from git |
| `t` | next theme |
| `q` | quit |

## Run

```sh
cargo run              # inside any git repo
cargo run -- path/to/repo
cargo test
```

Needs `git` on `PATH`. Rust 1.88 or newer.

## How it's built

- **`App` trait**: `init`, `update(msg) -> Cmd`, `view` and `keys` (a `Keymap` of
  bindings) live in `src/app.rs`. Side effects are returned as `Cmd` values.
- **Layout** is plain class strings, e.g. `row("h-1 gap-1", ..)` and `grow`.
- **`GitRepo` trait** (`src/git.rs`) hides git. `CliGit` shells out to `git`;
  `FakeGit` returns fixed data so tests are deterministic.
- **`TestApp` snapshots** (`tests/ui.rs`) drive the app with keys and compare the
  rendered screen to `tests/snapshots/*.txt`.

See the [cascivo-tui guide](https://github.com/cascivo/cascivo/blob/main/docs/USING-CASCIVO-TUI.md)
and [FEEDBACK.md](FEEDBACK.md) for what was awkward while building this.

## License

MIT
