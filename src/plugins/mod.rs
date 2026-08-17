//! leetctl plugins
//!
//! + chrome cookie parser
//! + leetcode API
//!
//! ## login to `leetcode.com`
//! leetctl uses chrome cookie directly, do not need to login, please make sure you have logged in `leetcode.com` before using `leetctl`
//!

// FIXME: Read cookies from local storage. (issue #122)
mod chrome;
mod leetcode;
pub use leetcode::LeetCode;
