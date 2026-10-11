use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use colored::*;
use chrono::{DateTime, Utc};

pub const OAUTH_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

const CID_RAW: &[u8] = &[49, 48, 55, 49, 48, 48, 54, 48, 54, 48, 53, 57, 49, 45, 116, 109, 104, 115, 115, 105, 110, 50, 104, 50, 49, 108, 99, 114, 101, 50, 51, 53, 118, 116, 111, 108, 111, 106, 104, 52, 103, 52, 48, 51, 101, 112, 46, 97, 112, 112, 115, 46, 103, 111, 111, 103, 108, 101, 117, 115, 101, 114, 99, 111, 110, 116, 101, 110, 116, 46, 99, 111, 109];
const CS_RAW: &[u8] = &[71, 79, 67, 83, 80, 88, 45, 75, 53, 56, 70, 87, 82, 52, 56, 54, 76, 100, 76, 74, 49, 109, 76, 66, 56, 115, 88, 67, 52, 122, 54, 113, 68, 65, 102];

pub fn get_client_id() -> &'static str {
    std::str::from_utf8(CID_RAW).unwrap_or("")
}

pub fn get_client_secret() -> &'static str {
    std::str::from_utf8(CS_RAW).unwrap_or("")
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OAuthTokenData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StandaloneToken {
    pub token: OAuthTokenData,
    #[serde(default = "default_auth_method")]
    pub auth_method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saved_at: Option<String>,
}

fn default_auth_method() -> String {
    "oauth".to_string()
}

pub fn gemini_dir() -> PathBuf {
    dirs_home().join(".gemini")
}

pub fn accounts_dir() -> PathBuf {
    gemini_dir().join("accounts")
}

pub fn active_token_path() -> PathBuf {
    gemini_dir().join("jetski-standalone-oauth-token")
}

fn dirs_home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn read_active_token() -> Result<StandaloneToken, String> {
    let p = active_token_path();
    if !p.exists() {
        return Err(format!("Active token not found at {}", p.display()));
    }
    let data = fs::read_to_string(&p).map_err(|e| format!("Failed to read active token: {}", e))?;
    serde_json::from_str(&data).map_err(|e| format!("Invalid JSON in active token: {}", e))
}

pub fn write_active_token(token: &StandaloneToken) -> Result<(), String> {
    let p = active_token_path();
    let tmp = p.with_extension("tmp");
    let content = serde_json::to_string_pretty(token).map_err(|e| e.to_string())?;
    fs::write(&tmp, content).map_err(|e| format!("Failed to write tmp token: {}", e))?;
    fs::rename(&tmp, &p).map_err(|e| format!("Failed to atomically replace active token: {}", e))?;
    Ok(())
}

pub fn list_accounts() -> Result<Vec<(String, StandaloneToken, bool)>, String> {
    let dir = accounts_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    let active_res = read_active_token().ok();
    let active_refresh = active_res.as_ref().and_then(|t| t.token.refresh_token.as_deref());

    let mut list = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| format!("Cannot read accounts dir: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            if let Ok(data) = fs::read_to_string(&path) {
                if let Ok(tok) = serde_json::from_str::<StandaloneToken>(&data) {
                    let is_active = match (active_refresh, tok.token.refresh_token.as_deref()) {
                        (Some(a), Some(b)) => a == b,
                        _ => false,
                    };
                    list.push((name, tok, is_active));
                }
            }
        }
    }

    list.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(list)
}

pub fn load_account(name: &str) -> Result<StandaloneToken, String> {
    let path = accounts_dir().join(format!("{}.json", name));
    if !path.exists() {
        return Err(format!("Account '{}' not found in {}", name, accounts_dir().display()));
    }
    let data = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&data).map_err(|e| format!("Invalid profile format: {}", e))
}

pub fn save_account(name: &str) -> Result<(), String> {
    let mut token = read_active_token()?;
    token.saved_at = Some(Utc::now().to_rfc3339());
    let path = accounts_dir().join(format!("{}.json", name));
    let dir = accounts_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(&token).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| format!("Failed to save account: {}", e))?;
    println!("{} Account saved as '{}' [{}]", "[ok]".green().bold(), name.cyan().bold(), path.display());
    Ok(())
}

pub fn switch_account(target: &str) -> Result<(), String> {
    let accounts = list_accounts()?;
    if accounts.is_empty() {
        return Err("No saved accounts found. Use 'agx account save <name>' first.".to_string());
    }

    let target_name = if target == "next" {
        let current_idx = accounts.iter().position(|(_, _, active)| *active);
        let next_idx = match current_idx {
            Some(idx) => (idx + 1) % accounts.len(),
            None => 0,
        };
        accounts[next_idx].0.clone()
    } else {
        target.to_string()
    };

    let target_token = load_account(&target_name)?;
    write_active_token(&target_token)?;
    println!("{} Switched active account to '{}'", "[ok]".green().bold(), target_name.cyan().bold());

    // Sync to IDE database if present
    sync_ide_state(&target_token);

    Ok(())
}

