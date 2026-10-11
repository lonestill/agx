use std::process::Command;
use colored::*;
use crate::account::{read_active_token, format_expiry};
use crate::patcher::{discover_targets, GateState};

pub fn print_system_info() {
    println!("{}", "=== Antigravity System Environment ===".bold());

    // 1. CLI version
    let cli_ver = match Command::new("agy").arg("--version").output() {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => "not found in PATH".dimmed().to_string(),
    };
    println!("  {:<24} {}", "CLI (agy):".cyan(), cli_ver);

    // 2. Desktop app
    let app_ver = match Command::new("defaults")
        .args(["read", "/Applications/Antigravity.app/Contents/Info.plist", "CFBundleShortVersionString"])
        .output()
    {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => "not installed or inaccessible".dimmed().to_string(),
    };
    println!("  {:<24} {}", "Desktop App (2.0):".cyan(), app_ver);

    // 3. IDE app
    let ide_ver = match Command::new("defaults")
        .args(["read", "/Applications/Antigravity IDE.app/Contents/Info.plist", "CFBundleShortVersionString"])
        .output()
    {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => "not installed".dimmed().to_string(),
    };
    println!("  {:<24} {}", "IDE App:".cyan(), ide_ver);

    // 4. Active Account
    println!("\n{}", "=== Active Account ===".bold());
    match read_active_token() {
        Ok(t) => {
            let exp = format_expiry(t.token.expiry.as_deref());
            let method = t.auth_method.as_str();
            println!("  {:<24} {}", "Auth Method:".cyan(), method);
            println!("  {:<24} {}", "Token Status:".cyan(), exp);
            if let Some(saved) = t.saved_at {
                println!("  {:<24} {}", "Profile Saved At:".cyan(), saved);
            }
        }
        Err(e) => {
            println!("  {}", e.yellow());
        }
    }

    // 5. Binary Gates
    println!("\n{}", "=== Binary Gates ===".bold());
    for target in discover_targets() {
        let status_str = match target.state {
            GateState::Patched => "PATCHED (Bypassed)".green().bold(),
            GateState::Unpatched => "UNPATCHED (Original)".yellow().bold(),
            GateState::NotFound => "NOT FOUND".dimmed(),
            GateState::Unknown => "UNKNOWN".dimmed(),
        };
        let p_str = target.path.map(|p| p.display().to_string()).unwrap_or_else(|| "none".to_string());
        println!("  {:<12} {:<24} [{}]", target.name.bold(), status_str, p_str.dimmed());
    }

    // 6. Project & Upstream
    println!("\n{}", "=== Repository ===".bold());
    println!("  {:<24} {}", "GitHub:".cyan(), "https://github.com/lonestill/agx".underline().bold());
    println!();
}

