# Changes in agx (Cross-Platform & Windows Fixes)

## Summary
Comprehensive cross-platform compatibility fixes resolving token calculation, conversation database scanning, skills directory resolution, and dynamic binary location reporting on Windows.

---

### 1. `src/stats.rs` — Cross-Platform Path Resolution & Robust SQLite Concurrency
- **Fixed Home Path Resolution:** Replaced `std::env::var("HOME").unwrap_or_else(|_| "/Users/a123".to_string())` with `crate::account::gemini_dir().join("antigravity/conversations")`. On Windows environments where `%HOME%` is not defined, paths previously resolved to non-existent locations, resulting in `0 conversation databases` scanned.
- **WAL Concurrency & Lock Handling:** Added `busy_timeout(Duration::from_millis(1500))` on open SQLite database connections.
- **Immutable URI Fallback:** Added an automated URI fallback opening mode (`file:<path>?mode=ro&immutable=1` with `SQLITE_OPEN_URI`) to cleanly bypass active WAL index (`.db-shm` / `.db-wal`) locks on Windows when Antigravity IDE is actively writing to session databases.
- **Code Cleanliness:** Removed unused `std::path::PathBuf` import.

---

### 2. `src/skills.rs` — Dynamic Skills Path
- **Fixed Hardcoded User Path:** Updated `skills_dir()` from the hardcoded `/Users/a123` fallback to use `crate::account::gemini_dir().join("config/skills")`, correctly locating user skills across Windows (`%USERPROFILE%`), macOS, and Linux.

---

### 3. `src/tui.rs` — Dynamic Binary Path Discovery
- **Dynamic Toolkit Location:** In `render_system_tab`, replaced the static string (`~/.local/bin/agx (/Users/a123/...)`) with dynamic resolution via `std::env::current_exe()`, accurately displaying the binary's actual location on all platforms.

---

### 4. `README.md` — Documentation Updates
- Removed obsolete warnings and notes stating that `agx stats` is not supported on Windows across English, Russian, and Hebrew documentation sections.
