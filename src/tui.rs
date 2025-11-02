mod examples;

use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table, TableState, Tabs,
        Wrap,
    },
    Frame, Terminal,
};
use ratatui::crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use serde::{Deserialize, Serialize};
use std::io;

use crate::{core::database::models::{Cluster, Config, Job, Status}, tui::examples::generate_sample_data};

impl Status {
    fn is_finished(&self) -> bool {
        matches!(
            self,
            Status::Completed | Status::Failed | Status::Timeout | Status::FailedSubmission
        )
    }

    fn is_active(&self) -> bool {
        matches!(self, Status::Running)
    }

    fn is_queued(&self) -> bool {
        matches!(self, Status::Queued | Status::VirtualQueue)
    }

    fn color(&self) -> Color {
        match self {
            Status::Completed => Color::Green,
            Status::Running => Color::Cyan,
            Status::Queued | Status::VirtualQueue => Color::Yellow,
            Status::Failed | Status::FailedSubmission => Color::Red,
            Status::Timeout => Color::Magenta,
            Status::Created => Color::Gray,
        }
    }

    fn all_variants() -> Vec<Status> {
        vec![
            Status::Created,
            Status::Queued,
            Status::VirtualQueue,
            Status::Running,
            Status::Completed,
            Status::Failed,
            Status::FailedSubmission,
            Status::Timeout,
        ]
    }
}

