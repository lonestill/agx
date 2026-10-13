# agx

[![Latest Release](https://img.shields.io/github/v/release/lonestill/agx?style=flat-square&color=blue)](https://github.com/lonestill/agx/releases/latest)
[![CI Release](https://img.shields.io/github/actions/workflow/status/lonestill/agx/release.yml?style=flat-square&label=CI)](https://github.com/lonestill/agx/actions)
[![License](https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-blue?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-green?style=flat-square)](https://github.com/lonestill/agx/releases/latest)
[![Rust](https://img.shields.io/badge/rust-stable-orange?style=flat-square&logo=rust)](https://www.rust-lang.org)

**Universal high-performance multitool for Google Antigravity & Jetski** — multi-account switcher, quota auto-rotator, binary gate patcher, token analytics, and interactive TUI.

<p align="center">
  <img src="assets/tui-accounts.png" alt="agx interactive TUI dashboard" width="850">
</p>

[English](#english) • [Русский](#русский) • [עברית](#עברית)

---

## English

A native, zero-bullshit multitool for Google Antigravity & Jetski. Written in Rust.

Launches a 7-tab interactive TUI dashboard by default, with dedicated CLI subcommands for automation and scripting.

**100% Cross-Platform**: Full native feature parity across **Windows**, **macOS**, and **Linux** — every CLI command, TUI tab, background path resolution, and credential discovery operates identically on all operating systems.

### Installation

**One-line automatic install (macOS & Linux):**

```bash
curl -fsSL https://raw.githubusercontent.com/lonestill/agx/master/install.sh | bash
```

**Windows:**

Download `agx-v*-windows-x86_64.zip` from [GitHub Releases](https://github.com/lonestill/agx/releases/latest) and place `agx.exe` into any folder in your `PATH` (e.g. `%USERPROFILE%\.local\bin`).

Or download pre-built native binaries from [GitHub Releases](https://github.com/lonestill/agx/releases/latest):

| OS / Architecture | Package |
|---|---|
| **macOS Universal** (M1–M4 & Intel) | `agx-v*-macos-universal.tar.gz` |
| **macOS Apple Silicon** (arm64) | `agx-v*-macos-arm64.tar.gz` |
| **macOS Intel** (x86_64) | `agx-v*-macos-x86_64.tar.gz` |
| **Linux** (x86_64) | `agx-v*-linux-x86_64.tar.gz` |
| **Windows** (x86_64 MSVC) | `agx-v*-windows-x86_64.zip` |

Or compile from source:

```bash
git clone https://github.com/lonestill/agx.git
cd agx
cargo build --release
# Binary placed at target/release/agx (or target/release/agx.exe on Windows)
```

### What it does

> **Full Cross-Platform Parity**: Every feature below works seamlessly across **Windows**, **macOS**, and **Linux** with automated native path discovery (`%USERPROFILE%` / `$HOME`), non-blocking SQLite WAL access, and multi-platform credential extraction.

1. **Multi-Account & Quota Auto-Rotation**:
   - Stores multiple Google OAuth profiles (`~/.gemini/accounts/` or `%USERPROFILE%\.gemini\accounts\`).
   - Automatically auto-discovers credentials across platforms: active token files, IDE state DB (`state.vscdb`), and Windows Credential Manager.
   - Live 5-hour rolling and weekly quota countdowns.
   - `agx account best` automatically rotates to the account with the highest available quota.
2. **Token & Cost Analytics (`agx stats`)**:
   - Directly inspects Antigravity conversation SQLite databases on Windows, macOS, and Linux.
   - Decodes protobuf metadata at ~25,000 generations/second.
   - Non-blocking SQLite reader with immutable URI mode (`mode=ro&immutable=1`) and busy timeouts — works concurrently while the IDE is running.
   - Calculates prompt cache hit rates and estimated API cost per model.
3. **Surgical Storage Cleaner (`agx clean`)**:
   - Reclaims ~1 GB of disposable Chromium V8 caches, GPU shaders, and background task process logs across all systems.
   - **Guaranteed safe**: strict hardcoded blacklist permanently blocks chats, transcripts, projects (`scratch/`), and tokens.
   - Interactive checkbox selection with explicit confirmation (`[y/N]`).
4. **Binary Gate Patcher (`agx patch`)**:
   - Inspects and patches authentication & eligibility gates in `agy` CLI, `language_server`, and IDE `main.js`.
   - Automatic `.agybak` backups and ad-hoc code-signing (`codesign -f -s -` on macOS).
5. **System Health Doctor (`agx doctor`)**:
   - Pings Google OAuth2 and CloudCode endpoints with live RTT measurement.
   - Audits token expiry, helper processes, and code signatures.
6. **Agent Skills Manager (`agx skills`)**:
   - Scans, validates frontmatter, audits integrity, and scaffolds new agent skills in `~/.gemini/config/skills/`.
7. **Isolated Command Runner (`agx run`)**:
   - Temporarily swaps credentials, runs a command under another profile, and restores your active token even if the process exits with an error.

### Quick Start

```bash
# Launch interactive TUI (Default)
agx

# Run system health check
agx doctor

# Check conversation token usage & estimated API cost
agx stats

# Switch to the account with the most remaining quota
agx account best

# Clean disposable caches safely (prompts [y/N])
agx clean
```

### TUI Tabs & Hotkeys

| Tab | Name | Key Features |
|---|---|---|
| `[1]` | **Accounts** | Profile list, live 5h/weekly quotas, `Enter`/`s` to switch, `b` for auto-best, `r` to reload |
| `[2]` | **Gates** | Binary patch states, `Enter`/`p` to patch, `a` to patch all, `R` to restore original `.agybak` |
| `[3]` | **Cleaner** | Disposable cache categories, `Space` to toggle, `Enter`/`c` to clean selected |
| `[4]` | **Stats** | Model breakdown, input/cached/output tokens, cost calculation, `r` to refresh |
| `[5]` | **Skills** | Two-pane skill browser, audit status (`[OK]`/`[ERR]`), `j`/`k` to browse details, `r` to rescan |
| `[6]` | **Doctor** | API latency pings, signature checks, process audit, `r` to re-run |
| `[7]` | **System** | Application versions, file paths, and repository info |

**Global Navigation**: `1`-`7` jump to tab • `Tab` / `Shift+Tab` cycle tabs • `j`/`k` or `↑`/`↓` navigate items • `q`/`Esc` exit.

<p align="center">
  <img src="assets/tui-stats.png" alt="Token & Cost Analytics" width="49%">
  <img src="assets/tui-gates.png" alt="Binary Gates & Modding" width="49%">
</p>
<p align="center">
  <img src="assets/tui-cleaner.png" alt="Storage Cleaner" width="49%">
  <img src="assets/tui-system.png" alt="System Information" width="49%">
</p>

### CLI Reference

```bash
agx                             # Interactive TUI dashboard
agx account ls                  # List saved profiles
agx account switch <name>       # Switch active profile
agx account best                # Auto-switch to best quota
agx account save <name>         # Snapshot active credentials
agx quota [--all]               # Check quota from terminal
agx stats                       # Token & cost summary
agx doctor                      # System health check
agx skills ls                   # List installed agent skills
agx skills check                # Audit YAML frontmatter
agx skills new <name>           # Scaffold new skill
agx run -a <account> -- <cmd>   # Isolated one-shot execution
agx clean                       # Interactive cache purge
agx patch status                # Check binary gates
agx patch apply all             # Patch CLI, Manager, and IDE
agx export [file.tar.gz]        # Export vault backup
agx import <file.tar.gz>        # Restore vault backup
agx info                        # Environment & versions
```

---

## Русский

Нативный швейцарский нож для Google Antigravity и Jetski на Rust. Без корпоративной воды и раздутого UI.

По умолчанию запускает 7-вкладочный интерактивный TUI-дашборд. Любая команда также доступна через CLI для скриптов и автоматизации.

**100% Кроссплатформенность**: Полный паритет возможностей на **Windows**, **macOS** и **Linux** — все функции, команды, автоматический поиск путей и TUI работают одинаково надёжно на любой ОС без исключений.

### Установка

Готовые бинарники для всех платформ лежат в [GitHub Releases](https://github.com/lonestill/agx/releases/latest) (macOS Universal/ARM/Intel, Linux x86_64, Windows x64).

**Быстрая установка в одну строку (macOS и Linux):**

```bash
curl -fsSL https://raw.githubusercontent.com/lonestill/agx/master/install.sh | bash
```

**Windows:**
Скачайте архив `agx-v*-windows-x86_64.zip` со страницы [GitHub Releases](https://github.com/lonestill/agx/releases/latest) и поместите `agx.exe` в любую папку из переменной `PATH` (например, `%USERPROFILE%\.local\bin`).

Либо сборка из исходников:

```bash
git clone https://github.com/lonestill/agx.git
cd agx
cargo build --release
```

### Основные возможности

> **Все возможности работают на всех системах**: Никаких ограничений и вырезанных команд — всё от аналитики токенов до патчера бинарников и TUI работает на **Windows**, **macOS** и **Linux**.

1. **Мультиаккаунты и авторотация квот**:
   - Управление профилями Google OAuth в `~/.gemini/accounts/` (`%USERPROFILE%\.gemini\accounts\`).
   - Автоматический поиск токенов: из активных файлов, баз IDE (`state.vscdb`) и Windows Credential Manager.
   - Мониторинг 5-часовых и недельных скользящих лимитов с таймером сброса.
   - `agx account best` моментально находит аккаунт с максимальной свободной квотой и переключается на него.
2. **Аналитика токенов и затрат (`agx stats`)**:
   - Читает базы данных переписок Antigravity на всех поддерживаемых ОС.
   - Парсит varint/protobuf метаданные со скоростью ~25 000 генераций/сек.
   - Неблокирующее чтение баз SQLite через URI `mode=ro&immutable=1` с `busy_timeout` — работает даже при открытом IDE на Windows без конфликтов блокировок.
   - Считает процент попадания в промпт-кэш и расчётную стоимость по моделям.
3. **Безопасная очистка кэша (`agx clean`)**:
   - Находит около 1 GB мусора на любой ОС: кэш Chromium V8, шейдеры GPU, логи stdout/stderr фоновых задач.
   - **Гарантия безопасности**: жесткий блэклист в коде навсегда блокирует доступ к базам чатов, транскриптам диалогов, папкам проектов (`scratch/`) и токенам.
   - Выбор категорий чекбоксами и строгое подтверждение перед удалением (`[y/N]`).
4. **Патчер бинарников (`agx patch`)**:
   - Находит и снимает проверки доступности и авторизации в `agy`, `language_server` и `main.js`.
   - Автоматически создает бэкапы `.agybak` и переподписывает код (ad-hoc через `codesign` на macOS).
5. **Диагностика системы (`agx doctor`)**:
   - Замеряет пинг до Google OAuth и CloudCode API в миллисекундах.
   - Проверяет срок действия токенов, запущенные процессы и целостность подписей на всех системах.
6. **Менеджер скиллов (`agx skills`)**:
   - Просмотр, аудит структуры и создание новых скиллов агента в `~/.gemini/config/skills/`.
7. **Изолированный запуск (`agx run`)**:
   - Временно подменяет токен на указанный профиль, выполняет команду и гарантированно возвращает активный токен на место, даже если команда упала с ошибкой.

### Управление в TUI

- `1`..`7` — прямой переход на вкладку
- `Tab` / `Shift+Tab` — переключение вкладок по кругу
- `j` / `k` или `↑` / `↓` — выбор строки в текущей вкладке
- `Enter` / `s` — переключить аккаунт (вкладка 1) / применить патч (вкладка 2)
- `b` — автовыбор аккаунта с лучшей квотой (вкладка 1)
- `Space` — переключить чекбокс очистки (вкладка 3)
- `c` — удалить выбранный кэш (вкладка 3)
- `r` — обновить данные (квоты, токены, скиллы или диагностику)
- `q` / `Esc` — выход

---

## עברית

כלי עבודה מקורי, מהיר ומדויק עבור Google Antigravity ו-Jetski, כתוב ב-Rust. ללא שכבות מיותרות או ממשק מסורבל.

בהרצה רגילה מפעיל ממשק TUI אינטראקטיבי עם 7 לשוניות. כל היכולות זמינות גם כפקודות CLI לאוטומציה וסקריפטים.

**תמיכה מלאה בכל הפלטפורמות (100% Cross-Platform)**: כל הפונקציות, פקודות ה-CLI, ניהול הנתיבים וממשק ה-TUI פועלים באופן שווה וזהה ב-**Windows**, **macOS** ו-**Linux** ללא החרגות.

### התקנה

קבצים בינאריים מוכנים להורדה זמינים ב-[GitHub Releases](https://github.com/lonestill/agx/releases/latest) (עבור macOS, Linux ו-Windows).

**התקנה מהירה בשורה אחת (macOS ו-Linux):**

```bash
curl -fsSL https://raw.githubusercontent.com/lonestill/agx/master/install.sh | bash
```

**Windows:**
הורידו את קובץ ה-ZIP `agx-v*-windows-x86_64.zip` מ-[GitHub Releases](https://github.com/lonestill/agx/releases/latest) וחלצו את `agx.exe` לתיקייה הנמצאת ב-`PATH`.

או הידור ישיר מקוד המקור:

```bash
git clone https://github.com/lonestill/agx.git
cd agx
cargo build --release
```

### יכולות מרכזיות

> **כל היכולות זמינות בכל מערכות ההפעלה**: תמיכה מלאה ב-**Windows**, **macOS** ו-**Linux** עם התאמת נתיבים אוטומטית (`%USERPROFILE%` / `$HOME`), עקיפת נעילות SQLite WAL וזיהוי מפתחות בכל מערכת.

1. **ניהול חשבונות ורוטציית מכסות אוטומטית**:
   - שמירה ומעבר בין חשבונות Google OAuth (`~/.gemini/accounts/` או `%USERPROFILE%\.gemini\accounts\`).
   - זיהוי אוטומטי של טוקנים מקבצי סביבה, מסדי IDE (`state.vscdb`) ומנהל האישורים של Windows (Credential Manager).
   - מעקב בזמן אמת אחרי חלונות מכסה (5 שעות ושבועי) עם זמני איפוס.
   - הפקודה `agx account best` מאתרת את החשבון בעל המכסה הגבוהה ביותר ועוברת אליו אוטומטית.
2. **ניתוח טוקנים ועלויות (`agx stats`)**:
   - קורא ישירות את מסדי הנתונים של השיחות ב-SQLite בכל מערכות ההפעלה.
   - מפענח נתוני Protobuf במהירות של כ-25,000 בקשות בשנייה.
   - קריאה שאינה חוסמת באמצעות URI `mode=ro&immutable=1` וזמני המתנה (busy_timeout) — מאפשר קריאה במקביל ל-IDE פעיל.
   - מחשב יעילות מטמון (Prompt Caching) ועלות משוערת לפי דגם.
3. **ניקוי מטמון בטוח ומדויק (`agx clean`)**:
   - מפנה כ-1 GB של קבצי מטמון בכל הפלטפורמות (Chromium V8, שיידרים של GPU, ולוגים של משימות רקע).
   - **הגנה מוחלטת**: מנגנון נעילה קשיח מונע מחיקה של היסטוריית שיחות, פרויקטים (`scratch/`), קבצי משתמש ומפתחות.
   - בחירה אינטראקטיבית עם אישור מפורש לפני ביצוע (`[y/N]`).
4. **עקיפת חסימות בקבצים בינאריים (`agx patch`)**:
   - בודק ומחיל טלאים עבור רכיבי `agy`, `language_server` ו-`main.js`.
   - גיבוי אוטומטי (`.agybak`) וחתימה מחדש (ad-hoc ב-macOS).
5. **בדיקת תקינות מערכת (`agx doctor`)**:
   - בדיקת זמני תגובה (Latency במילישניות) מול שרתי Google OAuth ו-CloudCode.
   - בדיקת תוקף טוקנים, תהליכים פעילים ותקינות חתימות.
6. **ניהול מיומנויות סוכן (`agx skills`)**:
   - סריקה, בדיקת תקינות ויצירת תבניות חדשות ב-`~/.gemini/config/skills/`.
7. **הרצת פקודות מבודדת (`agx run`)**:
   - החלפת חשבון זמנית, הרצת פקודה והחזרת החשבון המקורי באופן מובטח.

### מקשי קיצור ב-TUI

- `1` עד `7` — מעבר ישיר בין לשוניות
- `Tab` / `Shift+Tab` — מעבר עוקב בין לשוניות
- `j` / `k` או `↑` / `↓` — ניווט ברשימות
- `Enter` / `s` — החלפת חשבון (לשונית 1) / החלת טלאי (לשונית 2)
- `b` — מעבר אוטומטי לחשבון עם המכסה הגבוהה ביותר
- `Space` — סימון פריטים לניקוי (לשונית 3)
- `c` — ביצוע ניקוי לקטגוריות שנבחרו (לשונית 3)
- `r` — רענון נתונים
- `q` / `Esc` — יציאה

---

## License

MIT / Apache-2.0
