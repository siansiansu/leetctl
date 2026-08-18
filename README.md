# leetctl

[![build](https://github.com/siansiansu/leetctl/workflows/leetctl/badge.svg)](https://github.com/siansiansu/leetctl/actions)
[![crate](https://img.shields.io/crates/v/leetctl.svg)](https://crates.io/crates/leetctl)
[![doc](https://img.shields.io/badge/current-docs-brightgreen.svg)](https://docs.rs/leetctl/)
[![LICENSE](https://img.shields.io/crates/l/leetctl.svg)](https://choosealicense.com/licenses/mit/)

LeetCode from the command line. Pick a problem, solve it in your editor, test and submit — without
opening a browser.

Beyond the usual: **eleven curated problem sets are compiled into the binary**, so you can draw a
random problem from Blind 75, NeetCode 250, or a company list, narrowed by topic and difficulty:

```sh
leetctl pick -S blind75 -D medium              # random medium problem from Blind 75
leetctl pick -t dynamic-programming -D hard    # random hard DP problem
leetctl list -S neetcode150 --stat             # progress through NeetCode 150
```

No network call and no LeetCode Premium subscription is needed to filter by a set.

## Install

```sh
# Linux build deps: system SQLite + OpenSSL headers + pkg-config
#   (Debian/Ubuntu: libsqlite3-dev libssl-dev pkg-config). macOS ships both.
cargo install leetctl
```

Python filtering scripts (the `--plan` flag) require the optional `pym` feature:

```sh
cargo install leetctl --features pym
```

Nix users can `nix build` / `nix develop` against the bundled [`flake.nix`](./flake.nix).

## Quickstart

Sign in to LeetCode in **Chrome** first — on macOS and Linux leetctl reads its cookies
automatically (on Windows, set them manually). See [Cookies](./docs/cookies.md) for manual setup
and environment-variable overrides.

```sh
leetctl pick 1        # print a problem's description
leetctl edit 1        # open the solution file in your editor
leetctl test 1        # run the sample test cases
leetctl exec 1        # submit the solution
```

Run `leetctl --help` (or `leetctl <command> --help`) for the full, always-current list of commands
and flags. The headline ones:

| Command | Alias | What it does |
| --- | --- | --- |
| `pick` | `p` | Pick a random problem, or one by id, `--name`, or `--daily`. Narrow the pool with `--set`, `--tag`, `--difficulty`, `--query` |
| `edit` | `e` | Open a problem's code file; `--lang` overrides the configured language, `--daily` opens today's challenge |
| `test` | `t` | Run test cases; `--watch` re-runs on save, `--daily` targets today's challenge |
| `exec` | `x` | Submit the solution |
| `list` | `l` | List/filter problems by set, category, tag, difficulty, id range, or `--query` |
| `tui` | | Browse, filter, read, test and submit in a terminal UI; `--set` / `--difficulty` open pre-filtered |
| `sets` | | Show the bundled problem sets; `--sources` adds provenance |
| `stat` | `s` | Show a chart of your submissions |
| `data` | `d` | Manage the local cache (`--update`, `--delete`) |
| `completions` | `c` | Generate shell completions (`bash`, `elvish`, `fish`, `powershell`, `zsh`) |

### The terminal UI

```sh
leetctl tui                      # every problem
leetctl tui -S blind75 -D medium # opens pre-filtered
```

One table of every problem with live filtering (`/` search, `s` set, `d` difficulty, `u` unsolved,
`t` tag), the description on `enter`, today's challenge on `D`, and `e` / `t` / `S` to edit, test and
submit without leaving the screen. `?` lists every key. Filters and progress counts come from the
same engine as `leetctl list`, so the footer matches `leetctl list --stat`.

Full reference, including the threading contract: [Terminal UI](./docs/tui.md).

### Picking a problem

`pick` is random by default — give it no target and it draws one problem from everything in your
cache. Filters narrow what it draws from:

```sh
leetctl pick                                  # anything
leetctl pick -D medium                        # any medium problem
leetctl pick -t graph -D hard                 # hard graph problem
leetctl pick -S google -q DL                  # from the Google set: unsolved, not premium-locked
```

Filters compose, and `-q`'s flags cover status as well as difficulty (`d` done, `D` not done,
`l` locked, `L` not locked, `s`/`S` starred). If a combination matches nothing, leetctl says which
filters produced the empty set instead of failing obscurely.

Naming a problem outright — `leetctl pick 1` or `leetctl pick --daily` — is a different thing from
drawing one at random, so it cannot be combined with filters; leetctl rejects the combination
rather than accepting filters it would then ignore.

### Spaced repetition

Solving a problem once teaches you the trick; solving it again three weeks later is what keeps it.
An accepted submission puts the problem in a review deck and schedules it, and `leetctl review`
tells you what has come back around.

```sh
leetctl review                 # what is due today
leetctl review next            # open the most overdue one
leetctl review grade 1 hard    # "I got it, but it hurt"
leetctl list --due -S blind75  # due problems from the Blind 75
```

The schedule is SM-2, the algorithm Anki grew out of: four grades, an ease per problem, and
intervals that stretch as long as you keep recalling it. In the TUI, `r` narrows the table to what
is due and `1`-`4` grade the problem on screen. Full reference: [Spaced repetition](./docs/srs.md).

### Problem sets

```sh
leetctl sets              # slug, name, size
leetctl sets --sources    # plus where each came from and how old the data is
```

| Slug | Problems | Source |
| --- | ---: | --- |
| `blind75` | 75 | NeetCode |
| `neetcode150` | 150 | NeetCode |
| `neetcode250` | 250 | NeetCode |
| `neetcode-all` | 450 | NeetCode |
| `top-interview-150` | 150 | LeetCode study plan |
| `top-100-liked` | 100 | LeetCode study plan |
| `leetcode-75` | 75 | LeetCode study plan |
| `google` | 488 | Community snapshot, 2022 |
| `facebook` | 371 | Community snapshot, 2022 |
| `amazon` | 592 | Community snapshot, 2022 |
| `microsoft` | 363 | Community snapshot, 2022 |

The company sets are a community-collected **2022** snapshot, not live LeetCode company tags (those
are Premium-gated) — treat them accordingly. Full provenance, licensing, and how to regenerate or
add a set: [Problem sets](./docs/sets.md).

<details>
<summary>Shell completions</summary>

By default the shell is inferred from `$SHELL`:

```sh
eval "$(leetctl completions)"
```

Copy that line into `.bash_profile` or `.zshrc`. Pass a shell explicitly to target another:

```sh
leetctl completions fish
```

</details>

## Documentation

- [Problem sets](./docs/sets.md) — what ships, where it came from, and how to regenerate it.
- [Configuration](./docs/configuration.md) — the full `leetcode.toml` reference: editor, code
  generation, filename templates, and storage paths.
- [Cookies](./docs/cookies.md) — automatic Chrome cookies, manual setup from any browser,
  `leetcode.cn` support, and environment-variable overrides.
- [Editors & LSP](./docs/editors.md) — getting rust-analyzer (and other language servers) working
  with generated solution files.
- [Spaced repetition](./docs/srs.md) — the `leetctl review` deck: the SM-2 schedule, the commands,
  and how submissions grade themselves.
- [Scripting](./docs/scripting.md) — filtering problems with custom Python plans.

## Development

`make` on its own prints the target list; there is nothing in it a plain `cargo` command could not
do, it just saves remembering which flags CI uses.

```sh
make            # the target list
make check      # fmt-check + clippy + tests — everything CI gates on
make run ARGS="review --all"
```

| Target | Does |
| --- | --- |
| `build` / `release` | debug or optimised build |
| `install` | `cargo install --path .` into `~/.cargo/bin` |
| `run` | run the debug binary, arguments in `ARGS` |
| `test` | the test suite |
| `test-pym` | plus the optional pyo3 `--plan` path (needs a Python dev install) |
| `test-ci` | exactly what CI runs, via `cargo-nextest` |
| `lint` / `fmt` / `fmt-check` | clippy with warnings denied; format; fail on unformatted |
| `check` | `fmt-check` + `lint` + `test` |
| `sets` | regenerate `data/sets/*.toml` — see [Problem sets](./docs/sets.md) |
| `doc` | build and open the API docs |
| `clean` | remove `target/` |

CI runs the suite with `--all-features` on macOS and Linux, so a change that only compiles without
`pym` fails there — `make test-pym` catches it locally.

## Credits

leetctl is a fork of [clearloop/leetcode-cli](https://github.com/clearloop/leetcode-cli), which is
where everything except the problem sets and the filtered `pick` comes from. It keeps the same
`~/.leetcode` config and cache layout, so an existing leetcode-cli setup carries over unchanged.

Problem set data comes from [neetcode-gh/leetcode](https://github.com/neetcode-gh/leetcode) (MIT),
[neetcode.io](https://neetcode.io/practice) for the NeetCode 250 membership,
[hxu296/leetcode-company-wise-problems-2022](https://github.com/hxu296/leetcode-company-wise-problems-2022)
(MIT), and LeetCode's public study-plan endpoints. Only problem identifiers are stored, never
problem text.

## License

MIT
