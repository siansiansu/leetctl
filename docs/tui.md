# Terminal UI

> **Being built.** This is the design document for `leetctl tui`, written before the code. The
> [roadmap](#roadmap) below tracks which phases have landed; anything in a pending phase does not
> exist yet. Sections describing behavior are the contract the implementation is held to, not a
> record of what ships today.

`leetctl tui` opens an interactive problem browser: one table of every problem, live filtering by
set / difficulty / tag / free text, the problem description inline, and edit / test / submit without
leaving the screen.

```sh
leetctl tui                          # full catalog
leetctl tui --set blind75            # open pre-filtered
leetctl tui -S neetcode150 -D medium
```

Everything it shows comes from the same cache and the same filter engine as `leetctl list`, so a
count in the TUI footer matches `leetctl list --stat` for the equivalent flags. That is a design
constraint, not a coincidence — see [One filter engine](#one-filter-engine).

## Keys

| Key | Where | Does |
| --- | --- | --- |
| `j` / `k`, `↓` / `↑` | list, detail | move / scroll |
| `g` `g`, `G` | list, detail | top / bottom |
| `Ctrl-d` / `Ctrl-u` | list, detail | page down / up |
| `Enter` | list | open the description |
| `Esc` | detail, overlays | back |
| `/` | list | filter prompt (token substring, `!token` negates) |
| `s` | list | set picker |
| `d` | list | cycle difficulty: all → easy → medium → hard |
| `u` | list | toggle unsolved-only |
| `t` | list | filter by tag |
| `D` | list | jump to today's daily challenge |
| `e` | list, detail | open the solution file in `$EDITOR` |
| `t` | detail | run the sample tests |
| `S` | detail | submit (asks first) |
| `?` | anywhere | help |
| `q` | list | quit |

`e` prepares the solution file exactly as `leetctl edit` does, including the language markers and
the test-case file, then hands the terminal to your editor. Editor resolution is unchanged —
`$VISUAL`, then `$EDITOR`, then `code.editor` from the config. See [editors](./editors.md).

## Architecture

### The loop

One `std::sync::mpsc` channel is the only place events converge. Three producers:

- an **input thread** turning crossterm key events into `Msg::Key`,
- **tokio tasks** for anything touching the cache or the network,
- a **spinner ticker**, alive only while a test or submit is in flight.

The UI thread does `draw` → `recv` → `handle` → drain the queue, and never blocks on I/O. State
lives in one `Model`; every transition goes through `Model::handle(Msg)`, which makes the whole
frontend testable without a terminal. This is the pattern smew uses (`src/tui/app/run.rs`).

### Threading contract

The runtime lives where it always did — `src/bin/leetctl.rs`. `TuiArgs::run` takes a
`tokio::runtime::Handle`, then puts the entire synchronous UI loop on a blocking-pool thread with
`spawn_blocking`. Backend work is `handle.spawn(async move { … tx.send(Msg::…) })`; results carry a
generation counter, and results from a superseded generation are dropped rather than applied.

Diesel is synchronous. Its calls ride inside those spawned tasks, on runtime worker threads — never
on the UI thread. They are single-table reads against a local SQLite file, so there is no
`spawn_blocking` wrapper per query; the thing that matters is that the render loop only ever
receives messages.

### Suspending for the editor

`e` cannot just spawn the editor: two readers on the same tty fight over keystrokes. So the UI
thread sets a `suspended` flag, the input thread parks after its current 150 ms poll, the terminal
is restored (alt screen off, raw mode off), the editor runs to completion, and then the terminal is
re-initialized and the input thread unparked. The editor owns the terminal for as long as it is
open, which is why blocking the UI thread here is correct rather than a compromise.

### One filter engine

`ProblemFilters` + `filters::apply` in `src/filters.rs` is shared by `leetctl list`, `--stat`, and
the TUI. The TUI adds only the interactive text matcher on top of it. A filter that behaves
differently in the two frontends is therefore a bug in one shared function, not a divergence to
reconcile.

### Why the library layer changes first (phase 1)

Three things in the cache layer are fine for a CLI and fatal for a TUI, so they are fixed before any
TUI code is written:

- `Cache::get_question` prints a banner to stdout (`src/cache/mod.rs`). Any print lands on top of
  the alt screen. It moves to `Problem::banner()`, printed by the commands that want it.
- `VerifyResult`'s `Display` impl **writes to SQLite** (`Cache::new()` + `update_after_ac`, in
  `src/cache/models.rs`) on submit success, and `expect`s its way through failures. A render path
  must not write to a database, and must not panic. That write moves into `Cache::exec_problem`,
  where the submit actually completes.
- `conn()` panics when the database cannot be opened. It returns a `Result`.

Two invariants follow, and future work should keep them: **the cache layer does not print**, and
**`Display` impls have no side effects**.

`leetctl tui` also forces `colored` off process-wide, so any string it borrows from a `Display` impl
arrives free of ANSI escapes, and initializes the logger at `off` unless `--debug` is set — a stray
`info!` would otherwise paint over the screen on a cold cache.

## Deliberately not in v1

- **Watch mode.** `e` then `t` already closes the loop; `leetctl test --watch` stays a CLI feature.
- **Mouse, themes, configurable keys.** Fifteen keys and three screens do not need an indirection
  layer, and difficulty colors follow the CLI's existing green / yellow / red.
- **Category, id-range, and `--plan` filters.** They exist on `leetctl list` and are a poor fit for
  an interactive panel.
- **Language switching, starring, the stat chart, a settings screen.**

## Roadmap

| Phase | Scope | Status |
| ---: | --- | --- |
| 0 | This document | Merged |
| 1 | Print-free, panic-free, side-effect-free cache layer | Merged |
| 2 | Shared filter / stats engine; extract the code-file scaffold | Merged |
| 3 | `leetctl tui`, event loop, problem table | Pending |
| 4 | Filtering: text, set, difficulty, tag | Pending |
| 5 | Detail view, help, daily challenge | Pending |
| 6 | Edit, test, submit | Pending |

Phases 1 and 2 are behavior-preserving refactors of the existing commands: `leetctl list`, `pick`,
`edit`, `test`, and `exec` must produce identical output across them, with one documented exception
(the first-run "is on the run..." banner no longer appears mid-`test`, because the cache layer
stopped printing).

## Checking it by hand

The automated tests render each screen through ratatui's `TestBackend` and drive `Model::handle`
with message sequences, which covers layout and state but not the terminal itself. What needs a
human:

1. Delete the cache and start `leetctl tui` — the download runs with no log lines leaking onto the
   screen.
2. `/two !sum`, then `s` → `blind75`, then `d` twice: the footer counts match
   `leetctl list -S blind75 -D medium --stat`.
3. `Enter`, `D`, `?` — description, daily, help.
4. `e` with a terminal editor (`nvim`) and with a windowed one (`code -w`): keystrokes during the
   edit are not swallowed by the TUI, and the screen comes back intact.
5. `t` shows a spinner then a result; `S` asks first, and an accepted submission flips the row to
   solved immediately. Expired cookies show the error in full.
6. Resize to 20×10 without a panic. After `q`, after `Ctrl-C`, and after a forced panic, `stty -a`
   shows raw mode off.
