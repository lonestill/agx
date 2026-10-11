use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use regex::bytes::Regex;

pub const BAK_EXT: &str = ".agybak";

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum GateState {
    Unpatched,
    Patched,
    NotFound,
    Unknown,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TargetInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub path: Option<PathBuf>,
    pub state: GateState,
    pub has_backup: bool,
}

pub fn discover_targets() -> Vec<TargetInfo> {
    vec![
        inspect_cli_target(),
        inspect_manager_target(),
        inspect_ide_target(),
    ]
}

pub fn inspect_cli_target() -> TargetInfo {
    let path = find_cli_binary();
    let (state, has_backup) = match &path {
        Some(p) => (check_cli_gate(p), p.with_extension(format!("{}{}", p.extension().and_then(|s| s.to_str()).unwrap_or(""), BAK_EXT)).exists() || PathBuf::from(format!("{}{}", p.display(), BAK_EXT)).exists()),
        None => (GateState::NotFound, false),
    };

    TargetInfo {
        name: "cli",
        description: "Antigravity CLI (agy) - eligibility gate",
        path,
        state,
        has_backup,
    }
}

pub fn inspect_manager_target() -> TargetInfo {
    let path = find_manager_binary();
    let (state, has_backup) = match &path {
        Some(p) => (check_manager_gate(p), PathBuf::from(format!("{}{}", p.display(), BAK_EXT)).exists()),
        None => (GateState::NotFound, false),
    };

    TargetInfo {
        name: "manager",
        description: "Antigravity Manager (language_server) - hasValidAuth gate",
        path,
        state,
        has_backup,
    }
}

pub fn inspect_ide_target() -> TargetInfo {
    let path = find_ide_main_js();
    let (state, has_backup) = match &path {
        Some(p) => (check_ide_gate(p), PathBuf::from(format!("{}{}", p.display(), BAK_EXT)).exists()),
        None => (GateState::NotFound, false),
    };

    TargetInfo {
        name: "ide",
        description: "Antigravity IDE (main.js) - isGoogleInternal gate",
        path,
        state,
        has_backup,
    }
}

fn home_dir() -> PathBuf {
    crate::account::home_dir()
}

fn which_binary(names: &[&str]) -> Option<PathBuf> {
    let cmd = if cfg!(target_os = "windows") { "where" } else { "which" };
    for name in names {
        if let Ok(out) = Command::new(cmd).arg(name).output() {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                for line in text.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        let p = PathBuf::from(trimmed);
                        if p.is_file() {
                            return Some(p);
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn find_running_process_path(name: &str) -> Option<PathBuf> {
    let script = format!(
        "$p = Get-Process -Name '*{}*' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Path -ErrorAction SilentlyContinue; if ($p) {{ $p }}",
        name
    );
    if let Ok(out) = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    let p = PathBuf::from(trimmed);
                    if p.is_file() {
                        return Some(p);
                    }
                }
            }
        }
    }
    None
}

#[allow(dead_code)]
#[cfg(not(target_os = "windows"))]
fn find_running_process_path(_name: &str) -> Option<PathBuf> {
    None
}

