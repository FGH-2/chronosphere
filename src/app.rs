use crate::clipboard;
use crate::config;
use crate::engagement::{
    CredKind, CredentialProfile, Engagement, JobRecord, JobStatus, Target,
};
use crate::exec::{Executor, FocusResult, SpawnRequest};
use crate::library::{CategoryFile, CommandEntry, CommandLibrary, CommandVariant};
use crate::render::{self, RenderContext};
use crate::ui::{self, splash::SplashState};
use crate::vim::{Action, KeyParser, Mode};

use anyhow::{Context, Result};
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEvent,
    KeyEventKind, KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures::StreamExt;
use nucleo_matcher::{Matcher, Utf32Str, pattern::Pattern};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{Stdout, stdout};
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Categories,
    Commands,
    Preview,
    Jobs,
}

impl Focus {
    pub fn next(self) -> Self {
        match self {
            Focus::Categories => Focus::Commands,
            Focus::Commands => Focus::Preview,
            Focus::Preview => Focus::Jobs,
            Focus::Jobs => Focus::Categories,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Focus::Categories => Focus::Jobs,
            Focus::Commands => Focus::Categories,
            Focus::Preview => Focus::Commands,
            Focus::Jobs => Focus::Preview,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FlashMessage {
    pub text: String,
    pub is_error: bool,
    pub at: Instant,
}

#[derive(Default)]
pub struct EngagementModal {
    pub available: Vec<String>,
    pub cursor: usize,
    pub new_name_prompt: Option<String>,
}

#[derive(Default)]
pub struct TargetModal {
    pub state: TargetModalState,
}

pub enum TargetModalState {
    List {
        cursor: usize,
    },
    Edit {
        fields: Vec<(TargetEditField, String)>,
        focused: usize,
        original_name: Option<String>,
    },
}

impl Default for TargetModalState {
    fn default() -> Self {
        TargetModalState::List { cursor: 0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetEditField {
    Name,
    Ip,
    Hostname,
    Dc,
    Lhost,
    Lport,
    Notes,
}

impl TargetEditField {
    pub fn label(self) -> &'static str {
        match self {
            TargetEditField::Name => "name",
            TargetEditField::Ip => "ip",
            TargetEditField::Hostname => "hostname",
            TargetEditField::Dc => "dc",
            TargetEditField::Lhost => "lhost",
            TargetEditField::Lport => "lport",
            TargetEditField::Notes => "notes",
        }
    }
    pub fn all() -> &'static [TargetEditField] {
        &[
            TargetEditField::Name,
            TargetEditField::Ip,
            TargetEditField::Hostname,
            TargetEditField::Dc,
            TargetEditField::Lhost,
            TargetEditField::Lport,
            TargetEditField::Notes,
        ]
    }
}

#[derive(Default)]
pub struct CredsModal {
    pub state: CredsModalState,
}

pub enum CredsModalState {
    List {
        cursor: usize,
    },
    Edit {
        fields: Vec<(CredEditField, String)>,
        focused: usize,
        original_name: Option<String>,
    },
}

impl Default for CredsModalState {
    fn default() -> Self {
        CredsModalState::List { cursor: 0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredEditField {
    Name,
    Username,
    Domain,
    Kind,
    Password,
    NtHash,
    Ticket,
    Notes,
}

impl CredEditField {
    pub fn label(self) -> &'static str {
        match self {
            CredEditField::Name => "name",
            CredEditField::Username => "username",
            CredEditField::Domain => "domain",
            CredEditField::Kind => "kind",
            CredEditField::Password => "password",
            CredEditField::NtHash => "nt_hash",
            CredEditField::Ticket => "ticket_path",
            CredEditField::Notes => "notes",
        }
    }
    pub fn all() -> &'static [CredEditField] {
        &[
            CredEditField::Name,
            CredEditField::Username,
            CredEditField::Domain,
            CredEditField::Kind,
            CredEditField::Password,
            CredEditField::NtHash,
            CredEditField::Ticket,
            CredEditField::Notes,
        ]
    }
}

#[derive(Default)]
pub struct ToolsModal;

pub struct EditModal {
    pub source_command_id: Option<String>,
    pub command_title: String,
    pub textarea: tui_textarea::TextArea<'static>,
    pub interactive: bool,
    pub category_id: String,
    pub save_as_prompt: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub category_id: String,
    pub category_name: String,
    pub command_id: String,
    pub title: String,
}

pub enum Modal {
    None,
    Help,
    Engagement(EngagementModal),
    Target(TargetModal),
    Creds(CredsModal),
    Tools(ToolsModal),
    Search {
        matches: Vec<SearchHit>,
        cursor: usize,
    },
    Edit(EditModal),
}

pub struct App {
    pub library: CommandLibrary,
    pub library_sources: Vec<PathBuf>,
    pub engagement: Option<Engagement>,
    pub executor: Option<Executor>,

    pub mode: Mode,
    pub focus: Focus,

    pub selected_category: usize,
    pub selected_command: usize,
    pub selected_job: usize,
    pub multi_selected: HashSet<(String, String)>,
    pub marks: BTreeMap<char, (String, String)>,
    pub last_action: Option<Action>,

    pub key_parser: KeyParser,
    pub command_line_buf: String,
    pub search_buf: String,

    pub modal: Modal,

    pub jobs: Vec<JobRecord>,

    pub flash: Option<FlashMessage>,
    pub splash: Option<SplashState>,
    pub running: bool,

    pub _watcher: Option<notify::RecommendedWatcher>,
    pub library_reload_pending: bool,
    pub library_reload_rx: Option<tokio::sync::mpsc::UnboundedReceiver<()>>,
}

impl App {
    pub async fn new() -> Result<Self> {
        let library_sources = collect_library_sources(None);
        let library =
            CommandLibrary::load(&library_sources.iter().map(|p| p.as_path()).collect::<Vec<_>>())
                .context("load command library")?;

        let mut app = Self {
            library,
            library_sources,
            engagement: None,
            executor: None,
            mode: Mode::Normal,
            focus: Focus::Commands,
            selected_category: 0,
            selected_command: 0,
            selected_job: 0,
            multi_selected: HashSet::new(),
            marks: BTreeMap::new(),
            last_action: None,
            key_parser: KeyParser::new(),
            command_line_buf: String::new(),
            search_buf: String::new(),
            modal: Modal::None,
            jobs: Vec::new(),
            flash: None,
            splash: Some(SplashState::new()),
            running: true,
            _watcher: None,
            library_reload_pending: false,
            library_reload_rx: None,
        };
        app.try_open_last_engagement().await;
        app.setup_library_watcher();
        Ok(app)
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut stdout = stdout();
        enable_raw_mode().context("enable raw mode")?;
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .context("enter alt screen")?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("init terminal")?;

        let result = self.event_loop(&mut terminal).await;

        disable_raw_mode().ok();
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .ok();
        terminal.show_cursor().ok();
        result
    }

    async fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<()> {
        let mut events = EventStream::new();

        while self.running {
            terminal.draw(|f| ui::draw(f, self))?;
            // While the splash animation is on screen we tick ≈16 fps for smooth
            // motion; once dismissed we drop back to 2 Hz to keep job polling cheap.
            let tick_ms = if self.splash.is_some() { 60 } else { 500 };
            let sleep = tokio::time::sleep(Duration::from_millis(tick_ms));
            tokio::select! {
                ev = events.next() => {
                    if let Some(Ok(Event::Key(ke))) = ev {
                        if ke.kind == KeyEventKind::Press {
                            self.handle_key(ke);
                        }
                    }
                }
                _ = sleep => {
                    self.tick();
                }
            }
            if self.library_reload_pending {
                self.library_reload_pending = false;
                self.reload_library();
            }
            if let Some(rx) = self.library_reload_rx.as_mut() {
                while rx.try_recv().is_ok() {
                    self.library_reload_pending = true;
                }
            }
            if let Some(flash) = &self.flash {
                if flash.at.elapsed() > Duration::from_secs(4) {
                    self.flash = None;
                }
            }
        }
        Ok(())
    }

    fn tick(&mut self) {
        self.poll_jobs();
    }

    fn poll_jobs(&mut self) {
        let executor = match self.executor.as_ref() {
            Some(e) => e,
            None => return,
        };
        let updates = executor.poll(&self.jobs);
        if updates.is_empty() {
            return;
        }
        for u in updates {
            if let Some(slot) = self.jobs.iter_mut().find(|j| j.id == u.id) {
                *slot = u.clone();
            }
            if let Some(eng) = self.engagement.as_mut() {
                eng.history.update(&u);
            }
        }
    }

    // === key handling ====================================================

    fn handle_key(&mut self, ke: KeyEvent) {
        // Splash screen: any key dismisses it and is consumed. Exception: Ctrl-C
        // / Ctrl-Q still quits so a hung TUI is escapable on first launch.
        if self.splash.is_some() {
            let is_quit = matches!(ke.code, KeyCode::Char('c') | KeyCode::Char('q'))
                && ke.modifiers.contains(KeyModifiers::CONTROL);
            self.splash = None;
            self.key_parser.reset();
            if is_quit {
                self.running = false;
            }
            return;
        }
        // mode-specific handling
        match self.mode {
            Mode::Command => self.handle_command_mode_key(ke),
            Mode::Search | Mode::SearchGlobal => self.handle_search_mode_key(ke),
            Mode::Insert => self.handle_insert_mode_key(ke),
            Mode::Normal => self.handle_normal_mode_key(ke),
        }
    }

    fn handle_normal_mode_key(&mut self, ke: KeyEvent) {
        if !matches!(self.modal, Modal::None | Modal::Help | Modal::Tools(_)) {
            self.handle_modal_key(ke);
            return;
        }
        if matches!(self.modal, Modal::Help | Modal::Tools(_)) {
            if matches!(ke.code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?')) {
                self.modal = Modal::None;
                self.key_parser.reset();
                return;
            }
        }
        let action = self.key_parser.feed_normal(ke);
        self.execute_action(action, false);
    }

    fn execute_action(&mut self, action: Action, repeating: bool) {
        match action.clone() {
            Action::None => {}
            Action::Quit => self.running = false,
            Action::FocusLeft => self.focus = self.focus.prev(),
            Action::FocusRight => self.focus = self.focus.next(),
            Action::Down(n) => self.move_cursor(n as i32),
            Action::Up(n) => self.move_cursor(-(n as i32)),
            Action::PageDown => self.move_cursor(8),
            Action::PageUp => self.move_cursor(-8),
            Action::Top => self.cursor_to(0),
            Action::Bottom => self.cursor_to(i32::MAX),
            Action::Run { count } => {
                self.run_current(count);
                if !repeating {
                    self.last_action = Some(action);
                }
            }
            Action::RunAllVisible => {
                self.run_multi_or_all();
                if !repeating {
                    self.last_action = Some(action);
                }
            }
            Action::YankResolved => self.yank(false),
            Action::YankRaw => self.yank(true),
            Action::EditInline => self.open_edit_modal(),
            Action::ToggleSelect => self.toggle_select_current(),
            Action::ClearSelection => self.multi_selected.clear(),
            Action::OpenActiveJob => self.open_active_job(),
            Action::NextJob => self.selected_job = self.selected_job.saturating_add(1),
            Action::PrevJob => self.selected_job = self.selected_job.saturating_sub(1),
            Action::KillJob => self.kill_selected_job(),
            Action::RepeatLast => {
                if let Some(a) = self.last_action.clone() {
                    self.execute_action(a, true);
                }
            }
            Action::EnterCommandMode => {
                self.mode = Mode::Command;
                self.command_line_buf.clear();
            }
            Action::EnterSearchMode => {
                self.mode = Mode::Search;
                self.search_buf.clear();
                self.recompute_search(false);
            }
            Action::EnterSearchGlobalMode => {
                self.mode = Mode::SearchGlobal;
                self.search_buf.clear();
                self.recompute_search(true);
            }
            Action::OpenHelp => self.modal = Modal::Help,
            Action::SetMark(ch) => self.set_mark(ch),
            Action::JumpMark(ch) => self.jump_mark(ch),
            Action::Refresh => self.reload_library(),
            Action::Tick => self.tick(),
        }
    }

    fn move_cursor(&mut self, delta: i32) {
        match self.focus {
            Focus::Categories => {
                let len = self.library.categories.len();
                if len == 0 {
                    return;
                }
                let new = (self.selected_category as i32 + delta)
                    .clamp(0, len as i32 - 1) as usize;
                if new != self.selected_category {
                    self.selected_category = new;
                    self.selected_command = 0;
                }
            }
            Focus::Commands => {
                let len = self.visible_commands().len();
                if len == 0 {
                    return;
                }
                let new = (self.selected_command as i32 + delta).clamp(0, len as i32 - 1) as usize;
                self.selected_command = new;
            }
            Focus::Jobs => {
                let len = self.jobs.len();
                if len == 0 {
                    return;
                }
                let new = (self.selected_job as i32 + delta).clamp(0, len as i32 - 1) as usize;
                self.selected_job = new;
            }
            Focus::Preview => {}
        }
    }

    fn cursor_to(&mut self, pos: i32) {
        match self.focus {
            Focus::Categories => {
                let len = self.library.categories.len();
                self.selected_category = pos.clamp(0, len.saturating_sub(1) as i32) as usize;
                self.selected_command = 0;
            }
            Focus::Commands => {
                let len = self.visible_commands().len();
                self.selected_command = pos.clamp(0, len.saturating_sub(1) as i32) as usize;
            }
            Focus::Jobs => {
                let len = self.jobs.len();
                self.selected_job = pos.clamp(0, len.saturating_sub(1) as i32) as usize;
            }
            Focus::Preview => {}
        }
    }

    // === command-line mode ===============================================

    fn handle_command_mode_key(&mut self, ke: KeyEvent) {
        match ke.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.command_line_buf.clear();
            }
            KeyCode::Enter => {
                let cmd = std::mem::take(&mut self.command_line_buf);
                self.mode = Mode::Normal;
                self.execute_palette(&cmd);
            }
            KeyCode::Backspace => {
                self.command_line_buf.pop();
            }
            KeyCode::Char(c) => self.command_line_buf.push(c),
            _ => {}
        }
    }

    fn execute_palette(&mut self, raw: &str) {
        let raw = raw.trim();
        if raw.is_empty() {
            return;
        }
        let mut parts = raw.split_whitespace();
        let cmd = parts.next().unwrap_or("");
        let rest: Vec<&str> = parts.collect();
        match cmd {
            "q" | "quit" => self.running = false,
            "help" | "h" => self.modal = Modal::Help,
            "engagement" | "eng" => self.open_engagement_modal(rest.as_slice()),
            "target" | "t" => self.open_target_modal(rest.as_slice()),
            "creds" | "c" => self.open_creds_modal(rest.as_slice()),
            "tools" => self.modal = Modal::Tools(ToolsModal),
            "reload" => self.reload_library(),
            "splash" | "dashboard" => {
                self.splash = Some(SplashState::new());
                self.modal = Modal::None;
            }
            "write" | "w" => self.maybe_save_inline_edit_as(rest.first().copied()),
            "search" => {
                self.mode = Mode::SearchGlobal;
                self.search_buf = rest.join(" ");
                self.recompute_search(true);
            }
            other => self.flash_error(format!("unknown command :{}", other)),
        }
    }

    // === search mode =====================================================

    fn handle_search_mode_key(&mut self, ke: KeyEvent) {
        let global = matches!(self.mode, Mode::SearchGlobal);
        match ke.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.search_buf.clear();
                self.modal = Modal::None;
            }
            KeyCode::Enter => {
                self.commit_search_selection();
            }
            KeyCode::Backspace => {
                self.search_buf.pop();
                self.recompute_search(global);
            }
            KeyCode::Down => {
                if let Modal::Search { matches, cursor, .. } = &mut self.modal {
                    if !matches.is_empty() && *cursor + 1 < matches.len() {
                        *cursor += 1;
                    }
                }
            }
            KeyCode::Up => {
                if let Modal::Search { cursor, .. } = &mut self.modal {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
            }
            KeyCode::Char(c) => {
                self.search_buf.push(c);
                self.recompute_search(global);
            }
            _ => {}
        }
    }

    fn recompute_search(&mut self, global: bool) {
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT.match_paths());
        let pattern =
            Pattern::parse(&self.search_buf, nucleo_matcher::pattern::CaseMatching::Smart, nucleo_matcher::pattern::Normalization::Smart);
        let mut hits: Vec<(u16, SearchHit)> = Vec::new();
        let categories: Vec<_> = if global {
            self.library.categories.iter().collect()
        } else {
            self.current_category().map(|c| vec![c]).unwrap_or_default()
        };
        for cat in categories {
            for cmd in &cat.commands {
                let mut s = String::new();
                s.push_str(&cmd.title);
                s.push(' ');
                s.push_str(&cmd.id);
                s.push(' ');
                s.push_str(&cmd.tags.join(" "));
                let mut buf = Vec::new();
                let utf32 = Utf32Str::new(&s, &mut buf);
                if let Some(score) = pattern.score(utf32, &mut matcher) {
                    hits.push((
                        score as u16,
                        SearchHit {
                            category_id: cat.id.clone(),
                            category_name: cat.display_name.clone(),
                            command_id: cmd.id.clone(),
                            title: cmd.title.clone(),
                        },
                    ));
                }
            }
        }
        hits.sort_by(|a, b| b.0.cmp(&a.0));
        let matches: Vec<SearchHit> = hits.into_iter().map(|(_, h)| h).collect();
        let _ = global;
        self.modal = Modal::Search {
            matches,
            cursor: 0,
        };
    }

    fn commit_search_selection(&mut self) {
        if let Modal::Search { matches, cursor, .. } = &self.modal {
            if let Some(hit) = matches.get(*cursor) {
                let cat_id = hit.category_id.clone();
                let cmd_id = hit.command_id.clone();
                if let Some(cat_idx) = self
                    .library
                    .categories
                    .iter()
                    .position(|c| c.id == cat_id)
                {
                    self.selected_category = cat_idx;
                    let visible = self.visible_commands();
                    if let Some(pos) = visible.iter().position(|c| c.id == cmd_id) {
                        self.selected_command = pos;
                    }
                    self.focus = Focus::Commands;
                }
            }
        }
        self.modal = Modal::None;
        self.mode = Mode::Normal;
        self.search_buf.clear();
    }

    // === insert mode (edit modal) ========================================

    fn handle_insert_mode_key(&mut self, ke: KeyEvent) {
        if !matches!(self.modal, Modal::Edit(_)) {
            self.mode = Mode::Normal;
            return;
        }
        // If a save-as prompt is open, route keys to it.
        let prompt_open = match &self.modal {
            Modal::Edit(em) => em.save_as_prompt.is_some(),
            _ => false,
        };
        if prompt_open {
            self.handle_edit_save_prompt_key(ke);
            return;
        }
        let ctrl = ke.modifiers.contains(KeyModifiers::CONTROL);
        match ke.code {
            KeyCode::Esc => {
                self.modal = Modal::None;
                self.mode = Mode::Normal;
            }
            KeyCode::Char('s') if ctrl => self.commit_edit_modal_run(),
            KeyCode::Char('w') if ctrl => self.commit_edit_modal_save_prompt(),
            _ => {
                if let Modal::Edit(em) = &mut self.modal {
                    em.textarea.input(ke);
                }
            }
        }
    }

    fn handle_edit_save_prompt_key(&mut self, ke: KeyEvent) {
        let em = match &mut self.modal {
            Modal::Edit(em) => em,
            _ => return,
        };
        let prompt = match em.save_as_prompt.as_mut() {
            Some(p) => p,
            None => return,
        };
        match ke.code {
            KeyCode::Esc => {
                em.save_as_prompt = None;
            }
            KeyCode::Enter => {
                let id = std::mem::take(prompt);
                em.save_as_prompt = None;
                if !id.is_empty() {
                    self.maybe_save_inline_edit_as(Some(id.as_str()));
                }
            }
            KeyCode::Backspace => {
                prompt.pop();
            }
            KeyCode::Char(c) if !ke.modifiers.contains(KeyModifiers::CONTROL) => {
                prompt.push(c);
            }
            _ => {}
        }
    }

    fn commit_edit_modal_run(&mut self) {
        let (resolved, category_id, title, interactive) = match &self.modal {
            Modal::Edit(em) => (
                em.textarea.lines().join("\n"),
                em.category_id.clone(),
                em.command_title.clone(),
                em.interactive,
            ),
            _ => return,
        };
        self.modal = Modal::None;
        self.mode = Mode::Normal;
        self.spawn_resolved(resolved, Some(category_id), title, interactive);
    }

    fn commit_edit_modal_save_prompt(&mut self) {
        if let Modal::Edit(em) = &mut self.modal {
            em.save_as_prompt = Some(String::new());
        }
    }

    fn maybe_save_inline_edit_as(&mut self, name: Option<&str>) {
        let name = match name {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => {
                self.flash_error("usage: :write <new_id>".into());
                return;
            }
        };
        let (resolved, category_id, title, interactive, source) = match &self.modal {
            Modal::Edit(em) => (
                em.textarea.lines().join("\n"),
                em.category_id.clone(),
                em.command_title.clone(),
                em.interactive,
                em.source_command_id.clone(),
            ),
            _ => {
                self.flash_error(":write only valid while editing".into());
                return;
            }
        };
        if let Err(err) = self.write_override(&category_id, &name, &title, &resolved, interactive, source.as_deref()) {
            self.flash_error(format!("save failed: {}", err));
        } else {
            self.flash_ok(format!("saved {}:{}", category_id, name));
            self.modal = Modal::None;
            self.mode = Mode::Normal;
            self.reload_library();
        }
    }

    fn write_override(
        &self,
        category_id: &str,
        new_id: &str,
        title: &str,
        template: &str,
        interactive: bool,
        derived_from: Option<&str>,
    ) -> Result<()> {
        let eng = self
            .engagement
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no engagement active"))?;
        let overrides = Engagement::overrides_dir(&eng.dir);
        fs::create_dir_all(&overrides)?;
        let path = overrides.join(format!("{}.toml", category_id));
        let mut file = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            toml::from_str::<CategoryFile>(&raw).unwrap_or_else(|_| CategoryFile {
                category: category_id.to_string(),
                display_name: None,
                icon: None,
                order: None,
                command: Vec::new(),
            })
        } else {
            CategoryFile {
                category: category_id.to_string(),
                display_name: None,
                icon: None,
                order: None,
                command: Vec::new(),
            }
        };
        let description = derived_from.map(|src| format!("derived from {}", src));
        file.command.retain(|c| c.id != new_id);
        file.command.push(CommandEntry {
            id: new_id.to_string(),
            title: title.to_string(),
            template: template.to_string(),
            when: None,
            requires: Vec::new(),
            tags: vec!["override".to_string()],
            interactive,
            description,
            variants: Vec::<CommandVariant>::new(),
        });
        let body = toml::to_string_pretty(&file).context("serialize override toml")?;
        fs::write(&path, body)?;
        Ok(())
    }

