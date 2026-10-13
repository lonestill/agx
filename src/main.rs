mod account;
mod quota;
mod patcher;
mod system;
mod cleaner;
mod vault;
mod tui;
mod doctor;
mod run;
mod stats;
mod skills;

use clap::{Parser, Subcommand};
use colored::*;
use std::path::PathBuf;
use account::*;
use quota::*;
use patcher::*;
use system::*;

#[derive(Parser)]
#[command(name = "agx")]
#[command(author = "Depraved")]
#[command(version = "1.3.4")]
#[command(
    about = "Universal high-performance Antigravity multitool (Accounts, Quotas, Cleaner, Vault, Binary Modding, Stats, Doctor, Skills, TUI)",
    long_about = "agx launches an interactive TUI dashboard by default when run without arguments.\nYou can also run dedicated subcommands for scriptable CLI execution.\n\nGitHub Repository: https://github.com/lonestill/agx"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}


#[derive(Subcommand)]
enum Commands {
    /// Launch the interactive TUI dashboard (default behavior)
    Tui,

    /// Manage Antigravity profiles and OAuth accounts
    #[command(visible_aliases = ["acct", "profile", "a"])]
    Account {
        #[command(subcommand)]
        cmd: AccountCommands,
    },

    /// Check Google AI / Antigravity quota and rate limits
    #[command(visible_aliases = ["usage", "limits", "q"])]
    Quota {
        /// Account name to check (defaults to active account)
        account: Option<String>,

        /// Check quota across all saved accounts
        #[arg(short, long)]
        all: bool,
    },

    /// Inspect and patch binary eligibility and auth gates
    #[command(visible_aliases = ["mod", "p"])]
    Patch {
        #[command(subcommand)]
        cmd: PatchCommands,
    },

    /// Scan and clean heavy caches and task logs (with confirmation)
    #[command(visible_aliases = ["purge", "gc"])]
    Clean,

    /// Manage configuration & account backup vault
    Vault {
        #[command(subcommand)]
        cmd: VaultCommands,
    },

    /// Export accounts and configurations to a .tar.gz vault
    Export {
        /// Output path for the archive
        file: Option<String>,
    },

    /// Import accounts and configurations from a .tar.gz vault
    Import {
        /// Path to the archive
        file: String,
    },

    /// Show system versions, environment, and gate status
    #[command(visible_aliases = ["sys", "env", "i"])]
    Info,

    /// Run Antigravity environment health checks and diagnostics
    #[command(visible_aliases = ["doc", "checkup"])]
    Doctor,

    /// Execute a command using an isolated temporary account without corrupting global state
    Run {
        /// Account name to use
        #[arg(short, long)]
        account: String,

        /// Command and arguments to execute
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },

    /// Calculate token usage and estimated API cost from conversation databases
    #[command(visible_aliases = ["tokens", "usage-stats"])]
    Stats,

    /// Manage Antigravity agent skills in ~/.gemini/config/skills/
    #[command(visible_aliases = ["skill", "sk"])]
    Skills {
        #[command(subcommand)]
        cmd: SkillsCommands,
    },
}

#[derive(Subcommand)]
enum SkillsCommands {
    /// List all installed skills and their frontmatter metadata
    #[command(visible_alias = "ls")]
    List,

    /// Audit installed skills for valid YAML frontmatter and file structure
    Check,

    /// Scaffold a new skill template directory
    New {
        /// Name of the new skill
        name: String,
    },
}


#[derive(Subcommand)]
enum AccountCommands {
    /// List all saved accounts and show active status
    #[command(visible_alias = "ls")]
    List,

    /// Show currently active account details
    #[command(visible_alias = "whoami")]
    Current,

    /// Switch active account to <name> or 'next'
    Switch {
        /// Account name or 'next'
        name: String,
    },

    /// Automatically find and switch to the account with the highest available quota
    #[command(visible_aliases = ["auto", "rotate"])]
    Best,

    /// Save current active credentials as a named account
    Save {
        /// Profile name to save as
        name: String,
    },

    /// Delete a saved account profile
    #[command(visible_alias = "rm")]
    Delete {
        /// Account name to delete
        name: String,
    },

    /// Force refresh OAuth2 access token for active or named account
    Refresh {
        /// Account name (defaults to active)
        name: Option<String>,
    },
}

#[derive(Subcommand)]
enum PatchCommands {
    /// Check gate status of all targets (cli, manager, ide)
    #[command(visible_aliases = ["st", "check"])]
    Status,

    /// Apply binary patch to target ('cli', 'manager', 'ide', or 'all')
    Apply {
        /// Target name
        #[arg(default_value = "all")]
        target: String,

        /// Explicit path to target file
        #[arg(long, short)]
        path: Option<PathBuf>,
    },

