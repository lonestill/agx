use std::io;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, Tabs},
    Frame, Terminal,
};

use crate::account::{list_accounts, load_account, switch_account, switch_best};
use crate::quota::fetch_quota_summary;
use crate::patcher::{discover_targets, patch_target, restore_target, GateState, TargetInfo};
use crate::cleaner::{scan_clean_categories, purge_category, format_bytes, CleanCategory};
use crate::stats::{collect_antigravity_tokens, ModelUsage};
use crate::skills::{scan_skills, SkillInfo};
use crate::doctor::{self, DoctorReport, DoctorStatus};

#[derive(Clone)]
pub struct AccountRow {
    pub name: String,
    pub is_active: bool,
    pub token_exp: String,
    pub gemini_5h: String,
    pub gemini_wk: String,
    pub claude_5h: String,
    pub claude_wk: String,
    pub is_loading: bool,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct StatsSnapshot {
    pub sessions_count: usize,
    pub requests_count: u64,
    pub models: Vec<ModelUsage>,
    pub total_in: u64,
    pub total_cache: u64,
    pub total_out: u64,
    pub total_cost: f64,
    pub cache_pct: f64,
    #[allow(dead_code)]
    pub is_loading: bool,
}

pub struct AppState {
    pub active_tab: usize,
    pub rows: Arc<Mutex<Vec<AccountRow>>>,
    pub selected_account: usize,
    pub targets: Vec<TargetInfo>,
    pub selected_target: usize,
    pub clean_categories: Vec<CleanCategory>,
    pub selected_clean: usize,
    pub stats_data: Arc<Mutex<Option<StatsSnapshot>>>,
    pub skills: Vec<SkillInfo>,
    pub selected_skill: usize,
    pub doctor_report: Arc<Mutex<Option<DoctorReport>>>,
    pub status: String,
    pub status_color: Color,
    pub app_version: String,
    pub cli_version: String,
    pub ide_version: String,
}

impl AppState {
    pub fn new() -> Self {
        let app_ver = std::process::Command::new("defaults")
            .args(["read", "/Applications/Antigravity.app/Contents/Info.plist", "CFBundleShortVersionString"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|| "2.22.0".to_string());

        let cli_ver = std::process::Command::new("agy")
            .arg("--version")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|| "1.3.3".to_string());

        let ide_ver = std::process::Command::new("defaults")
            .args(["read", "/Applications/Antigravity IDE.app/Contents/Info.plist", "CFBundleShortVersionString"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|| "2.5.5".to_string());

        let targets = discover_targets();
        let clean_categories = scan_clean_categories();
        let skills = scan_skills();

        let raw_accounts = list_accounts().unwrap_or_default();
        let mut rows = Vec::new();

        for (name, token, is_active) in &raw_accounts {
            let exp = match &token.token.expiry {
                Some(e) if e.len() >= 10 => e[..10].to_string(),
                _ => "—".to_string(),
            };

            rows.push(AccountRow {
                name: name.clone(),
                is_active: *is_active,
                token_exp: exp,
                gemini_5h: "loading...".to_string(),
                gemini_wk: "—".to_string(),
                claude_5h: "loading...".to_string(),
                claude_wk: "—".to_string(),
                is_loading: true,
                error: None,
            });
        }

        let selected_account = rows.iter().position(|r| r.is_active).unwrap_or(0);
        let rows_arc = Arc::new(Mutex::new(rows));
        let stats_arc = Arc::new(Mutex::new(None));
        let doctor_arc = Arc::new(Mutex::new(None));

        // Background loaders
        Self::spawn_quota_loader(Arc::clone(&rows_arc));
        Self::spawn_stats_loader(Arc::clone(&stats_arc));
        Self::spawn_doctor_loader(Arc::clone(&doctor_arc));

        Self {
            active_tab: 0,
            rows: rows_arc,
            selected_account,
            targets,
            selected_target: 0,
            clean_categories,
            selected_clean: 0,
            stats_data: stats_arc,
            skills,
            selected_skill: 0,
            doctor_report: doctor_arc,
            status: "ready".to_string(),
            status_color: Color::DarkGray,
            app_version: app_ver,
            cli_version: cli_ver,
            ide_version: ide_ver,
        }
    }

    fn spawn_quota_loader(rows_arc: Arc<Mutex<Vec<AccountRow>>>) {
        thread::spawn(move || {
            let accounts_to_fetch: Vec<String> = {
                let lock = rows_arc.lock().unwrap();
                lock.iter().map(|r| r.name.clone()).collect()
            };

            for name in accounts_to_fetch {
                let res = load_account(&name).and_then(|mut tok| fetch_quota_summary(&mut tok));
                let mut lock = rows_arc.lock().unwrap();
                if let Some(row) = lock.iter_mut().find(|r| r.name == name) {
                    row.is_loading = false;
                    match res {
                        Ok(groups) => {
                            for g in groups {
                                let lname = g.name.to_lowercase();
                                if lname.contains("gemini") {
                                    if let Some(w) = g.five_hour {
                                        let pct = (w.fraction.unwrap_or(0.0) * 100.0).round() as u16;
                                        let reset = match w.reset_time {
                                            Some(dt) => {
                                                let diff = dt - chrono::Utc::now();
                                                let s = diff.num_seconds();
                                                if s <= 0 { "0m".to_string() } else { format!("{}h{:02}m", s / 3600, (s % 3600) / 60) }
                                            }
                                            None => "".to_string(),
                                        };
                                        row.gemini_5h = if reset.is_empty() { format!("{}%", pct) } else { format!("{}% ({})", pct, reset) };
                                    }
                                    if let Some(w) = g.weekly {
                                        let pct = (w.fraction.unwrap_or(0.0) * 100.0).round() as u16;
                                        row.gemini_wk = format!("{}%", pct);
                                    }
                                } else if lname.contains("claude") || lname.contains("gpt") {
                                    if let Some(w) = g.five_hour {
                                        let pct = (w.fraction.unwrap_or(0.0) * 100.0).round() as u16;
                                        let reset = match w.reset_time {
                                            Some(dt) => {
                                                let diff = dt - chrono::Utc::now();
                                                let s = diff.num_seconds();
                                                if s <= 0 { "0m".to_string() } else { format!("{}h{:02}m", s / 3600, (s % 3600) / 60) }
                                            }
                                            None => "".to_string(),
                                        };
                                        row.claude_5h = if reset.is_empty() { format!("{}%", pct) } else { format!("{}% ({})", pct, reset) };
                                    }
                                    if let Some(w) = g.weekly {
                                        let pct = (w.fraction.unwrap_or(0.0) * 100.0).round() as u16;
                                        row.claude_wk = format!("{}%", pct);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            row.error = Some(e);
                            row.gemini_5h = "err".to_string();
                            row.claude_5h = "err".to_string();
                        }
                    }
                }
            }
        });
    }

    fn spawn_stats_loader(stats_arc: Arc<Mutex<Option<StatsSnapshot>>>) {
        thread::spawn(move || {
            let (sessions, reqs, by_model) = collect_antigravity_tokens();
            let mut total_in = 0;
            let mut total_cache = 0;
            let mut total_out = 0;
            let mut total_cost = 0.0;

            let mut models: Vec<ModelUsage> = by_model.into_values().collect();
            models.sort_by_key(|m| std::cmp::Reverse(m.total()));

            for m in &models {
                total_in += m.input;
                total_cache += m.cache;
                total_out += m.output;
                total_cost += m.cost();
            }

            let cache_pct = if (total_in + total_cache) > 0 {
                (total_cache as f64 / (total_in + total_cache) as f64) * 100.0
            } else {
                0.0
            };

            let mut lock = stats_arc.lock().unwrap();
            *lock = Some(StatsSnapshot {
                sessions_count: sessions,
                requests_count: reqs,
                models,
                total_in,
                total_cache,
                total_out,
                total_cost,
                cache_pct,
                is_loading: false,
            });
        });
    }

    fn spawn_doctor_loader(doctor_arc: Arc<Mutex<Option<DoctorReport>>>) {
        thread::spawn(move || {
            let report = doctor::check_doctor();
            let mut lock = doctor_arc.lock().unwrap();
            *lock = Some(report);
        });
    }

    pub fn refresh_quotas(&mut self) {
        let mut lock = self.rows.lock().unwrap();
        for r in lock.iter_mut() {
            r.is_loading = true;
            r.gemini_5h = "loading...".to_string();
            r.claude_5h = "loading...".to_string();
            r.error = None;
        }
        self.status = "refreshing quotas...".to_string();
        self.status_color = Color::Cyan;
        Self::spawn_quota_loader(Arc::clone(&self.rows));
    }

    pub fn refresh_stats(&mut self) {
        let mut lock = self.stats_data.lock().unwrap();
        *lock = None;
        self.status = "re-calculating token usage...".to_string();
        self.status_color = Color::Cyan;
        Self::spawn_stats_loader(Arc::clone(&self.stats_data));
    }

    pub fn refresh_skills(&mut self) {
        self.skills = scan_skills();
        if self.selected_skill >= self.skills.len() && !self.skills.is_empty() {
            self.selected_skill = self.skills.len() - 1;
        }
        self.status = format!("scanned {} skills", self.skills.len());
        self.status_color = Color::Green;
    }

    pub fn refresh_doctor(&mut self) {
        let mut lock = self.doctor_report.lock().unwrap();
        *lock = None;
        self.status = "running system health check...".to_string();
        self.status_color = Color::Cyan;
        Self::spawn_doctor_loader(Arc::clone(&self.doctor_report));
    }

    pub fn switch_selected_account(&mut self) {
        let name = {
            let lock = self.rows.lock().unwrap();
            if lock.is_empty() { return; }
            lock[self.selected_account].name.clone()
        };

        match switch_account(&name) {
            Ok(_) => {
                let mut lock = self.rows.lock().unwrap();
                for r in lock.iter_mut() {
                    r.is_active = r.name == name;
                }
                self.status = format!("active account switched to '{}'", name);
                self.status_color = Color::Green;
            }
            Err(e) => {
                self.status = format!("switch failed: {}", e);
                self.status_color = Color::Red;
            }
        }
    }

    pub fn switch_to_best_account(&mut self) {
        self.status = "calculating best quota...".to_string();
        self.status_color = Color::Cyan;
        match switch_best() {
            Ok(msg) => {
                if let Ok(accounts) = list_accounts() {
                    let mut lock = self.rows.lock().unwrap();
                    for r in lock.iter_mut() {
                        r.is_active = accounts.iter().any(|(n, _, a)| n == &r.name && *a);
                    }
                }
                self.status = msg;
                self.status_color = Color::Green;
            }
            Err(e) => {
                self.status = format!("auto-switch error: {}", e);
                self.status_color = Color::Red;
            }
        }
    }

    pub fn patch_selected_gate(&mut self) {
        if self.selected_target < self.targets.len() {
            let name = self.targets[self.selected_target].name;
            match patch_target(name) {
                Ok(msg) => {
                    self.targets = discover_targets();
                    self.status = msg;
                    self.status_color = Color::Green;
                }
                Err(e) => {
                    self.status = format!("patch error: {}", e);
                    self.status_color = Color::Red;
                }
            }
        }
    }

    pub fn patch_all_gates(&mut self) {
        match patch_target("all") {
            Ok(msg) => {
                self.targets = discover_targets();
                self.status = msg;
                self.status_color = Color::Green;
            }
            Err(e) => {
                self.status = format!("patch error: {}", e);
                self.status_color = Color::Red;
            }
        }
    }

    pub fn restore_selected_gate(&mut self) {
        if self.selected_target < self.targets.len() {
            let name = self.targets[self.selected_target].name;
            match restore_target(name) {
                Ok(msg) => {
                    self.targets = discover_targets();
                    self.status = msg;
                    self.status_color = Color::Yellow;
                }
                Err(e) => {
                    self.status = format!("restore error: {}", e);
                    self.status_color = Color::Red;
                }
            }
        }
    }

    pub fn toggle_clean_item(&mut self) {
        if self.selected_clean < self.clean_categories.len() {
            self.clean_categories[self.selected_clean].enabled = !self.clean_categories[self.selected_clean].enabled;
        }
    }

    pub fn execute_clean(&mut self) {
        let mut freed = 0u64;
        let mut count = 0;
        for cat in &self.clean_categories {
            if cat.enabled && cat.total_bytes > 0 {
                if let Ok(b) = purge_category(cat) {
                    freed += b;
                    count += 1;
                }
            }
        }
        self.clean_categories = scan_clean_categories();
        if count > 0 {
            self.status = format!("cleaned {} categories, freed {}", count, format_bytes(freed));
            self.status_color = Color::Green;
        } else {
            self.status = "no categories selected for cleaning".to_string();
            self.status_color = Color::Yellow;
        }
    }

    pub fn next_row(&mut self) {
        match self.active_tab {
            0 => {
                let len = self.rows.lock().unwrap().len();
                if len > 0 { self.selected_account = (self.selected_account + 1) % len; }
            }
            1 => {
                if !self.targets.is_empty() {
                    self.selected_target = (self.selected_target + 1) % self.targets.len();
                }
            }
            2 => {
                if !self.clean_categories.is_empty() {
                    self.selected_clean = (self.selected_clean + 1) % self.clean_categories.len();
                }
            }
            4 => {
                if !self.skills.is_empty() {
                    self.selected_skill = (self.selected_skill + 1) % self.skills.len();
                }
            }
            _ => {}
        }
    }

    pub fn prev_row(&mut self) {
        match self.active_tab {
            0 => {
                let len = self.rows.lock().unwrap().len();
                if len > 0 {
                    self.selected_account = if self.selected_account == 0 { len - 1 } else { self.selected_account - 1 };
                }
            }
            1 => {
                if !self.targets.is_empty() {
                    self.selected_target = if self.selected_target == 0 { self.targets.len() - 1 } else { self.selected_target - 1 };
                }
            }
            2 => {
                if !self.clean_categories.is_empty() {
                    self.selected_clean = if self.selected_clean == 0 { self.clean_categories.len() - 1 } else { self.selected_clean - 1 };
                }
            }
            4 => {
                if !self.skills.is_empty() {
                    self.selected_skill = if self.selected_skill == 0 { self.skills.len() - 1 } else { self.selected_skill - 1 };
                }
            }
            _ => {}
        }
    }

    pub fn next_tab(&mut self) {
        self.active_tab = (self.active_tab + 1) % 7;
    }

    pub fn prev_tab(&mut self) {
        self.active_tab = if self.active_tab == 0 { 6 } else { self.active_tab - 1 };
    }
}

pub fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new();

    loop {
        terminal.draw(|f| ui(f, &state))?;

        if event::poll(Duration::from_millis(150))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Tab => {
                            if key.modifiers.contains(KeyModifiers::SHIFT) {
                                state.prev_tab();
                            } else {
                                state.next_tab();
                            }
                        }
                        KeyCode::BackTab => state.prev_tab(),
                        KeyCode::Char('1') => state.active_tab = 0,
                        KeyCode::Char('2') => state.active_tab = 1,
                        KeyCode::Char('3') => state.active_tab = 2,
                        KeyCode::Char('4') => state.active_tab = 3,
                        KeyCode::Char('5') => state.active_tab = 4,
                        KeyCode::Char('6') => state.active_tab = 5,
                        KeyCode::Char('7') => state.active_tab = 6,
                        KeyCode::Down | KeyCode::Char('j') => state.next_row(),
                        KeyCode::Up | KeyCode::Char('k') => state.prev_row(),
                        KeyCode::Char(' ') if state.active_tab == 2 => state.toggle_clean_item(),
                        KeyCode::Enter => {
                            if state.active_tab == 0 {
                                state.switch_selected_account();
                            } else if state.active_tab == 1 {
                                state.patch_selected_gate();
                            } else if state.active_tab == 2 {
                                state.execute_clean();
                            }
                        }
                        KeyCode::Char('s') if state.active_tab == 0 => state.switch_selected_account(),
                        KeyCode::Char('b') if state.active_tab == 0 => state.switch_to_best_account(),
                        KeyCode::Char('r') => {
                            match state.active_tab {
                                0 => state.refresh_quotas(),
                                3 => state.refresh_stats(),
                                4 => state.refresh_skills(),
                                5 => state.refresh_doctor(),
                                _ => state.refresh_quotas(),
                            }
                        }
                        KeyCode::Char('p') => {
                            if state.active_tab == 1 {
                                state.patch_selected_gate();
                            } else {
                                state.patch_all_gates();
                            }
                        }
                        KeyCode::Char('a') if state.active_tab == 1 => state.patch_all_gates(),
                        KeyCode::Char('R') if state.active_tab == 1 => state.restore_selected_gate(),
                        KeyCode::Char('c') if state.active_tab == 2 => state.execute_clean(),
                        _ => {}
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn ui(f: &mut Frame, state: &AppState) {
    let size = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Top tabs + header
            Constraint::Length(1), // Blank separator
            Constraint::Min(6),    // Main tab content
            Constraint::Length(1), // Blank separator
            Constraint::Length(1), // Bottom status bar
        ])
        .split(size);

    // 1. Top Bar: Title & Tabs
    render_tab_bar(f, chunks[0], state);

    // 2. Active Tab Content
    match state.active_tab {
        0 => render_accounts_tab(f, chunks[2], state),
        1 => render_gates_tab(f, chunks[2], state),
        2 => render_cleaner_tab(f, chunks[2], state),
        3 => render_stats_tab(f, chunks[2], state),
        4 => render_skills_tab(f, chunks[2], state),
        5 => render_doctor_tab(f, chunks[2], state),
        _ => render_system_tab(f, chunks[2], state),
    }

    // 3. Bottom Status Bar & Hotkeys
    render_status_bar(f, chunks[4], state);
}

fn render_tab_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let tab_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(12), Constraint::Min(60), Constraint::Length(20)])
        .split(area);

