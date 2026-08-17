# leetctl

A LeetCode client for the terminal: pick a problem, solve it in your editor, test and submit.
Eleven curated problem sets are compiled into the binary so filtering by set needs no network call.

## Build & test

```sh
cargo build
cargo test                            # what CI runs, as `cargo nextest run --release --all-features`
cargo test --features pym             # the pyo3 `--plan` path; needs a Python dev install
cargo clippy --all-targets
cargo fmt --check                     # CI fails on formatting
python3 scripts/gen_sets.py           # regenerate data/sets/*.toml (stdlib only)
```

CI (`.github/workflows/rust.yml`) runs the suite with `--all-features` on macOS and Linux, so a
change that only compiles without `pym` will fail there.

## Layout

| Path | Holds |
| --- | --- |
| `src/cli.rs` | clap parser and the single subcommand dispatch |
| `src/cmd/` | one module per subcommand; `mod.rs` has the shared `populated_problems` |
| `src/cache/` | SQLite (diesel) cache wrapping the API client: `mod.rs` operations, `models.rs` types, `parser.rs` JSON → model |
| `src/plugins/` | `leetcode.rs` HTTP/GraphQL client, `chrome.rs` cookie extraction |
| `src/sets/` | compile-time curated sets; `REGISTRY` maps slug → `include_str!`'d TOML |
| `src/helper.rs` | filter primitives, `Difficulty`, HTML → text, path builders |
| `data/sets/` | generated set files — never hand-edit |

## In flight: the terminal UI

A ratatui frontend (`leetctl tui`) is being added in phases, tracked in
[docs/tui.md](./docs/tui.md#roadmap). It shares the library layer with the CLI, which imposes rules
the CLI alone did not need. Breaking one produces a corrupted screen or a database write from a
render path — neither shows up as a test failure.

- **The cache layer does not print.** `src/cache/` and `src/plugins/` return values; commands do the
  printing. A `println!` there lands on top of the alt screen.
- **`Display` impls have no side effects.** Formatting must stay pure and panic-free; the TUI
  formats results while rendering.
- **Filtering has one implementation**, shared by `leetctl list`, `--stat`, and the TUI. Adding a
  filter to only one of them is a divergence, not a feature.
- **The TUI loop never blocks on I/O.** Backend work goes through `Handle::spawn` and returns as a
  `Msg` on the channel; results carry a generation counter and stale ones are dropped. See
  [docs/tui.md](./docs/tui.md#threading-contract).

Phases 1 and 2 refactor the existing commands behind these rules and must not change their output.

## Docs

[configuration](./docs/configuration.md) · [cookies](./docs/cookies.md) ·
[editors](./docs/editors.md) · [scripting](./docs/scripting.md) · [sets](./docs/sets.md) ·
[tui](./docs/tui.md)
