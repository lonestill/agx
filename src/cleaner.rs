use std::fs;
use std::path::{Path, PathBuf};
use colored::*;

#[derive(Debug, Clone)]
pub struct CleanCategory {
    #[allow(dead_code)]
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub paths: Vec<PathBuf>,
    pub total_bytes: u64,
    pub enabled: bool,
}

impl CleanCategory {
    pub fn size_formatted(&self) -> String {
        format_bytes(self.total_bytes)
    }
}

pub fn format_bytes(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb >= 1024.0 {
        format!("{:.2} GB", mb / 1024.0)
    } else {
        format!("{:.1} MB", mb)
    }
}

fn home_dir() -> PathBuf {
    crate::account::home_dir()
}

/// Hardcoded safety check to guarantee we NEVER delete chats, code projects, or transcripts.
pub fn is_path_safe_to_delete(path: &Path) -> bool {
    let s = path.to_string_lossy();
    let s_norm = s.replace('\\', "/");

    // 1. Blacklist: instant rejection if it touches any user data or transcripts
    if s_norm.contains("/conversations")
        || s_norm.contains("/scratch")
        || s_norm.contains("/accounts")
        || s_norm.contains("/config")
        || s_norm.contains("transcript")
        || s_norm.contains(".user_uploaded")
        || s_norm.contains(".git")
        || s_norm.ends_with(".db")
        || s_norm.ends_with(".vscdb")
        || (s_norm.ends_with(".json") && s_norm.contains("/accounts/"))
    {
        return false;
    }

    // 2. Strict Whitelist: must belong to known disposable cache / temp categories
    s_norm.contains("Cache")
        || s_norm.contains("GPUCache")
        || s_norm.contains("Code Cache")
        || s_norm.contains(".tempmediaStorage")
        || s_norm.ends_with("/.system_generated/tasks")
        || (s_norm.ends_with("/tasks") && s_norm.contains("/.system_generated/"))
        || s_norm.ends_with("/crashes")
        || s_norm.ends_with("/cache")
}

pub fn scan_clean_categories() -> Vec<CleanCategory> {
    let home = home_dir();
    let mut categories = Vec::new();

    // 1. Electron & Chromium Caches (macOS, Windows, Linux)
    let mut electron_paths = vec![
        home.join("Library/Caches/com.google.antigravity"),
        home.join("Library/Caches/com.google.antigravity-ide"),
        home.join("Library/Application Support/Antigravity/Cache"),
        home.join("Library/Application Support/Antigravity/Code Cache"),
        home.join("Library/Application Support/Antigravity/GPUCache"),
        home.join("Library/Application Support/Antigravity IDE/Cache"),
        home.join("Library/Application Support/Antigravity IDE/Code Cache"),
        home.join("Library/Application Support/Antigravity IDE/GPUCache"),
    ];

    if let Some(local_appdata) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        electron_paths.push(local_appdata.join("Antigravity/Cache"));
        electron_paths.push(local_appdata.join("Antigravity/Code Cache"));
        electron_paths.push(local_appdata.join("Antigravity/GPUCache"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        electron_paths.push(appdata.join("Antigravity IDE/Cache"));
        electron_paths.push(appdata.join("Antigravity IDE/Code Cache"));
        electron_paths.push(appdata.join("Antigravity IDE/GPUCache"));
    }

    let (existing_electron, electron_bytes) = gather_existing_and_size(&electron_paths);
    categories.push(CleanCategory {
        id: "electron_cache",
        title: "Electron & Web Caches",
        description: "Chromium V8 compilation cache, GPU shader cache, and web HTTP caches (regenerates automatically)",
        paths: existing_electron,
        total_bytes: electron_bytes,
        enabled: true,
    });

    // 2. Temporary Media Storage (.tempmediaStorage in brain)
    // Disposable agent temporary screenshots & media, DOES NOT touch chats or user uploaded files
    let mut temp_media_paths = Vec::new();
    let brain_dir = home.join(".gemini/antigravity/brain");
    if brain_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&brain_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let media = p.join(".tempmediaStorage");
                    if media.is_dir() {
                        temp_media_paths.push(media);
                    }
                }
            }
        }
    }
    let (existing_media, media_bytes) = gather_existing_and_size(&temp_media_paths);
    categories.push(CleanCategory {
        id: "temp_media",
        title: "Temporary Media & Screenshots",
        description: "Agent transient media buffer (.tempmediaStorage). Leaves artifacts, transcripts, and uploads intact",
        paths: existing_media,
        total_bytes: media_bytes,
        enabled: true,
    });

    // 3. Background Task Output Logs (.system_generated/tasks only!)
    // Finished terminal command execution outputs. STRICTLY leaves transcript.jsonl, messages, and steps intact.
    let mut task_log_paths = Vec::new();
    if brain_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&brain_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let tasks = p.join(".system_generated/tasks");
                    if tasks.is_dir() {
                        task_log_paths.push(tasks);
                    }
                }
            }
        }
    }
    let (existing_tasks, task_bytes) = gather_existing_and_size(&task_log_paths);
    categories.push(CleanCategory {
        id: "task_logs",
        title: "Background Task Output Logs",
        description: "Terminal process stdout/stderr logs (.system_generated/tasks). Preserves all chat transcripts & history",
        paths: existing_tasks,
        total_bytes: task_bytes,
        enabled: true,
    });

    // 4. CLI temp caches and crash logs
    let cli_cache_paths = vec![
        home.join(".gemini/antigravity/cache"),
        home.join(".gemini/antigravity-cli/cache"),
        home.join(".gemini/antigravity/crashes"),
        home.join(".gemini/antigravity-cli/crashes"),
    ];
    let (existing_cli, cli_bytes) = gather_existing_and_size(&cli_cache_paths);
    categories.push(CleanCategory {
        id: "cli_cache",
        title: "CLI Caches & Crash Dumps",
        description: "Temporary CLI cache directories and crash telemetry logs",
        paths: existing_cli,
        total_bytes: cli_bytes,
        enabled: true,
    });

    categories
}

