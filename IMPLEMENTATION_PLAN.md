# leetcode-rs / leetctl — Implementation Plan

Fork of [clearloop/leetcode-cli](https://github.com/clearloop/leetcode-cli) (MIT), rebranded to
`leetctl`, plus two new capabilities: filtered random picking, and curated problem sets.

| | |
| --- | --- |
| crates.io package | `leetctl` (verified available 2026-08-17; `leetcode-rs`/`leet-rs` are taken) |
| binary | `leetctl` |
| GitHub repo | `siansiansu/leetcode-rs` |
| config root | `~/.leetcode` (unchanged — drop-in with upstream config/cache) |

---

## Stage 1 — Migration & rebrand — ✅ Done

**Goal**: upstream source tree lives here under its own identity, compiling clean.

- Copied `src/`, `tests/`, `docs/`, `.github/`, `flake.nix`, `rust-toolchain.toml`, `rustfmt.toml`, `Cargo.lock`.
- `Cargo.toml`: package `leetctl` v0.1.0, bin `leetctl` (`src/bin/leetctl.rs`), own author/repo/homepage/keywords.
- All `leetcode_cli` / `leetcode-cli` / `clearloop` references rewritten; CLI invocations in docs and
  `after_help` strings now read `leetctl <cmd>`.
- `LICENSE`: retains clearloop's 2019 MIT copyright alongside ours, as MIT requires for a fork.
- `docs/editors.md` links back to the upstream issue it actually cites.

**Success criteria**: `cargo check --all-targets` clean. ✅ (exit 0)

---

## Stage 2 — Problem-set data layer

**Goal**: curated interview lists available offline, regeneratable from authoritative sources.

### Sources (all verified reachable & unauthenticated on 2026-08-17)

| Set slug | Name | Count | Source | License |
| --- | --- | --- | --- | --- |
| `blind75` | Blind 75 | 75 | `neetcode-gh/leetcode` → `.problemSiteData.json` | MIT |
| `neetcode150` | NeetCode 150 | 150 | same | MIT |
| `neetcode-all` | NeetCode All | 450 | same (every entry) | MIT |
| `top-interview-150` | LeetCode Top Interview 150 | 150 | LeetCode GraphQL `studyPlanV2Detail` | LeetCode |
| `top-100-liked` | LeetCode Top 100 Liked | 100 | same | LeetCode |
| `leetcode-75` | LeetCode 75 | 75 | same | LeetCode |
| `google` | Google | 488 | `hxu296/leetcode-company-wise-problems-2022` | MIT |
| `facebook` | Meta / Facebook | 371 | same | MIT |
| `amazon` | Amazon | 592 | same | MIT |
| `microsoft` | Microsoft | 363 | same | MIT |

**NeetCode 250 is not obtainable.** `.problemSiteData.json` is NeetCode's own data file and carries
only `blind75` and `neetcode150` membership flags across its 450 entries — there is no 250 flag and
no public API exposing one (`neetcode.io/api/problems` serves the SPA shell, not JSON). Shipping
`neetcode-all` (450) in its place; noted for the user rather than silently substituted.

**Company data is 2022-vintage community-scraped frequency data**, not live LeetCode company tags
(those are Premium-gated). Each company set records its vintage and is labelled stale in
`leetctl sets` output and in `docs/sets.md`.

### Identifier resolution

Sets store `fid` (LeetCode frontend id) — the key `Problem` is filtered on. Sources give slugs, not
always ids (company CSVs give only a URL; one NeetCode `code` value is malformed: `135-candy`).
So the generator resolves **slug → fid** through `https://leetcode.com/api/problems/all/`
(4028 problems, `stat.question__title_slug` → `stat.frontend_question_id`) rather than parsing ids
out of source-specific fields. Unresolvable slugs are reported and skipped, never guessed.

### Artifacts

```
data/sets/<slug>.toml     # committed, one file per set
scripts/gen_sets.py       # regenerates all of the above from source
src/sets/mod.rs           # registry + lookup
docs/sets.md              # provenance, licensing, staleness, regeneration
```

`data/sets/<slug>.toml` shape:

```toml
slug = "blind75"
name = "Blind 75"
description = "The original Blind 75 list, as tracked by NeetCode."
source_url = "https://github.com/neetcode-gh/leetcode"
source_license = "MIT"
generated_at = "2026-08-17"
stale = false            # true for the 2022 company lists

[[problems]]
fid = 217
slug = "contains-duplicate"
```

`src/sets/mod.rs`:

- `const REGISTRY: &[(&str, &str)]` — slug → `include_str!()` of its TOML. Const, so the set of
  valid slugs is known at compile time and can drive clap's `PossibleValuesParser` and shell
  completions.
- `pub fn slugs() -> Vec<&'static str>` — derived from `REGISTRY`, no second hand-maintained list.
- `pub fn get(slug: &str) -> Result<ProblemSet>` — parses **only** the requested set.
- `pub fn all_meta() -> Result<Vec<SetMeta>>` — for `leetctl sets`.

**Success criteria**: `leetctl sets` prints all 10 with correct counts; a unit test parses every
embedded TOML and asserts non-empty, unique fids, and count matching a per-set expected value.

---

## Stage 3 — Filtered random pick

**Goal**: `leetctl pick` picks at random from a set narrowed by topic tag, difficulty, and problem set.

`pick` is *already* random when no `id`/`--name`/`--daily` is given (`pick.rs:87-92`), and already
supports `--tag` (topic tag, via the LeetCode tag API + local `tags` cache) and `--query`. The gap is
difficulty as a first-class flag, problem sets, and discoverability. So:

| Flag | Behavior |
| --- | --- |
| `-t, --tag <slug>` | already exists — topic tag (`array`, `dynamic-programming`, …) |
| `-D, --difficulty <easy\|medium\|hard>` | new — clap `ValueEnum`, filters on `Problem::level` |
| `-S, --set <slug>` | new — restrict to a curated set; values from `sets::slugs()` |

No `--random` flag: random is what `pick` already does with no target argument, and a flag that
turns on the default behavior is redundant surface. Documented in `after_help` instead, which `pick`
currently lacks entirely.

`--difficulty` maps to `Difficulty::level() -> i32` and a single `retain` on `Problem::level`; it does
not build a synthetic `--query` string. `--query`'s existing `e`/`m`/`h` chars keep working — the two
compose (both are `retain`s over the same vec).

`--set` also lands on `list`, so `leetctl list --set blind75 --stat` reports progress through a set.
Same `sets::get()` call, no duplicated filtering logic.

New `leetctl sets` subcommand: table of slug / name / count / source, with stale ones flagged.

**Success criteria**:
- `leetctl pick --set blind75 --difficulty medium --tag dynamic-programming` picks only from that
  intersection, and picks a different problem across repeated runs.
- `leetctl list --set neetcode150 --stat` shows counts summing to the set's size (minus any not in cache).
- `leetctl pick --set bogus` fails with the valid slugs listed, before touching the network.
- Unit tests for the difficulty/set filter predicates over a fixture `Vec<Problem>`.

---

## Stage 4 — Docs

**Goal**: the two new capabilities are documented where a reader looks for them.

- `README.md`: rewrite for `leetctl` — install, quickstart, command table incl. `sets`, upstream credit.
- `docs/sets.md`: the provenance table above, staleness caveats, `scripts/gen_sets.py` usage,
  how to propose a new set.
- `docs/configuration.md`, `docs/cookies.md`, `docs/editors.md`, `docs/scripting.md`: already
  rebranded in Stage 1; re-read for anything that reads wrong under the new name.

**Success criteria**: `cargo test` green (README is `include_str!`'d as crate docs, so its fenced
blocks must not break doctests), and `leetctl --help` matches what README claims.

---

## Out of the three-stage spine

- Premium problems (8 in the NeetCode list) exist in the sets but are `locked` in cache; `-q L`
  filters them. Documented, not special-cased.
- Company sets are large (up to 592) and frequency-ordered upstream; sets store membership only,
  because `pick` samples uniformly and `list` sorts by fid. Frequency is not carried.
