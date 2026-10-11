use std::process::Command;
use std::time::Instant;
use colored::*;
use crate::account::{read_active_token, format_expiry};
use crate::patcher::{discover_targets, GateState};
use crate::cleaner::{scan_clean_categories, format_bytes};

#[derive(Debug, Clone, PartialEq)]
pub enum DoctorStatus {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone)]
pub struct DoctorItem {
    pub title: String,
    pub status: DoctorStatus,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct DoctorReport {
    pub items: Vec<DoctorItem>,
    pub ok_count: usize,
    pub warn_count: usize,
    pub fail_count: usize,
}

pub fn check_doctor() -> DoctorReport {
    let mut items = Vec::new();
    let mut ok_count = 0;
    let mut warn_count = 0;
    let mut fail_count = 0;

    // 1. Check Active Account & Token
    match read_active_token() {
        Ok(t) => {
            let exp = format_expiry(t.token.expiry.as_deref());
            if exp.contains("expired") {
                items.push(DoctorItem {
                    title: "Active OAuth Credentials".to_string(),
                    status: DoctorStatus::Warn,
                    detail: format!("Token is {}. Run 'agx account refresh' or re-login.", exp),
                });
                warn_count += 1;
            } else {
                items.push(DoctorItem {
                    title: "Active OAuth Credentials".to_string(),
                    status: DoctorStatus::Ok,
                    detail: format!("Method: {}, Status: {}", t.auth_method, exp),
                });
                ok_count += 1;
            }
        }
        Err(e) => {
            items.push(DoctorItem {
                title: "Active OAuth Credentials".to_string(),
                status: DoctorStatus::Fail,
                detail: format!("Credentials error: {}", e),
            });
            fail_count += 1;
        }
    }

    // 2. Network: Google OAuth API
    let t0 = Instant::now();
    let oauth_resp = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()
        .and_then(|c| c.get("https://oauth2.googleapis.com/tokeninfo").send().ok());

    match oauth_resp {
        Some(_) => {
            let elapsed = t0.elapsed().as_millis();
            items.push(DoctorItem {
                title: "Google OAuth API (oauth2.googleapis.com)".to_string(),
                status: DoctorStatus::Ok,
                detail: format!("Connected ({}ms)", elapsed),
            });
            ok_count += 1;
        }
        None => {
            items.push(DoctorItem {
                title: "Google OAuth API (oauth2.googleapis.com)".to_string(),
                status: DoctorStatus::Fail,
                detail: "Unreachable. Check internet / VPN proxy connection.".to_string(),
            });
            fail_count += 1;
        }
    }

    // 3. Network: CloudCode Quota API
    let t1 = Instant::now();
    let cloudcode_resp = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()
        .and_then(|c| c.get("https://cloudcode-pa.googleapis.com").send().ok());

    match cloudcode_resp {
        Some(_) => {
            let elapsed = t1.elapsed().as_millis();
            items.push(DoctorItem {
                title: "CloudCode Pa API (cloudcode-pa.googleapis.com)".to_string(),
                status: DoctorStatus::Ok,
                detail: format!("Connected ({}ms)", elapsed),
            });
            ok_count += 1;
        }
        None => {
            items.push(DoctorItem {
                title: "CloudCode Pa API (cloudcode-pa.googleapis.com)".to_string(),
                status: DoctorStatus::Warn,
                detail: "Unreachable or restricted. Internal quota checks may be delayed.".to_string(),
            });
            warn_count += 1;
        }
    }

    // 4. Binary Eligibility & Auth Gates
    let targets = discover_targets();
    let mut unpatched = Vec::new();
    for t in &targets {
        if t.state == GateState::Unpatched {
            unpatched.push(t.name);
        }
    }
    if unpatched.is_empty() {
        items.push(DoctorItem {
            title: "Binary Eligibility & Auth Gates".to_string(),
            status: DoctorStatus::Ok,
            detail: "All detected targets (CLI, Manager, IDE) are patched.".to_string(),
        });
        ok_count += 1;
    } else {
        items.push(DoctorItem {
            title: "Binary Eligibility & Auth Gates".to_string(),
            status: DoctorStatus::Warn,
            detail: format!("Unpatched components: {:?}. Run 'agx patch apply all'.", unpatched),
        });
        warn_count += 1;
    }

    // 5. Code signing check (macOS only)
    if cfg!(target_os = "macos") {
        let mut sign_ok = true;
        for t in &targets {
            if let Some(p) = &t.path {
                let out = Command::new("codesign").args(["-v", p.to_str().unwrap_or("")]).output();
                if let Ok(o) = out {
                    if !o.status.success() {
                        sign_ok = false;
                    }
                }
            }
        }
        if sign_ok {
            items.push(DoctorItem {
                title: "macOS Code Signatures".to_string(),
                status: DoctorStatus::Ok,
                detail: "All target binaries pass signature verification.".to_string(),
            });
            ok_count += 1;
        } else {
            items.push(DoctorItem {
                title: "macOS Code Signatures".to_string(),
                status: DoctorStatus::Warn,
                detail: "Invalid signature on patched binary. Run 'agx patch apply all' to re-sign.".to_string(),
            });
            warn_count += 1;
        }
    }

    // 6. Running background processes & helpers
    let mut lang_server_pids = Vec::new();
    let mut helper_pids = Vec::new();
    if cfg!(target_os = "windows") {
        let task_out = Command::new("tasklist").output().ok();
        if let Some(o) = task_out {
            let text = String::from_utf8_lossy(&o.stdout);
            for line in text.lines() {
                if line.contains("language_server") {
                    lang_server_pids.push("active".to_string());
                } else if line.contains("Antigravity") {
                    helper_pids.push("active".to_string());
                }
            }
        }
    } else {
        let ps_out = Command::new("ps").args(["-ax", "-o", "pid,command"]).output().ok();
        if let Some(o) = ps_out {
            let text = String::from_utf8_lossy(&o.stdout);
            for line in text.lines() {
                if line.contains("language_server") && !line.contains("grep") {
                    if let Some(pid) = line.split_whitespace().next() {
                        lang_server_pids.push(pid.to_string());
                    }
                } else if line.contains("Antigravity Helper") && !line.contains("grep") {
                    if let Some(pid) = line.split_whitespace().next() {
                        helper_pids.push(pid.to_string());
                    }
                }
            }
        }
    }
    items.push(DoctorItem {
        title: "Active Antigravity Processes".to_string(),
        status: DoctorStatus::Ok,
        detail: format!("Language servers: {}, Helper processes: {}", lang_server_pids.len(), helper_pids.len()),
    });
    ok_count += 1;

    // 7. Recoverable Disk Cache
    let cats = scan_clean_categories();
    let total_waste: u64 = cats.iter().map(|c| c.total_bytes).sum();
    if total_waste > 500 * 1024 * 1024 {
        items.push(DoctorItem {
            title: "Disk Cache Waste".to_string(),
            status: DoctorStatus::Warn,
            detail: format!("{} found. Run 'agx clean' to free up disk space.", format_bytes(total_waste)),
        });
        warn_count += 1;
    } else {
        items.push(DoctorItem {
            title: "Disk Cache Waste".to_string(),
            status: DoctorStatus::Ok,
            detail: format!("{} recoverable.", format_bytes(total_waste)),
        });
        ok_count += 1;
    }

    DoctorReport {
        items,
        ok_count,
        warn_count,
        fail_count,
    }
}

pub fn run_doctor() {
    println!("{}", "=== Antigravity System Doctor ===".bold());
    let report = check_doctor();

    for item in &report.items {
        let badge = match item.status {
            DoctorStatus::Ok => "[OK]".green().bold(),
            DoctorStatus::Warn => "[WARN]".yellow().bold(),
            DoctorStatus::Fail => "[FAIL]".red().bold(),
        };
        println!("  {:<45} {}", item.title.bold(), badge);
        println!("    └─ {}", item.detail);
    }

    println!("\n  {}", "-".repeat(50).dimmed());
    println!("  Doctor Summary: {} passed, {} warnings, {} failures\n",
        report.ok_count.to_string().green().bold(),
        report.warn_count.to_string().yellow().bold(),
        report.fail_count.to_string().red().bold()
    );
}