fn gather_existing_and_size(paths: &[PathBuf]) -> (Vec<PathBuf>, u64) {
    let mut existing = Vec::new();
    let mut total = 0u64;
    for p in paths {
        if p.exists() && is_path_safe_to_delete(p) {
            existing.push(p.clone());
            total += dir_size(p);
        }
    }
    (existing, total)
}

fn dir_size(path: &Path) -> u64 {
    if path.is_file() {
        return path.metadata().map(|m| m.len()).unwrap_or(0);
    }
    let mut total = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                total += p.metadata().map(|m| m.len()).unwrap_or(0);
            } else if p.is_dir() {
                total += dir_size(&p);
            }
        }
    }
    total
}

pub fn purge_category(cat: &CleanCategory) -> Result<u64, String> {
    let mut deleted = 0u64;
    for path in &cat.paths {
        if !is_path_safe_to_delete(path) {
            eprintln!("{} Safety block: refused to delete non-whitelisted path: {}", "[SAFETY]".red().bold(), path.display());
            continue;
        }

        if path.exists() {
            let size = dir_size(path);
            if path.is_dir() {
                if let Err(e) = fs::remove_dir_all(path) {
                    eprintln!("{} Failed to remove {}: {}", "[!!]".yellow(), path.display(), e);
                } else {
                    deleted += size;
                }
            } else if path.is_file() {
                if let Err(e) = fs::remove_file(path) {
                    eprintln!("{} Failed to remove {}: {}", "[!!]".yellow(), path.display(), e);
                } else {
                    deleted += size;
                }
            }
        }
    }
    Ok(deleted)
}

pub fn run_interactive_clean() {
    println!("{}", "=== Antigravity Storage & Cache Scanner ===".bold());
    println!("{}", "Safety status: Protected paths (conversations, scratch, accounts, transcripts) are strictly blacklisted.\n".dimmed());

    let categories = scan_clean_categories();

    let mut total_recoverable = 0u64;
    for (i, cat) in categories.iter().enumerate() {
        println!("  [{}] {} ({})", (i + 1).to_string().cyan().bold(), cat.title.bold(), cat.size_formatted().green());
        println!("      Description: {}", cat.description.dimmed());
        println!("      Target paths ({} items):", cat.paths.len());
        for p in cat.paths.iter().take(3) {
            println!("        - {}", p.display().to_string().dimmed());
        }
        if cat.paths.len() > 3 {
            println!("        - ... and {} more", cat.paths.len() - 3);
        }
        total_recoverable += cat.total_bytes;
        println!();
    }

    println!("  {}", "-".repeat(60).dimmed());
    println!("  Total recoverable disk space: {}", format_bytes(total_recoverable).green().bold());

    if total_recoverable == 0 {
        println!("\n  {}", "Storage is already clean. Nothing to purge.".cyan());
        return;
    }

    print!("\n{} Delete the above cache and log files? [y/N]: ", ">>".yellow().bold());
    use std::io::Write;
    let _ = std::io::stdout().flush();

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_ok() {
        let trimmed = input.trim().to_lowercase();
        if trimmed == "y" || trimmed == "yes" {
            let mut freed = 0u64;
            for cat in &categories {
                if let Ok(b) = purge_category(cat) {
                    freed += b;
                }
            }
            println!("\n{} Successfully freed {}", "[ok]".green().bold(), format_bytes(freed).cyan().bold());
        } else {
            println!("\n{} Operation cancelled. No files were touched.", "[..]".yellow());
        }
    }
}