    /// Restore pristine original binary from backup
    Restore {
        /// Target name
        #[arg(default_value = "all")]
        target: String,

        /// Explicit path to target file
        #[arg(long, short)]
        path: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum VaultCommands {
    /// Export accounts and configurations to a .tar.gz backup
    Export {
        /// Output archive file path
        file: Option<String>,
    },

    /// Import accounts and configurations from a .tar.gz backup
    Import {
        /// Path to archive
        file: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        None | Some(Commands::Tui) => {
            if let Err(e) = tui::run_tui() {
                eprintln!("{} Failed to run TUI: {}", "[!!]".red().bold(), e);
                std::process::exit(1);
            }
        }
        Some(Commands::Account { cmd }) => handle_account(cmd),
        Some(Commands::Quota { account, all }) => handle_quota(account, all),
        Some(Commands::Patch { cmd }) => handle_patch(cmd),
        Some(Commands::Clean) => cleaner::run_interactive_clean(),
        Some(Commands::Vault { cmd }) => match cmd {
            VaultCommands::Export { file } => {
                let _ = vault::export_backup(file);
            }
            VaultCommands::Import { file } => {
                if let Err(e) = vault::import_backup(&file) {
                    eprintln!("{} {}", "[!!]".red().bold(), e);
                }
            }
        },
        Some(Commands::Export { file }) => {
            let _ = vault::export_backup(file);
        }
        Some(Commands::Import { file }) => {
            if let Err(e) = vault::import_backup(&file) {
                eprintln!("{} {}", "[!!]".red().bold(), e);
            }
        }
        Some(Commands::Info) => print_system_info(),
        Some(Commands::Doctor) => doctor::run_doctor(),
        Some(Commands::Run { account, command }) => {
            match run::run_with_account(&account, &command) {
                Ok(code) => std::process::exit(code),
                Err(e) => {
                    eprintln!("{} {}", "[!!]".red().bold(), e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Stats) => stats::print_token_stats(),
        Some(Commands::Skills { cmd }) => match cmd {
            SkillsCommands::List => skills::list_skills(),
            SkillsCommands::Check => skills::check_skills(),
            SkillsCommands::New { name } => {
                if let Err(e) = skills::create_skill(&name) {
                    eprintln!("{} {}", "[!!]".red().bold(), e);
                }
            }
        },
    }
}


fn handle_account(cmd: AccountCommands) {
    match cmd {
        AccountCommands::List => {
            println!("{}", "=== Saved Antigravity Accounts ===".bold());
            match list_accounts() {
                Ok(accounts) => {
                    if accounts.is_empty() {
                        println!("  {}", "No saved accounts found. Use 'agx account save <name>'".yellow());
                    } else {
                        for (name, token, is_active) in accounts {
                            let prefix = if is_active {
                                "* ".green().bold()
                            } else {
                                "  ".normal()
                            };
                            let name_styled = if is_active {
                                name.cyan().bold()
                            } else {
                                name.white()
                            };
                            let exp = format_expiry(token.token.expiry.as_deref());
                            let saved = token.saved_at.as_deref().unwrap_or("unknown");
                            println!("{:<2} {:<18} {:<24} (saved: {})", prefix, name_styled, exp, saved.dimmed());
                        }
                    }
                }
                Err(e) => eprintln!("{} {}", "[!!]".red().bold(), e),
            }
        }
        AccountCommands::Current => {
            match read_active_token() {
                Ok(token) => {
                    let accounts = list_accounts().unwrap_or_default();
                    let active_name = accounts
                        .iter()
                        .find(|(_, _, is_active)| *is_active)
                        .map(|(n, _, _)| n.as_str())
                        .unwrap_or("Current · unsaved");

                    println!("{}", "=== Active Account ===".bold());
                    println!("  {:<16} {}", "Profile:".cyan(), active_name.bold());
                    println!("  {:<16} {}", "Token Status:".cyan(), format_expiry(token.token.expiry.as_deref()));
                    println!("  {:<16} {}", "Auth Method:".cyan(), token.auth_method);
                    if let Some(exp) = token.token.expiry {
                        println!("  {:<16} {}", "Expires At:".cyan(), exp);
                    }
                }
                Err(e) => eprintln!("{} {}", "[!!]".red().bold(), e),
            }
        }
        AccountCommands::Switch { name } => {
            if let Err(e) = switch_account(&name) {
                eprintln!("{} {}", "[!!]".red().bold(), e);
                std::process::exit(1);
            }
        }
        AccountCommands::Best => {
            println!("{}", "Analyzing quota across all accounts...".dimmed());
            match switch_best() {
                Ok(msg) => println!("{} {}", "[ok]".green().bold(), msg),
                Err(e) => eprintln!("{} {}", "[!!]".red().bold(), e),
            }
        }
        AccountCommands::Save { name } => {
            if let Err(e) = save_account(&name) {
                eprintln!("{} {}", "[!!]".red().bold(), e);
                std::process::exit(1);
            }
        }
        AccountCommands::Delete { name } => {
            if let Err(e) = delete_account(&name) {
                eprintln!("{} {}", "[!!]".red().bold(), e);
                std::process::exit(1);
            }
        }
        AccountCommands::Refresh { name } => {
            let res = match name {
                Some(n) => {
                    let mut token = match load_account(&n) {
                        Ok(t) => t,
                        Err(e) => {
                            eprintln!("{} {}", "[!!]".red().bold(), e);
                            return;
                        }
                    };
                    match refresh_account_token(&mut token) {
                        Ok(_) => {
                            let _ = save_account(&n);
                            println!("{} Refreshed token for account '{}'", "[ok]".green().bold(), n.cyan());
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                }
                None => {
                    let mut token = match read_active_token() {
                        Ok(t) => t,
                        Err(e) => {
                            eprintln!("{} {}", "[!!]".red().bold(), e);
                            return;
                        }
                    };
                    match refresh_account_token(&mut token) {
                        Ok(_) => {
                            let _ = write_active_token(&token);
                            println!("{} Refreshed active token successfully", "[ok]".green().bold());
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                }
            };

            if let Err(e) = res {
                eprintln!("{} {}", "[!!]".red().bold(), e);
            }
        }
    }
}

fn handle_quota(account: Option<String>, all: bool) {
    if all {
        let accounts = match list_accounts() {
            Ok(a) => a,
            Err(e) => {
                eprintln!("{} {}", "[!!]".red().bold(), e);
                return;
            }
        };

        if accounts.is_empty() {
            println!("  {}", "No saved accounts to check. Use 'agx quota' for active token.".yellow());
            return;
        }

        for (name, mut token, is_active) in accounts {
            let active_marker = if is_active { " (active)" } else { "" };
            println!("\n{} Account: {}{}", "---".dimmed(), name.cyan().bold(), active_marker.green());
            display_account_quota(&mut token);
        }
    } else {
        let (name, mut token) = match account {
            Some(n) => {
                let t = match load_account(&n) {
                    Ok(tok) => tok,
                    Err(e) => {
                        eprintln!("{} {}", "[!!]".red().bold(), e);
                        return;
                    }
                };
                (n, t)
            }
            None => {
                let t = match read_active_token() {
                    Ok(tok) => tok,
                    Err(e) => {
                        eprintln!("{} {}", "[!!]".red().bold(), e);
                        return;
                    }
                };
                ("Active Account".to_string(), t)
            }
        };

        println!("\n{} Quota & Rate Limits: {}", "---".dimmed(), name.cyan().bold());
        display_account_quota(&mut token);
    }
}

fn display_account_quota(token: &mut StandaloneToken) {
    match fetch_quota_summary(token) {
        Ok(groups) => {
            if groups.is_empty() {
                println!("  {}", "No quota groups returned by Google API".dimmed());
                return;
            }

            println!("  {:<32} {:<24} {:<24}", "Model / Service".bold(), "5-Hour Limit".bold(), "Weekly Limit".bold());
            println!("  {}", "-".repeat(80).dimmed());

            for g in groups {
                let (five_pct, five_reset) = render_window_metric(g.five_hour.as_ref());
                let (week_pct, week_reset) = render_window_metric(g.weekly.as_ref());

                let five_str = format!("{} ({})", five_pct, five_reset);
                let week_str = format!("{} ({})", week_pct, week_reset);

                println!("  {:<32} {:<34} {:<34}", g.name.white(), five_str, week_str);
            }
        }
        Err(e) => {
            eprintln!("  {} {}", "[!!]".red(), e);
        }
    }
}

fn handle_patch(cmd: PatchCommands) {
    match cmd {
        PatchCommands::Status => {
            println!("{}", "=== Antigravity Binary Gate Status ===".bold());
            let targets = discover_targets();
            for t in targets {
                let status_str = match t.state {
                    GateState::Patched => "PATCHED (Bypassed)".green().bold(),
                    GateState::Unpatched => "UNPATCHED (Original)".yellow().bold(),
                    GateState::NotFound => "NOT FOUND".dimmed(),
                    GateState::Unknown => "UNKNOWN".dimmed(),
                };
                let backup_str = if t.has_backup {
                    "[backup: yes]".green()
                } else {
                    "[backup: no]".dimmed()
                };
                let p_str = t.path.map(|p| p.display().to_string()).unwrap_or_else(|| "none".to_string());
                println!("  {:<10} {:<24} {:<16} ({})", t.name.bold(), status_str, backup_str, p_str.dimmed());
            }
            println!();
        }
        PatchCommands::Apply { target, path } => {
            match patch_target_with_path(&target, path.as_deref()) {
                Ok(msg) => println!("{} {}", "[ok]".green().bold(), msg),
                Err(e) => {
                    eprintln!("{} Patch failed: {}", "[!!]".red().bold(), e);
                    std::process::exit(1);
                }
            }
        }
        PatchCommands::Restore { target, path } => {
            match restore_target_with_path(&target, path.as_deref()) {
                Ok(msg) => println!("{} {}", "[ok]".green().bold(), msg),
                Err(e) => {
                    eprintln!("{} Restore failed: {}", "[!!]".red().bold(), e);
                    std::process::exit(1);
                }
            }
        }
    }
}
