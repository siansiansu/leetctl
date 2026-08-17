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

## Stage 2 — Problem-set data layer — ✅ Done

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

Unresolvable slugs **fail generation** rather than being skipped, so a "Blind 75" holding 74
problems can never ship. `EXPECTED_COUNTS` pins every set's size in both the generator and the Rust
tests. `RENAMED_SLUGS` carries the seven problems LeetCode renamed since 2022, each verified against
the live index by frontend id.

**NeetCode 250 is not obtainable.** `.problemSiteData.json` is NeetCode's own data file and carries
only `blind75` and `neetcode150` membership flags across its 450 entries — there is no 250 flag and
no public API exposing one (`neetcode.io/api/problems` serves the SPA shell, not JSON). Shipping
`neetcode-all` (450) in its place; noted for the user rather than silently substituted.

**Company data is a 2022-vintage community-scraped snapshot of company tags**, not live LeetCode
company tags (those are Premium-gated), and not a ranked frequency list — upstream's occurrence
counts are not carried, since sampling is uniform. Each set records `source_as_of = "2022"`, which
`leetctl sets --sources` prints.

### Identifier resolution

Sets store `fid` (LeetCode frontend id) — the key `Problem` is filtered on. Sources give slugs, not
always ids (company CSVs give only a URL; one NeetCode `code` value is malformed: `135-candy`).
So the generator resolves **slug → fid** through `https://leetcode.com/api/problems/all/`
(4028 problems, `stat.question__title_slug` → `stat.frontend_question_id`) rather than parsing ids
out of source-specific fields. An unresolvable slug fails the run, and is never guessed at.

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
source_as_of = "2026-08-17"   # vintage of the data ("2022" for the company sets)
generated_at = "2026-08-17"   # when it was last fetched

problems = [
  { fid = 217, slug = "contains-duplicate" },
]
```

`src/sets/mod.rs`:

- `const REGISTRY: &[(&str, &str)]` — slug → `include_str!()` of its TOML. Being const, it compiles
  the accepted CLI spellings into the binary (driving clap's `PossibleValuesParser` and shell
  completions) and fails the build if a file is missing. It does **not** validate that the TOML
  parses, that the inner `slug` matches the key, or that counts are right — those are test-time
  checks in `src/sets/mod.rs`.
- `pub fn slugs() -> Vec<&'static str>` — derived from `REGISTRY`, no second hand-maintained list.
- `pub fn get(slug: &str) -> Result<ProblemSet>` — parses **only** the requested set.
- `pub fn all() -> Result<Vec<ProblemSet>>` — for `leetctl sets`.

**Success criteria**: `leetctl sets` prints all 10 with correct counts; a unit test parses every
embedded TOML and asserts non-empty, unique fids, and count matching a per-set expected value.

---

## Stage 3 — Filtered random pick — ✅ Done

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

New `leetctl sets` subcommand: slug / name / count, with `--sources` adding source URL,
licence, and data vintage.

**Success criteria**:
- `leetctl list --set blind75 --stat` reports `Listed: 75`. ✅ verified against a real cache
- `leetctl pick --set bogus` fails with the valid slugs listed, before touching the network. ✅
- `leetctl pick 1 --set blind75` is rejected rather than silently ignoring `--set`. ✅
- Deterministic unit tests for the filter predicates and clap parsing — no probabilistic assertion
  about repeated picks. ✅ 24 unit tests + 11 pre-existing

---

## Stage 4 — Docs — ✅ Done

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


---

## Design review — Codex pre-implementation pass

Run before Stage 2/3 code was written (`codex exec`, ANALYSIS-ONLY, against the plan plus
`pick.rs`/`list.rs`/`cli.rs`/`helper.rs`/`cache/`). Twelve findings, all adopted.

