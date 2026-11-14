# cascivo-tui feedback (from building gitview on 0.1.0)

Things that got in the way, with the local workaround used in this repo.

## 1. No way to move focus from `update`

- **Symptom:** `/` should put the cursor in the filter input. `Cmd` has no focus variant,
  and `Runtime::focus` is private, so an app cannot focus an id.
- **Repro:** bind `"/"` to a message that adds `input(..).id("filter")` to the view. Focus
  stays on the list (`Focus::sync` keeps the current id while it is still in the ring).
- **Workaround:** while filtering, the list is built without `.id(..)`, so it leaves the
  ring and `Focus::sync` falls through to `"filter"` (`src/app.rs`, `Gitview::focusable`).
  This works but depends on undocumented `sync` behaviour.
- **Suggested API:** `Cmd::Focus(String)` (and maybe `Cmd::FocusNext`/`FocusPrev`).

## 2. Text height ignores wrapping

- **Symptom:** a long line in `text(..)` wraps at render time, but its intrinsic height is
  `lines().count()`, so the wrapped tail is cut off.
- **Repro:** `col("", [text("one two three four").into(), text("x").into()])` rendered
  6 columns wide shows only `one`.
- **Workaround:** give the last text block `grow` so it takes the rest of the pane
  (`detail_view`).
- **Suggested API:** measure wrapped height against the cross-axis size, or document the
  limit next to `wrap`.

## 3. A `row` without a size steals free height

- **Symptom:** a one-line `row` between the tabs and a `grow` body took half the
  leftover height, pushing the body down by ten lines.
- **Cause:** documented ("a container fills the remaining space"), but easy to miss; a row
  of leaves usually wants its intrinsic height.
- **Workaround:** `row("h-1 ..", ..)`.
- **Suggested API:** give containers without `grow` their intrinsic size, or call this
  out in the Layout section with an example.

## 4. No effect command for I/O

- **Symptom:** loading data has to happen synchronously inside `init`/`update`; there is no
  `Cmd` that runs a closure and feeds its result back as a message.
- **Workaround:** git calls go through the `GitRepo` trait inside `update`. Fine for a
  local repo, but a slow call freezes the UI.
- **Suggested API:** `Cmd::Task(Box<dyn FnOnce() -> M + Send>)` run off the UI thread
  (TestApp could run it inline to stay deterministic).

## 5. The app cannot learn the starting theme

- **Symptom:** to cycle themes with `Cmd::SetTheme`, the app tracks an index. If the
  runtime starts from `CASCIVO_THEME`, the app's index is out of step.
- **Workaround:** `main` reads `CASCIVO_THEME` itself and passes the same name to both the
  app (`with_theme`) and `RunOptions`.
- **Suggested API:** `Cmd::NextTheme` / `Cmd::PrevTheme`, or pass the active theme name to
  `view`/`update`.

## 6. Minor

- `input` renders with a leading space (`" héllo"`), so a prefix label plus `gap-1`
  shows two spaces. A `px-0`-style override or no built-in padding would help.
- `TestApp` has no snapshot helper; `tests/ui.rs` has a ~18-line one
  (`UPDATE_SNAPSHOTS=1` to accept). A small `assert_screen(name)` in the crate, or a
  documented `insta` recipe, would save every adopter writing it.