fn get_candidate_app_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let home = home_dir();

    if let Ok(val) = std::env::var("ANTIGRAVITY_HOME") {
        dirs.push(PathBuf::from(val));
    }

    dirs.push(PathBuf::from("/Applications/Antigravity.app"));
    dirs.push(PathBuf::from("/Applications/Antigravity IDE.app"));
    dirs.push(PathBuf::from("/Applications/Google Antigravity.app"));
    dirs.push(home.join("Applications/Antigravity.app"));
    dirs.push(home.join("Applications/Antigravity IDE.app"));

    dirs.push(PathBuf::from("/opt/Antigravity"));
    dirs.push(PathBuf::from("/opt/antigravity"));
    dirs.push(PathBuf::from("/opt/antigravity-ide"));
    dirs.push(PathBuf::from("/opt/Google/Antigravity"));
    dirs.push(home.join(".local/share/Antigravity"));
    dirs.push(home.join(".local/share/antigravity"));

    let mut win_bases = Vec::new();
    if let Some(la) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        win_bases.push(la.join("Programs"));
        win_bases.push(la);
    }
    if let Some(ap) = std::env::var_os("APPDATA").map(PathBuf::from) {
        win_bases.push(ap.join("Programs"));
        win_bases.push(ap);
    }
    if let Some(pf) = std::env::var_os("ProgramFiles").map(PathBuf::from) {
        win_bases.push(pf);
    }
    if let Some(pf86) = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from) {
        win_bases.push(pf86);
    }
    win_bases.push(PathBuf::from("C:\\Program Files"));
    win_bases.push(PathBuf::from("C:\\Program Files (x86)"));
    win_bases.push(home.join("AppData/Local/Programs"));
    win_bases.push(home.join("AppData/Local"));
    win_bases.push(home.join("AppData/Roaming"));

    let folder_names = [
        "Antigravity",
        "Antigravity IDE",
        "antigravity",
        "antigravity-ide",
        "Google/Antigravity",
        "Google/Antigravity IDE",
        "Google Antigravity",
        "Google",
    ];

    for base in &win_bases {
        for name in &folder_names {
            dirs.push(base.join(name));
        }
    }

    dirs.push(PathBuf::from("C:\\Antigravity"));
    dirs.push(PathBuf::from("C:\\Antigravity IDE"));
    dirs.push(PathBuf::from("D:\\Antigravity"));
    dirs.push(PathBuf::from("D:\\Antigravity IDE"));
    dirs.push(home.join(".antigravity"));
    dirs.push(home.join(".gemini/antigravity"));

    dirs
}

pub fn find_cli_binary() -> Option<PathBuf> {
    for var in &["AGX_CLI_PATH", "AGX_AGY_PATH"] {
        if let Ok(v) = std::env::var(var) {
            let p = PathBuf::from(v);
            if p.is_file() {
                return Some(p);
            }
        }
    }

    if let Some(p) = which_binary(&["agy.exe", "agy.cmd", "agy.bat", "agy"]) {
        return Some(p);
    }

    let home = home_dir();
    let direct_cands = [
        home.join(".local/bin/agy.exe"),
        home.join(".local/bin/agy.cmd"),
        home.join(".local/bin/agy"),
        home.join("bin/agy.exe"),
        home.join("bin/agy.cmd"),
        home.join("bin/agy"),
        PathBuf::from("/usr/local/bin/agy"),
    ];

    for c in &direct_cands {
        if c.is_file() {
            return Some(c.clone());
        }
    }

    let candidates = get_candidate_app_dirs();
    let relative_paths = [
        "resources/bin/agy.exe",
        "resources/bin/agy.cmd",
        "resources/bin/agy",
        "bin/agy.exe",
        "bin/agy.cmd",
        "bin/agy",
        "agy.exe",
        "agy.cmd",
        "agy",
        "Contents/Resources/bin/agy",
        "Contents/Resources/bin/agy.exe",
    ];

    for base in &candidates {
        for rel in &relative_paths {
            let p = base.join(rel);
            if p.is_file() {
                return Some(p);
            }
        }
    }

    None
}