    // App label
    let app_span = Paragraph::new(Span::styled(" agx v1.3 ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)));
    f.render_widget(app_span, tab_chunks[0]);

    // Tabs
    let tab_titles = vec![
        " [1] Accounts ",
        " [2] Gates ",
        " [3] Cleaner ",
        " [4] Stats ",
        " [5] Skills ",
        " [6] Doctor ",
        " [7] System ",
    ];

    let tabs = Tabs::new(tab_titles)
        .select(state.active_tab)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD).bg(Color::Rgb(40, 45, 60)))
        .divider("│");

    f.render_widget(tabs, tab_chunks[1]);

    // Tab hint
    let hint = Paragraph::new("Tab / 1-7 to switch").style(Style::default().fg(Color::DarkGray)).alignment(Alignment::Right);
    f.render_widget(hint, tab_chunks[2]);
}

fn render_accounts_tab(f: &mut Frame, area: Rect, state: &AppState) {
    let header = Row::new(vec![
        Cell::from(""),
        Cell::from(""),
        Cell::from("ACCOUNT").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("GEMINI 5H").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("GEMINI WK").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("CLAUDE 5H").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("CLAUDE WK").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("TOKEN").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ])
    .bottom_margin(1);

    let rows_guard = state.rows.lock().unwrap();
    let rows: Vec<Row> = rows_guard
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let is_sel = i == state.selected_account;
            let cursor = if is_sel { "▶" } else { " " };
            let active_mark = if r.is_active { "●" } else { " " };

            let row_style = if is_sel {
                Style::default().fg(Color::White).bg(Color::Rgb(32, 38, 50)).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            let name_style = if r.is_active {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let g5h_style = if r.gemini_5h.starts_with("0%") {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else if r.gemini_5h.contains("loading") {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Green)
            };

            let c5h_style = if r.claude_5h.starts_with("0%") {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else if r.claude_5h.contains("loading") {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Green)
            };

            Row::new(vec![
                Cell::from(cursor).style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Cell::from(active_mark).style(Style::default().fg(Color::Green)),
                Cell::from(r.name.clone()).style(name_style),
                Cell::from(r.gemini_5h.clone()).style(g5h_style),
                Cell::from(r.gemini_wk.clone()),
                Cell::from(r.claude_5h.clone()).style(c5h_style),
                Cell::from(r.claude_wk.clone()),
                Cell::from(r.token_exp.clone()).style(Style::default().fg(Color::DarkGray)),
            ])
            .style(row_style)
        })
        .collect();

    let widths = [
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(18),
        Constraint::Length(16),
        Constraint::Length(12),
        Constraint::Length(16),
        Constraint::Length(12),
        Constraint::Min(12),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::NONE));

