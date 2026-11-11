//! Account snapshot and language share models.
//!
//! Defines the data structures that flow through the pipeline:
//! GitHub API -> collect -> AccountSnapshot -> render -> output.

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Window mode used to sum contributions for dragon stage progression.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub enum ContributionWindow {
    /// Current calendar year (January 1 through `observed_on`), resetting every January 1st.
    #[default]
    Annual,
    /// Trailing 365-day rolling window ending on `observed_on`.
    Rolling365,
}

/// One language observed in a user's public repositories along with its GitHub hex color.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct LanguageShare {
    /// Language name (e.g., `"Rust"`, `"TypeScript"`).
    pub name: String,
    /// GitHub hex color string (e.g., `"#dea584"` or `"#3178c6"`).
    pub hex_color: String,
    /// Number of repositories or weight where this language is primary.
    pub repo_count: u32,
}

/// One day of GitHub contribution activity.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct ContributionDay {
    pub date: NaiveDate,
    pub count: u32,
}

/// Complete snapshot of a GitHub user's public activity used to render the dragon.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct AccountSnapshot {
    pub login: String,
    pub account_id: u64,
    pub observed_on: NaiveDate,
    /// Total contributions across the observed window (drives dragon growth from Egg to Legendary).
    pub total_contributions: u64,
    /// Contributions made in the current calendar month.
    pub monthly_contributions: u64,
    /// Current daily contribution streak.
    pub current_streak: u32,
    /// Longest daily contribution streak.
    pub longest_streak: u32,
    /// Top languages ranked by repository share.
    pub languages: Vec<LanguageShare>,
}

/// Rank languages by repository count, descending.
pub fn rank_languages(map: BTreeMap<String, (String, u32)>) -> Vec<LanguageShare> {
    let mut langs: Vec<LanguageShare> = map
        .into_iter()
        .map(|(name, (hex, count))| LanguageShare {
            name,
            hex_color: hex,
            repo_count: count,
        })
        .collect();
    langs.sort_by_key(|l| std::cmp::Reverse(l.repo_count));
    langs.truncate(12);
    langs
}

/// Sum a calendar window of contribution days.
pub fn summarize_calendar_window(
    days: &[ContributionDay],
    observed_on: NaiveDate,
    window: ContributionWindow,
) -> (u64, u64, u32, u32) {
    let year_start = NaiveDate::from_ymd_opt(observed_on.year(), 1, 1).unwrap_or(observed_on);
    let rolling_start = observed_on - Duration::days(364);

    let (start, _end) = match window {
        ContributionWindow::Annual => (year_start, observed_on),
        ContributionWindow::Rolling365 => (rolling_start, observed_on),
    };

    let mut total: u64 = 0;
    let mut monthly: u64 = 0;
    let mut current_streak: u32 = 0;
    let mut longest_streak: u32 = 0;
    let mut running_streak: u32 = 0;

    for day in days {
        if day.date < start || day.date > observed_on {
            continue;
        }
        let count = u64::from(day.count);
        total += count;
        if day.date.month() == observed_on.month() && day.date.year() == observed_on.year() {
            monthly += count;
        }
        if count > 0 {
            running_streak += 1;
            current_streak = running_streak;
        } else {
            running_streak = 0;
        }
        longest_streak = longest_streak.max(current_streak);
    }

    (total, monthly, current_streak, longest_streak)
}