pub fn find_manager_binary() -> Option<PathBuf> {
    for var in &["AGX_MANAGER_PATH", "AGX_LANGUAGE_SERVER_PATH"] {
        if let Ok(v) = std::env::var(var) {
            let p = PathBuf::from(v);
            if p.is_file() {
                return Some(p);
            }
        }
    }

    if let Some(p) = which_binary(&["language_server.exe", "language_server"]) {
        return Some(p);
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(p) = find_running_process_path("language_server") {
            return Some(p);
        }
        if let Some(antigravity_exe) = find_running_process_path("Antigravity") {
            if let Some(parent) = antigravity_exe.parent() {
                let check_sub = [
                    parent.join("resources/bin/language_server.exe"),
                    parent.join("resources/bin/language_server"),
                    parent.join("bin/language_server.exe"),
                    parent.join("bin/language_server"),
                    parent.join("language_server.exe"),
                ];
                for c in &check_sub {
                    if c.is_file() {
                        return Some(c.clone());
                    }
                }
            }
        }
    }

    let candidates = get_candidate_app_dirs();
    let relative_paths = [
        "resources/bin/language_server.exe",
        "resources/bin/language_server",
        "resources/app/bin/language_server.exe",
        "resources/app/bin/language_server",
        "bin/language_server.exe",
        "bin/language_server",
        "language_server.exe",
        "language_server",
        "Contents/Resources/bin/language_server",
        "Contents/Resources/bin/language_server.exe",
    ];

    for base in &candidates {
        for rel in &relative_paths {
            let p = base.join(rel);
            if p.is_file() {
                return Some(p);
            }
        }

        if base.is_dir() {
            if let Ok(entries) = fs::read_dir(base) {
                for entry in entries.flatten() {
                    let ep = entry.path();
                    if ep.is_dir() {
                        let name = ep.file_name().unwrap_or_default().to_string_lossy();
                        if name.starts_with("app-") || name == "resources" || name == "bin" {
                            for rel in &relative_paths {
                                let p = ep.join(rel);
                                if p.is_file() {
                                    return Some(p);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(local_appdata) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        for search_root in &[local_appdata.clone(), local_appdata.join("Programs")] {
            if search_root.is_dir() {
                if let Ok(entries) = fs::read_dir(search_root) {
                    for entry in entries.flatten() {
                        let ep = entry.path();
                        if ep.is_dir() {
                            let name_lower = ep.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                            if name_lower.contains("antigravity") {
                                for rel in &relative_paths {
                                    let p = ep.join(rel);
                                    if p.is_file() {
                                        return Some(p);
                                    }
                                }
                                if let Ok(sub_entries) = fs::read_dir(&ep) {
                                    for sub_entry in sub_entries.flatten() {
                                        let sep = sub_entry.path();
                                        if sep.is_dir() {
                                            for rel in &relative_paths {
                                                let p = sep.join(rel);
                                                if p.is_file() {
                                                    return Some(p);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

pub fn find_ide_main_js() -> Option<PathBuf> {
    for var in &["AGX_IDE_PATH", "AGX_MAIN_JS_PATH"] {
        if let Ok(v) = std::env::var(var) {
            let p = PathBuf::from(v);
            if p.is_file() {
                return Some(p);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(antigravity_exe) = find_running_process_path("Antigravity") {
            if let Some(parent) = antigravity_exe.parent() {
                let check_sub = [
                    parent.join("resources/app/out/main.js"),
                    parent.join("resources/app/main.js"),
                    parent.join("app/out/main.js"),
                ];
                for c in &check_sub {
                    if c.is_file() {
                        return Some(c.clone());
                    }
                }
            }
        }
    }

    let candidates = get_candidate_app_dirs();
    let relative_paths = [
        "resources/app/out/main.js",
        "resources/app/main.js",
        "resources/app.asar.unpacked/out/main.js",
        "resources/app.asar.unpacked/main.js",
        "app/out/main.js",
        "app/main.js",
        "Contents/Resources/app/out/main.js",
        "Contents/Resources/app/main.js",
    ];

    for base in &candidates {
        for rel in &relative_paths {
            let p = base.join(rel);
            if p.is_file() {
                return Some(p);
            }
        }

        if base.is_dir() {
            if let Ok(entries) = fs::read_dir(base) {
                for entry in entries.flatten() {
                    let ep = entry.path();
                    if ep.is_dir() {
                        let name = ep.file_name().unwrap_or_default().to_string_lossy();
                        if name.starts_with("app-") || name == "resources" {
                            for rel in &relative_paths {
                                let p = ep.join(rel);
                                if p.is_file() {
                                    return Some(p);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(local_appdata) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        for search_root in &[local_appdata.clone(), local_appdata.join("Programs")] {
            if search_root.is_dir() {
                if let Ok(entries) = fs::read_dir(search_root) {
                    for entry in entries.flatten() {
                        let ep = entry.path();
                        if ep.is_dir() {
                            let name_lower = ep.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                            if name_lower.contains("antigravity") {
                                for rel in &relative_paths {
                                    let p = ep.join(rel);
                                    if p.is_file() {
                                        return Some(p);
                                    }
                                }
                                if let Ok(sub_entries) = fs::read_dir(&ep) {
                                    for sub_entry in sub_entries.flatten() {
                                        let sep = sub_entry.path();
                                        if sep.is_dir() {
                                            for rel in &relative_paths {
                                                let p = sep.join(rel);
                                                if p.is_file() {
                                                    return Some(p);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

pub fn check_cli_gate(path: &Path) -> GateState {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(_) => return GateState::Unknown,
    };

    // arm64 patterns
    let arm64_unpatched = Regex::new(
        r"(?-u)\x02\x20\x40\x39[\x02\x22\x42\x62\x82\xa2\xc2\xe2].[\x00-\x07]\x37...[\x94-\x97]\xe0\x07\x0e\xa9\xe2\x0f\x0f\xa9\xe4\x17\x10\xa9\xe6\x1f\x11\xa9"
    ).unwrap();
    let arm64_patched = Regex::new(
        r"(?-u)\x22\x00\x80\x52[\x02\x22\x42\x62\x82\xa2\xc2\xe2].[\x00-\x07]\x37...[\x94-\x97]\xe0\x07\x0e\xa9\xe2\x0f\x0f\xa9\xe4\x17\x10\xa9\xe6\x1f\x11\xa9"
    ).unwrap();

    // x64 patterns
    let x64_unpatched = Regex::new(
        r"(?-u)\x48\x85\xc0\x0f\x84....\x80\x78\x08\x00\x0f\x85....\xe8....\x48\x89\x84\x24\xd8\x00\x00\x00\x48\x89\x9c\x24\xe0\x00\x00\x00\x48\x89\x8c\x24\xe8\x00\x00\x00"
    ).unwrap();
    let x64_patched = Regex::new(
        r"(?-u)\x48\x85\xc0\x0f\x84....\x48\x85\xc0\x90\x0f\x85....\xe8....\x48\x89\x84\x24\xd8\x00\x00\x00\x48\x89\x9c\x24\xe0\x00\x00\x00\x48\x89\x8c\x24\xe8\x00\x00\x00"
    ).unwrap();

    if arm64_patched.is_match(&bytes) || x64_patched.is_match(&bytes) {
        GateState::Patched
    } else if arm64_unpatched.is_match(&bytes) || x64_unpatched.is_match(&bytes) {
        GateState::Unpatched
    } else {
        GateState::NotFound
    }
}

pub fn check_manager_gate(path: &Path) -> GateState {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(_) => return GateState::Unknown,
    };

    let arm64_unpatched = Regex::new(
        r"(?-u)\x03\x20\x40\x39[\x03\x23\x43\x63\x83\xa3\xc3\xe3].[\x00-\x07]\x36\xe3\x13\x48\xa9\x03\x10\x06\xa9"
    ).unwrap();
    let arm64_patched = Regex::new(
        r"(?-u)\x23\x00\x80\x52\x03\x20\x00\x39\xe3\x13\x48\xa9\x03\x10\x06\xa9"
    ).unwrap();

    let x64_unpatched = Regex::new(
        r"(?-u)\x80\x78\x08\x00\x74.\x48\x8b\x54\x24\x70\x48\x89\x50\x60"
    ).unwrap();
    let x64_patched = Regex::new(
        r"(?-u)\xc6\x40\x08\x01\x90\x90\x48\x8b\x54\x24\x70\x48\x89\x50\x60"
    ).unwrap();

    if arm64_patched.is_match(&bytes) || x64_patched.is_match(&bytes) {
        GateState::Patched
    } else if arm64_unpatched.is_match(&bytes) || x64_unpatched.is_match(&bytes) {
        GateState::Unpatched
    } else {
        GateState::NotFound
    }
}

pub fn check_ide_gate(path: &Path) -> GateState {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(_) => return GateState::Unknown,
    };

    let ide_unpatched = Regex::new(r"(resetIsTierGCPTos\(\),)this\.[A-Za-z_$0-9]+\.isGoogleInternal").unwrap();
    let ide_patched = Regex::new(r"resetIsTierGCPTos\(\),true").unwrap();

    if ide_patched.is_match(&bytes) {
        GateState::Patched
    } else if ide_unpatched.is_match(&bytes) {
        GateState::Unpatched
    } else {
        GateState::NotFound
    }
}

pub fn patch_target_with_path(name: &str, custom_path: Option<&Path>) -> Result<String, String> {
    match name {
        "cli" => {
            let path = match custom_path {
                Some(p) => p.to_path_buf(),
                None => find_cli_binary().ok_or_else(|| {
                    "CLI binary (agy) not found. Set AGX_CLI_PATH or use --path <path>".to_string()
                })?,
            };
            patch_cli(&path)
        }
        "manager" => {
            let path = match custom_path {
                Some(p) => p.to_path_buf(),
                None => find_manager_binary().ok_or_else(|| {
                    "Manager binary (language_server) not found. Close Antigravity and check path, or set AGX_MANAGER_PATH / use --path <path>".to_string()
                })?,
            };
            patch_manager(&path)
        }
        "ide" => {
            let path = match custom_path {
                Some(p) => p.to_path_buf(),
                None => find_ide_main_js().ok_or_else(|| {
                    "IDE main.js not found. Set AGX_IDE_PATH or use --path <path>".to_string()
                })?,
            };
            patch_ide(&path)
        }
        "all" => {
            let mut results = Vec::new();
            let mut errors = Vec::new();
            if let Some(p) = find_cli_binary() {
                match patch_cli(&p) { Ok(m) => results.push(m), Err(e) => errors.push(e) }
            }
            if let Some(p) = find_manager_binary() {
                match patch_manager(&p) { Ok(m) => results.push(m), Err(e) => errors.push(e) }
            }
            if let Some(p) = find_ide_main_js() {
                match patch_ide(&p) { Ok(m) => results.push(m), Err(e) => errors.push(e) }
            }
            if results.is_empty() && errors.is_empty() {
                Err("No components (CLI, Manager, IDE) found to patch. Close Antigravity and set AGX_MANAGER_PATH or use --path.".to_string())
            } else if errors.is_empty() {
                Ok(results.join(" · "))
            } else if !results.is_empty() {
                Ok(format!("{} (Skipped/Errors: {})", results.join(" · "), errors.join("; ")))
            } else {
                Err(errors.join(" | "))
            }
        }
        _ => Err(format!("Unknown patch target: '{}'. Valid targets: cli, manager, ide, all", name)),
    }
}

pub fn patch_target(name: &str) -> Result<String, String> {
    patch_target_with_path(name, None)
}

pub fn restore_target_with_path(name: &str, custom_path: Option<&Path>) -> Result<String, String> {
    match name {
        "cli" => {
            let path = match custom_path {
                Some(p) => p.to_path_buf(),
                None => find_cli_binary().ok_or_else(|| "CLI binary not found".to_string())?,
            };
            restore_file(&path)
        }
        "manager" => {
            let path = match custom_path {
                Some(p) => p.to_path_buf(),
                None => find_manager_binary().ok_or_else(|| "Manager binary not found".to_string())?,
            };
            restore_file(&path)
        }
        "ide" => {
            let path = match custom_path {
                Some(p) => p.to_path_buf(),
                None => find_ide_main_js().ok_or_else(|| "IDE main.js not found".to_string())?,
            };
            restore_file(&path)
        }
        "all" => {
            let mut results = Vec::new();
            let mut errors = Vec::new();
            if let Some(p) = find_cli_binary() {
                match restore_file(&p) { Ok(m) => results.push(m), Err(e) => errors.push(e) }
            }
            if let Some(p) = find_manager_binary() {
                match restore_file(&p) { Ok(m) => results.push(m), Err(e) => errors.push(e) }
            }
            if let Some(p) = find_ide_main_js() {
                match restore_file(&p) { Ok(m) => results.push(m), Err(e) => errors.push(e) }
            }
            if results.is_empty() && errors.is_empty() {
                Err("No backups (.agybak) found to restore".to_string())
            } else if errors.is_empty() {
                Ok(results.join(" · "))
            } else {
                Err(errors.join(" | "))
            }
        }
        _ => Err(format!("Unknown target: '{}'", name)),
    }
}

pub fn restore_target(name: &str) -> Result<String, String> {
    restore_target_with_path(name, None)
}

fn backup_path_for(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}{}", path.display(), BAK_EXT))
}

fn make_backup(path: &Path) -> Result<PathBuf, String> {
    let bak = backup_path_for(path);
    if !bak.exists() {
        fs::copy(path, &bak).map_err(|e| format!("Failed to create backup at {}: {}", bak.display(), e))?;
    }
    Ok(bak)
}

fn safe_write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Ok(metadata) = fs::metadata(path) {
        let mut perms = metadata.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        if perms.readonly() {
            perms.set_readonly(false);
            let _ = fs::set_permissions(path, perms);
        }
    }

    match fs::write(path, bytes) {
        Ok(_) => Ok(()),
        Err(e) => {
            #[cfg(target_os = "windows")]
            {
                let temp_bak = path.with_extension(format!("agytmp_{}", std::process::id()));
                if fs::rename(path, &temp_bak).is_ok() {
                    let write_res = fs::write(path, bytes);
                    if write_res.is_ok() {
                        let _ = fs::remove_file(&temp_bak);
                        return Ok(());
                    } else {
                        let _ = fs::rename(&temp_bak, path);
                    }
                }
            }
            Err(format!(
                "Failed to write file {}: {}. Close Antigravity and any background language_server processes, then retry.",
                path.display(),
                e
            ))
        }
    }
}

fn restore_file(path: &Path) -> Result<String, String> {
    let bak = backup_path_for(path);
    if !bak.exists() {
        return Err(format!("No backup file found at {}", bak.display()));
    }
    let bytes = fs::read(&bak).map_err(|e| format!("Failed to read backup: {}", e))?;
    safe_write_file(path, &bytes)?;
    re_codesign_if_mac(path);
    Ok(format!("Restored {}", path.file_name().unwrap_or_default().to_string_lossy()))
}

pub fn patch_cli(path: &Path) -> Result<String, String> {
    let state = check_cli_gate(path);
    if state == GateState::Patched {
        return Ok("CLI is already patched".to_string());
    }
    if state != GateState::Unpatched {
        return Err(format!("Gate signature not found in CLI (status: {:?})", state));
    }

    make_backup(path)?;
    let mut bytes = fs::read(path).map_err(|e| e.to_string())?;

    let arm64_unpatched = Regex::new(
        r"(?-u)\x02\x20\x40\x39[\x02\x22\x42\x62\x82\xa2\xc2\xe2].[\x00-\x07]\x37...[\x94-\x97]\xe0\x07\x0e\xa9\xe2\x0f\x0f\xa9\xe4\x17\x10\xa9\xe6\x1f\x11\xa9"
    ).unwrap();

    let mut applied = false;
    if let Some(m) = arm64_unpatched.find(&bytes) {
        let off = m.start();
        bytes[off..off + 4].copy_from_slice(b"\x22\x00\x80\x52");
        applied = true;
    } else {
        let x64_unpatched = Regex::new(
            r"(?-u)\x48\x85\xc0\x0f\x84....\x80\x78\x08\x00\x0f\x85....\xe8....\x48\x89\x84\x24\xd8\x00\x00\x00\x48\x89\x9c\x24\xe0\x00\x00\x00\x48\x89\x8c\x24\xe8\x00\x00\x00"
        ).unwrap();
        if let Some(m) = x64_unpatched.find(&bytes) {
            let off = m.start() + 9;
            bytes[off..off + 4].copy_from_slice(b"\x48\x85\xc0\x90");
            applied = true;
        }
    }

    if !applied {
        return Err("Failed to resolve unique patch location in CLI".to_string());
    }

    safe_write_file(path, &bytes)?;
    re_codesign_if_mac(path);
    Ok("CLI patched (eligibility check bypassed)".to_string())
}

pub fn patch_manager(path: &Path) -> Result<String, String> {
    let state = check_manager_gate(path);
    if state == GateState::Patched {
        return Ok("Manager is already patched".to_string());
    }
    if state != GateState::Unpatched {
        return Err(format!("Gate signature not found in Manager (status: {:?})", state));
    }

    make_backup(path)?;
    let mut bytes = fs::read(path).map_err(|e| e.to_string())?;

    let arm64_unpatched = Regex::new(
        r"(?-u)\x03\x20\x40\x39[\x03\x23\x43\x63\x83\xa3\xc3\xe3].[\x00-\x07]\x36\xe3\x13\x48\xa9\x03\x10\x06\xa9"
    ).unwrap();

    let mut applied = false;
    if let Some(m) = arm64_unpatched.find(&bytes) {
        let off = m.start();
        bytes[off..off + 8].copy_from_slice(b"\x23\x00\x80\x52\x03\x20\x00\x39");
        applied = true;
    } else {
        let x64_unpatched = Regex::new(
            r"(?-u)\x80\x78\x08\x00\x74.\x48\x8b\x54\x24.\x48\x89\x50\x60"
        ).unwrap();
        if let Some(m) = x64_unpatched.find(&bytes) {
            let off = m.start();
            bytes[off..off + 6].copy_from_slice(b"\xc6\x40\x08\x01\x90\x90");
            applied = true;
        }
    }

    if !applied {
        return Err("Failed to resolve unique patch location in Manager".to_string());
    }

    safe_write_file(path, &bytes)?;
    re_codesign_if_mac(path);
    Ok("Manager patched (hasValidAuth=true)".to_string())
}

pub fn patch_ide(path: &Path) -> Result<String, String> {
    let state = check_ide_gate(path);
    if state == GateState::Patched {
        return Ok("IDE is already patched".to_string());
    }
    if state != GateState::Unpatched {
        return Err("IDE gate pattern not found in main.js".to_string());
    }

    make_backup(path)?;
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let ide_re = Regex::new(r"(resetIsTierGCPTos\(\),)this\.[A-Za-z_$0-9]+\.isGoogleInternal").unwrap();

    let patched = ide_re.replace(&bytes, &b"${1}true"[..]);
    safe_write_file(path, &patched)?;

    // Purge IDE JS caches invalidated by the patch
    let home = home_dir();
    let mut cache_dirs = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        cache_dirs.push(appdata.join("Antigravity IDE/CachedData"));
        cache_dirs.push(appdata.join("Antigravity IDE/Code Cache/js"));
    }
    cache_dirs.push(home.join("AppData/Roaming/Antigravity IDE/CachedData"));
    cache_dirs.push(home.join("AppData/Roaming/Antigravity IDE/Code Cache/js"));
    cache_dirs.push(home.join("Library/Application Support/Antigravity IDE/CachedData"));
    cache_dirs.push(home.join("Library/Application Support/Antigravity IDE/Code Cache/js"));
    cache_dirs.push(home.join(".config/Antigravity IDE/CachedData"));
    cache_dirs.push(home.join(".config/Antigravity IDE/Code Cache/js"));

    for c in cache_dirs {
        if c.is_dir() {
            let _ = fs::remove_dir_all(&c);
        }
    }

    Ok("IDE patched (isGoogleInternal=true)".to_string())
}

fn re_codesign_if_mac(path: &Path) {
    if cfg!(target_os = "macos") {
        let _ = Command::new("/usr/bin/codesign")
            .args(["--force", "--sign", "-", "--preserve-metadata=identifier,entitlements,flags,runtime"])
            .arg(path)
            .status();
    }
}