| # | Finding | Resolution |
| --- | --- | --- |
| 1 | **Critical** — an empty filtered pool panics at `pick.rs`'s `random_range(0..0)` (`rand-0.10.1/src/rng.rs:164` asserts `!range.is_empty()`). Pre-existing: `pick -q eh` triggers it today. | `candidates()` returns `Error::NoProblemsMatch` naming the active filters; never returns an empty vec. Covered by tests. |
| 2 | **Critical** — target precedence was silent: `pick 1 --set blind75` ignored `--set`, and `--daily` fetched over the network even when a name or id would win. | `id`, `--name`, `--daily` are mutually exclusive; filters conflict with `id` and `--daily`. `--name` still searches within the filtered pool. Clap rejects the rest. |
| 3 | High — skipping unresolvable slugs lets a short set ship. | Generator raises `GenerationError`; `EXPECTED_COUNTS` pins each size; verified with a negative control. |
| 4 | High — confirmed `fid` is the right key (`Problem::id` is LeetCode's internal id, which is what tag `squash` compares — `helper.rs:91`). | Kept `fid`, kept `slug` alongside for auditing. |
| 5 | High — plan overstated what the const registry validates. | Wording corrected above; the checks moved into tests. |
| 6 | High — `toml::de::Error` converts to `Error::Config`, whose message blames the user's `leetcode.toml`. Embedded set data must not produce that. | New `Error::SetData` variant, deliberately not a `#[from]`. |
| 7 | Medium — `--difficulty` is justified sugar, but the 1/2/3 mapping was written in two places. | `helper::Difficulty` owns it; `helper::filter`'s `e`/`m`/`h` and `Problem::display_level` both route through it. |
| 8 | Medium — omitting `--random` is correct; fix discoverability instead. | `pick`'s description now reads "Pick a random problem, or select one by id, name, or daily challenge", plus an examples block it previously lacked. |
| 9 | Medium — `"LeetCode"` is not a license; a `stale` boolean is subjective and goes stale itself. | `source_license` spells out "Proprietary (LeetCode); problem identifiers only"; `stale` replaced by an objective `source_as_of` date. |
| 10 | Medium — calling them "frequency lists" while discarding frequency oversells them. | Renamed to 2022 company-tag snapshots in every user-facing string; membership-only is stated outright. |
| 11 | Medium — the Reqwest retry re-ran the whole command, re-rolling the random pick, unbounded. | Target is resolved once; only the description fetch retries, bounded by `MAX_FETCH_ATTEMPTS`. |
| 12 | Medium — "repeated runs pick a different problem" is a flaky test. | Replaced with deterministic filter-predicate and clap-parsing tests (24 unit tests). |

Beyond the review: `list` gained `--difficulty` too, since `pick -D medium` working while
`list -D medium` did not was a surprising asymmetry for zero conceptual cost.

Also fixed in passing: `pick` used to `eprintln!` a failure and return `Ok(())`, exiting 0 on error.
It now propagates.


---

## Post-implementation review — Codex second pass

Codex re-reviewed the written code against its own 12 findings. Verdict: 10 fully resolved, 1
partial, 1 unrelated pre-existing issue — plus **one real regression the rewrite introduced**.
All five are now fixed.

| Finding | Fix |
| --- | --- |
| **High — `pick <id>` and `pick --daily` broke on a fresh install.** `target_fid` returned early for those targets, so `candidates()` — which held the empty-cache download — never ran. `get_question` then hit sqlite first and failed with a diesel `NotFound` that the Reqwest-only retry could not catch. | Cache population moved out to `cmd::populated_problems`, called in `run()` before any target resolves. `list` uses the same helper, which also removes its `Box::pin(self.run())` re-entry — an infinite loop if a download kept returning nothing. Verified end to end against an isolated empty `$HOME`. |
| Medium — `--plan` parsed and was then silently ignored in any build without the optional `pym` feature, and `describe_filters` would report it as an applied filter. Pre-existing upstream. | `cmd::ensure_plan_supported` rejects `--plan` with an actionable message when the feature is absent. Both `pick` and `list` call it. |
| Medium — the generator's "nothing is written" claim held per-set but not per-run: a failure in the tenth set left the first nine rewritten, and a plain `open(…, "w")` could truncate a file on an interrupted write. | All ten sets are fetched, validated, and rendered before anything is written; each file is then replaced via a temp file + `os.replace`. |
| Low — `Difficulty` did not actually own the level mapping: `1/2/3` was still hard-coded in the coloured list row (`cache/models.rs`) and in `list --stat`'s counters. | Both route through `Difficulty::from_level` now. |
| Low — no test exercised the `NoProblemsMatch` path itself, only that filtering could produce an empty vec. | The emptiness check moved into `narrow_offline`, which is directly testable; three tests now assert the error and its filter description. |

Also fixed in passing: `src/bin/leetctl.rs` printed errors with `{:?}` and exited 0. Every `Error`
variant carries a written-for-humans message that no user had ever seen. It now prints `Display` on
stderr and exits non-zero.

Final gates: `cargo fmt --check` clean, `cargo clippy --all-targets --all-features` clean,
37 tests passing (26 new unit tests + 11 pre-existing), both the default and `pym` feature builds
checked.