    f.render_widget(table, area);
}

fn render_gates_tab(f: &mut Frame, area: Rect, state: &AppState) {
    let header = Row::new(vec![
        Cell::from(""),
        Cell::from("TARGET").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("STATUS").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("BACKUP").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("BINARY PATH").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ])
    .bottom_margin(1);

    let rows: Vec<Row> = state
        .targets
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let is_sel = i == state.selected_target;
            let cursor = if is_sel { "▶" } else { " " };

            let row_style = if is_sel {
                Style::default().fg(Color::White).bg(Color::Rgb(32, 38, 50)).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            let (status_text, status_color) = match t.state {
                GateState::Patched => ("● PATCHED (Bypassed)", Color::Green),
                GateState::Unpatched => ("○ UNPATCHED (Stock)", Color::Yellow),
                GateState::NotFound => ("✕ NOT FOUND", Color::DarkGray),
                GateState::Unknown => ("? UNKNOWN", Color::DarkGray),
            };

            let backup_str = if t.has_backup { "yes (.agybak)" } else { "none" };
            let p_str = t.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "—".to_string());

            Row::new(vec![
                Cell::from(cursor).style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Cell::from(t.name.to_uppercase()),
                Cell::from(status_text).style(Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
                Cell::from(backup_str),
                Cell::from(p_str).style(Style::default().fg(Color::DarkGray)),
            ])
            .style(row_style)
        })
        .collect();

    let widths = [
        Constraint::Length(2),
        Constraint::Length(12),
        Constraint::Length(24),
        Constraint::Length(16),
        Constraint::Min(25),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::NONE));

    f.render_widget(table, area);
}

