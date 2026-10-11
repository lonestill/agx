use std::path::{Path, PathBuf};
use std::process::Command;
use colored::*;
use chrono::Utc;

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn export_backup(out_path: Option<String>) -> Result<PathBuf, String> {
    let home = home_dir();
    let gemini = home.join(".gemini");
    if !gemini.exists() {
        return Err("~/.gemini directory does not exist".to_string());
    }

    let default_name = format!("agx-backup-{}.tar.gz", Utc::now().format("%Y%m%d-%H%M%S"));
    let target = match out_path {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir().unwrap_or_else(|_| home.clone()).join(default_name),
    };

    let mut items_to_archive = Vec::new();
    let candidates = [
        ("accounts", gemini.join("accounts")),
        ("jetski-standalone-oauth-token", gemini.join("jetski-standalone-oauth-token")),
        ("config", gemini.join("config")),
        ("antigravity-cli/settings.json", gemini.join("antigravity-cli/settings.json")),
    ];

    for (name, path) in &candidates {
        if path.exists() {
            items_to_archive.push(*name);
        }
    }

    if items_to_archive.is_empty() {
        return Err("No accounts or configurations found to backup".to_string());
    }

    let mut cmd = Command::new("tar");
    cmd.arg("-czf")
       .arg(&target)
       .arg("-C")
       .arg(&gemini);

    for item in items_to_archive {
        cmd.arg(item);
    }

    let status = cmd.status().map_err(|e| format!("Failed to invoke tar: {}", e))?;
    if !status.success() {
        return Err(format!("tar exited with error code {:?}", status.code()));
    }

    println!("{} Vault backup successfully created: {}", "[ok]".green().bold(), target.display().to_string().cyan().bold());
    Ok(target)
}

pub fn import_backup(src_path: &str) -> Result<(), String> {
    let p = Path::new(src_path);
    if !p.exists() {
        return Err(format!("Backup archive '{}' not found", src_path));
    }

    let home = home_dir();
    let gemini = home.join(".gemini");
    if !gemini.exists() {
        std::fs::create_dir_all(&gemini).map_err(|e| e.to_string())?;
    }

    println!("{}", "=== Antigravity Vault Import ===".bold());
    println!("  Archive: {}", p.display().to_string().cyan());
    println!("  Target:  {}", gemini.display().to_string().yellow());

    print!("\n{} Extract and restore accounts and configurations? [y/N]: ", ">>".yellow().bold());
    use std::io::Write;
    let _ = std::io::stdout().flush();

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_ok() {
        let trimmed = input.trim().to_lowercase();
        if trimmed != "y" && trimmed != "yes" {
            println!("{} Import cancelled.", "[..]".yellow());
            return Ok(());
        }
    }

    let status = Command::new("tar")
        .arg("-xzf")
        .arg(p)
        .arg("-C")
        .arg(&gemini)
        .status()
        .map_err(|e| format!("Failed to extract archive: {}", e))?;

    if !status.success() {
        return Err("tar extraction failed".to_string());
    }

    println!("{} Configurations and accounts successfully restored!", "[ok]".green().bold());
    Ok(())
}
