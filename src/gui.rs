use serde::{Deserialize, Serialize};
use crate::account::{self, StandaloneToken};
use crate::quota;
use crate::patcher::{self, GateState};
use crate::cleaner;
use crate::doctor::{self, DoctorStatus};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AccountGuiItem {
    pub name: String,
    pub is_active: bool,
    pub expiry: String,
    pub email: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WindowMetricGui {
    pub pct: u32,
    pub reset_str: String,
    pub disabled: bool,
    pub has_data: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AccountQuotasGui {
    pub gemini_5h: WindowMetricGui,
    pub gemini_wk: WindowMetricGui,
    pub claude_5h: WindowMetricGui,
    pub claude_wk: WindowMetricGui,
    pub raw_groups: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TargetGuiItem {
    pub name: String,
    pub description: String,
    pub path: Option<String>,
    pub state: String,
    pub has_backup: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CleanCatGuiItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub bytes: u64,
    pub size_formatted: String,
    pub paths_count: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DoctorItemGui {
    pub title: String,
    pub status: String,
    pub detail: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DoctorReportGui {
    pub items: Vec<DoctorItemGui>,
    pub ok_count: usize,
    pub warn_count: usize,
    pub fail_count: usize,
}

fn get_accounts_blocking() -> Result<Vec<AccountGuiItem>, String> {
    let list = account::list_accounts()?;
    let items = list
        .into_iter()
        .map(|(name, token, is_active)| {
            let expiry = account::format_expiry_plain(token.token.expiry.as_deref());
            AccountGuiItem {
                name,
                is_active,
                expiry,
                email: token.email,
            }
        })
        .collect();
    Ok(items)
}

fn map_window(w_opt: Option<&quota::WindowSummary>) -> WindowMetricGui {
    match w_opt {
        None => WindowMetricGui {
            pct: 0,
            reset_str: "—".to_string(),
            disabled: false,
            has_data: false,
        },
        Some(w) => {
            if w.disabled {
                return WindowMetricGui {
                    pct: 0,
                    reset_str: "disabled".to_string(),
                    disabled: true,
                    has_data: true,
                };
            }
            let pct = match w.fraction {
                Some(f) => ((f * 100.0).round() as u32).min(100),
                None => 0,
            };
            let reset_str = match w.reset_time {
                None => "unlimited".to_string(),
                Some(rt) => {
                    let now = chrono::Utc::now();
                    if rt <= now {
                        "reset ready".to_string()
                    } else {
                        let diff = rt - now;
                        let secs = diff.num_seconds();
                        let hours = secs / 3600;
                        let mins = (secs % 3600) / 60;
                        if hours >= 24 {
                            let days = hours / 24;
                            let rem_h = hours % 24;
                            format!("resets in {}d {}h", days, rem_h)
                        } else if hours > 0 {
                            format!("resets in {}h {}m", hours, mins)
                        } else {
                            format!("resets in {}m", mins.max(1))
                        }
                    }
                }
            };
            WindowMetricGui {
                pct,
                reset_str,
                disabled: false,
                has_data: true,
            }
        }
    }
}

fn get_account_quota_blocking(name: String) -> Result<AccountQuotasGui, String> {
    let _refresh_guard = TOKEN_REFRESH_LOCK.lock().map_err(|_| "Refresh lock failed".to_string())?;
    let original = account::load_account(&name)?;
    let mut token = original.clone();
    if account::token_needs_refresh(&token) {
        account::refresh_account_token(&mut token)?;
        persist_gui_token(&name, &original, &token)?;
    }
    let before_quota = token.clone();
    let groups_result = quota::fetch_quota_summary(&mut token);
    if token.token.access_token != before_quota.token.access_token || token.token.expiry != before_quota.token.expiry {
        persist_gui_token(&name, &before_quota, &token)?;
    }
    let groups = groups_result?;
    let mut gemini_5h = None;
    let mut gemini_wk = None;
    let mut claude_5h = None;
    let mut claude_wk = None;
    let mut raw_groups = Vec::new();

    for g in &groups {
        raw_groups.push(g.name.clone());
        let lname = g.name.to_lowercase();
        if lname.contains("gemini") {
            gemini_5h = g.five_hour.as_ref();
            gemini_wk = g.weekly.as_ref();
        } else if lname.contains("claude") || lname.contains("gpt") {
            claude_5h = g.five_hour.as_ref();
            claude_wk = g.weekly.as_ref();
        }
    }

    Ok(AccountQuotasGui {
        gemini_5h: map_window(gemini_5h),
        gemini_wk: map_window(gemini_wk),
        claude_5h: map_window(claude_5h),
        claude_wk: map_window(claude_wk),
        raw_groups,
    })
}

fn switch_account_cmd_blocking(name: String) -> Result<String, String> {
    account::switch_account(&name)?;
    Ok(format!("Switched to {}", name))
}

fn auto_best_cmd_blocking() -> Result<String, String> {
    account::switch_best()
}

fn save_account_cmd_blocking(name: String, token_json: String) -> Result<String, String> {
    let clean_name = name.trim().to_string();
    if clean_name.is_empty() {
        return Err("Account name cannot be empty".to_string());
    }

    let parsed: StandaloneToken = if token_json.trim().starts_with('{') {
        account::parse_raw_token_data(&token_json)
            .ok_or_else(|| "Failed to parse token JSON structure".to_string())?
    } else {
        // Assume access token string or file
        account::read_active_token()?
    };

    account::save_account_token(&clean_name, &parsed)?;
    Ok(format!("Saved account '{}'", clean_name))
}

fn delete_account_cmd_blocking(name: String) -> Result<String, String> {
    account::delete_account(&name)?;
    Ok(format!("Deleted account '{}'", name))
}

fn get_patch_targets_blocking() -> Result<Vec<TargetGuiItem>, String> {
    let targets = patcher::discover_targets();
    let items = targets
        .into_iter()
        .map(|t| {
            let state_str = match t.state {
                GateState::Patched => "patched",
                GateState::Unpatched => "pristine",
                GateState::NotFound => "not_found",
                GateState::Unknown => "unknown",
            };
            TargetGuiItem {
                name: t.name.to_string(),
                description: t.description.to_string(),
                path: t.path.map(|p| p.to_string_lossy().to_string()),
                state: state_str.to_string(),
                has_backup: t.has_backup,
            }
        })
        .collect();
    Ok(items)
}

fn apply_patch_cmd_blocking(target: String) -> Result<String, String> {
    patcher::patch_target(&target)
}

fn restore_patch_cmd_blocking(target: String) -> Result<String, String> {
    patcher::restore_target(&target)
}

fn get_clean_categories_blocking() -> Result<Vec<CleanCatGuiItem>, String> {
    let cats = cleaner::scan_clean_categories();
    let items = cats
        .into_iter()
        .map(|c| CleanCatGuiItem {
            id: c.id.to_string(),
            title: c.title.to_string(),
            description: c.description.to_string(),
            bytes: c.total_bytes,
            size_formatted: c.size_formatted(),
            paths_count: c.paths.len(),
        })
        .collect();
    Ok(items)
}

fn purge_clean_categories_blocking(ids: Vec<String>) -> Result<String, String> {
    let cats = cleaner::scan_clean_categories();
    let mut total_freed: u64 = 0;
    for cat in cats {
        if ids.contains(&cat.id.to_string()) {
            if let Ok(bytes) = cleaner::purge_category(&cat) {
                total_freed += bytes;
            }
        }
    }
    Ok(format!("Freed {}", cleaner::format_bytes(total_freed)))
}

fn get_doctor_report_blocking() -> Result<DoctorReportGui, String> {
    let report = doctor::check_doctor();
    let items = report
        .items
        .into_iter()
        .map(|item| {
            let status_str = match item.status {
                DoctorStatus::Ok => "ok",
                DoctorStatus::Warn => "warn",
                DoctorStatus::Fail => "fail",
            };
            DoctorItemGui {
                title: item.title,
                status: status_str.to_string(),
                detail: item.detail,
            }
        })
        .collect();

    Ok(DoctorReportGui {
        items,
        ok_count: report.ok_count,
        warn_count: report.warn_count,
        fail_count: report.fail_count,
    })
}

static TOKEN_REFRESH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// Do not overwrite a deleted/reimported profile or switch back to a previous account.
fn persist_gui_token(name: &str, original: &StandaloneToken, updated: &StandaloneToken) -> Result<(), String> {
    let _guard = MUTATION_LOCK.lock().map_err(|_| "Operation lock failed".to_string())?;
    let path = account::accounts_dir().join(format!("{name}.json"));
    if path.exists() {
        let current = account::load_account(name)?;
        if account::same_credentials(&current, original) {
            account::save_account_token(name, updated)?;
        }
    }
    if let Ok(active) = account::read_active_token() {
        if account::same_credentials(&active, original) {
            account::write_active_token(updated)?;
        }
    }
    Ok(())
}

#[derive(Serialize)]
pub struct TokenRefreshReport {
    updated: Vec<String>,
    failed: Vec<String>,
}

#[tauri::command]
pub async fn refresh_due_tokens() -> Result<TokenRefreshReport, String> {
    run_blocking(|| {
        let mut report = TokenRefreshReport { updated: vec![], failed: vec![] };
        let names: Vec<_> = account::list_accounts()?.into_iter().map(|(name, _, _)| name).collect();
        for name in names {
            let _guard = TOKEN_REFRESH_LOCK.lock().map_err(|_| "Refresh lock failed".to_string())?;
            let Ok(original) = account::load_account(&name) else { continue };
            if !account::token_needs_refresh(&original) { continue; }
            let mut updated = original.clone();
            match account::refresh_account_token(&mut updated)
                .and_then(|_| persist_gui_token(&name, &original, &updated)) {
                Ok(()) => report.updated.push(name),
                Err(_) => report.failed.push(name),
            }
        }
        Ok(report)
    }).await
}

static MUTATION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// Filesystem, blocking HTTP and child processes must never run on the window thread.
async fn run_blocking<T: Send + 'static>(
    task: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| format!("Background task failed: {error}"))?
}

#[tauri::command]
pub async fn get_accounts() -> Result<Vec<AccountGuiItem>, String> {
    run_blocking(move || get_accounts_blocking()).await
}

#[tauri::command]
pub async fn get_account_quota(name: String) -> Result<AccountQuotasGui, String> {
    run_blocking(move || get_account_quota_blocking(name)).await
}

#[tauri::command]
pub async fn switch_account_cmd(name: String) -> Result<String, String> {
    run_blocking(move || {
        let _guard = MUTATION_LOCK.lock().map_err(|_| "Operation lock failed".to_string())?;
        switch_account_cmd_blocking(name)
    }).await
}

#[tauri::command]
pub async fn auto_best_cmd() -> Result<String, String> {
    run_blocking(move || {
        let _guard = MUTATION_LOCK.lock().map_err(|_| "Operation lock failed".to_string())?;
        auto_best_cmd_blocking()
    }).await
}

#[tauri::command]
pub async fn save_account_cmd(name: String, token_json: String) -> Result<String, String> {
    run_blocking(move || {
        let _guard = MUTATION_LOCK.lock().map_err(|_| "Operation lock failed".to_string())?;
        save_account_cmd_blocking(name, token_json)
    }).await
}

#[tauri::command]
pub async fn delete_account_cmd(name: String) -> Result<String, String> {
    run_blocking(move || {
        let _guard = MUTATION_LOCK.lock().map_err(|_| "Operation lock failed".to_string())?;
        delete_account_cmd_blocking(name)
    }).await
}

#[tauri::command]
pub async fn get_patch_targets() -> Result<Vec<TargetGuiItem>, String> {
    run_blocking(move || get_patch_targets_blocking()).await
}

#[tauri::command]
pub async fn apply_patch_cmd(target: String) -> Result<String, String> {
    run_blocking(move || {
        let _guard = MUTATION_LOCK.lock().map_err(|_| "Operation lock failed".to_string())?;
        apply_patch_cmd_blocking(target)
    }).await
}

#[tauri::command]
pub async fn restore_patch_cmd(target: String) -> Result<String, String> {
    run_blocking(move || {
        let _guard = MUTATION_LOCK.lock().map_err(|_| "Operation lock failed".to_string())?;
        restore_patch_cmd_blocking(target)
    }).await
}

#[tauri::command]
pub async fn get_clean_categories() -> Result<Vec<CleanCatGuiItem>, String> {
    run_blocking(move || get_clean_categories_blocking()).await
}

#[tauri::command]
pub async fn purge_clean_categories(ids: Vec<String>) -> Result<String, String> {
    run_blocking(move || {
        let _guard = MUTATION_LOCK.lock().map_err(|_| "Operation lock failed".to_string())?;
        purge_clean_categories_blocking(ids)
    }).await
}

#[tauri::command]
pub async fn get_doctor_report() -> Result<DoctorReportGui, String> {
    run_blocking(move || get_doctor_report_blocking()).await
}

#[tauri::command]
pub async fn check_update() -> Result<crate::updater::UpdateInfo, String> {
    run_blocking(crate::updater::check).await
}
#[tauri::command]
pub async fn install_update() -> Result<String, String> {
    run_blocking(crate::updater::install).await
}
#[tauri::command]
pub fn restart_after_update(app: tauri::AppHandle) -> Result<(), String> {
    if !crate::updater::installed() { return Err("No update has been installed".into()); }
    app.restart();
}

pub fn run_gui() -> Result<(), String> {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            check_update,
            install_update,
            restart_after_update,
            get_accounts,
            refresh_due_tokens,
            get_account_quota,
            switch_account_cmd,
            auto_best_cmd,
            save_account_cmd,
            delete_account_cmd,
            get_patch_targets,
            apply_patch_cmd,
            restore_patch_cmd,
            get_clean_categories,
            purge_clean_categories,
            get_doctor_report
        ])
        .run(tauri::generate_context!())
        .map_err(|e| format!("Tauri runtime error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::run_blocking;

    #[test]
    fn blocking_work_runs_off_the_calling_thread() {
        let caller = std::thread::current().id();
        let worker = tauri::async_runtime::block_on(run_blocking(|| {
            Ok(std::thread::current().id())
        })).unwrap();
        assert_ne!(caller, worker);
    }

    #[test]
    fn background_errors_reach_the_frontend() {
        let result = tauri::async_runtime::block_on(run_blocking(|| {
            Err::<(), _>("service unavailable".to_string())
        }));
        assert_eq!(result.unwrap_err(), "service unavailable");
    }
}