fn render_cleaner_tab(f: &mut Frame, area: Rect, state: &AppState) {
    let header = Row::new(vec![
        Cell::from(""),
        Cell::from("SEL").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("CATEGORY").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("ITEMS").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("SIZE").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("PATH").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ])
    .bottom_margin(1);

    let rows: Vec<Row> = state
        .clean_categories
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let is_sel = i == state.selected_clean;
            let cursor = if is_sel { "▶" } else { " " };
            let check = if c.enabled { "[x]" } else { "[ ]" };
            let check_color = if c.enabled { Color::Green } else { Color::DarkGray };

            let row_style = if is_sel {
                Style::default().fg(Color::White).bg(Color::Rgb(32, 38, 50)).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            let size_str = format_bytes(c.total_bytes);
            let size_style = if c.total_bytes > 100 * 1024 * 1024 {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let path_str = c.paths.first()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "—".to_string());

            Row::new(vec![
                Cell::from(cursor).style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Cell::from(check).style(Style::default().fg(check_color).add_modifier(Modifier::BOLD)),
                Cell::from(c.title),
                Cell::from(c.paths.len().to_string()),
                Cell::from(size_str).style(size_style),
                Cell::from(path_str).style(Style::default().fg(Color::DarkGray)),
            ])
            .style(row_style)
        })
        .collect();

    let widths = [
        Constraint::Length(2),
        Constraint::Length(5),
        Constraint::Length(28),
        Constraint::Length(8),
        Constraint::Length(12),
        Constraint::Min(20),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::NONE));

    f.render_widget(table, area);
}

