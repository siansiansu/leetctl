# Spaced repetition

Solving a LeetCode problem once teaches you the trick. Solving it again three weeks later, from a
blank file, is what makes the trick yours. `leetctl review` keeps a deck of the problems you have
solved and tells you which ones are due today.

```sh
leetctl review                 # what is due today
leetctl review next            # open the most overdue one
leetctl review grade 1 hard    # "I got it, but it hurt"
leetctl review stats           # deck breakdown
```

The deck fills itself: an accepted `leetctl exec` enrolls the problem and grades it `good`, and a
rejected submission of a problem already in the deck grades it `again`. Grading by hand overrides
that — only you know whether you recalled the approach or rediscovered it.

## The schedule

SM-2, the algorithm Anki grew out of. Each card carries an **ease** (how well this problem sticks
for you), an **interval** in days, a **repetition count**, and a **lapse count**.

| Grade | Meaning | Interval | Ease |
| --- | --- | --- | --- |
| `again` | did not recall the approach | back to 1 day, repetitions reset | −0.20 |
| `hard` | recalled it, painfully | ×1.2 | −0.15 |
| `good` | recalled it | ×ease | unchanged |
| `easy` | instant | ×ease×1.3 | +0.15 |

Ease is clamped to `[1.3, 2.5]` and starts at `2.5`. Intervals are capped at 365 days.

A card's **first** review does not multiply anything — there is no interval yet — so it uses fixed
openers instead: `again` → 1 day, `hard` → 2, `good` → 4, `easy` → 7. Four days rather than the
classic one, because a card only enters this deck once you have already solved the problem.

Due dates are **local calendar days**, not timestamps. Grading at 23:50 with a one-day interval
makes the card due tomorrow, available from midnight — not from 23:50 tomorrow.

## Commands

| Command | Does |
| --- | --- |
| `leetctl review` | list cards due today or earlier, most overdue first |
| `leetctl review --all` | list the whole deck |
| `leetctl review next` | print the most overdue card's description, as `leetctl pick` would |
| `leetctl review grade <id> <again\|hard\|good\|easy>` | grade a card, enrolling it if new |
| `leetctl review add <id>` | enroll a problem, due today |
| `leetctl review drop <id>` | remove a problem from the deck |
| `leetctl review stats` | due / new / young / mature counts, and total lapses |

Ids are frontend ids — the number `leetctl list` shows and `leetctl pick <id>` takes.

## Everywhere else

The deck is a filter like any other, so it composes with the ones that were already there:

```sh
leetctl list --due                  # due problems
leetctl list --due -S blind75       # due problems from Blind 75
leetctl pick --due                  # a random due problem
leetctl pick --due -D hard
```

In the TUI, `r` toggles due-only, the footer carries the due count, a `●` in the list marks a due
row, and `1` / `2` / `3` / `4` grade the open problem `again` / `hard` / `good` / `easy`.

## Where it lives

| Path | Holds |
| --- | --- |
| `src/srs.rs` | the SM-2 math and the calendar-day helpers |
| `src/cache/reviews.rs` | the `reviews` table's operations |
| `src/cmd/review.rs` | the `leetctl review` subcommand |

The table is created by `Cache::new` alongside `problems` and `tags`, so there is no migration step
— an existing cache gains an empty deck the next time any command runs.

```sql
CREATE TABLE IF NOT EXISTS reviews (
  fid           INTEGER NOT NULL PRIMARY KEY,  -- frontend id, what `leetctl pick <id>` takes
  ease          FLOAT   NOT NULL,
  interval_days INTEGER NOT NULL,
  repetitions   INTEGER NOT NULL,
  lapses        INTEGER NOT NULL,
  due_day       INTEGER NOT NULL,              -- days since 1970-01-01, local
  last_day      INTEGER NOT NULL
)
```

Two rules the rest of the codebase already lives by apply here too, and are easy to break:

- **The scheduling math never reads the clock.** `srs::today()` is the one clock read, called
  once per command and passed down, which is what makes the ladder testable and keeps a single
  invocation from straddling midnight.
- **The cache layer does not print** and **`Display` impls have no side effects** — see
  [CLAUDE.md](../CLAUDE.md). Auto-grading happens in `Cache::exec_problem`, next to the existing
  `update_after_ac`, and not in `VerifyResult`'s `Display`.

## Roadmap

| Phase | Scope | Status |
| ---: | --- | --- |
| 0 | this doc + memory | done |
| 1 | `src/srs.rs` SM-2 engine, `reviews` table, `src/cache/reviews.rs` | done |
| 2 | `leetctl review` subcommand | done |
| 3 | auto-grade from submissions, `--due` on `list` and `pick` | done |
| 4 | TUI: due toggle, due badge, footer count, grade keys | done |
