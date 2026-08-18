//! The spaced-repetition deck's corner of the cache
//!
//! Separate from `mod.rs` because that module glob-imports `problems::dsl::*`, and both tables
//! have a `fid` column — the two dsl globs cannot coexist in one scope. Splitting the queries out
//! is cheaper than qualifying every column reference in either of them.
//!
//! Like the rest of `src/cache/`, nothing here prints: these return values and the commands render
//! them.
use super::Cache;
use super::models::ReviewCard;
use super::schemas::reviews::dsl::*;
use crate::err::Error;
use crate::srs::{Day, Grade, Schedule};
use diesel::prelude::*;

impl Cache {
    /// The card for a problem, or `None` when the problem is not in the deck.
    pub fn review_card(&self, problem_fid: i32) -> Result<Option<ReviewCard>, Error> {
        Ok(reviews
            .filter(fid.eq(problem_fid))
            .first::<ReviewCard>(&mut self.conn()?)
            .optional()?)
    }

    /// The whole deck, most overdue first.
    pub fn review_cards(&self) -> Result<Vec<ReviewCard>, Error> {
        Ok(reviews
            .order(due_day.asc())
            .load::<ReviewCard>(&mut self.conn()?)?)
    }

    /// The cards due on or before `day`, most overdue first.
    pub fn due_review_cards(&self, day: Day) -> Result<Vec<ReviewCard>, Error> {
        Ok(reviews
            .filter(due_day.le(day))
            .order(due_day.asc())
            .load::<ReviewCard>(&mut self.conn()?)?)
    }

    /// The frontend ids due on or before `day` — what the shared filter engine narrows on.
    pub fn due_review_fids(&self, day: Day) -> Result<Vec<i32>, Error> {
        Ok(reviews
            .filter(due_day.le(day))
            .order(due_day.asc())
            .select(fid)
            .load::<i32>(&mut self.conn()?)?)
    }

    /// How many problems the deck tracks, due or not.
    pub fn review_count(&self) -> Result<i64, Error> {
        Ok(reviews.count().get_result(&mut self.conn()?)?)
    }

    /// Put a problem in the deck, due immediately. A problem already in it keeps its schedule.
    pub fn enroll_review(&self, problem_fid: i32, today: Day) -> Result<(), Error> {
        let card = ReviewCard::new(problem_fid, Schedule::default(), today);
        diesel::insert_or_ignore_into(reviews)
            .values(&card)
            .execute(&mut self.conn()?)?;

        Ok(())
    }

    /// Grade a problem, enrolling it first if it is new, and return where that leaves it.
    pub fn grade_review(
        &self,
        problem_fid: i32,
        grade: Grade,
        today: Day,
    ) -> Result<ReviewCard, Error> {
        let current = self
            .review_card(problem_fid)?
            .map(|card| card.schedule())
            .unwrap_or_default();

        self.write_schedule(problem_fid, current.next(grade), today)
    }

    /// Grade a problem only if it is already in the deck.
    ///
    /// This is what a rejected submission uses: failing a problem you have never solved says
    /// nothing about recall, and enrolling it there would fill the deck with problems you have
    /// not learned yet.
    pub fn grade_enrolled_review(
        &self,
        problem_fid: i32,
        grade: Grade,
        today: Day,
    ) -> Result<Option<ReviewCard>, Error> {
        let Some(card) = self.review_card(problem_fid)? else {
            return Ok(None);
        };

        self.write_schedule(problem_fid, card.schedule().next(grade), today)
            .map(Some)
    }

    /// Store a schedule as the card for `problem_fid`, replacing whatever was there.
    fn write_schedule(
        &self,
        problem_fid: i32,
        schedule: Schedule,
        today: Day,
    ) -> Result<ReviewCard, Error> {
        let card = ReviewCard::new(problem_fid, schedule, today);
        diesel::replace_into(reviews)
            .values(&card)
            .execute(&mut self.conn()?)?;

        Ok(card)
    }

    /// Take a problem out of the deck. `false` when it was not in it.
    pub fn drop_review(&self, problem_fid: i32) -> Result<bool, Error> {
        let removed =
            diesel::delete(reviews.filter(fid.eq(problem_fid))).execute(&mut self.conn()?)?;

        Ok(removed > 0)
    }
}