fn format_tok(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.2}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn render_stats_tab(f: &mut Frame, area: Rect, state: &AppState) {
    let guard = state.stats_data.lock().unwrap();

    let snapshot = match guard.as_ref() {
        Some(s) => s,
        None => {
            let p = Paragraph::new(vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("  ⏳ Reading conversation databases and decoding protobufs... ", Style::default().fg(Color::Cyan)),
                    Span::styled("(takes ~1-2s)", Style::default().fg(Color::DarkGray)),
                ]),
            ]);
            f.render_widget(p, area);
            return;
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Top summary card
            Constraint::Min(6),    // Models table
        ])
        .split(area);

    // Summary Card
    let summary_line = Line::from(vec![
        Span::styled("  Conversations: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{} dbs ({} gens)", snapshot.sessions_count, snapshot.requests_count), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled("   │   Total Tokens: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format_tok(snapshot.total_in + snapshot.total_cache + snapshot.total_out), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("   │   Prompt Cache: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:.1}%", snapshot.cache_pct), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::styled("   │   Est API Value: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("${:.2}", snapshot.total_cost), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ]);

    let card = Paragraph::new(vec![summary_line])
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));
    f.render_widget(card, chunks[0]);

    // Table
    let header = Row::new(vec![
        Cell::from("MODEL").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("GENS").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("INPUT").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("CACHED").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("OUTPUT").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("TOTAL").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("EST COST").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ])
    .bottom_margin(1);

    let rows: Vec<Row> = snapshot.models
        .iter()
        .map(|m| {
            Row::new(vec![
                Cell::from(m.name.clone()).style(Style::default().fg(Color::Cyan)),
                Cell::from(m.count.to_string()),
                Cell::from(format_tok(m.input)),
                Cell::from(format_tok(m.cache)).style(Style::default().fg(Color::Green)),
                Cell::from(format_tok(m.output)),
                Cell::from(format_tok(m.total())).style(Style::default().add_modifier(Modifier::BOLD)),
                Cell::from(format!("${:.2}", m.cost())).style(Style::default().fg(Color::Yellow)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(28),
        Constraint::Length(8),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Min(10),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::NONE));

    f.render_widget(table, chunks[1]);
}

fn render_skills_tab(f: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    // Left pane: Table of skills
    let header = Row::new(vec![
        Cell::from(""),
        Cell::from("SKILL NAME").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("STATUS").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("DESCRIPTION").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ])
    .bottom_margin(1);

    let rows: Vec<Row> = state.skills
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let is_sel = i == state.selected_skill;
            let cursor = if is_sel { "▶" } else { " " };

            let row_style = if is_sel {
                Style::default().fg(Color::White).bg(Color::Rgb(32, 38, 50)).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            let status_badge = if s.valid {
                Span::styled("[OK]", Style::default().fg(Color::Green))
            } else {
                Span::styled("[ERR]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            };

            let desc_short = if s.description.len() > 35 {
                format!("{}...", &s.description[..32])
            } else {
                s.description.clone()
            };

            Row::new(vec![
                Cell::from(cursor).style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Cell::from(s.name.clone()).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Cell::from(status_badge),
                Cell::from(desc_short).style(Style::default().fg(Color::DarkGray)),
            ])
            .style(row_style)
        })
        .collect();

    let widths = [
        Constraint::Length(2),
        Constraint::Length(24),
        Constraint::Length(8),
        Constraint::Min(20),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::RIGHT).border_style(Style::default().fg(Color::DarkGray)));

    f.render_widget(table, chunks[0]);

    // Right pane: Detail card of selected skill
    if state.selected_skill < state.skills.len() {
        let sel = &state.skills[state.selected_skill];
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(" Skill Audit & Details ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));

        let mut lines = vec![
            Line::from(vec![
                Span::styled("  Name:        ", Style::default().fg(Color::DarkGray)),
                Span::styled(&sel.name, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("  Directory:   ", Style::default().fg(Color::DarkGray)),
                Span::styled(&sel.folder_name, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("  Integrity:   ", Style::default().fg(Color::DarkGray)),
                if sel.valid {
                    Span::styled("Valid YAML Frontmatter", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
                } else {
                    Span::styled("Audit Issues Detected", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                },
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Description:", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            ]),
        ];

        for desc_chunk in sel.description.as_bytes().chunks(48) {
            if let Ok(s) = std::str::from_utf8(desc_chunk) {
                lines.push(Line::from(vec![
                    Span::styled("    ", Style::default()),
                    Span::styled(s, Style::default().fg(Color::Gray)),
                ]));
            }
        }

        if !sel.issues.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  Issues:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]));
            for issue in &sel.issues {
                lines.push(Line::from(vec![
                    Span::styled("    └─ ", Style::default().fg(Color::Red)),
                    Span::styled(issue, Style::default().fg(Color::Red)),
                ]));
            }
        }

        let p = Paragraph::new(lines).block(block);
        f.render_widget(p, chunks[1]);
    }
}

fn render_doctor_tab(f: &mut Frame, area: Rect, state: &AppState) {
    let guard = state.doctor_report.lock().unwrap();

    let report = match guard.as_ref() {
        Some(r) => r,
        None => {
            let p = Paragraph::new(vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("  ⏳ Running diagnostics and testing Google endpoints... ", Style::default().fg(Color::Cyan)),
                    Span::styled("(checking OAuth, CloudCode, gates, codesigning)", Style::default().fg(Color::DarkGray)),
                ]),
            ]);
            f.render_widget(p, area);
            return;
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Top summary
            Constraint::Min(6),    // Checklist
        ])
        .split(area);

    let summary_line = Line::from(vec![
        Span::styled("  Health Status: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{} passed", report.ok_count), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::styled("   │   ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{} warnings", report.warn_count), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled("   │   ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{} failures", report.fail_count), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
    ]);

    let card = Paragraph::new(vec![summary_line])
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));
    f.render_widget(card, chunks[0]);

    let header = Row::new(vec![
        Cell::from("DIAGNOSTIC ITEM").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("STATUS").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Cell::from("DETAILS & SUGGESTION").style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ])
    .bottom_margin(1);

    let rows: Vec<Row> = report.items
        .iter()
        .map(|item| {
            let (status_text, status_color) = match item.status {
                DoctorStatus::Ok => ("[OK]", Color::Green),
                DoctorStatus::Warn => ("[WARN]", Color::Yellow),
                DoctorStatus::Fail => ("[FAIL]", Color::Red),
            };

            Row::new(vec![
                Cell::from(item.title.clone()).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Cell::from(status_text).style(Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
                Cell::from(item.detail.clone()).style(Style::default().fg(Color::Gray)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(36),
        Constraint::Length(10),
        Constraint::Min(35),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::NONE));

    f.render_widget(table, chunks[1]);
}

fn render_system_tab(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let lines = vec![
        Line::from(vec![
            Span::styled("  Installed Binaries & Applications", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("    Antigravity 2.0 (Desktop): ", Style::default().fg(Color::DarkGray)),
            Span::styled(&state.app_version, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("   [/Applications/Antigravity.app]", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("    Antigravity CLI (agy):      ", Style::default().fg(Color::DarkGray)),
            Span::styled(&state.cli_version, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("   [~/.local/bin/agy]", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("    Antigravity IDE:            ", Style::default().fg(Color::DarkGray)),
            Span::styled(&state.ide_version, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("   [/Applications/Antigravity IDE.app]", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Storage Locations", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("    Active Token:   ", Style::default().fg(Color::DarkGray)),
            Span::styled("~/.gemini/jetski-standalone-oauth-token", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("    Saved Profiles: ", Style::default().fg(Color::DarkGray)),
            Span::styled("~/.gemini/accounts/*.json", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("    Toolkit Path:   ", Style::default().fg(Color::DarkGray)),
            Span::styled("~/.local/bin/agx  (/Users/a123/.gemini/antigravity/scratch/agx)", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Project & Repository", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("    GitHub Repo:    ", Style::default().fg(Color::DarkGray)),
            Span::styled("https://github.com/lonestill/agx", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
    ];

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn render_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let key_hints = match state.active_tab {
        0 => "enter/s: switch   b: auto-best   r: reload   1-7/Tab: tabs   q: exit",
        1 => "enter/p: patch selected   a: patch all   R: restore backup   1-7/Tab: tabs   q: exit",
        2 => "space: toggle item   enter/c: purge selected   1-7/Tab: tabs   q: exit",
        3 => "r: recalculate stats   1-7/Tab: tabs   q: exit",
        4 => "j/k: select skill   r: rescan   1-7/Tab: tabs   q: exit",
        5 => "r: re-run diagnostics   1-7/Tab: tabs   q: exit",
        _ => "1-7/Tab: tabs   q: exit",
    };

    let line = Line::from(vec![
        Span::styled(&state.status, Style::default().fg(state.status_color).add_modifier(Modifier::BOLD)),
        Span::styled("   │   ", Style::default().fg(Color::DarkGray)),
        Span::styled(key_hints, Style::default().fg(Color::Gray)),
    ]);

    let p = Paragraph::new(line);
    f.render_widget(p, area);
}
