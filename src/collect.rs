//! Collects a GitHub account's public commit/contribution history and language hex colors.
//!
//! Uses direct HTTPS requests (`https://api.github.com/graphql` when `GITHUB_TOKEN` or `GH_TOKEN`
//! is set, or the public GitHub REST/contributions endpoints when no token is provided), with
//! `gh api graphql` as an automatic fallback if local CLI auth is preferred.

use std::collections::BTreeMap;
use std::env;
use std::io::ErrorKind;
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use chrono::{DateTime, NaiveDate, Utc};
use serde_json::{json, Value};

use crate::history::{
    rank_languages, summarize_calendar_window, AccountSnapshot, ContributionDay, ContributionWindow,
};
use crate::languages::language_hex;

const REQUEST_CONNECT_TIMEOUT_SECS: u64 = 5;
const REQUEST_READ_TIMEOUT_SECS: u64 = 15;
const RETRY_ATTEMPTS: u8 = 3;

/// Shared HTTPS agent with explicit connect/read/write timeouts.
fn github_agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(REQUEST_CONNECT_TIMEOUT_SECS))
            .timeout_read(Duration::from_secs(REQUEST_READ_TIMEOUT_SECS))
            .timeout_write(Duration::from_secs(REQUEST_READ_TIMEOUT_SECS))
            .build()
    })
}

/// GET a JSON document with explicit timeouts and exponential backoff retry (with jitter).
fn get_json_with_retry(url: &str) -> Result<Value, String> {
    let mut last_err = String::new();
    for attempt in 0..RETRY_ATTEMPTS {
        match github_agent()
            .get(url)
            .set("User-Agent", "git-dragon")
            .set("Accept", "application/vnd.github+json")
            .call()
        {
            Ok(resp) => match resp.into_json::<Value>() {
                Ok(v) => return Ok(v),
                Err(e) => last_err = format!("invalid JSON from {url}: {e}"),
            },
            Err(e) => last_err = format!("request to {url} failed: {e}"),
        }
        if attempt + 1 < RETRY_ATTEMPTS {
            let jitter_millis = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| u64::from(d.subsec_millis()))
                .unwrap_or(0)
                % 250;
            std::thread::sleep(Duration::from_millis(
                400_u64 * (1_u64 << attempt) + jitter_millis,
            ));
        }
    }
    Err(last_err)
}

const ACCOUNT_QUERY: &str = r#"
query($login: String!) {
  user(login: $login) {
    databaseId
    repositories(first: 100, ownerAffiliations: OWNER, isFork: false, orderBy: {field: UPDATED_AT, direction: DESC}) {
      nodes {
        primaryLanguage {
          name
          color
        }
      }
    }
    contributionsCollection {
      contributionCalendar {
        weeks {
          contributionDays {
            date
            contributionCount
          }
        }
      }
    }
  }
}
"#;

/// Collect an `AccountSnapshot` for `login` using the default `Annual` window.
pub fn collect_account_snapshot(login: &str) -> Result<AccountSnapshot, String> {
    collect_account_snapshot_with_window(login, ContributionWindow::Annual)
}

/// Collect an `AccountSnapshot` for `login` across `window` (`Annual` or `Rolling365`).
pub fn collect_account_snapshot_with_window(
    login: &str,
    window: ContributionWindow,
) -> Result<AccountSnapshot, String> {
    // 1. If GITHUB_TOKEN or GH_TOKEN is available, query https://api.github.com/graphql directly via HTTPS (no `gh` CLI required).
    if let Ok(token) = env::var("GITHUB_TOKEN").or_else(|_| env::var("GH_TOKEN")) {
        let trimmed = token.trim();
        if !trimmed.is_empty() {
            if let Ok(root) = fetch_graphql_via_https(login, trimmed) {
                return parse_graphql_snapshot(login, &root, window);
            }
        }
    }

    // 2. Otherwise try `gh api graphql` if the user has the GitHub CLI installed and logged in locally.
    if let Ok(root) = fetch_graphql_via_gh_cli(login) {
        return parse_graphql_snapshot(login, &root, window);
    }

    // 3. Fallback: tokenless public HTTPS endpoints (`api.github.com/users/<login>/repos` + `github-contributions-api.jogruber.de`).
    fetch_public_rest_snapshot(login, window)
}

fn fetch_graphql_via_https(login: &str, token: &str) -> Result<Value, String> {
    let payload = json!({
        "query": ACCOUNT_QUERY,
        "variables": { "login": login }
    });
    let resp = github_agent()
        .post("https://api.github.com/graphql")
        .set("Authorization", &format!("Bearer {token}"))
        .set("User-Agent", "git-dragon")
        .set("Accept", "application/vnd.github+json")
        .send_json(payload)
        .map_err(|e| format!("GitHub GraphQL HTTPS request failed: {e}"))?;

    resp.into_json::<Value>()
        .map_err(|e| format!("Invalid JSON from GitHub GraphQL API: {e}"))
}