    // === modal interactions ==============================================

    fn handle_modal_key(&mut self, ke: KeyEvent) {
        if matches!(ke.code, KeyCode::Esc) {
            self.modal = Modal::None;
            return;
        }
        match &mut self.modal {
            Modal::Engagement(_) => self.handle_engagement_modal(ke),
            Modal::Target(_) => self.handle_target_modal(ke),
            Modal::Creds(_) => self.handle_creds_modal(ke),
            _ => {}
        }
    }

    // engagement modal
    fn open_engagement_modal(&mut self, rest: &[&str]) {
        if rest.first() == Some(&"new") {
            let m = EngagementModal {
                available: Engagement::list(&config::engagements_root()),
                cursor: 0,
                new_name_prompt: Some(rest.get(1).map(|s| s.to_string()).unwrap_or_default()),
            };
            self.modal = Modal::Engagement(m);
            return;
        }
        if let Some(name) = rest.first() {
            self.switch_engagement(name);
            return;
        }
        let avail = Engagement::list(&config::engagements_root());
        let cursor = avail
            .iter()
            .position(|n| {
                self.engagement
                    .as_ref()
                    .map(|e| &e.meta.name == n)
                    .unwrap_or(false)
            })
            .unwrap_or(0);
        self.modal = Modal::Engagement(EngagementModal {
            available: avail,
            cursor,
            new_name_prompt: None,
        });
    }