// Configuration structures
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColumnConfig {
    pub columns: Vec<ColumnType>,
    pub sort_by: ColumnType,
    pub sort_ascending: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ColumnType {
    Id,
    JobName,
    ConfigId,
    Status,
    SubmitTime,
    EndTime,
    // FIXME ExitCode,
    JobId,
}

impl ColumnType {
    fn name(&self) -> &str {
        match self {
            ColumnType::Id => "ID",
            ColumnType::JobName => "Job Name",
            ColumnType::ConfigId => "Config",
            ColumnType::Status => "Status",
            ColumnType::SubmitTime => "Submit Time",
            ColumnType::EndTime => "End Time",
            // FIXME ColumnType::ExitCode => "Exit Code",
            ColumnType::JobId => "Job ID",
        }
    }

    fn width(&self) -> u16 {
        match self {
            ColumnType::Id => 6,
            ColumnType::JobName => 25,
            ColumnType::ConfigId => 8,
            ColumnType::Status => 15,
            ColumnType::SubmitTime => 15,
            ColumnType::EndTime => 15,
            // FIXME ColumnType::ExitCode => 10,
            ColumnType::JobId => 12,
        }
    }
}

impl Default for ColumnConfig {
    fn default() -> Self {
        Self {
            columns: vec![
                ColumnType::Id,
                ColumnType::JobName,
                ColumnType::ConfigId,
                ColumnType::Status,
                ColumnType::SubmitTime,
                ColumnType::EndTime,
            ],
            sort_by: ColumnType::Id,
            sort_ascending: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobFilter {
    pub statuses: Vec<Status>,
    pub config_ids: Vec<i32>,
}

impl Default for JobFilter {
    fn default() -> Self {
        Self {
            statuses: vec![],
            config_ids: vec![],
        }
    }
}

// Main application state
pub enum AppMode {
    JobMonitoring(JobTab),
    LogViewer,
    ScriptViewer,
    ConfigMonitoring,
    ArchiveMonitoring,
    ColumnConfig,
    FilterConfig(FilterSection),
    Menu,
}

#[derive(Clone, Copy, PartialEq)]
pub enum JobTab {
    Finished,
    Active,
    Queued,
}

#[derive(Clone, Copy, PartialEq)]
pub enum FilterSection {
    Status,
    Config,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ActionTarget {
    Selected,
    All,
}

pub struct App {
    mode: AppMode,
    jobs: Vec<Job>,
    configs: Vec<Config>,
    clusters: Vec<Cluster>,
    job_table_state: TableState,
    column_config: ColumnConfig,
    job_filter: JobFilter,
    log_scroll: u16,
    script_scroll: u16,
    menu_state: ListState,
    column_list_state: ListState,
    filter_status_list_state: ListState,
    filter_config_list_state: ListState,
    selected_action_list_state: ListState,
    all_action_list_state: ListState,
    action_target: ActionTarget,
    show_actions_popup: bool,
    show_confirmation_popup: bool,
    confirmation_message: String,
    pending_action: Option<(String, ActionTarget)>,
    current_log: Option<String>,
    current_script: Option<String>,
}

impl App {
    pub fn new(jobs: Vec<Job>, configs: Vec<Config>, clusters: Vec<Cluster>) -> Self {
        let mut app = Self {
            mode: AppMode::JobMonitoring(JobTab::Finished),
            jobs,
            configs,
            clusters,
            job_table_state: TableState::default(),
            column_config: ColumnConfig::default(),
            job_filter: JobFilter::default(),
            log_scroll: 0,
            script_scroll: 0,
            menu_state: ListState::default(),
            column_list_state: ListState::default(),
            filter_status_list_state: ListState::default(),
            filter_config_list_state: ListState::default(),
            selected_action_list_state: ListState::default(),
            all_action_list_state: ListState::default(),
            action_target: ActionTarget::Selected,
            show_actions_popup: false,
            show_confirmation_popup: false,
            confirmation_message: String::new(),
            pending_action: None,
            current_log: None,
            current_script: None,
        };
        app.job_table_state.select(Some(0));
        app.selected_action_list_state.select(Some(0));
        app.all_action_list_state.select(Some(0));
        app
    }

    fn get_filtered_jobs(&self, tab: JobTab) -> Vec<&Job> {
        self.jobs
            .iter()
            .filter(|job| {
                // Filter by tab
                let tab_match = match tab {
                    JobTab::Finished => job.status.is_finished(),
                    JobTab::Active => job.status.is_active(),
                    JobTab::Queued => job.status.is_queued(),
                };

                if !tab_match {
                    return false;
                }

                // Apply status filter
                if !self.job_filter.statuses.is_empty()
                    && !self.job_filter.statuses.contains(&job.status)
                {
                    return false;
                }

                // Apply config filter
                if !self.job_filter.config_ids.is_empty()
                    && !self.job_filter.config_ids.contains(&job.config_id)
                {
                    return false;
                }

                true
            })
            .collect()
    }

    fn get_job_counts(&self) -> (usize, usize, usize, usize) {
        let finished = self.get_filtered_jobs(JobTab::Finished).len();
        let active = self.get_filtered_jobs(JobTab::Active).len();
        let queued = self.get_filtered_jobs(JobTab::Queued).len();
        (finished, active, queued, finished + active + queued)
    }

    fn selected_job(&self, tab: JobTab) -> Option<&Job> {
        let jobs = self.get_filtered_jobs(tab);
        self.job_table_state.selected().and_then(|i| jobs.get(i).copied())
    }

    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        loop {
            terminal.draw(|f| self.draw(f))?;

            match event::read()? {
                Event::Key(key) => {
                    if key.code == KeyCode::Char('q') && matches!(self.mode, AppMode::JobMonitoring(_)) {
                        return Ok(());
                    }
                    self.handle_input(key.code, key.modifiers);
                }
                Event::Mouse(mouse) => {
                    self.handle_mouse(mouse);
                }
                _ => {}
            }
        }
    }

    fn handle_mouse(&mut self, mouse: event::MouseEvent) {
        use event::MouseEventKind;
        
        match &self.mode {
            AppMode::JobMonitoring(tab) => {
                let current_tab = *tab;
                if self.show_actions_popup && !self.show_confirmation_popup {
                    // Handle mouse in actions popup
                    match mouse.kind {
                        MouseEventKind::ScrollDown => {
                            match self.action_target {
                                ActionTarget::Selected => {
                                    let i = self.selected_action_list_state.selected().unwrap_or(0);
                                    self.selected_action_list_state.select(Some((i + 1).min(3)));
                                }
                                ActionTarget::All => {
                                    let i = self.all_action_list_state.selected().unwrap_or(0);
                                    self.all_action_list_state.select(Some((i + 1).min(2)));
                                }
                            }
                        }
                        MouseEventKind::ScrollUp => {
                            match self.action_target {
                                ActionTarget::Selected => {
                                    let i = self.selected_action_list_state.selected().unwrap_or(0);
                                    self.selected_action_list_state.select(Some(i.saturating_sub(1)));
                                }
                                ActionTarget::All => {
                                    let i = self.all_action_list_state.selected().unwrap_or(0);
                                    self.all_action_list_state.select(Some(i.saturating_sub(1)));
                                }
                            }
                        }
                        _ => {}
                    }
                } else if !self.show_confirmation_popup {
                    // Handle mouse in job table
                    match mouse.kind {
                        MouseEventKind::ScrollDown => {
                            let jobs = self.get_filtered_jobs(current_tab);
                            if !jobs.is_empty() {
                                let i = self.job_table_state.selected().unwrap_or(0);
                                self.job_table_state.select(Some((i + 1).min(jobs.len() - 1)));
                            }
                        }
                        MouseEventKind::ScrollUp => {
                            let i = self.job_table_state.selected().unwrap_or(0);
                            self.job_table_state.select(Some(i.saturating_sub(1)));
                        }
                        _ => {}
                    }
                }
            }
            AppMode::LogViewer => {
                match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        self.log_scroll = self.log_scroll.saturating_add(3);
                    }
                    MouseEventKind::ScrollUp => {
                        self.log_scroll = self.log_scroll.saturating_sub(3);
                    }
                    _ => {}
                }
            }
            AppMode::ScriptViewer => {
                match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        self.script_scroll = self.script_scroll.saturating_add(3);
                    }
                    MouseEventKind::ScrollUp => {
                        self.script_scroll = self.script_scroll.saturating_sub(3);
                    }
                    _ => {}
                }
            }
            AppMode::Menu => {
                match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        let i = self.menu_state.selected().unwrap_or(0);
                        self.menu_state.select(Some((i + 1).min(2)));
                    }
                    MouseEventKind::ScrollUp => {
                        let i = self.menu_state.selected().unwrap_or(0);
                        self.menu_state.select(Some(i.saturating_sub(1)));
                    }
                    _ => {}
                }
            }
            AppMode::FilterConfig(section) => {
                let current_section = *section;
                match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        match current_section {
                            FilterSection::Status => {
                                let i = self.filter_status_list_state.selected().unwrap_or(0);
                                let max = Status::all_variants().len().saturating_sub(1);
                                self.filter_status_list_state.select(Some((i + 1).min(max)));
                            }
                            FilterSection::Config => {
                                let i = self.filter_config_list_state.selected().unwrap_or(0);
                                let max = self.configs.len().saturating_sub(1);
                                self.filter_config_list_state.select(Some((i + 1).min(max)));
                            }
                        }
                    }
                    MouseEventKind::ScrollUp => {
                        match current_section {
                            FilterSection::Status => {
                                let i = self.filter_status_list_state.selected().unwrap_or(0);
                                self.filter_status_list_state.select(Some(i.saturating_sub(1)));
                            }
                            FilterSection::Config => {
                                let i = self.filter_config_list_state.selected().unwrap_or(0);
                                self.filter_config_list_state.select(Some(i.saturating_sub(1)));
                            }
                        }
                    }
                    _ => {}
                }
            }
            AppMode::ColumnConfig => {
                match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        let i = self.column_list_state.selected().unwrap_or(0);
                        self.column_list_state.select(Some((i + 1).min(self.column_config.columns.len())));
                    }
                    MouseEventKind::ScrollUp => {
                        let i = self.column_list_state.selected().unwrap_or(0);
                        self.column_list_state.select(Some(i.saturating_sub(1)));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn handle_input(&mut self, key: KeyCode, modifiers: KeyModifiers) {
        match &self.mode {
            AppMode::JobMonitoring(tab) => {
                let current_tab = *tab;
                match key {
                    KeyCode::Char('m') => {
                        self.mode = AppMode::Menu;
                        self.menu_state.select(Some(0));
                    }
                    KeyCode::Char('c') => {
                        self.mode = AppMode::ColumnConfig;
                        self.column_list_state.select(Some(0));
                    }
                    KeyCode::Char('f') => {
                        self.mode = AppMode::FilterConfig(FilterSection::Status);
                        self.filter_status_list_state.select(Some(0));
                    }
                    KeyCode::Enter => {
                        if !self.show_actions_popup && !self.show_confirmation_popup {
                            if let Some(job) = self.selected_job(current_tab) {
                                self.current_log = job.get_stdout().ok();
                                self.log_scroll = 0;
                                self.mode = AppMode::LogViewer;
                            }
                        } else if self.show_confirmation_popup {
                            // Confirm action
                            if let Some((action, target)) = self.pending_action.take() {
                                self.execute_action(&action, target, current_tab);
                            }
                            self.show_confirmation_popup = false;
                            self.show_actions_popup = false;
                        } else if self.show_actions_popup {
                            // Select action and show confirmation
                            let action_name = match self.action_target {
                                ActionTarget::Selected => {
                                    let i = self.selected_action_list_state.selected().unwrap_or(0);
                                    match i {
                                        0 => "Cancel Job",
                                        1 => "Archive Job",
                                        2 => "Re-run Job",
                                        3 => "Delete Job",
                                        _ => return,
                                    }
                                }
                                ActionTarget::All => {
                                    let i = self.all_action_list_state.selected().unwrap_or(0);
                                    match i {
                                        0 => "Cancel All Jobs",
                                        1 => "Archive All Jobs",
                                        2 => "Delete All Jobs",
                                        _ => return,
                                    }
                                }
                            };
                            self.show_confirmation(action_name, current_tab);
                        }
                    }
                    KeyCode::Char('s') => {
                        if !self.show_actions_popup && !self.show_confirmation_popup {
                            if let Some(job) = self.selected_job(current_tab) {
                                self.current_script = job.get_script().ok();
                                self.script_scroll = 0;
                                self.mode = AppMode::ScriptViewer;
                            }
                        }
                    }
                    KeyCode::Char('a') => {
                        if !self.show_confirmation_popup {
                            self.show_actions_popup = true;
                            self.action_target = ActionTarget::Selected;
                            self.selected_action_list_state.select(Some(0));
                            self.all_action_list_state.select(Some(0));
                        }
                    }
                    KeyCode::Esc | KeyCode::Char('q') => {
                        if self.show_confirmation_popup {
                            self.show_confirmation_popup = false;
                            self.pending_action = None;
                        } else if self.show_actions_popup {
                            self.show_actions_popup = false;
                        }
                    }
                    KeyCode::Tab => {
                        if self.show_actions_popup && !self.show_confirmation_popup {
                            self.action_target = match self.action_target {
                                ActionTarget::Selected => ActionTarget::All,
                                ActionTarget::All => ActionTarget::Selected,
                            };
                        } else if !self.show_actions_popup && !self.show_confirmation_popup {
                            self.mode = AppMode::JobMonitoring(match current_tab {
                                JobTab::Finished => JobTab::Active,
                                JobTab::Active => JobTab::Queued,
                                JobTab::Queued => JobTab::Finished,
                            });
                            self.job_table_state.select(Some(0));
                        }
                    }
                    KeyCode::Down => {
                        if self.show_actions_popup && !self.show_confirmation_popup {
                            match self.action_target {
                                ActionTarget::Selected => {
                                    let i = self.selected_action_list_state.selected().unwrap_or(0);
                                    self.selected_action_list_state.select(Some((i + 1).min(3)));
                                }
                                ActionTarget::All => {
                                    let i = self.all_action_list_state.selected().unwrap_or(0);
                                    self.all_action_list_state.select(Some((i + 1).min(2)));
                                }
                            }
                        } else if !self.show_actions_popup && !self.show_confirmation_popup {
                            let jobs = self.get_filtered_jobs(current_tab);
                            if !jobs.is_empty() {
                                let i = self.job_table_state.selected().unwrap_or(0);
                                self.job_table_state.select(Some((i + 1).min(jobs.len() - 1)));
                            }
                        }
                    }
                    KeyCode::Up => {
                        if self.show_actions_popup && !self.show_confirmation_popup {
                            match self.action_target {
                                ActionTarget::Selected => {
                                    let i = self.selected_action_list_state.selected().unwrap_or(0);
                                    self.selected_action_list_state.select(Some(i.saturating_sub(1)));
                                }
                                ActionTarget::All => {
                                    let i = self.all_action_list_state.selected().unwrap_or(0);
                                    self.all_action_list_state.select(Some(i.saturating_sub(1)));
                                }
                            }
                        } else if !self.show_actions_popup && !self.show_confirmation_popup {
                            let i = self.job_table_state.selected().unwrap_or(0);
                            self.job_table_state.select(Some(i.saturating_sub(1)));
                        }
                    }
                    _ => {}
                }
            }
            AppMode::LogViewer | AppMode::ScriptViewer => match key {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.mode = AppMode::JobMonitoring(JobTab::Finished);
                }
                KeyCode::Down => {
                    if matches!(self.mode, AppMode::LogViewer) {
                        self.log_scroll = self.log_scroll.saturating_add(1);
                    } else {
                        self.script_scroll = self.script_scroll.saturating_add(1);
                    }
                }
                KeyCode::Up => {
                    if matches!(self.mode, AppMode::LogViewer) {
                        self.log_scroll = self.log_scroll.saturating_sub(1);
                    } else {
                        self.script_scroll = self.script_scroll.saturating_sub(1);
                    }
                }
                KeyCode::PageDown => {
                    if matches!(self.mode, AppMode::LogViewer) {
                        self.log_scroll = self.log_scroll.saturating_add(10);
                    } else {
                        self.script_scroll = self.script_scroll.saturating_add(10);
                    }
                }
                KeyCode::PageUp => {
                    if matches!(self.mode, AppMode::LogViewer) {
                        self.log_scroll = self.log_scroll.saturating_sub(10);
                    } else {
                        self.script_scroll = self.script_scroll.saturating_sub(10);
                    }
                }
                _ => {}
            },
            AppMode::Menu => match key {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.mode = AppMode::JobMonitoring(JobTab::Finished);
                }
                KeyCode::Down => {
                    let i = self.menu_state.selected().unwrap_or(0);
                    self.menu_state.select(Some((i + 1).min(2)));
                }
                KeyCode::Up => {
                    let i = self.menu_state.selected().unwrap_or(0);
                    self.menu_state.select(Some(i.saturating_sub(1)));
                }
                KeyCode::Enter => {
                    match self.menu_state.selected() {
                        Some(0) => self.mode = AppMode::JobMonitoring(JobTab::Finished),
                        Some(1) => self.mode = AppMode::ConfigMonitoring,
                        Some(2) => self.mode = AppMode::ArchiveMonitoring,
                        _ => {}
                    }
                }
                _ => {}
            },
            AppMode::ConfigMonitoring | AppMode::ArchiveMonitoring => match key {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.mode = AppMode::JobMonitoring(JobTab::Finished);
                }
                _ => {}
            },
            AppMode::ColumnConfig => match key {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.mode = AppMode::JobMonitoring(JobTab::Finished);
                }
                KeyCode::Down => {
                    let i = self.column_list_state.selected().unwrap_or(0);
                    self.column_list_state.select(Some((i + 1).min(self.column_config.columns.len())));
                }
                KeyCode::Up => {
                    let i = self.column_list_state.selected().unwrap_or(0);
                    self.column_list_state.select(Some(i.saturating_sub(1)));
                }
                _ => {}
            },
            AppMode::FilterConfig(section) => {
                let current_section = *section;
                match key {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        self.mode = AppMode::JobMonitoring(JobTab::Finished);
                    }
                    KeyCode::Tab => {
                        self.mode = AppMode::FilterConfig(match current_section {
                            FilterSection::Status => FilterSection::Config,
                            FilterSection::Config => FilterSection::Status,
                        });
                    }
                    KeyCode::Down => {
                        match current_section {
                            FilterSection::Status => {
                                let i = self.filter_status_list_state.selected().unwrap_or(0);
                                let max = Status::all_variants().len().saturating_sub(1);
                                self.filter_status_list_state.select(Some((i + 1).min(max)));
                            }
                            FilterSection::Config => {
                                let i = self.filter_config_list_state.selected().unwrap_or(0);
                                let max = self.configs.len().saturating_sub(1);
                                self.filter_config_list_state.select(Some((i + 1).min(max)));
                            }
                        }
                    }
                    KeyCode::Up => {
                        match current_section {
                            FilterSection::Status => {
                                let i = self.filter_status_list_state.selected().unwrap_or(0);
                                self.filter_status_list_state.select(Some(i.saturating_sub(1)));
                            }
                            FilterSection::Config => {
                                let i = self.filter_config_list_state.selected().unwrap_or(0);
                                self.filter_config_list_state.select(Some(i.saturating_sub(1)));
                            }
                        }
                    }
                    KeyCode::Char(' ') | KeyCode::Enter => {
                        match current_section {
                            FilterSection::Status => {
                                if let Some(i) = self.filter_status_list_state.selected() {
                                    let status = Status::all_variants()[i].clone();
                                    if self.job_filter.statuses.contains(&status) {
                                        self.job_filter.statuses.retain(|s| s != &status);
                                    } else {
                                        self.job_filter.statuses.push(status);
                                    }
                                }
                            }
                            FilterSection::Config => {
                                if let Some(i) = self.filter_config_list_state.selected() {
                                    let config_id = self.configs[i].id;
                                    if self.job_filter.config_ids.contains(&config_id) {
                                        self.job_filter.config_ids.retain(|id| id != &config_id);
                                    } else {
                                        self.job_filter.config_ids.push(config_id);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            },
        }
    }

    fn show_confirmation(&mut self, action_name: &str, tab: JobTab) {
        let message = match action_name {
            "Cancel Job" => {
                if let Some(job) = self.selected_job(tab) {
                    format!("Cancel job #{} '{}'?", job.id, job.job_name)
                } else {
                    return;
                }
            }
            "Archive Job" => {
                if let Some(job) = self.selected_job(tab) {
                    format!("Archive job #{} '{}'?", job.id, job.job_name)
                } else {
                    return;
                }
            }
            "Re-run Job" => {
                if let Some(job) = self.selected_job(tab) {
                    format!("Re-run job #{} '{}'?", job.id, job.job_name)
                } else {
                    return;
                }
            }
            "Delete Job" => {
                if let Some(job) = self.selected_job(tab) {
                    format!("⚠ DELETE job #{} '{}'? This cannot be undone!", job.id, job.job_name)
                } else {
                    return;
                }
            }
            "Cancel All Jobs" => {
                let count = self.get_filtered_jobs(tab).len();
                format!("Cancel {} visible job(s) in this view?", count)
            }
            "Archive All Jobs" => {
                let count = self.get_filtered_jobs(tab).len();
                format!("Archive {} visible job(s) in this view?", count)
            }
            "Delete All Jobs" => {
                let count = self.get_filtered_jobs(tab).len();
                format!("⚠ DELETE {} visible job(s)? This cannot be undone!", count)
            }
            _ => return,
        };

        self.confirmation_message = message;
        self.pending_action = Some((action_name.to_string(), self.action_target));
        self.show_confirmation_popup = true;
    }

    fn execute_action(&mut self, action_name: &str, target: ActionTarget, tab: JobTab) {
        // TODO: Implement actual job actions here
        match (action_name, target) {
            ("Cancel Job", ActionTarget::Selected) => {
                if let Some(job) = self.selected_job(tab) {
                    println!("Cancelling job #{}", job.id);
                    // Call actual cancel logic
                }
            }
            ("Archive Job", ActionTarget::Selected) => {
                if let Some(job) = self.selected_job(tab) {
                    println!("Archiving job #{}", job.id);
                    // Call actual archive logic
                }
            }
            ("Re-run Job", ActionTarget::Selected) => {
                if let Some(job) = self.selected_job(tab) {
                    println!("Re-running job #{}", job.id);
                    // Call actual re-run logic
                }
            }
            ("Delete Job", ActionTarget::Selected) => {
                if let Some(job) = self.selected_job(tab) {
                    println!("Deleting job #{}", job.id);
                    // Call actual delete logic
                }
            }
            ("Cancel All Jobs", ActionTarget::All) => {
                let jobs = self.get_filtered_jobs(tab);
                println!("Cancelling {} jobs", jobs.len());
                // Call actual cancel all logic
            }
            ("Archive All Jobs", ActionTarget::All) => {
                let jobs = self.get_filtered_jobs(tab);
                println!("Archiving {} jobs", jobs.len());
                // Call actual archive all logic
            }
            ("Delete All Jobs", ActionTarget::All) => {
                let jobs = self.get_filtered_jobs(tab);
                println!("Deleting {} jobs", jobs.len());
                // Call actual delete all logic
            }
            _ => {}
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        match &self.mode {
            AppMode::JobMonitoring(tab) => {
                self.draw_job_monitoring(f, *tab);
                if self.show_actions_popup {
                    self.draw_actions_popup(f);
                }
                if self.show_confirmation_popup {
                    self.draw_confirmation_popup(f);
                }
            }
            AppMode::LogViewer => self.draw_log_viewer(f),
            AppMode::ScriptViewer => self.draw_script_viewer(f),
            AppMode::Menu => self.draw_menu(f),
            AppMode::ConfigMonitoring => self.draw_config_monitoring(f),
            AppMode::ArchiveMonitoring => self.draw_archive_monitoring(f),
            AppMode::ColumnConfig => self.draw_column_config(f),
            AppMode::FilterConfig(section) => self.draw_filter_config(f, *section),
        }
    }

    fn draw_job_monitoring(&mut self, f: &mut Frame, tab: JobTab) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(2),
            ])
            .split(f.area());

        // Tabs (no border, cleaner look)
        let tab_titles = vec!["Finished", "Active", "Queued"];
        let tabs = Tabs::new(tab_titles)
            .select(match tab {
                JobTab::Finished => 0,
                JobTab::Active => 1,
                JobTab::Queued => 2,
            })
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .divider(" | ");
        f.render_widget(tabs, chunks[0]);

        // Job counts with border and filters info
        let (finished, active, queued, total) = self.get_job_counts();
        let mut counts_spans = vec![
            Span::styled("Finished: ", Style::default().fg(Color::Gray)),
            Span::styled(finished.to_string(), Style::default().fg(Color::Green)),
            Span::raw("  "),
            Span::styled("Active: ", Style::default().fg(Color::Gray)),
            Span::styled(active.to_string(), Style::default().fg(Color::Cyan)),
            Span::raw("  "),
            Span::styled("Queued: ", Style::default().fg(Color::Gray)),
            Span::styled(queued.to_string(), Style::default().fg(Color::Yellow)),
            Span::raw("  "),
            Span::styled("Total: ", Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD)),
            Span::styled(total.to_string(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ];

        // Add filter info if any filters are active
        if !self.job_filter.statuses.is_empty() || !self.job_filter.config_ids.is_empty() {
            counts_spans.push(Span::raw("  |  "));
            counts_spans.push(Span::styled("Filters: ", Style::default().fg(Color::Magenta)));
            
            if !self.job_filter.statuses.is_empty() {
                let status_str = self.job_filter.statuses.iter()
                    .map(|s| format!("{:?}", s))
                    .collect::<Vec<_>>()
                    .join(", ");
                counts_spans.push(Span::styled(status_str, Style::default().fg(Color::Yellow)));
            }
            
            if !self.job_filter.config_ids.is_empty() {
                if !self.job_filter.statuses.is_empty() {
                    counts_spans.push(Span::raw(" | "));
                }
                let config_str = format!("Configs: {}", self.job_filter.config_ids.iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(", "));
                counts_spans.push(Span::styled(config_str, Style::default().fg(Color::Cyan)));
            }
        }

        let counts_line = Line::from(counts_spans);
        let counts = Paragraph::new(counts_line)
            .block(Block::default().borders(Borders::ALL).title("Summary"))
            .alignment(Alignment::Left);
        f.render_widget(counts, chunks[1]);

        // Job table with tab name in title
        let tab_name = match tab {
            JobTab::Finished => "Finished",
            JobTab::Active => "Active",
            JobTab::Queued => "Queued",
        };
        
        let jobs = self.get_filtered_jobs(tab);
        let headers = self.column_config.columns.iter().map(|c| Cell::from(c.name()).style(Style::default().fg(Color::Yellow)));
        let header = Row::new(headers).style(Style::default().add_modifier(Modifier::BOLD)).height(1);

        let rows: Vec<Row> = jobs.iter().map(|job| {
            let cells: Vec<Cell> = self.column_config.columns.iter().map(|col| {
                match col {
                    ColumnType::Id => Cell::from(job.id.to_string()),
                    ColumnType::JobName => Cell::from(job.job_name.clone()),
                    ColumnType::ConfigId => Cell::from(job.config_id.to_string()),
                    ColumnType::Status => Cell::from(format!("{:?}", job.status)).style(Style::default().fg(job.status.color())),
                    ColumnType::SubmitTime => Cell::from(job.submit_time.map(|t| t.to_string()).unwrap_or_default()),
                    ColumnType::EndTime => Cell::from(job.end_time.map(|t| t.to_string()).unwrap_or_default()),
                    // FIXME ColumnType::ExitCode => Cell::from(job.exit_code.map(|c| c.to_string()).unwrap_or_default()),
                    ColumnType::JobId => Cell::from(job.job_id.clone().unwrap_or_default()),
                }
            }).collect();
            Row::new(cells).height(1)
        }).collect();

        let widths: Vec<Constraint> = self.column_config.columns.iter().map(|c| Constraint::Length(c.width())).collect();

        let table = Table::new(rows, widths)
            .header(header)
            .block(Block::default().borders(Borders::ALL).title(format!("Jobs - {}", tab_name)))
            .row_highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        f.render_stateful_widget(table, chunks[2], &mut self.job_table_state);

        // Help bar
        let help = Paragraph::new("q: Quit | Tab: Switch Tab | ↑↓: Navigate | Enter: Logs | s: Script | a: Actions | m: Menu | c: Columns | f: Filters")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        f.render_widget(help, chunks[3]);
    }

    fn draw_log_viewer(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(2)])
            .split(f.area());

        let log_text = self.current_log.as_deref().unwrap_or("No log available");
        let paragraph = Paragraph::new(log_text)
            .block(Block::default().borders(Borders::ALL).title("Log Viewer (stdout)"))
            .wrap(Wrap { trim: false })
            .scroll((self.log_scroll, 0));
        f.render_widget(paragraph, chunks[0]);

        let help = Paragraph::new("Esc/q: Back | ↑↓: Scroll | PgUp/PgDn: Page")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        f.render_widget(help, chunks[1]);
    }

    fn draw_script_viewer(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(2)])
            .split(f.area());

        let script_text = self.current_script.as_deref().unwrap_or("No script available");
        let paragraph = Paragraph::new(script_text)
            .block(Block::default().borders(Borders::ALL).title("Script Viewer"))
            .wrap(Wrap { trim: false })
            .scroll((self.script_scroll, 0));
        f.render_widget(paragraph, chunks[0]);

        let help = Paragraph::new("Esc/q: Back | ↑↓: Scroll | PgUp/PgDn: Page")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        f.render_widget(help, chunks[1]);
    }

    fn draw_menu(&mut self, f: &mut Frame) {
        let area = centered_rect(60, 50, f.area());
        let items = vec![
            ListItem::new("Job Monitoring"),
            ListItem::new("Configuration Monitoring"),
            ListItem::new("Archive Monitoring"),
        ];
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Main Menu"))
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");
        f.render_stateful_widget(list, area, &mut self.menu_state);
    }

    fn draw_config_monitoring(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(2)])
            .split(f.area());

        let rows: Vec<Row> = self.configs.iter().map(|cfg| {
            Row::new(vec![
                Cell::from(cfg.id.to_string()),
                Cell::from(cfg.config_name.clone()),
                Cell::from(cfg.cluster_id.to_string()),
            ])
        }).collect();

        let table = Table::new(
            rows,
            [Constraint::Length(8), Constraint::Length(30), Constraint::Length(12)],
        )
        .header(Row::new(vec!["ID", "Name", "Cluster ID"]).style(Style::default().add_modifier(Modifier::BOLD)))
        .block(Block::default().borders(Borders::ALL).title("Configurations"));

        f.render_widget(table, chunks[0]);

        let help = Paragraph::new("Esc/q: Back")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        f.render_widget(help, chunks[1]);
    }

    fn draw_archive_monitoring(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(2)])
            .split(f.area());

        let archived_jobs: Vec<&Job> = self.jobs.iter().filter(|j| j.archived.is_some()).collect();
        let text = if archived_jobs.is_empty() {
            "No archived jobs"
        } else {
            "Archive management interface (rename, delete, merge, restore)"
        };

        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Archive Monitoring"))
            .alignment(Alignment::Center);
        f.render_widget(paragraph, chunks[0]);

        let help = Paragraph::new("Esc/q: Back")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        f.render_widget(help, chunks[1]);
    }

    fn draw_column_config(&mut self, f: &mut Frame) {
        let area = centered_rect(60, 60, f.area());
        let items: Vec<ListItem> = self.column_config.columns.iter()
            .map(|col| ListItem::new(col.name()))
            .collect();
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Column Configuration"))
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");
        f.render_stateful_widget(list, area, &mut self.column_list_state);
    }

    fn draw_filter_config(&mut self, f: &mut Frame, section: FilterSection) {
        let area = centered_rect(80, 70, f.area());
        
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Status filter list
        let status_items: Vec<ListItem> = Status::all_variants().iter().map(|status| {
            let checked = if self.job_filter.statuses.contains(status) { "[x]" } else { "[ ]" };
            let text = format!("{} {:?}", checked, status);
            ListItem::new(text).style(Style::default().fg(status.color()))
        }).collect();

        let status_list = List::new(status_items)
            .block(Block::default()
                .borders(Borders::ALL)
                .title("Status Filter")
                .border_style(if matches!(section, FilterSection::Status) {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                }))
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        // Config filter list
        let config_items: Vec<ListItem> = self.configs.iter().map(|config| {
            let checked = if self.job_filter.config_ids.contains(&config.id) { "[x]" } else { "[ ]" };
            let text = format!("{} {} (ID: {})", checked, config.config_name, config.id);
            ListItem::new(text)
        }).collect();

        let config_list = List::new(config_items)
            .block(Block::default()
                .borders(Borders::ALL)
                .title("Config Filter")
                .border_style(if matches!(section, FilterSection::Config) {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                }))
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        // Render based on active section
        if matches!(section, FilterSection::Status) {
            f.render_stateful_widget(status_list, chunks[0], &mut self.filter_status_list_state);
            f.render_widget(config_list, chunks[1]);
        } else {
            f.render_widget(status_list, chunks[0]);
            f.render_stateful_widget(config_list, chunks[1], &mut self.filter_config_list_state);
        }

        // Help text at the bottom
        let help_area = Rect {
            x: area.x,
            y: area.y + area.height,
            width: area.width,
            height: 2,
        };
        let help = Paragraph::new("Esc: Back | Tab: Switch Section | ↑↓: Navigate | Space/Enter: Toggle | Active filters apply immediately")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        f.render_widget(help, help_area);
    }

    fn draw_actions_popup(&mut self, f: &mut Frame) {
        let area = centered_rect(70, 50, f.area());
        
        // First, render a filled block to completely cover the area below
        let clear_block = Block::default()
            .style(Style::default().bg(Color::Black));
        f.render_widget(clear_block.clone(), area);
        
        // Main border
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title("Job Actions")
            .style(Style::default().bg(Color::Black));
        f.render_widget(block, area);

        let inner_area = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width - 2,
            height: area.height - 2,
        };

        // Clear the inner area too
        f.render_widget(clear_block, inner_area);

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inner_area);

        // Selected job actions
        let selected_items: Vec<ListItem> = vec![
            ListItem::new("Cancel Job").style(Style::default().bg(Color::Black)),
            ListItem::new("Archive Job").style(Style::default().bg(Color::Black)),
            ListItem::new("Re-run Job").style(Style::default().bg(Color::Black)),
            ListItem::new("Delete Job").style(Style::default().bg(Color::Black)),
        ];
        let selected_list = List::new(selected_items)
            .block(Block::default()
                .borders(Borders::ALL)
                .title("Selected Job")
                .border_style(if matches!(self.action_target, ActionTarget::Selected) {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::Gray)
                })
                .style(Style::default().bg(Color::Black)))
            .style(Style::default().bg(Color::Black))
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        // All jobs actions
        let all_items: Vec<ListItem> = vec![
            ListItem::new("Cancel All Jobs").style(Style::default().bg(Color::Black)),
            ListItem::new("Archive All Jobs").style(Style::default().bg(Color::Black)),
            ListItem::new("Delete All Jobs").style(Style::default().bg(Color::Black)),
        ];
        let all_list = List::new(all_items)
            .block(Block::default()
                .borders(Borders::ALL)
                .title("All Visible Jobs")
                .border_style(if matches!(self.action_target, ActionTarget::All) {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::Gray)
                })
                .style(Style::default().bg(Color::Black)))
            .style(Style::default().bg(Color::Black))
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        if matches!(self.action_target, ActionTarget::Selected) {
            f.render_stateful_widget(selected_list, chunks[0], &mut self.selected_action_list_state);
            f.render_widget(all_list, chunks[1]);
        } else {
            f.render_widget(selected_list, chunks[0]);
            f.render_stateful_widget(all_list, chunks[1], &mut self.all_action_list_state);
        }

        // Help text
        let help_area = Rect {
            x: area.x,
            y: area.y + area.height,
            width: area.width,
            height: 1,
        };
        let help = Paragraph::new("Tab: Switch Column | ↑↓: Navigate | Enter: Select | Esc/q: Cancel")
            .style(Style::default().fg(Color::Gray).bg(Color::Black))
            .alignment(Alignment::Center);
        f.render_widget(help, help_area);
    }

    fn draw_confirmation_popup(&mut self, f: &mut Frame) {
        let area = centered_rect(60, 30, f.area());
        
        // First, render a filled block to completely cover the area below
        let clear_block = Block::default()
            .style(Style::default().bg(Color::Black));
        f.render_widget(clear_block.clone(), area);
        
        // Main border
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red))
            .title("Confirm Action")
            .style(Style::default().bg(Color::Black));
        f.render_widget(block, area);

        let inner_area = Rect {
            x: area.x + 2,
            y: area.y + 2,
            width: area.width - 4,
            height: area.height - 4,
        };

        // Clear the inner area too
        f.render_widget(clear_block, inner_area);

        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                &self.confirmation_message,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD).bg(Color::Black)
            )),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "Press Enter to confirm, Esc/q to cancel",
                Style::default().fg(Color::Gray).bg(Color::Black)
            )),
        ];

        let paragraph = Paragraph::new(text)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .style(Style::default().bg(Color::Black));
        f.render_widget(paragraph, inner_area);
    }

    fn draw_job_actions(&mut self, f: &mut Frame) {
        let area = centered_rect(40, 40, f.area());
        let items = vec![
            ListItem::new("Cancel Job"),
            ListItem::new("Archive Job"),
            ListItem::new("Re-run Job"),
            ListItem::new("Delete Job"),
        ];
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Job Actions"))
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");
        f.render_stateful_widget(list, area, &mut self.all_action_list_state);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn launch_tui() -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (jobs, configs, clusters) = generate_sample_data();
    let mut app = App::new(jobs, configs, clusters);
    let res = app.run(&mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}