pub fn switch_best() -> Result<String, String> {
    let accounts = list_accounts()?;
    if accounts.is_empty() {
        return Err("No saved accounts found".to_string());
    }

    let mut scored_accounts: Vec<(String, f64, bool)> = Vec::new();

    for (name, mut token, is_active) in accounts {
        let score = match crate::quota::fetch_quota_summary(&mut token) {
            Ok(groups) => {
                let mut s = 0.0;
                for g in groups {
                    let lname = g.name.to_lowercase();
                    if lname.contains("gemini") {
                        if let Some(w) = g.five_hour {
                            let f = w.fraction.unwrap_or(0.0);
                            if f <= 0.001 {
                                s = 0.0; // rate-limited
                                break;
                            }
                            s += f * 0.7;
                        }
                        if let Some(w) = g.weekly {
                            s += w.fraction.unwrap_or(0.0) * 0.3;
                        }
                    }
                }
                s
            }
            Err(_) => 0.0,
        };
        scored_accounts.push((name, score, is_active));
    }

    scored_accounts.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let (best_name, best_score, best_is_active) = &scored_accounts[0];
    if *best_is_active {
        return Ok(format!("Current account '{}' is already the best available ({:.1}% quota)", best_name, best_score * 100.0));
    }

    switch_account(best_name)?;
    Ok(format!("Auto-switched to best account '{}' ({:.1}% quota)", best_name, best_score * 100.0))
}

pub fn delete_account(name: &str) -> Result<(), String> {
    let path = accounts_dir().join(format!("{}.json", name));
    if !path.exists() {
        return Err(format!("Account '{}' not found", name));
    }
    fs::remove_file(&path).map_err(|e| format!("Failed to delete {}: {}", name, e))?;
    println!("{} Deleted account '{}'", "[ok]".green().bold(), name.yellow());
    Ok(())
}

pub fn refresh_account_token(token_data: &mut StandaloneToken) -> Result<String, String> {
    let refresh = match &token_data.token.refresh_token {
        Some(r) if !r.is_empty() => r,
        _ => return Err("No refresh token available in credentials".to_string()),
    };

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let params = [
        ("client_id", get_client_id()),
        ("client_secret", get_client_secret()),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh),
    ];

    let resp = client
        .post(OAUTH_TOKEN_URL)
        .form(&params)
        .send()
        .map_err(|e| format!("Network error while refreshing token: {}", e))?;

    if !resp.status().is_success() {
        let code = resp.status();
        return Err(format!("OAuth refresh failed with HTTP {}", code));
    }

    #[derive(Deserialize)]
    struct RefreshResp {
        access_token: String,
        expires_in: Option<i64>,
        token_type: Option<String>,
    }

    let parsed: RefreshResp = resp.json().map_err(|e| format!("Malformed OAuth response: {}", e))?;
    token_data.token.access_token = Some(parsed.access_token.clone());
    if let Some(tt) = parsed.token_type {
        token_data.token.token_type = Some(tt);
    }
    if let Some(exp_sec) = parsed.expires_in {
        let exp_time = Utc::now() + chrono::Duration::seconds(exp_sec);
        token_data.token.expiry = Some(exp_time.to_rfc3339());
    }

    Ok(parsed.access_token)
}

pub fn format_expiry(expiry_str: Option<&str>) -> String {
    match expiry_str {
        None => "unknown".dimmed().to_string(),
        Some(s) => match DateTime::parse_from_rfc3339(s) {
            Ok(dt) => {
                let now = Utc::now();
                let delta = dt.with_timezone(&Utc) - now;
                let secs = delta.num_seconds();
                if secs > 0 {
                    let mins = secs / 60;
                    let hours = mins / 60;
                    let days = hours / 24;
                    if days > 0 {
                        format!("valid ({}d {}h)", days, hours % 24).green().to_string()
                    } else if hours > 0 {
                        format!("valid ({}h {}m)", hours, mins % 60).green().to_string()
                    } else {
                        format!("valid ({}m)", mins).green().to_string()
                    }
                } else {
                    let past_mins = (-secs) / 60;
                    if past_mins > 60 {
                        format!("expired ({}h ago)", past_mins / 60).red().to_string()
                    } else {
                        format!("expired ({}m ago)", past_mins).red().to_string()
                    }
                }
            }
            Err(_) => "invalid date".dimmed().to_string(),
        },
    }
}

pub fn sync_ide_state(token: &StandaloneToken) {
    let home = dirs_home();
    let mut dbs = vec![
        home.join("Library/Application Support/Antigravity IDE/User/globalStorage/state.vscdb"),
        home.join(".config/Antigravity IDE/User/globalStorage/state.vscdb"),
    ];

    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        dbs.push(appdata.join("Antigravity IDE/User/globalStorage/state.vscdb"));
    }

    if let Some(access) = &token.token.access_token {
        for db in &dbs {
            if db.exists() {
                if let Ok(conn) = rusqlite::Connection::open(db) {
                    let _ = conn.execute(
                        "INSERT INTO ItemTable(key,value) VALUES('antigravityUnifiedStateSync.oauthToken', ?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value;",
                        rusqlite::params![access],
                    );
                }
            }
        }
    }
}
