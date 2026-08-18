//! When a solved problem comes back: the SM-2 schedule, and the calendar it counts in
use chrono::{Local, NaiveDate};

/// A calendar day, counted from 1970-01-01 in the machine's local timezone.
///
/// The deck works in whole local days rather than timestamps so that grading a card at 23:50 with
/// a one-day interval makes it due from midnight, not from 23:50 the next day.
pub type Day = i32;

/// Where a card's ease starts, and the window it is kept in.
///
/// The floor is SM-2's own: below it a repeatedly-failed card stops growing at all and the deck
/// turns into a treadmill. The ceiling keeps a run of `easy` grades from launching a problem years
/// out on the strength of three good days.
pub const INITIAL_EASE: f32 = 2.5;
const MIN_EASE: f32 = 1.3;
const MAX_EASE: f32 = 2.5;

/// No card is scheduled further out than this. A 2.5 ease compounds past a year in eight reviews,
/// and an interview problem you have not touched in a year is not really in a deck.
const MAX_INTERVAL_DAYS: i32 = 365;

/// An interval this long or longer counts as learned, for `leetctl review stats`. Anki's own
/// young/mature line, and it lands about where a problem stops feeling fresh.
pub const MATURE_INTERVAL_DAYS: i32 = 21;

const HARD_MULTIPLIER: f32 = 1.2;
const EASY_BONUS: f32 = 1.3;

/// How well the last recall went.
///
/// The grade is about recall, not correctness — an accepted submission that took forty minutes and
/// two hints is `hard`, which is why [`Grade`] is worth typing by hand even though a submission
/// grades itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Grade {
    /// Did not recall the approach.
    Again,
    /// Recalled it, painfully.
    Hard,
    /// Recalled it.
    Good,
    /// Instant.
    Easy,
}

impl Grade {
    pub fn as_str(self) -> &'static str {
        match self {
            Grade::Again => "again",
            Grade::Hard => "hard",
            Grade::Good => "good",
            Grade::Easy => "easy",
        }
    }

    /// How this grade moves the ease, before clamping.
    fn ease_delta(self) -> f32 {
        match self {
            Grade::Again => -0.20,
            Grade::Hard => -0.15,
            Grade::Good => 0.0,
            Grade::Easy => 0.15,
        }
    }

    /// The interval a card gets when there is nothing to multiply — its first review, or the one
    /// right after a lapse.
    ///
    /// Four days for `good` rather than SM-2's one, because a card only enters this deck once the
    /// problem has already been solved; day one would be re-reading a solution still in memory.
    fn opening_interval(self) -> i32 {
        match self {
            Grade::Again => 1,
            Grade::Hard => 2,
            Grade::Good => 4,
            Grade::Easy => 7,
        }
    }
}

/// Everything the schedule remembers about one card, with no notion of which problem it is or of
/// what day it is today. Pure input and output of [`Schedule::next`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Schedule {
    pub ease: f32,
    pub interval_days: i32,
    pub repetitions: i32,
    pub lapses: i32,
}

impl Default for Schedule {
    /// A card that has just entered the deck: due immediately, nothing learned about it yet.
    fn default() -> Self {
        Schedule {
            ease: INITIAL_EASE,
            interval_days: 0,
            repetitions: 0,
            lapses: 0,
        }
    }
}

impl Schedule {
    /// The schedule after grading this card. Total, and independent of the clock.
    pub fn next(self, grade: Grade) -> Self {
        let ease = (self.ease + grade.ease_delta()).clamp(MIN_EASE, MAX_EASE);

        // A lapse throws the old interval away, and a first review has none to grow, so both fall
        // back to the fixed openers instead of a multiplier.
        let interval_days = match grade {
            Grade::Again => grade.opening_interval(),
            _ if self.repetitions == 0 => grade.opening_interval(),
            Grade::Hard => grow(self.interval_days, HARD_MULTIPLIER),
            Grade::Good => grow(self.interval_days, ease),
            Grade::Easy => grow(self.interval_days, ease * EASY_BONUS),
        };

        Schedule {
            ease,
            interval_days,
            repetitions: match grade {
                Grade::Again => 0,
                _ => self.repetitions + 1,
            },
            lapses: match grade {
                Grade::Again => self.lapses + 1,
                _ => self.lapses,
            },
        }
    }

    /// Whether this card counts as learned rather than still in circulation.
    pub fn is_mature(&self) -> bool {
        self.interval_days >= MATURE_INTERVAL_DAYS
    }
}

/// Stretch an interval, never below a day and never past the cap.
fn grow(interval_days: i32, multiplier: f32) -> i32 {
    let stretched = (interval_days as f32 * multiplier).round() as i32;
    stretched.clamp(1, MAX_INTERVAL_DAYS)
}

/// Today, in the local timezone.
///
/// The one clock read in the deck. Commands call it once and pass the result down, so a long
/// invocation cannot straddle midnight and schedule half its cards against a different today.
pub fn today() -> Day {
    day_of(Local::now().date_naive())
}