    fn handle_engagement_modal(&mut self, ke: KeyEvent) {
        let m = match &mut self.modal {
            Modal::Engagement(m) => m,
            _ => return,
        };
        if let Some(prompt) = m.new_name_prompt.as_mut() {
            match ke.code {
                KeyCode::Enter => {
                    let name = std::mem::take(prompt);
                    self.modal = Modal::None;
                    if !name.is_empty() {
                        self.create_engagement(&name);
                    }
                }
                KeyCode::Backspace => {
                    prompt.pop();
                }
                KeyCode::Char(c) => prompt.push(c),
                _ => {}
            }
            return;
        }
        match ke.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if !m.available.is_empty() && m.cursor + 1 < m.available.len() {
                    m.cursor += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if m.cursor > 0 {
                    m.cursor -= 1;
                }
            }
            KeyCode::Char('n') => {
                m.new_name_prompt = Some(String::new());
            }
            KeyCode::Enter => {
                if let Some(name) = m.available.get(m.cursor).cloned() {
                    self.modal = Modal::None;
                    self.switch_engagement(&name);
                }
            }
            _ => {}
        }
    }

    fn create_engagement(&mut self, name: &str) {
        let root = config::engagements_root();
        if let Err(err) = fs::create_dir_all(&root) {
            self.flash_error(format!("create root: {}", err));
            return;
        }
        match Engagement::create(&root, name) {
            Ok(eng) => {
                self.adopt_engagement(eng);
                self.flash_ok(format!("engagement '{}' created", name));
            }
            Err(err) => self.flash_error(format!("create failed: {}", err)),
        }
    }

    fn switch_engagement(&mut self, name: &str) {
        let dir = config::engagements_root().join(name);
        match Engagement::load(dir) {
            Ok(eng) => {
                let n = eng.meta.name.clone();
                self.adopt_engagement(eng);
                self.flash_ok(format!("engagement '{}' active", n));
            }
            Err(err) => self.flash_error(format!("load failed: {}", err)),
        }
    }

    fn adopt_engagement(&mut self, eng: Engagement) {
        if let Some(parent) = config::last_engagement_marker().parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(config::last_engagement_marker(), &eng.meta.name);
        self.executor = Some(Executor::init(&eng));
        self.jobs = eng.history.recent.clone();
        self.engagement = Some(eng);
        self.library_sources =
            collect_library_sources(self.engagement.as_ref().map(|e| e.dir.as_path()));
        self.setup_library_watcher();
        self.reload_library();
    }

    async fn try_open_last_engagement(&mut self) {
        let marker = config::last_engagement_marker();
        let name = match fs::read_to_string(&marker) {
            Ok(s) => s.trim().to_string(),
            Err(_) => return,
        };
        if name.is_empty() {
            return;
        }
        let dir = config::engagements_root().join(&name);
        if !dir.exists() {
            return;
        }
        match Engagement::load(dir) {
            Ok(eng) => {
                self.adopt_engagement(eng);
            }
            Err(err) => tracing::warn!(?err, "failed to load last engagement"),
        }
    }

    // target modal
    fn open_target_modal(&mut self, rest: &[&str]) {
        if self.engagement.is_none() {
            self.flash_error("create or switch to an engagement first (:engagement new <name>)".into());
            return;
        }
        if let Some(name) = rest.first() {
            if let Some(eng) = self.engagement.as_mut() {
                if eng.targets.set_active(name) {
                    let _ = eng.save_targets();
                    self.flash_ok(format!("target '{}' active", name));
                } else {
                    self.flash_error(format!("no target named '{}'", name));
                }
            }
            return;
        }
        self.modal = Modal::Target(TargetModal {
            state: TargetModalState::List { cursor: 0 },
        });
    }

    fn handle_target_modal(&mut self, ke: KeyEvent) {
        let m = match &mut self.modal {
            Modal::Target(m) => m,
            _ => return,
        };
        match &mut m.state {
            TargetModalState::List { cursor } => {
                let eng = self.engagement.as_mut();
                let len = eng.as_ref().map(|e| e.targets.targets.len()).unwrap_or(0);
                match ke.code {
                    KeyCode::Down | KeyCode::Char('j') => {
                        if len > 0 && *cursor + 1 < len {
                            *cursor += 1;
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if *cursor > 0 {
                            *cursor -= 1;
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(eng) = eng {
                            if let Some(name) = eng.targets.targets.get(*cursor).map(|t| t.name.clone()) {
                                eng.targets.set_active(&name);
                                let _ = eng.save_targets();
                            }
                        }
                    }
                    KeyCode::Char('a') => {
                        let fields = TargetEditField::all()
                            .iter()
                            .map(|f| (*f, String::new()))
                            .collect();
                        m.state = TargetModalState::Edit {
                            fields,
                            focused: 0,
                            original_name: None,
                        };
                    }
                    KeyCode::Char('e') => {
                        if let Some(eng) = eng {
                            if let Some(t) = eng.targets.targets.get(*cursor) {
                                let fields = target_to_fields(t);
                                m.state = TargetModalState::Edit {
                                    fields,
                                    focused: 0,
                                    original_name: Some(t.name.clone()),
                                };
                            }
                        }
                    }
                    KeyCode::Char('d') => {
                        if let Some(eng) = eng {
                            if let Some(t) = eng.targets.targets.get(*cursor).cloned() {
                                eng.targets.remove(&t.name);
                                let _ = eng.save_targets();
                                if *cursor > 0 {
                                    *cursor -= 1;
                                }
                            }
                        }
                    }
                    KeyCode::Char('L') => {
                        if let Some(ip) = crate::engagement::target::detect_tun0_ip() {
                            if let Some(eng) = eng {
                                if let Some(t) = eng.targets.active_mut() {
                                    t.lhost = Some(ip.clone());
                                    let _ = eng.save_targets();
                                    self.flash_ok(format!("lhost set to {}", ip));
                                }
                            }
                        } else {
                            self.flash_error("could not detect tun0".into());
                        }
                    }
                    _ => {}
                }
            }
            TargetModalState::Edit {
                fields,
                focused,
                original_name,
            } => match ke.code {
                KeyCode::Tab | KeyCode::Down => {
                    *focused = (*focused + 1) % fields.len();
                }
                KeyCode::BackTab | KeyCode::Up => {
                    *focused = if *focused == 0 {
                        fields.len() - 1
                    } else {
                        *focused - 1
                    };
                }
                KeyCode::Backspace => {
                    if let Some((_, v)) = fields.get_mut(*focused) {
                        v.pop();
                    }
                }
                KeyCode::Char(c) => {
                    if let Some((_, v)) = fields.get_mut(*focused) {
                        v.push(c);
                    }
                }
                KeyCode::Enter => {
                    let new_target = match fields_to_target(fields) {
                        Ok(t) => t,
                        Err(err) => {
                            self.flash_error(format!("invalid target: {}", err));
                            return;
                        }
                    };
                    let original = original_name.clone();
                    if let Some(eng) = self.engagement.as_mut() {
                        if let Some(orig) = original {
                            if orig != new_target.name {
                                eng.targets.remove(&orig);
                                if eng.targets.active.as_deref() == Some(&orig) {
                                    eng.targets.active = Some(new_target.name.clone());
                                }
                            }
                        }
                        let n = new_target.name.clone();
                        eng.targets.upsert(new_target);
                        if eng.targets.active.is_none() {
                            eng.targets.active = Some(n);
                        }
                        let _ = eng.save_targets();
                    }
                    let new_m = TargetModal {
                        state: TargetModalState::List { cursor: 0 },
                    };
                    self.modal = Modal::Target(new_m);
                }
                _ => {}
            },
        }
    }

    // creds modal
    fn open_creds_modal(&mut self, rest: &[&str]) {
        if self.engagement.is_none() {
            self.flash_error("create or switch to an engagement first".into());
            return;
        }
        if let Some(name) = rest.first() {
            if let Some(eng) = self.engagement.as_mut() {
                if eng.profiles.set_active(name) {
                    let _ = eng.save_profiles();
                    self.flash_ok(format!("profile '{}' active", name));
                } else {
                    self.flash_error(format!("no profile named '{}'", name));
                }
            }
            return;
        }
        self.modal = Modal::Creds(CredsModal {
            state: CredsModalState::List { cursor: 0 },
        });
    }

    fn handle_creds_modal(&mut self, ke: KeyEvent) {
        let m = match &mut self.modal {
            Modal::Creds(m) => m,
            _ => return,
        };
        match &mut m.state {
            CredsModalState::List { cursor } => {
                let eng = self.engagement.as_mut();
                let len = eng.as_ref().map(|e| e.profiles.profiles.len()).unwrap_or(0);
                match ke.code {
                    KeyCode::Down | KeyCode::Char('j') => {
                        if len > 0 && *cursor + 1 < len {
                            *cursor += 1;
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if *cursor > 0 {
                            *cursor -= 1;
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(eng) = eng {
                            if let Some(name) = eng.profiles.profiles.get(*cursor).map(|p| p.name.clone()) {
                                eng.profiles.set_active(&name);
                                let _ = eng.save_profiles();
                            }
                        }
                    }
                    KeyCode::Char('a') => {
                        let fields = CredEditField::all()
                            .iter()
                            .map(|f| (*f, String::new()))
                            .collect();
                        m.state = CredsModalState::Edit {
                            fields,
                            focused: 0,
                            original_name: None,
                        };
                    }
                    KeyCode::Char('e') => {
                        if let Some(eng) = eng {
                            if let Some(p) = eng.profiles.profiles.get(*cursor) {
                                let fields = profile_to_fields(p);
                                m.state = CredsModalState::Edit {
                                    fields,
                                    focused: 0,
                                    original_name: Some(p.name.clone()),
                                };
                            }
                        }
                    }
                    KeyCode::Char('d') => {
                        if let Some(eng) = eng {
                            if let Some(p) = eng.profiles.profiles.get(*cursor).cloned() {
                                eng.profiles.remove(&p.name);
                                let _ = eng.save_profiles();
                                if *cursor > 0 {
                                    *cursor -= 1;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            CredsModalState::Edit {
                fields,
                focused,
                original_name,
            } => match ke.code {
                KeyCode::Tab | KeyCode::Down => {
                    *focused = (*focused + 1) % fields.len();
                }
                KeyCode::BackTab | KeyCode::Up => {
                    *focused = if *focused == 0 {
                        fields.len() - 1
                    } else {
                        *focused - 1
                    };
                }
                KeyCode::Backspace => {
                    if let Some((_, v)) = fields.get_mut(*focused) {
                        v.pop();
                    }
                }
                KeyCode::Char(c) => {
                    if let Some((_, v)) = fields.get_mut(*focused) {
                        v.push(c);
                    }
                }
                KeyCode::Enter => {
                    let prof = match fields_to_profile(fields) {
                        Ok(p) => p,
                        Err(err) => {
                            self.flash_error(format!("invalid profile: {}", err));
                            return;
                        }
                    };
                    let original = original_name.clone();
                    if let Some(eng) = self.engagement.as_mut() {
                        if let Some(orig) = original {
                            if orig != prof.name {
                                eng.profiles.remove(&orig);
                                if eng.profiles.active.as_deref() == Some(&orig) {
                                    eng.profiles.active = Some(prof.name.clone());
                                }
                            }
                        }
                        let n = prof.name.clone();
                        eng.profiles.upsert(prof);
                        if eng.profiles.active.is_none() {
                            eng.profiles.active = Some(n);
                        }
                        let _ = eng.save_profiles();
                    }
                    let new_m = CredsModal {
                        state: CredsModalState::List { cursor: 0 },
                    };
                    self.modal = Modal::Creds(new_m);
                }
                _ => {}
            },
        }
    }

    // === command running =================================================

    fn run_current(&mut self, count: u32) {
        let cmd = match self.current_command() {
            Some(c) => c.clone(),
            None => return,
        };
        let cat_id = match self.current_category() {
            Some(c) => c.id.clone(),
            None => return,
        };
        let ctx = self.render_context();
        let template = cmd.applicable_template(&|w: &str| crate::render::condition::evaluate(w, &ctx));
        let rendered = match render::render(template, &ctx) {
            Ok(r) => r,
            Err(err) => {
                self.flash_error(format!("render failed: {}", err));
                return;
            }
        };
        for _ in 0..count.max(1) {
            self.spawn_resolved(
                rendered.resolved.clone(),
                Some(cat_id.clone()),
                cmd.title.clone(),
                cmd.interactive,
            );
        }
    }

    fn run_multi_or_all(&mut self) {
        let entries: Vec<(String, CommandEntry)> = if self.multi_selected.is_empty() {
            // run all visible in current category
            let cat = match self.current_category() {
                Some(c) => c.clone(),
                None => return,
            };
            let ctx = self.render_context();
            cat.commands
                .iter()
                .filter(|c| {
                    c.is_applicable(&|w: &str| crate::render::condition::evaluate(w, &ctx))
                })
                .map(|c| (cat.id.clone(), c.clone()))
                .collect()
        } else {
            let mut acc = Vec::new();
            for (cat_id, cmd_id) in &self.multi_selected {
                if let Some(cat) = self.library.category(cat_id) {
                    if let Some(cmd) = cat.commands.iter().find(|c| &c.id == cmd_id) {
                        acc.push((cat_id.clone(), cmd.clone()));
                    }
                }
            }
            acc
        };
        let ctx = self.render_context();
        for (cat_id, cmd) in entries {
            let tmpl = cmd
                .applicable_template(&|w: &str| crate::render::condition::evaluate(w, &ctx))
                .to_string();
            match render::render(&tmpl, &ctx) {
                Ok(r) => self.spawn_resolved(r.resolved, Some(cat_id), cmd.title, cmd.interactive),
                Err(err) => self.flash_error(format!("render: {}", err)),
            }
        }
        self.multi_selected.clear();
    }

    fn spawn_resolved(
        &mut self,
        resolved: String,
        category_id: Option<String>,
        title: String,
        interactive: bool,
    ) {
        let exe = match self.executor.as_ref() {
            Some(e) => e,
            None => {
                self.flash_error("no engagement / executor".into());
                return;
            }
        };
        let req = SpawnRequest {
            command_id: category_id.map(|cid| format!("{}.{}", cid, slug(&title))),
            command_title: title,
            resolved,
            interactive,
            target: self
                .engagement
                .as_ref()
                .and_then(|e| e.active_target().map(|t| t.name.clone())),
            profile: self
                .engagement
                .as_ref()
                .and_then(|e| e.active_profile().map(|p| p.name.clone())),
        };
        match exe.spawn(req) {
            Ok(rec) => {
                if let Some(eng) = self.engagement.as_mut() {
                    let _ = eng.history.append(&rec);
                }
                self.jobs.push(rec);
                self.flash_ok("spawned".into());
            }
            Err(err) => self.flash_error(format!("spawn failed: {}", err)),
        }
    }

    fn open_edit_modal(&mut self) {
        let cmd = match self.current_command() {
            Some(c) => c.clone(),
            None => return,
        };
        let cat_id = match self.current_category() {
            Some(c) => c.id.clone(),
            None => return,
        };
        let ctx = self.render_context();
        let template = cmd.applicable_template(&|w: &str| crate::render::condition::evaluate(w, &ctx));
        let rendered = match render::render(template, &ctx) {
            Ok(r) => r,
            Err(err) => {
                self.flash_error(format!("render: {}", err));
                return;
            }
        };
        let mut ta = tui_textarea::TextArea::new(rendered.resolved.lines().map(|l| l.to_string()).collect());
        ta.set_line_number_style(crate::ui::theme::Theme::muted());
        ta.set_block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(crate::ui::theme::Theme::border_active())
                .style(crate::ui::theme::Theme::panel()),
        );
        self.modal = Modal::Edit(EditModal {
            source_command_id: Some(cmd.id.clone()),
            command_title: cmd.title.clone(),
            textarea: ta,
            interactive: cmd.interactive,
            category_id: cat_id,
            save_as_prompt: None,
        });
        self.mode = Mode::Insert;
    }

    fn yank(&mut self, raw: bool) {
        let cmd = match self.current_command() {
            Some(c) => c.clone(),
            None => return,
        };
        let text = if raw {
            cmd.template.clone()
        } else {
            let ctx = self.render_context();
            let template = cmd
                .applicable_template(&|w: &str| crate::render::condition::evaluate(w, &ctx));
            match render::render(template, &ctx) {
                Ok(r) => r.resolved,
                Err(err) => {
                    self.flash_error(format!("render: {}", err));
                    return;
                }
            }
        };
        match clipboard::copy(&text) {
            Ok(()) => self.flash_ok(if raw { "yanked raw template".into() } else { "yanked resolved command".into() }),
            Err(err) => self.flash_error(format!("clipboard: {}", err)),
        }
    }

    fn toggle_select_current(&mut self) {
        if let (Some(cat), Some(cmd)) = (self.current_category().cloned(), self.current_command()) {
            let key = (cat.id, cmd.id.clone());
            if !self.multi_selected.insert(key.clone()) {
                self.multi_selected.remove(&key);
            }
        }
    }

    fn open_active_job(&mut self) {
        let exe = match self.executor.as_ref() {
            Some(e) => e,
            None => return,
        };
        let job = match self.jobs.iter().rev().nth(self.selected_job) {
            Some(j) => j.clone(),
            None => return,
        };
        match exe.focus_job(&job) {
            Ok(FocusResult::Focused) => self.flash_ok(format!("focused window {}", job.tmux_window.unwrap_or_default())),
            Ok(FocusResult::AttachCommand(cmd)) => {
                if let Err(err) = clipboard::copy(&cmd) {
                    tracing::warn!(?err, "clipboard");
                }
                self.flash_ok(format!("attach cmd yanked: {}", cmd));
            }
            Ok(FocusResult::Unfocusable) => self.flash_error("job has no tmux window".into()),
            Err(err) => self.flash_error(format!("focus: {}", err)),
        }
    }

    fn kill_selected_job(&mut self) {
        let job_id = match self.jobs.iter().rev().nth(self.selected_job).map(|j| j.id.clone()) {
            Some(id) => id,
            None => return,
        };
        let job_clone = self.jobs.iter().find(|j| j.id == job_id).cloned();
        if let (Some(exe), Some(job)) = (self.executor.as_ref(), job_clone) {
            if let Err(err) = exe.kill_job(&job) {
                self.flash_error(format!("kill: {}", err));
            } else {
                if let Some(slot) = self.jobs.iter_mut().find(|j| j.id == job_id) {
                    slot.status = JobStatus::Killed;
                    slot.finished_at = Some(chrono::Utc::now());
                }
                if let Some(eng) = self.engagement.as_mut() {
                    if let Some(rec) = self.jobs.iter().find(|j| j.id == job_id) {
                        eng.history.update(rec);
                    }
                }
            }
        }
    }

    fn set_mark(&mut self, ch: char) {
        if let (Some(cat), Some(cmd)) = (self.current_category(), self.current_command()) {
            self.marks.insert(ch, (cat.id.clone(), cmd.id.clone()));
            self.flash_ok(format!("mark '{}' set", ch));
        }
    }

    fn jump_mark(&mut self, ch: char) {
        if let Some((cat_id, cmd_id)) = self.marks.get(&ch).cloned() {
            if let Some(cidx) = self.library.categories.iter().position(|c| c.id == cat_id) {
                self.selected_category = cidx;
                if let Some(pidx) = self.visible_commands().iter().position(|c| c.id == cmd_id) {
                    self.selected_command = pidx;
                }
                self.focus = Focus::Commands;
                return;
            }
        }
        self.flash_error(format!("no mark '{}'", ch));
    }

    // === library reload ==================================================

    fn setup_library_watcher(&mut self) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        let watcher_tx = tx.clone();
        let mut w: notify::RecommendedWatcher = match notify::recommended_watcher(
            move |res: notify::Result<notify::Event>| {
                if let Ok(ev) = res {
                    use notify::EventKind::*;
                    if matches!(ev.kind, Modify(_) | Create(_) | Remove(_)) {
                        let _ = watcher_tx.send(());
                    }
                }
            },
        ) {
            Ok(w) => w,
            Err(err) => {
                tracing::warn!(?err, "could not create library watcher");
                return;
            }
        };
        use notify::Watcher;
        for src in &self.library_sources {
            if src.exists() {
                let _ = w.watch(src, notify::RecursiveMode::Recursive);
            }
        }
        self._watcher = Some(w);
        self.library_reload_rx = Some(rx);
    }

    fn reload_library(&mut self) {
        let paths: Vec<&std::path::Path> = self.library_sources.iter().map(|p| p.as_path()).collect();
        match CommandLibrary::load(&paths) {
            Ok(lib) => {
                self.library = lib;
                self.selected_category = self
                    .selected_category
                    .min(self.library.categories.len().saturating_sub(1));
                self.selected_command = 0;
                self.flash_ok("library reloaded".into());
            }
            Err(err) => self.flash_error(format!("reload: {}", err)),
        }
    }

    // === helpers exposed to UI ==========================================

    pub fn current_category(&self) -> Option<&crate::library::Category> {
        self.library.categories.get(self.selected_category)
    }

    pub fn visible_commands(&self) -> Vec<CommandEntry> {
        let ctx = self.render_context();
        let cat = match self.current_category() {
            Some(c) => c,
            None => return Vec::new(),
        };
        cat.commands
            .iter()
            .filter(|c| c.is_applicable(&|w: &str| crate::render::condition::evaluate(w, &ctx)))
            .cloned()
            .collect()
    }

    pub fn current_command(&self) -> Option<CommandEntry> {
        self.visible_commands().get(self.selected_command).cloned()
    }

    pub fn command_is_applicable(&self, cmd: &CommandEntry) -> bool {
        let ctx = self.render_context();
        cmd.is_applicable(&|w: &str| crate::render::condition::evaluate(w, &ctx))
    }

    pub fn render_command_preview(&self, cmd: &CommandEntry) -> (String, Vec<String>) {
        let ctx = self.render_context();
        let tmpl = cmd.applicable_template(&|w: &str| crate::render::condition::evaluate(w, &ctx));
        match render::render(tmpl, &ctx) {
            Ok(r) => (r.resolved, r.unresolved),
            Err(err) => (format!("render error: {}", err), Vec::new()),
        }
    }

    fn render_context(&self) -> RenderContext {
        let mut ctx = RenderContext::default();
        if let Some(eng) = &self.engagement {
            ctx.target = eng.active_target().cloned();
            ctx.profile = eng.active_profile().cloned();
        }
        ctx
    }

    pub fn multi_selected_contains(&self, cmd_id: &str) -> bool {
        match self.current_category() {
            Some(cat) => self
                .multi_selected
                .contains(&(cat.id.clone(), cmd_id.to_string())),
            None => false,
        }
    }

    pub fn jobs_running_count(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| j.status == JobStatus::Running)
            .count()
    }
    pub fn jobs_completed_count(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| j.status == JobStatus::Completed)
            .count()
    }
    pub fn jobs_failed_count(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| matches!(j.status, JobStatus::Failed | JobStatus::Killed))
            .count()
    }
    pub fn jobs_last_n(&self, n: usize) -> Vec<&JobRecord> {
        let start = self.jobs.len().saturating_sub(n);
        self.jobs[start..].iter().collect()
    }

    fn flash_ok(&mut self, text: String) {
        self.flash = Some(FlashMessage {
            text,
            is_error: false,
            at: Instant::now(),
        });
    }
    fn flash_error(&mut self, text: String) {
        self.flash = Some(FlashMessage {
            text,
            is_error: true,
            at: Instant::now(),
        });
    }
}

// === conversion helpers =================================================

fn slug(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn collect_library_sources(engagement_dir: Option<&std::path::Path>) -> Vec<PathBuf> {
    let mut v = Vec::new();
    let builtin = config::builtin_commands_dir();
    if builtin.exists() {
        v.push(builtin);
    }
    if let Some(d) = engagement_dir {
        let local = Engagement::overrides_dir(d);
        if local.exists() {
            v.push(local);
        }
    }
    v
}

fn target_to_fields(t: &Target) -> Vec<(TargetEditField, String)> {
    TargetEditField::all()
        .iter()
        .map(|f| {
            let v = match f {
                TargetEditField::Name => t.name.clone(),
                TargetEditField::Ip => t.ip.clone().unwrap_or_default(),
                TargetEditField::Hostname => t.hostname.clone().unwrap_or_default(),
                TargetEditField::Dc => t.dc_name.clone().unwrap_or_default(),
                TargetEditField::Lhost => t.lhost.clone().unwrap_or_default(),
                TargetEditField::Lport => t.lport.map(|p| p.to_string()).unwrap_or_default(),
                TargetEditField::Notes => t.notes.clone().unwrap_or_default(),
            };
            (*f, v)
        })
        .collect()
}

fn fields_to_target(fields: &[(TargetEditField, String)]) -> Result<Target> {
    let mut t = Target::default();
    for (f, v) in fields {
        let trimmed = v.trim();
        match f {
            TargetEditField::Name => t.name = trimmed.to_string(),
            TargetEditField::Ip => {
                t.ip = (!trimmed.is_empty()).then(|| trimmed.to_string());
            }
            TargetEditField::Hostname => {
                t.hostname = (!trimmed.is_empty()).then(|| trimmed.to_string());
            }
            TargetEditField::Dc => {
                t.dc_name = (!trimmed.is_empty()).then(|| trimmed.to_string());
            }
            TargetEditField::Lhost => {
                t.lhost = (!trimmed.is_empty()).then(|| trimmed.to_string());
            }
            TargetEditField::Lport => {
                t.lport = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.parse().context("lport must be u16")?)
                };
            }
            TargetEditField::Notes => {
                t.notes = (!trimmed.is_empty()).then(|| trimmed.to_string());
            }
        }
    }
    if t.name.is_empty() {
        anyhow::bail!("name required");
    }
    Ok(t)
}

fn profile_to_fields(p: &CredentialProfile) -> Vec<(CredEditField, String)> {
    CredEditField::all()
        .iter()
        .map(|f| {
            let v = match f {
                CredEditField::Name => p.name.clone(),
                CredEditField::Username => p.username.clone(),
                CredEditField::Domain => p.domain.clone().unwrap_or_default(),
                CredEditField::Kind => p.kind.as_str().to_string(),
                CredEditField::Password => p.password.clone().unwrap_or_default(),
                CredEditField::NtHash => p.nt_hash.clone().unwrap_or_default(),
                CredEditField::Ticket => p.ticket_path.clone().unwrap_or_default(),
                CredEditField::Notes => p.notes.clone().unwrap_or_default(),
            };
            (*f, v)
        })
        .collect()
}

fn fields_to_profile(fields: &[(CredEditField, String)]) -> Result<CredentialProfile> {
    let mut p = CredentialProfile {
        name: String::new(),
        username: String::new(),
        domain: None,
        kind: CredKind::None,
        password: None,
        nt_hash: None,
        ticket_path: None,
        notes: None,
    };
    for (f, v) in fields {
        let trimmed = v.trim();
        match f {
            CredEditField::Name => p.name = trimmed.to_string(),
            CredEditField::Username => p.username = trimmed.to_string(),
            CredEditField::Domain => {
                p.domain = (!trimmed.is_empty()).then(|| trimmed.to_string());
            }
            CredEditField::Kind => {
                p.kind = match trimmed.to_ascii_lowercase().as_str() {
                    "plaintext" | "p" | "" => CredKind::Plaintext,
                    "ntlm" | "hash" | "h" => CredKind::Ntlm,
                    "kerberos" | "k" | "kerb" => CredKind::Kerberos,
                    "none" | "guest" => CredKind::None,
                    other => anyhow::bail!("unknown kind '{}'", other),
                };
            }
            CredEditField::Password => {
                p.password = (!trimmed.is_empty()).then(|| trimmed.to_string());
            }
            CredEditField::NtHash => {
                p.nt_hash = (!trimmed.is_empty()).then(|| trimmed.to_string());
            }
            CredEditField::Ticket => {
                p.ticket_path = (!trimmed.is_empty()).then(|| trimmed.to_string());
            }
            CredEditField::Notes => {
                p.notes = (!trimmed.is_empty()).then(|| trimmed.to_string());
            }
        }
    }
    if p.name.is_empty() {
        anyhow::bail!("name required");
    }
    if p.username.is_empty() {
        anyhow::bail!("username required");
    }
    Ok(p)
}
