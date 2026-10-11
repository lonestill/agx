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
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn find_cli_binary() -> Option<PathBuf> {
    // 1. which agy
    if let Ok(out) = Command::new("which").arg("agy").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let p = PathBuf::from(s);
            if p.exists() {
                return Some(p);
            }
        }
    }

    let cands = [
        home_dir().join(".local/bin/agy"),
        home_dir().join("bin/agy"),
        PathBuf::from("/usr/local/bin/agy"),
        PathBuf::from("/Applications/Antigravity.app/Contents/Resources/bin/agy"),
    ];

    for c in &cands {
        if c.exists() {
            return Some(c.clone());
        }
    }
    None
}

pub fn find_manager_binary() -> Option<PathBuf> {
    let cands = [
        PathBuf::from("/Applications/Antigravity.app/Contents/Resources/bin/language_server"),
        home_dir().join("Applications/Antigravity.app/Contents/Resources/bin/language_server"),
        PathBuf::from("/opt/Antigravity/resources/bin/language_server"),
    ];

    for c in &cands {
        if c.exists() {
            return Some(c.clone());
        }
    }
    None
}

pub fn find_ide_main_js() -> Option<PathBuf> {
    let cands = [
        PathBuf::from("/Applications/Antigravity IDE.app/Contents/Resources/app/out/main.js"),
        home_dir().join("Applications/Antigravity IDE.app/Contents/Resources/app/out/main.js"),
        PathBuf::from("/opt/antigravity-ide/resources/app/out/main.js"),
    ];

    for c in &cands {
        if c.exists() {
            return Some(c.clone());
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

pub fn patch_target(name: &str) -> Result<String, String> {
    match name {
        "cli" => {
            let path = find_cli_binary().ok_or_else(|| "CLI binary (agy) not found".to_string())?;
            patch_cli(&path)
        }
        "manager" => {
            let path = find_manager_binary().ok_or_else(|| "Manager binary (language_server) not found".to_string())?;
            patch_manager(&path)
        }
        "ide" => {
            let path = find_ide_main_js().ok_or_else(|| "IDE main.js not found".to_string())?;
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
            if errors.is_empty() {
                Ok(results.join(" · "))
            } else {
                Err(errors.join(" | "))
            }
        }
        _ => Err(format!("Unknown patch target: '{}'. Valid targets: cli, manager, ide, all", name)),
    }
}

pub fn restore_target(name: &str) -> Result<String, String> {
    match name {
        "cli" => {
            let path = find_cli_binary().ok_or_else(|| "CLI binary not found".to_string())?;
            restore_file(&path)
        }
        "manager" => {
            let path = find_manager_binary().ok_or_else(|| "Manager binary not found".to_string())?;
            restore_file(&path)
        }
        "ide" => {
            let path = find_ide_main_js().ok_or_else(|| "IDE main.js not found".to_string())?;
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
            if errors.is_empty() {
                Ok(results.join(" · "))
            } else {
                Err(errors.join(" | "))
            }
        }
        _ => Err(format!("Unknown target: '{}'", name)),
    }
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

fn restore_file(path: &Path) -> Result<String, String> {
    let bak = backup_path_for(path);
    if !bak.exists() {
        return Err(format!("No backup file found at {}", bak.display()));
    }
    fs::copy(&bak, path).map_err(|e| format!("Failed to restore from backup: {}", e))?;
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

    fs::write(path, &bytes).map_err(|e| format!("Failed to write patched CLI: {}", e))?;
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
            r"(?-u)\x80\x78\x08\x00\x74.\x48\x8b\x54\x24\x70\x48\x89\x50\x60"
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

    fs::write(path, &bytes).map_err(|e| format!("Failed to write patched Manager: {}", e))?;
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
    fs::write(path, &patched).map_err(|e| format!("Failed to write patched IDE: {}", e))?;
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