fn epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(1970, 1, 1).expect("1970-01-01 is a real date")
}

fn day_of(date: NaiveDate) -> Day {
    date.signed_duration_since(epoch()).num_days() as Day
}

/// The calendar date a day number lands on, for printing.
pub fn date_of(day: Day) -> NaiveDate {
    epoch() + chrono::Duration::days(day as i64)
}

/// `today`, `in 4d`, `2d late` — how a due date reads next to the day it is read on.
pub fn due_label(due: Day, today: Day) -> String {
    match due - today {
        0 => "today".to_string(),
        ahead if ahead > 0 => format!("in {ahead}d"),
        behind => format!("{}d late", -behind),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A card that has been through one `good` review: the state most of the ladder starts from.
    fn reviewed_once() -> Schedule {
        Schedule::default().next(Grade::Good)
    }

    #[test]
    fn a_first_good_review_opens_at_four_days() {
        // trace: repetitions == 0, so `good` takes its opening interval rather than ×ease.
        let card = Schedule::default().next(Grade::Good);

        assert_eq!(card.interval_days, 4);
        assert_eq!(card.repetitions, 1);
        assert_eq!(card.ease, INITIAL_EASE);
        assert_eq!(card.lapses, 0);
    }

    #[test]
    fn the_other_grades_have_their_own_openers() {
        let opener = |grade| Schedule::default().next(grade).interval_days;

        assert_eq!(opener(Grade::Again), 1);
        assert_eq!(opener(Grade::Hard), 2);
        assert_eq!(opener(Grade::Easy), 7);
    }

    #[test]
    fn a_second_good_review_multiplies_by_the_ease() {
        // trace: interval 4, ease stays 2.5 on `good`, 4 × 2.5 = 10.
        let card = reviewed_once().next(Grade::Good);

        assert_eq!(card.interval_days, 10);
        assert_eq!(card.repetitions, 2);
    }

    #[test]
    fn hard_grows_slowly_and_costs_ease() {
        // trace: interval 4 × 1.2 = 4.8 → 5; ease 2.5 − 0.15 = 2.35.
        let card = reviewed_once().next(Grade::Hard);

        assert_eq!(card.interval_days, 5);
        assert!((card.ease - 2.35).abs() < f32::EPSILON);
    }

    #[test]
    fn easy_takes_the_bonus_on_top_of_the_ease() {
        // trace: ease is already at the 2.5 ceiling, so +0.15 clamps back to 2.5;
        // interval 4 × 2.5 × 1.3 = 13.
        let card = reviewed_once().next(Grade::Easy);

        assert_eq!(card.interval_days, 13);
        assert_eq!(card.ease, MAX_EASE);
    }

    #[test]
    fn again_resets_the_interval_and_counts_a_lapse() {
        let card = reviewed_once().next(Grade::Good).next(Grade::Again);

        assert_eq!(card.interval_days, 1);
        assert_eq!(card.repetitions, 0);
        assert_eq!(card.lapses, 1);
        assert!((card.ease - 2.3).abs() < f32::EPSILON);
    }

    #[test]
    fn a_card_relearned_after_a_lapse_opens_again_rather_than_multiplying() {
        // repetitions went back to 0, so the next `good` is 4 days — not 1 × ease.
        let card = reviewed_once().next(Grade::Again).next(Grade::Good);

        assert_eq!(card.interval_days, 4);
    }

    #[test]
    fn ease_never_leaves_its_window() {
        let mut card = Schedule::default();
        for _ in 0..20 {
            card = card.next(Grade::Again);
        }
        assert_eq!(card.ease, MIN_EASE);

        for _ in 0..20 {
            card = card.next(Grade::Easy);
        }
        assert_eq!(card.ease, MAX_EASE);
    }

    #[test]
    fn intervals_stop_at_the_cap() {
        let mut card = Schedule::default();
        for _ in 0..20 {
            card = card.next(Grade::Easy);
        }

        assert_eq!(card.interval_days, MAX_INTERVAL_DAYS);
    }

    #[test]
    fn maturity_is_the_three_week_line() {
        let young = Schedule {
            interval_days: MATURE_INTERVAL_DAYS - 1,
            ..Default::default()
        };
        let mature = Schedule {
            interval_days: MATURE_INTERVAL_DAYS,
            ..Default::default()
        };

        assert!(!young.is_mature());
        assert!(mature.is_mature());
    }

    #[test]
    fn day_numbers_count_local_days_from_the_epoch() {
        assert_eq!(day_of(epoch()), 0);
        assert_eq!(
            day_of(NaiveDate::from_ymd_opt(1970, 1, 11).unwrap()),
            10,
            "ten days after the epoch is day ten"
        );
        assert_eq!(date_of(10), NaiveDate::from_ymd_opt(1970, 1, 11).unwrap());
    }

    #[test]
    fn due_labels_read_relative_to_today() {
        assert_eq!(due_label(100, 100), "today");
        assert_eq!(due_label(104, 100), "in 4d");
        assert_eq!(due_label(98, 100), "2d late");
    }
}
