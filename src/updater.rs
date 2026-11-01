use std::{fs::File, io::{Read, Write}, path::Path, sync::{atomic::{AtomicBool, Ordering}, Mutex}, time::Duration};
use reqwest::blocking::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const API: &str = "https://api.github.com/repos/lonestill/agx/releases/latest";
const MAX_ARCHIVE: u64 = 128 * 1024 * 1024;
const MAX_BINARY: u64 = 512 * 1024 * 1024;
static INSTALL_LOCK: Mutex<()> = Mutex::new(());
static INSTALLED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Deserialize)]
struct Asset { name: String, browser_download_url: String }
#[derive(Deserialize)]
struct Release { tag_name: String, #[serde(default)] draft: bool, #[serde(default)] prerelease: bool, assets: Vec<Asset> }
#[derive(Serialize)]
pub struct UpdateInfo { pub current_version: String, pub latest_version: String, pub available: bool, pub installable: bool, pub installed: bool, pub message: String }

fn client() -> Result<Client, String> {
    Client::builder().user_agent(concat!("agx/", env!("CARGO_PKG_VERSION")))
        .https_only(true).connect_timeout(Duration::from_secs(10)).timeout(Duration::from_secs(120))
        .build().map_err(|e| e.to_string())
}
fn fetch_release(client: &Client) -> Result<Release, String> {
    client.get(API).header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(20)).send().map_err(|e| format!("Cannot reach GitHub: {e}"))?
        .error_for_status().map_err(|e| format!("GitHub release check failed: {e}"))?
        .json().map_err(|e| format!("Invalid GitHub release: {e}"))
}
fn newer(tag: &str, current: &str) -> Result<bool, String> {
    let remote = Version::parse(tag.strip_prefix('v').unwrap_or(tag)).map_err(|_| "Invalid release version")?;
    let local = Version::parse(current).map_err(|_| "Invalid installed version")?;
    Ok(remote.pre.is_empty() && remote > local)
}
fn target() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Ok("macos-arm64"), ("macos", "x86_64") => Ok("macos-x86_64"),
        ("linux", "x86_64") => Ok("linux-x86_64"), ("windows", "x86_64") => Ok("windows-x86_64"),
        _ => Err("No release build for this platform".into()),
    }
}
fn assets(release: &Release) -> Result<(&Asset, &Asset), String> {
    let extension = if cfg!(windows) { "zip" } else { "tar.gz" };
    let name = format!("agx-{}-{}.{}", release.tag_name, target()?, extension);
    let binary = release.assets.iter().find(|asset| asset.name == name)
        .ok_or_else(|| format!("Release has no {name}"))?;
    let checksum = release.assets.iter().find(|asset| asset.name == "SHA256SUMS.txt")
        .ok_or("Release has no SHA256SUMS.txt")?;
    for asset in [binary, checksum] {
        let expected = format!("https://github.com/lonestill/agx/releases/download/{}/{}", release.tag_name, asset.name);
        if asset.browser_download_url != expected { return Err("Unexpected release download URL".into()); }
    }
    Ok((binary, checksum))
}
fn describe(release: &Release) -> Result<UpdateInfo, String> {
    let available = !release.draft && !release.prerelease && newer(&release.tag_name, env!("CARGO_PKG_VERSION"))?;
    let asset_result = assets(release);
    let installable = available && asset_result.is_ok();
    let message = if available {
        match asset_result { Ok(_) => format!("Version {} is available", release.tag_name), Err(error) => error }
    } else { format!("Up to date · GitHub {}", release.tag_name) };
    Ok(UpdateInfo { current_version: env!("CARGO_PKG_VERSION").into(), latest_version: release.tag_name.clone(), available, installable, installed: INSTALLED.load(Ordering::SeqCst), message })
}
pub fn check() -> Result<UpdateInfo, String> { let client = client()?; describe(&fetch_release(&client)?) }

