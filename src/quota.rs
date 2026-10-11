use std::time::Duration;
use serde::{Deserialize, Serialize};
use colored::*;
use chrono::{DateTime, Utc};
use crate::account::{StandaloneToken, refresh_account_token};

pub const QUOTA_URL: &str = "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary";

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Bucket {
    pub window: Option<String>,
    #[serde(rename = "remainingFraction")]
    pub remaining_fraction: Option<f64>,
    #[serde(rename = "resetTime")]
    pub reset_time: Option<String>,
    pub disabled: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct QuotaGroup {
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub buckets: Option<Vec<Bucket>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct QuotaResponseWrapper {
    pub response: Option<QuotaResponseInner>,
    pub groups: Option<Vec<QuotaGroup>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct QuotaResponseInner {
    pub groups: Option<Vec<QuotaGroup>>,
}

#[derive(Debug, Clone)]
pub struct WindowSummary {
    pub fraction: Option<f64>,
    pub reset_time: Option<DateTime<Utc>>,
    pub disabled: bool,
}

#[derive(Debug, Clone)]
pub struct GroupSummary {
    pub name: String,
    pub five_hour: Option<WindowSummary>,
    pub weekly: Option<WindowSummary>,
}

pub fn fetch_quota_summary(token_data: &mut StandaloneToken) -> Result<Vec<GroupSummary>, String> {
    // Check if token needs refresh
    let needs_refresh = match &token_data.token.expiry {
        Some(exp) => {
            if let Ok(dt) = DateTime::parse_from_rfc3339(exp) {
                (dt.with_timezone(&Utc) - Utc::now()).num_seconds() <= 30
            } else {
                false
            }
        }
        None => token_data.token.access_token.is_none(),
    };

    if needs_refresh {
        let _ = refresh_account_token(token_data);
    }

    let access_token = match &token_data.token.access_token {
        Some(t) if !t.is_empty() => t.clone(),
        _ => return Err("No OAuth access token available".to_string()),
    };

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .post(QUOTA_URL)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .header("User-Agent", "antigravity")
        .body("{}")
        .send();

    let resp = match resp {
        Ok(r) => r,
        Err(e) => return Err(format!("Network error contacting quota service: {}", e)),
    };

    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        // Retry once with a fresh token
        if let Ok(new_access) = refresh_account_token(token_data) {
            let retry = client
                .post(QUOTA_URL)
                .header("Authorization", format!("Bearer {}", new_access))
                .header("Content-Type", "application/json")
                .header("User-Agent", "antigravity")
                .body("{}")
                .send();
            match retry {
                Ok(r) if r.status().is_success() => return parse_quota_response(r),
                Ok(r) => return Err(format!("Quota service returned HTTP {} after refresh", r.status())),
                Err(e) => return Err(format!("Quota service retry failed: {}", e)),
            }
        }
        return Err("Unauthorized (401) and token refresh failed".to_string());
    }

    if !status.is_success() {
        return Err(format!("Quota service returned HTTP {}", status));
    }

    parse_quota_response(resp)
}

fn parse_quota_response(resp: reqwest::blocking::Response) -> Result<Vec<GroupSummary>, String> {
    let parsed: QuotaResponseWrapper = resp.json().map_err(|e| format!("Failed to parse quota JSON: {}", e))?;
    let raw_groups = parsed
        .response
        .and_then(|r| r.groups)
        .or(parsed.groups)
        .unwrap_or_default();

    let mut summaries = Vec::new();

    for (i, group) in raw_groups.into_iter().enumerate() {
        let name = group
            .display_name
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("Quota Group {}", i + 1));

        let mut five_hour = None;
        let mut weekly = None;

        for bucket in group.buckets.unwrap_or_default() {
            let win = match bucket.window.as_deref() {
                Some(w) => w,
                None => continue,
            };

            let dt = bucket.reset_time.as_deref().and_then(|s| {
                DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc))
            });

            let summary = WindowSummary {
                fraction: bucket.remaining_fraction,
                reset_time: dt,
                disabled: bucket.disabled.unwrap_or(false),
            };

            if win == "5h" {
                five_hour = Some(summary);
            } else if win == "weekly" {
                weekly = Some(summary);
            }
        }

        summaries.push(GroupSummary {
            name,
            five_hour,
            weekly,
        });
    }

    Ok(summaries)
}

pub fn render_window_metric(summary: Option<&WindowSummary>) -> (String, String) {
    match summary {
        None => ("no data".dimmed().to_string(), "—".dimmed().to_string()),
        Some(w) => {
            if w.disabled {
                return ("DISABLED".red().bold().to_string(), "—".dimmed().to_string());
            }

            let quota_str = match w.fraction {
                None => "no data".dimmed().to_string(),
                Some(f) if f <= 0.001 => "0% [RATE-LIMITED]".red().bold().to_string(),
                Some(f) => {
                    let pct = f * 100.0;
                    let text = format!("{:>5.1}%", pct);
                    if pct > 50.0 {
                        text.green().bold().to_string()
                    } else if pct > 20.0 {
                        text.yellow().bold().to_string()
                    } else {
                        text.red().bold().to_string()
                    }
                }
            };

            let reset_str = match w.reset_time {
                None => "not reported".dimmed().to_string(),
                Some(dt) => {
                    let now = Utc::now();
                    let diff = dt - now;
                    let secs = diff.num_seconds();
                    if secs <= 0 {
                        "resetting...".cyan().to_string()
                    } else {
                        let mins = secs / 60;
                        let hours = mins / 60;
                        let days = hours / 24;
                        if days > 0 {
                            format!("in {}d {}h", days, hours % 24).cyan().to_string()
                        } else if hours > 0 {
                            format!("in {}h {:02}m", hours, mins % 60).cyan().to_string()
                        } else {
                            format!("in {}m", mins).cyan().to_string()
                        }
                    }
                }
            };

            (quota_str, reset_str)
        }
    }
}
