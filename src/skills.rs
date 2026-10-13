use std::fs;
use std::path::PathBuf;
use colored::*;

#[derive(Debug, Clone)]
pub struct SkillInfo {
    pub name: String,
    pub folder_name: String,
    pub description: String,
    #[allow(dead_code)]
    pub path: PathBuf,
    pub valid: bool,
    pub issues: Vec<String>,
}

fn skills_dir() -> PathBuf {
    crate::account::gemini_dir().join("config/skills")
}

pub fn scan_skills() -> Vec<SkillInfo> {
    let dir = skills_dir();
    let mut skills = Vec::new();

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return skills,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let folder_name = entry.file_name().to_string_lossy().to_string();
            let skill_file = path.join("SKILL.md");
            let mut info = SkillInfo {
                name: folder_name.clone(),
                folder_name: folder_name.clone(),
                description: String::new(),
                path: path.clone(),
                valid: true,
                issues: Vec::new(),
            };

            if !skill_file.exists() {
                info.valid = false;
                info.issues.push("Missing SKILL.md".to_string());
                skills.push(info);
                continue;
            }

            match fs::read_to_string(&skill_file) {
                Ok(content) => {
                    // Parse YAML frontmatter between --- and ---
                    if content.starts_with("---") {
                        if let Some(end) = content[3..].find("---") {
                            let frontmatter = &content[3..3 + end];
                            for line in frontmatter.lines() {
                                let line = line.trim();
                                if let Some(stripped) = line.strip_prefix("name:") {
                                    info.name = stripped.trim().trim_matches('"').trim_matches('\'').to_string();
                                } else if let Some(stripped) = line.strip_prefix("description:") {
                                    info.description = stripped.trim().trim_matches('"').trim_matches('\'').to_string();
                                }
                            }
                        } else {
                            info.valid = false;
                            info.issues.push("Unclosed YAML frontmatter (missing closing ---)".to_string());
                        }
                    } else {
                        info.valid = false;
                        info.issues.push("No YAML frontmatter (must start with ---)".to_string());
                    }

                    if info.description.is_empty() {
                        info.valid = false;
                        info.issues.push("Missing or empty 'description' in frontmatter".to_string());
                    }
                }
                Err(e) => {
                    info.valid = false;
                    info.issues.push(format!("Failed to read SKILL.md: {}", e));
                }
            }

            skills.push(info);
        }
    }

    skills.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    skills
}

pub fn list_skills() {
    let skills = scan_skills();
    println!("{}", format!("=== Installed Skills ({}) ===", skills.len()).bold());
    println!("Directory: {}\n", skills_dir().display().to_string().dimmed());

    if skills.is_empty() {
        println!("  No skills found in {}", skills_dir().display());
        return;
    }

    println!("{:<25} {:<8} {}", "NAME".bold(), "STATUS".bold(), "DESCRIPTION".bold());
    println!("{}", "-".repeat(85).dimmed());

    for s in &skills {
        let status = if s.valid {
            "[OK]".green()
        } else {
            "[ERR]".red().bold()
        };

        let desc_trimmed = if s.description.len() > 50 {
            format!("{}...", &s.description[..47])
        } else {
            s.description.clone()
        };

        println!("{:<25} {:<8} {}", s.name.cyan().bold(), status, desc_trimmed);
        if !s.issues.is_empty() {
            for issue in &s.issues {
                println!("    └─ {}", issue.yellow());
            }
        }
    }
    println!();
}

pub fn check_skills() {
    println!("{}", "=== Skill Integrity Audit ===".bold());
    let skills = scan_skills();
    let mut err_count = 0;

    for s in &skills {
        if !s.valid {
            err_count += 1;
            println!("  {} {}:", "[FAIL]".red().bold(), s.folder_name.bold());
            for issue in &s.issues {
                println!("    └─ {}", issue.red());
            }
        } else {
            println!("  {} {} (name: '{}')", "[OK]".green().bold(), s.folder_name.bold(), s.name.cyan());
        }
    }

    println!("\n  Audit complete: {} total skills, {} healthy, {} issues found.\n",
        skills.len().to_string().bold(),
        (skills.len() - err_count).to_string().green().bold(),
        err_count.to_string().yellow().bold()
    );
}

pub fn create_skill(name: &str) -> Result<(), String> {
    let safe_name = name.trim().to_lowercase().replace(' ', "-");
    let target_dir = skills_dir().join(&safe_name);

    if target_dir.exists() {
        return Err(format!("Skill directory already exists: {}", target_dir.display()));
    }

    fs::create_dir_all(&target_dir).map_err(|e| format!("Failed to create directory: {}", e))?;
    fs::create_dir_all(target_dir.join("references")).ok();

    let skill_template = format!(
r#"---
name: "{}"
description: "Detailed description of what {} does and when to activate it."
---

# {} Skill

## Overview
Describe the core purpose and capabilities of this skill here.

## Instructions
1. Step-by-step guidance for the agent.
2. Important patterns and rules.

## References
Additional documentation can be placed in `references/`.
"#,
        safe_name, safe_name, safe_name
    );

    let skill_md_path = target_dir.join("SKILL.md");
    fs::write(&skill_md_path, skill_template).map_err(|e| format!("Failed to write SKILL.md: {}", e))?;

    println!("{} Created new skill '{}' at {}", "[created]".green().bold(), safe_name.bold(), skill_md_path.display());
    Ok(())
}