fn download(client: &Client, url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let response = client.get(url).send().map_err(|e| format!("Download failed: {e}"))?
        .error_for_status().map_err(|e| format!("Download failed: {e}"))?;
    if response.content_length().is_some_and(|length| length > limit) { return Err("Release file is too large".into()); }
    let mut data = Vec::new();
    response.take(limit + 1).read_to_end(&mut data).map_err(|e| e.to_string())?;
    if data.len() as u64 > limit { return Err("Release file is too large".into()); }
    Ok(data)
}
fn expected_checksum(text: &str, name: &str) -> Result<String, String> {
    let mut matching = text.lines().filter_map(|line| {
        let mut fields = line.split_whitespace();
        let hash = fields.next()?;
        let file = fields.next()?.trim_start_matches('*');
        if file != name { return None; }
        Some(hash.to_ascii_lowercase())
    });
    let hash = matching.next().ok_or("Archive checksum is missing")?;
    if matching.next().is_some() || hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid archive checksum".into());
    }
    Ok(hash)
}
fn verify(data: &[u8], expected: &str) -> Result<(), String> {
    let actual = format!("{:x}", Sha256::digest(data));
    if actual != expected { return Err("SHA-256 mismatch. Update was not installed.".into()); }
    Ok(())
}
fn extract(data: &[u8], path: &Path) -> Result<(), String> {
    let mut output = File::create(path).map_err(|e| e.to_string())?;
    if cfg!(windows) {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|e| e.to_string())?;
        let entry = zip.by_name("agx.exe").map_err(|_| "Archive has no agx.exe")?;
        if entry.size() == 0 || entry.size() > MAX_BINARY { return Err("Invalid binary size".into()); }
        let count = std::io::copy(&mut entry.take(MAX_BINARY + 1), &mut output).map_err(|e| e.to_string())?;
        if count == 0 || count > MAX_BINARY { return Err("Invalid binary size".into()); }
    } else {
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(data));
        let mut found = false;
        for entry in archive.entries().map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.path().map_err(|e| e.to_string())?.as_ref() != Path::new("agx") { continue; }
            if found || !entry.header().entry_type().is_file() || entry.size() == 0 || entry.size() > MAX_BINARY { return Err("Invalid binary in archive".into()); }
            let count = std::io::copy(&mut entry.take(MAX_BINARY + 1), &mut output).map_err(|e| e.to_string())?;
            if count == 0 || count > MAX_BINARY { return Err("Invalid binary size".into()); }
            found = true;
        }
        if !found { return Err("Archive has no agx binary".into()); }
    }
    output.flush().map_err(|e| e.to_string())?;
    output.sync_all().map_err(|e| e.to_string())?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn install() -> Result<String, String> {
    let _guard = INSTALL_LOCK.try_lock().map_err(|_| "An update is already running")?;
    if INSTALLED.load(Ordering::SeqCst) { return Err("Update installed. Restart agx first.".into()); }
    let client = client()?;
    let release = fetch_release(&client)?;
    let info = describe(&release)?;
    if !info.available { return Err("No newer stable version is available".into()); }
    let (asset, sums) = assets(&release)?;
    let checksums = download(&client, &sums.browser_download_url, 1024 * 1024)?;
    let checksum = expected_checksum(std::str::from_utf8(&checksums).map_err(|_| "Invalid checksums file")?, &asset.name)?;
    let archive = download(&client, &asset.browser_download_url, MAX_ARCHIVE)?;
    verify(&archive, &checksum)?;
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let binary = temp.path().join(if cfg!(windows) { "agx.exe" } else { "agx" });
    extract(&archive, &binary)?;
    self_replace::self_replace(&binary).map_err(|e| format!("Cannot replace agx. Check install directory permissions: {e}"))?;
    INSTALLED.store(true, Ordering::SeqCst);
    Ok(release.tag_name)
}
pub fn installed() -> bool { INSTALLED.load(Ordering::SeqCst) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn version_comparison_prevents_downgrades() {
        assert!(!newer("v1.3.6", "1.4.0").unwrap());
        assert!(!newer("v1.4.0", "1.4.0").unwrap());
        assert!(newer("v1.10.0", "1.9.0").unwrap());
        assert!(!newer("v2.0.0-beta.1", "1.4.0").unwrap());
        assert!(newer("not-a-version", "1.4.0").is_err());
    }
    #[test] fn checksum_must_match_exact_asset() {
        let hash = "a".repeat(64);
        assert_eq!(expected_checksum(&format!("{hash}  file.tar.gz\n"), "file.tar.gz").unwrap(), hash);
        assert!(expected_checksum(&format!("{hash}  other.tar.gz"), "file.tar.gz").is_err());
        assert!(expected_checksum("bad  file.tar.gz", "file.tar.gz").is_err());
        assert!(verify(b"corrupted", &hash).is_err());
        assert!(verify(b"abc", &format!("{:x}", Sha256::digest(b"abc"))).is_ok());
    }
    #[test] fn selects_current_platform_and_requires_checksum() {
        let extension = if cfg!(windows) { "zip" } else { "tar.gz" };
        let name = format!("agx-v9.0.0-{}.{}", target().unwrap(), extension);
        let asset = |name: String| Asset { browser_download_url: format!("https://github.com/lonestill/agx/releases/download/v9.0.0/{name}"), name };
        let mut release = Release { tag_name: "v9.0.0".into(), draft: false, prerelease: false, assets: vec![asset(name.clone()),asset("SHA256SUMS.txt".into())] };
        assert_eq!(assets(&release).unwrap().0.name, name);
        release.assets[0].browser_download_url = "https://example.com/agx".into();
        assert!(assets(&release).is_err());
        release.assets.clear(); assert!(assets(&release).is_err());
    }
    #[test] fn extracts_only_the_binary_and_ignores_other_entries() {
        if cfg!(windows) { return; }
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut builder = tar::Builder::new(encoder);
        for (name, bytes) in [("README.md", b"readme".as_slice()), ("agx", b"test-binary".as_slice())] {
            let mut header = tar::Header::new_gnu(); header.set_size(bytes.len() as u64); header.set_mode(0o755); header.set_cksum();
            builder.append_data(&mut header, name, bytes).unwrap();
        }
        let data = builder.into_inner().unwrap().finish().unwrap();
        let dir = tempfile::tempdir().unwrap(); let binary = dir.path().join("agx");
        extract(&data, &binary).unwrap();
        assert_eq!(std::fs::read(binary).unwrap(), b"test-binary");
        assert!(!dir.path().join("README.md").exists());
        assert!(extract(b"invalid archive", &dir.path().join("bad")).is_err());
    }
}