fn fetch_graphql_via_gh_cli(login: &str) -> Result<Value, String> {
    let output = Command::new("gh")
        .args(["api", "graphql", "-f"])
        .arg(format!("query={ACCOUNT_QUERY}"))
        .arg("-f")
        .arg(format!("login={login}"))
        .output()
        .map_err(|err| {
            if err.kind() == ErrorKind::NotFound {
                "GitHub CLI (`gh`) is not installed or not in PATH".to_owned()
            } else {
                format!("Failed to execute `gh`: {err}")
            }
        })?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }

    serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("Invalid JSON from `gh api graphql`: {err}"))
}

fn parse_graphql_snapshot(
    login: &str,
    root: &Value,
    window: ContributionWindow,
) -> Result<AccountSnapshot, String> {
    let user = root
        .get("data")
        .and_then(|d| d.get("user"))
        .filter(|u| !u.is_null())
        .ok_or_else(|| format!("GitHub user `{login}` not found"))?;

    let account_id = user.get("databaseId").and_then(Value::as_u64).unwrap_or(1);

    let mut lang_counts: BTreeMap<String, (String, u32)> = BTreeMap::new();
    if let Some(nodes) = user
        .get("repositories")
        .and_then(|r| r.get("nodes"))
        .and_then(Value::as_array)
    {
        for node in nodes {
            if let Some(lang) = node.get("primaryLanguage").filter(|l| !l.is_null()) {
                let name = lang
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("Unknown")
                    .to_owned();
                let color = lang
                    .get("color")
                    .and_then(Value::as_str)
                    .map_or_else(|| language_hex(&name), str::to_owned);
                let entry = lang_counts.entry(name).or_insert((color, 0));
                entry.1 += 1;
            }
        }
    }

    let mut days = Vec::new();
    if let Some(weeks) = user
        .get("contributionsCollection")
        .and_then(|c| c.get("contributionCalendar"))
        .and_then(|cal| cal.get("weeks"))
        .and_then(Value::as_array)
    {
        for week in weeks {
            if let Some(c_days) = week.get("contributionDays").and_then(Value::as_array) {
                for d in c_days {
                    if let (Some(date_str), Some(count)) = (
                        d.get("date").and_then(Value::as_str),
                        d.get("contributionCount").and_then(Value::as_u64),
                    ) {
                        if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                            days.push(ContributionDay {
                                date,
                                count: count as u32,
                            });
                        }
                    }
                }
            }
        }
    }

    let today: NaiveDate = DateTime::<Utc>::from(std::time::SystemTime::now()).date_naive();
    let (total_contributions, monthly_contributions, current_streak, longest_streak) =
        summarize_calendar_window(&days, today, window);

    Ok(AccountSnapshot {
        login: login.to_owned(),
        account_id,
        observed_on: today,
        total_contributions,
        monthly_contributions,
        current_streak,
        longest_streak,
        languages: rank_languages(lang_counts),
    })
}

fn fetch_public_rest_snapshot(
    login: &str,
    window: ContributionWindow,
) -> Result<AccountSnapshot, String> {
    let user_url = format!("https://api.github.com/users/{login}");
    let user_json: Value = get_json_with_retry(&user_url)
        .map_err(|e| format!("Failed to fetch GitHub user `{login}`: {e}"))?;

    let account_id = user_json.get("id").and_then(Value::as_u64).unwrap_or(1);

    let repos_url = format!("https://api.github.com/users/{login}/repos?per_page=100&sort=updated");
    let mut lang_counts: BTreeMap<String, (String, u32)> = BTreeMap::new();
    if let Ok(Value::Array(repos)) = get_json_with_retry(&repos_url) {
        for repo in repos {
            if repo.get("fork").and_then(Value::as_bool).unwrap_or(false) {
                continue;
            }
            if let Some(lang_name) = repo.get("language").and_then(Value::as_str) {
                let color = language_hex(lang_name);
                let entry = lang_counts
                    .entry(lang_name.to_owned())
                    .or_insert((color, 0));
                entry.1 += 1;
            }
        }
    }

    let contribs_url = format!("https://github-contributions-api.jogruber.de/v4/{login}");
    let mut days = Vec::new();
    if let Ok(contrib_json) = get_json_with_retry(&contribs_url) {
        if let Some(arr) = contrib_json.get("contributions").and_then(Value::as_array) {
            for item in arr {
                if let (Some(date_str), Some(count)) = (
                    item.get("date").and_then(Value::as_str),
                    item.get("count").and_then(Value::as_u64),
                ) {
                    if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                        days.push(ContributionDay {
                            date,
                            count: count as u32,
                        });
                    }
                }
            }
        }
    }

    let today: NaiveDate = DateTime::<Utc>::from(std::time::SystemTime::now()).date_naive();
    let (total_contributions, monthly_contributions, current_streak, longest_streak) =
        summarize_calendar_window(&days, today, window);

    Ok(AccountSnapshot {
        login: login.to_owned(),
        account_id,
        observed_on: today,
        total_contributions,
        monthly_contributions,
        current_streak,
        longest_streak,
        languages: rank_languages(lang_counts),
    })
}
