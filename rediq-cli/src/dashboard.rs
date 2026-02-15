//! Real-time terminal dashboard for Rediq monitoring

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Gauge, Paragraph, Row, Sparkline, Table, Wrap},
    Frame, Terminal,
};
use rediq::client::{Client, TaskInfo};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use chrono::{Local, TimeZone};

// ============================================================================
// Data Structures
// ============================================================================

/// Dashboard view modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    /// Main dashboard view
    Main,
    /// Task list for selected queue
    TaskList,
    /// Help overlay
    Help,
}

/// Task list panel state
struct TaskListState {
    /// Current task list
    tasks: Vec<TaskInfo>,
    /// Current page (0-indexed)
    page: usize,
    /// Tasks per page
    page_size: usize,
    /// Selected task index
    selected_task: usize,
    /// Queue name for current task list
    queue_name: String,
    /// Total available tasks
    total_available: usize,
}

/// Alert severity
#[derive(Debug, Clone, PartialEq, Eq)]
enum AlertSeverity {
    Warning,
    Critical,
}

/// Individual alert
struct Alert {
    /// Alert message
    #[allow(dead_code)]
    message: String,
    /// Alert severity
    severity: AlertSeverity,
    /// Whether alert is active
    active: bool,
}

/// Alert thresholds and state
struct AlertState {
    /// Dead letter queue threshold
    dead_threshold: u64,
    /// Error rate threshold (percentage)
    error_rate_threshold: f64,
    /// Current alerts
    alerts: Vec<Alert>,
}

/// Status type for operation messages
#[derive(Debug, Clone)]
enum StatusType {
    Success,
    Error,
    #[allow(dead_code)]
    Info,
    Warning,
}

/// Queue operation status message
struct OperationStatus {
    /// Status message
    message: String,
    /// Status type
    status_type: StatusType,
    /// Timestamp when status was set
    timestamp: Instant,
}

// ============================================================================
// History Trend Data Structures
// ============================================================================

/// A single data point in history
#[derive(Clone, Copy, Debug)]
struct HistoryPoint {
    /// Value at this point
    value: u64,
    /// Timestamp when this point was recorded
    timestamp: Instant,
}

/// History data for trend visualization
#[derive(Default)]
struct HistoryData {
    /// Pending task count history
    pending: VecDeque<HistoryPoint>,
    /// Active task count history
    active: VecDeque<HistoryPoint>,
    /// Completed task count history
    completed: VecDeque<HistoryPoint>,
    /// Dead letter queue history
    dead: VecDeque<HistoryPoint>,
    /// Error rate history (stored as integer: percentage * 10)
    error_rate: VecDeque<HistoryPoint>,
    /// Maximum history points to keep
    max_points: usize,
}

impl HistoryData {
    /// Create new history data with specified max points
    fn new(max_points: usize) -> Self {
        Self {
            max_points,
            ..Default::default()
        }
    }

    /// Add a new data point, trimming if necessary
    fn add_point(&mut self, metric: &str, value: u64) {
        let point = HistoryPoint {
            value,
            timestamp: Instant::now(),
        };

        let queue = match metric {
            "pending" => &mut self.pending,
            "active" => &mut self.active,
            "completed" => &mut self.completed,
            "dead" => &mut self.dead,
            "error_rate" => &mut self.error_rate,
            _ => return,
        };

        queue.push_back(point);
        if queue.len() > self.max_points {
            queue.pop_front();
        }
    }

    /// Get values as a vector for sparkline
    fn get_values(&self, metric: &str) -> Vec<u64> {
        match metric {
            "pending" => self.pending.iter().map(|p| p.value).collect(),
            "active" => self.active.iter().map(|p| p.value).collect(),
            "completed" => self.completed.iter().map(|p| p.value).collect(),
            "dead" => self.dead.iter().map(|p| p.value).collect(),
            "error_rate" => self.error_rate.iter().map(|p| p.value).collect(),
            _ => Vec::new(),
        }
    }

    /// Get the age of the oldest point in seconds
    fn oldest_age_secs(&self, metric: &str) -> u64 {
        let queue = match metric {
            "pending" => &self.pending,
            "active" => &self.active,
            "completed" => &self.completed,
            "dead" => &self.dead,
            "error_rate" => &self.error_rate,
            _ => return 0,
        };

        queue.front()
            .map(|p| p.timestamp.elapsed().as_secs())
            .unwrap_or(0)
    }

    /// Check if history has any data
    #[allow(dead_code)]
    fn has_data(&self) -> bool {
        !self.pending.is_empty()
            || !self.active.is_empty()
            || !self.completed.is_empty()
            || !self.dead.is_empty()
    }
}

pub struct DashboardState {
    /// Current view mode
    view_mode: ViewMode,

    /// Queue statistics
    queues: Vec<QueueStats>,
    /// Worker information
    workers: Vec<WorkerInfo>,
    /// Selected queue index
    selected_queue: usize,

    /// Task list state (when in TaskList view)
    task_list: Option<TaskListState>,

    /// Alert state
    alerts: AlertState,

    /// Operation status message
    operation_status: Option<OperationStatus>,

    /// Help overlay visibility
    show_help: bool,

    /// Last data update timestamp
    last_update: Instant,
    /// Total processed tasks
    total_processed: u64,
    /// Operations per second
    ops_per_sec: f64,

    /// Historical trend data
    history: HistoryData,

    /// Client for queue operations (stored as reference to avoid lifetime issues)
    _client: PhantomData<Client>,
}

use std::marker::PhantomData;

#[derive(Clone)]
struct QueueStats {
    name: String,
    pending: u64,
    active: u64,
    delayed: u64,
    retry: u64,
    dead: u64,
    completed: u64,
}

struct WorkerInfo {
    id: String,
    server: String,
    #[allow(dead_code)]
    queues: Vec<String>,
    status: String,
    processed: u64,
}

// ============================================================================
// Public API
// ============================================================================

pub async fn run(_redis_url: &str, refresh_interval_ms: u64, client: &Client) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let inspector = client.inspector();

    let mut state = DashboardState {
        view_mode: ViewMode::Main,
        queues: Vec::new(),
        workers: Vec::new(),
        selected_queue: 0,
        task_list: None,
        alerts: AlertState {
            dead_threshold: 100,
            error_rate_threshold: 20.0,
            alerts: Vec::new(),
        },
        operation_status: None,
        show_help: false,
        last_update: Instant::now(),
        total_processed: 0,
        ops_per_sec: 0.0,
        history: HistoryData::new(60), // Keep 60 data points (5 min at 500ms refresh)
        _client: PhantomData,
    };

    // Initial data fetch
    refresh_data(&inspector, &mut state).await;

    let refresh_duration = Duration::from_millis(refresh_interval_ms);
    let mut last_refresh = Instant::now();

    let result = run_dashboard(
        &mut terminal,
        &inspector,
        client,
        &mut state,
        refresh_duration,
        &mut last_refresh,
    )
    .await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

// ============================================================================
// Main Dashboard Loop
// ============================================================================

async fn run_dashboard(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    inspector: &rediq::client::Inspector,
    client: &Client,
    state: &mut DashboardState,
    refresh_duration: Duration,
    last_refresh: &mut Instant,
) -> Result<()> {
    loop {
        // Draw dashboard
        terminal.draw(|f| ui(f, state))?;

        // Handle input with timeout
        let timeout = refresh_duration
            .saturating_sub(last_refresh.elapsed())
            .max(Duration::from_millis(50));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                // Handle global quit keys first
                if key.code == KeyCode::Char('q') && state.view_mode == ViewMode::Main {
                    return Ok(());
                }

                // Handle input based on current view mode
                match state.view_mode {
                    ViewMode::Main => {
                        handle_main_input(state, key.code, client).await?;
                    }
                    ViewMode::TaskList => {
                        handle_task_list_input(state, key.code, inspector).await?;
                    }
                    ViewMode::Help => {
                        handle_help_input(state);
                    }
                }
            }
        }

        // Refresh data at interval
        if last_refresh.elapsed() >= refresh_duration {
            refresh_data(inspector, state).await;
            check_and_update_alerts(state);
            *last_refresh = Instant::now();

            // Clear expired operation status messages (after 3 seconds)
            if let Some(ref status) = state.operation_status {
                if status.timestamp.elapsed() > Duration::from_secs(3) {
                    state.operation_status = None;
                }
            }
        }
    }
}

// ============================================================================
// Input Handling
// ============================================================================

async fn handle_main_input(
    state: &mut DashboardState,
    key: KeyCode,
    client: &Client,
) -> Result<()> {
    match key {
        // Navigation
        KeyCode::Up | KeyCode::Left => {
            if state.selected_queue > 0 {
                state.selected_queue -= 1;
            }
        }
        KeyCode::Down | KeyCode::Right => {
            if !state.queues.is_empty() && state.selected_queue < state.queues.len() - 1 {
                state.selected_queue += 1;
            }
        }

        // Enter task list view
        KeyCode::Enter => {
            enter_task_list_view(state, client).await?;
        }

        // Queue operations
        KeyCode::Char('p') => {
            pause_selected_queue(state, client).await?;
        }
        KeyCode::Char('r') => {
            resume_selected_queue(state, client).await?;
        }
        KeyCode::Char('f') => {
            flush_selected_queue(state, client).await?;
        }

        // Help
        KeyCode::Char('?') => {
            state.show_help = true;
            state.view_mode = ViewMode::Help;
        }

        // Clear status with Esc
        KeyCode::Esc => {
            state.operation_status = None;
        }

        _ => {}
    }
    Ok(())
}

async fn handle_task_list_input(
    state: &mut DashboardState,
    key: KeyCode,
    inspector: &rediq::client::Inspector,
) -> Result<()> {
    if let Some(ref mut task_list) = state.task_list {
        match key {
            KeyCode::Esc => {
                // Return to main view
                state.view_mode = ViewMode::Main;
                state.task_list = None;
            }
            KeyCode::Up => {
                if task_list.selected_task > 0 {
                    task_list.selected_task -= 1;
                } else if task_list.page > 0 {
                    // Go to previous page
                    task_list.page -= 1;
                    task_list.selected_task = task_list.page_size - 1;
                    refresh_task_list(state, inspector).await;
                }
            }
            KeyCode::Down => {
                let max_idx = task_list.tasks.len().saturating_sub(1);
                if task_list.selected_task < max_idx {
                    task_list.selected_task += 1;
                } else if (task_list.selected_task + 1) % task_list.page_size == 0 && task_list.selected_task + 1 < task_list.total_available {
                    // Go to next page
                    task_list.page += 1;
                    task_list.selected_task = 0;
                    refresh_task_list(state, inspector).await;
                }
            }
            KeyCode::Char('r') => {
                // Refresh task list
                refresh_task_list(state, inspector).await;
            }
            _ => {}
        }
    }
    Ok(())
}

fn handle_help_input(state: &mut DashboardState) {
    if state.view_mode == ViewMode::Help {
        // Close help on Esc, q, or ?
        state.show_help = false;
        state.view_mode = ViewMode::Main;
    }
}

// ============================================================================
// Queue Operations
// ============================================================================

async fn enter_task_list_view(state: &mut DashboardState, client: &Client) -> Result<()> {
    if state.queues.is_empty() {
        set_operation_status(
            state,
            "No queues available".to_string(),
            StatusType::Warning,
        );
        return Ok(());
    }

    let queue_name = state.queues[state.selected_queue].name.clone();
    let queue_name_for_clone = queue_name.clone();
    let inspector = client.inspector();

    // Get task count first
    let total_available = match inspector.list_tasks(&queue_name, 1).await {
        Ok(_) => {
            // We'll get the actual count in refresh_task_list
            100 // Default, will be updated
        }
        Err(_) => {
            set_operation_status(
                state,
                format!("Failed to list tasks for queue '{}'", queue_name),
                StatusType::Error,
            );
            return Ok(());
        }
    };

    let page_size = 20;

    let task_list = TaskListState {
        tasks: Vec::new(),
        page: 0,
        page_size,
        selected_task: 0,
        queue_name: queue_name_for_clone,
        total_available,
    };

    state.task_list = Some(task_list);
    state.view_mode = ViewMode::TaskList;

    // Fetch initial task list
    if let Some(ref tl) = state.task_list {
        let inspector = client.inspector();
        match inspector.list_tasks(&tl.queue_name, tl.page_size).await {
            Ok(tasks) => {
                if let Some(ref mut task_list) = state.task_list {
                    let len = tasks.len();
                    task_list.total_available = len;
                    task_list.tasks = tasks;
                    // Update selected_task if needed
                    if task_list.selected_task >= len && len > 0 {
                        task_list.selected_task = len - 1;
                    }
                }
            }
            Err(_) => {
                set_operation_status(
                    state,
                    format!("Failed to load tasks for queue '{}'", queue_name),
                    StatusType::Error,
                );
            }
        }
    }

    Ok(())
}

async fn pause_selected_queue(state: &mut DashboardState, client: &Client) -> Result<()> {
    if state.queues.is_empty() {
        set_operation_status(
            state,
            "No queues available".to_string(),
            StatusType::Warning,
        );
        return Ok(());
    }

    let queue_name = &state.queues[state.selected_queue].name;

    match client.pause_queue(queue_name).await {
        Ok(_) => {
            set_operation_status(
                state,
                format!("Queue '{}' paused", queue_name),
                StatusType::Success,
            );
        }
        Err(e) => {
            set_operation_status(
                state,
                format!("Failed to pause queue: {}", e),
                StatusType::Error,
            );
        }
    }

    Ok(())
}

async fn resume_selected_queue(state: &mut DashboardState, client: &Client) -> Result<()> {
    if state.queues.is_empty() {
        set_operation_status(
            state,
            "No queues available".to_string(),
            StatusType::Warning,
        );
        return Ok(());
    }

    let queue_name = &state.queues[state.selected_queue].name;

    match client.resume_queue(queue_name).await {
        Ok(_) => {
            set_operation_status(
                state,
                format!("Queue '{}' resumed", queue_name),
                StatusType::Success,
            );
        }
        Err(e) => {
            set_operation_status(
                state,
                format!("Failed to resume queue: {}", e),
                StatusType::Error,
            );
        }
    }

    Ok(())
}

async fn flush_selected_queue(state: &mut DashboardState, client: &Client) -> Result<()> {
    if state.queues.is_empty() {
        set_operation_status(
            state,
            "No queues available".to_string(),
            StatusType::Warning,
        );
        return Ok(());
    }

    let queue_name = &state.queues[state.selected_queue].name.clone();

    match client.flush_queue(queue_name).await {
        Ok(count) => {
            set_operation_status(
                state,
                format!("Queue '{}' flushed ({} tasks removed)", queue_name, count),
                StatusType::Success,
            );
        }
        Err(e) => {
            set_operation_status(
                state,
                format!("Failed to flush queue: {}", e),
                StatusType::Error,
            );
        }
    }

    Ok(())
}

fn set_operation_status(state: &mut DashboardState, message: String, status_type: StatusType) {
    state.operation_status = Some(OperationStatus {
        message,
        status_type,
        timestamp: Instant::now(),
    });
}

// ============================================================================
// Data Refresh
// ============================================================================

async fn refresh_data(inspector: &rediq::client::Inspector, state: &mut DashboardState) {
    let now = Instant::now();

    // Fetch queues and workers concurrently
    let (queues_result, workers_result) = tokio::join!(
        async {
            if let Ok(queues) = inspector.list_queues().await {
                // Fetch all queue stats concurrently
                let queue_futures: Vec<_> = queues
                    .iter()
                    .map(|q| inspector.queue_stats(q))
                    .collect();

                let results = futures::future::join_all(queue_futures).await;

                let mut new_queues = Vec::new();
                for (i, stats_result) in results.into_iter().enumerate() {
                    if let Ok(stats) = stats_result {
                        new_queues.push(QueueStats {
                            name: queues[i].clone(),
                            pending: stats.pending,
                            active: stats.active,
                            delayed: stats.delayed,
                            retry: stats.retried,
                            dead: stats.dead,
                            completed: stats.completed,
                        });
                    }
                }
                Some(new_queues)
            } else {
                None
            }
        },
        async {
            inspector.list_workers().await.ok()
        }
    );

    // Update queues
    if let Some(new_queues) = queues_result {
        state.queues = new_queues;

        // Validate selected_queue index
        if !state.queues.is_empty() && state.selected_queue >= state.queues.len() {
            state.selected_queue = state.queues.len() - 1;
        }
    }

    // Update workers
    if let Some(workers) = workers_result {
        let mut new_workers = Vec::new();
        let mut total = 0;
        for worker in workers {
            total += worker.processed_total;
            new_workers.push(WorkerInfo {
                id: worker.id,
                server: worker.server_name,
                queues: worker.queues,
                status: worker.status,
                processed: worker.processed_total,
            });
        }
        state.workers = new_workers;

        // Calculate ops/sec
        let elapsed = now.duration_since(state.last_update).as_secs_f64();
        if elapsed > 0.0 {
            let delta = total.saturating_sub(state.total_processed);
            state.ops_per_sec = delta as f64 / elapsed;
        }
        state.total_processed = total;
    }

    state.last_update = now;

    // Record history data points
    if !state.queues.is_empty() {
        // Aggregate data across all queues for trends
        let total_pending: u64 = state.queues.iter().map(|q| q.pending).sum();
        let total_active: u64 = state.queues.iter().map(|q| q.active).sum();
        let total_completed: u64 = state.queues.iter().map(|q| q.completed).sum();
        let total_dead: u64 = state.queues.iter().map(|q| q.dead).sum();

        state.history.add_point("pending", total_pending);
        state.history.add_point("active", total_active);
        state.history.add_point("completed", total_completed);
        state.history.add_point("dead", total_dead);

        // Calculate and record error rate (as integer: percentage * 10)
        let total_retry: u64 = state.queues.iter().map(|q| q.retry).sum();
        let total_failures = total_dead + total_retry;
        let total_outcomes = total_completed + total_failures;
        let error_rate = if total_outcomes > 0 {
            ((total_failures as f64 / total_outcomes as f64) * 1000.0) as u64
        } else {
            0
        };
        state.history.add_point("error_rate", error_rate);
    }
}

async fn refresh_task_list(state: &mut DashboardState, inspector: &rediq::client::Inspector) {
    if let Some(ref mut task_list) = state.task_list {
        match inspector.list_tasks(&task_list.queue_name, task_list.page_size).await {
            Ok(mut tasks) => {
                // Fetch progress for each task concurrently
                let progress_futures: Vec<_> = tasks.iter()
                    .map(|task| inspector.get_task_progress(&task.id))
                    .collect();

                let results = futures::future::join_all(progress_futures).await;

                // Attach progress to tasks
                for (task, progress_result) in tasks.iter_mut().zip(results) {
                    if let Ok(progress) = progress_result {
                        task.progress = progress;
                    }
                }

                let len = tasks.len();
                task_list.total_available = len;
                task_list.tasks = tasks;
                // Update selected_task if needed
                if task_list.selected_task >= len && len > 0 {
                    task_list.selected_task = len - 1;
                }
            }
            Err(_) => {
                // Keep existing tasks on error
            }
        }
    }
}

// ============================================================================
// Alert System
// ============================================================================

fn check_and_update_alerts(state: &mut DashboardState) {
    let mut alerts = Vec::new();

    // Check dead letter queue threshold
    let total_dead: u64 = state.queues.iter().map(|q| q.dead).sum();
    if total_dead > state.alerts.dead_threshold {
        alerts.push(Alert {
            message: format!(
                "Dead letter queue: {} tasks (threshold: {})",
                total_dead, state.alerts.dead_threshold
            ),
            severity: AlertSeverity::Critical,
            active: true,
        });
    }

    // Check error rate threshold
    let error_rate = calculate_error_rate(state);
    if error_rate > state.alerts.error_rate_threshold {
        alerts.push(Alert {
            message: format!(
                "Error rate: {:.1}% (threshold: {:.1}%)",
                error_rate, state.alerts.error_rate_threshold
            ),
            severity: AlertSeverity::Warning,
            active: true,
        });
    }

    state.alerts.alerts = alerts;
}

fn calculate_error_rate(state: &DashboardState) -> f64 {
    let total_dead: u64 = state.queues.iter().map(|q| q.dead).sum();
    let total_retry: u64 = state.queues.iter().map(|q| q.retry).sum();
    let total_completed: u64 = state.queues.iter().map(|q| q.completed).sum();

    let total_failures = total_dead + total_retry;
    let total_outcomes = total_completed + total_failures;

    if total_outcomes == 0 {
        0.0
    } else {
        (total_failures as f64 / total_outcomes as f64) * 100.0
    }
}

fn error_rate_color(error_rate: f64) -> Color {
    if error_rate < 5.0 {
        Color::Green
    } else if error_rate < 20.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

// ============================================================================
// UI Rendering
// ============================================================================

fn ui(f: &mut Frame, state: &DashboardState) {
    let area = f.area();

    // Draw alert borders if there are active alerts
    for alert in &state.alerts.alerts {
        if alert.active {
            let border_style = match alert.severity {
                AlertSeverity::Warning => Style::default().fg(Color::Yellow),
                AlertSeverity::Critical => Style::default().fg(Color::Red),
            };

            // Flash effect - toggle visibility based on time
            let should_show = (Local::now().timestamp_millis() / 500) % 2 == 0;

            if should_show {
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style);
                f.render_widget(block, area);
            }
        }
    }

    // Draw based on view mode
    match state.view_mode {
        ViewMode::Main => {
            draw_main_view(f, state, area);
        }
        ViewMode::TaskList => {
            draw_main_view(f, state, area);
            draw_task_list_overlay(f, state, area);
        }
        ViewMode::Help => {
            draw_main_view(f, state, area);
            draw_help_overlay(f, area);
        }
    }

    // Draw operation status message if present
    if let Some(ref status) = state.operation_status {
        draw_status_message(f, status, area);
    }
}

fn draw_main_view(f: &mut Frame, state: &DashboardState, area: Rect) {
    // Title bar with timestamp and total processed
    let time_str = Local::now().format("%H:%M:%S").to_string();
    let elapsed = state.last_update.elapsed().as_millis();
    let ago_str = if elapsed < 1000 {
        format!("{}ms ago", elapsed)
    } else {
        format!("{}s ago", elapsed / 1000)
    };

    let help_hint = " ?:Help ";
    let title = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Rediq", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" Dashboard "),
            Span::styled(format!("Updated: {} ({})", time_str, ago_str), Style::default().fg(Color::DarkGray)),
            Span::raw(" "),
            Span::styled(format!("Total Processed: {}", state.total_processed), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("↑↓:Select Enter:Tasks p:Pause r:Resume f:Flush", Style::default().fg(Color::DarkGray)),
            Span::raw(" "),
            Span::styled(help_hint, Style::default().fg(Color::Cyan)),
            Span::styled("q:Quit", Style::default().fg(Color::DarkGray)),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, Rect { x: 0, y: 0, width: area.width, height: 4 });

    // Main layout - title bar + content (queues/workers/stats + history)
    let remaining_height = area.height - 4;
    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(remaining_height - 14), // Main content
            Constraint::Length(13),                     // History trends
        ])
        .split(Rect { x: 0, y: 4, width: area.width, height: remaining_height });

    // Main content section - split vertically
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(2, 3), // Upper: queues + workers
            Constraint::Ratio(1, 3), // Lower: stats
        ])
        .split(content_chunks[0]);

    // Upper section - queues and workers side by side
    let upper_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(main_chunks[0]);

    draw_queues_panel(f, state, upper_chunks[0]);
    draw_workers_panel(f, state, upper_chunks[1]);

    // Stats panel
    draw_stats_panel(f, state, main_chunks[1]);

    // History trends section
    draw_history_trends(f, state, content_chunks[1]);
}

fn draw_queues_panel(f: &mut Frame, state: &DashboardState, area: Rect) {
    let title = if !state.queues.is_empty() {
        format!("Queues ({})", state.queues.len())
    } else {
        "Queues".to_string()
    };

    let rows: Vec<Row> = state
        .queues
        .iter()
        .enumerate()
        .map(|(i, q)| {
            let style = if i == state.selected_queue {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(q.name.clone()),
                Cell::from(q.pending.to_string()).style(style_for_count(q.pending)),
                Cell::from(q.active.to_string()).style(Style::default().fg(Color::Yellow)),
                Cell::from(q.delayed.to_string()).style(Style::default().fg(Color::Cyan)),
                Cell::from(q.retry.to_string()).style(Style::default().fg(Color::Magenta)),
                Cell::from(q.dead.to_string()).style(Style::default().fg(Color::Red)),
                Cell::from(q.completed.to_string()).style(Style::default().fg(Color::Green)),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        &[
            Constraint::Length(15),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(8),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["Queue", "Pending", "Active", "Delayed", "Retry", "Dead", "Completed"])
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    )
    .block(Block::default().title(title).borders(Borders::ALL))
    .widths([
        Constraint::Percentage(23),
        Constraint::Percentage(13),
        Constraint::Percentage(13),
        Constraint::Percentage(14),
        Constraint::Percentage(13),
        Constraint::Percentage(13),
        Constraint::Percentage(11),
    ]);

    f.render_widget(table, area);
}

fn draw_workers_panel(f: &mut Frame, state: &DashboardState, area: Rect) {
    let title = if !state.workers.is_empty() {
        format!("Workers ({})", state.workers.len())
    } else {
        "Workers".to_string()
    };

    if state.workers.is_empty() {
        let paragraph = Paragraph::new("No workers registered")
            .block(Block::default().title(title).borders(Borders::ALL))
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center);
        f.render_widget(paragraph, area);
        return;
    }

    let rows: Vec<Row> = state
        .workers
        .iter()
        .take(area.height.saturating_sub(3) as usize)
        .map(|w| {
            Row::new(vec![
                Cell::from(shorten_id(&w.id, 20)),
                Cell::from(shorten_string(&w.server, 15)),
                Cell::from(w.status.clone()),
                Cell::from(w.processed.to_string()),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        &[
            Constraint::Max(25),
            Constraint::Max(20),
            Constraint::Max(10),
            Constraint::Max(10),
        ],
    )
    .header(
        Row::new(vec!["Worker", "Server", "Status", "Processed"])
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    )
    .block(Block::default().title(title).borders(Borders::ALL))
    .widths([
        Constraint::Percentage(30),
        Constraint::Percentage(30),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
    ]);

    f.render_widget(table, area);
}

fn draw_stats_panel(f: &mut Frame, state: &DashboardState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
        .split(area);

    // Left side - metrics
    let error_rate = calculate_error_rate(state);
    let metrics = if !state.queues.is_empty() {
        let total_pending: u64 = state.queues.iter().map(|q| q.pending).sum();
        let total_active: u64 = state.queues.iter().map(|q| q.active).sum();
        let total_delayed: u64 = state.queues.iter().map(|q| q.delayed).sum();
        let total_retry: u64 = state.queues.iter().map(|q| q.retry).sum();
        let total_dead: u64 = state.queues.iter().map(|q| q.dead).sum();
        let total_completed: u64 = state.queues.iter().map(|q| q.completed).sum();
        let total_in_queues: u64 = total_pending + total_active + total_delayed + total_retry;

        vec![
            Line::from(vec![
                Span::styled("In Queues:      ", Style::default().fg(Color::White)),
                Span::styled(total_in_queues.to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("├ Pending:     ", Style::default().fg(Color::White)),
                Span::styled(total_pending.to_string(), Style::default().fg(Color::Green)),
                Span::raw("  "),
                Span::styled("Active: ", Style::default().fg(Color::White)),
                Span::styled(total_active.to_string(), Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::styled("├ Delayed:     ", Style::default().fg(Color::White)),
                Span::styled(total_delayed.to_string(), Style::default().fg(Color::Cyan)),
                Span::raw("  "),
                Span::styled("Retry:  ", Style::default().fg(Color::White)),
                Span::styled(total_retry.to_string(), Style::default().fg(Color::Magenta)),
            ]),
            Line::from(vec![
                Span::styled("├ Dead:        ", Style::default().fg(Color::White)),
                Span::styled(total_dead.to_string(), Style::default().fg(Color::Red)),
                Span::raw("  "),
                Span::styled("Completed: ", Style::default().fg(Color::White)),
                Span::styled(total_completed.to_string(), Style::default().fg(Color::Green)),
            ]),
            Line::from(vec![
                Span::styled("└ Processed:   ", Style::default().fg(Color::White)),
                Span::styled(state.total_processed.to_string(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![]),  // Empty line for spacing
            Line::from(vec![
                Span::styled("Error Rate:    ", Style::default().fg(Color::White)),
                Span::styled(
                    format!("{:.1}%", error_rate),
                    Style::default().fg(error_rate_color(error_rate)).add_modifier(Modifier::BOLD)
                ),
            ]),
        ]
    } else {
        vec![Line::from("No queue data available")]
    };

    let metrics_paragraph = Paragraph::new(metrics)
        .block(Block::default().title("Metrics").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(metrics_paragraph, chunks[0]);

    // Right side - ops/sec and gauge
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(chunks[1]);

    let ops_text = vec![
        Line::from(vec![
            Span::styled("Ops/sec: ", Style::default().fg(Color::White)),
            Span::styled(format!("{:.1}", state.ops_per_sec), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Processed: ", Style::default().fg(Color::White)),
            Span::styled(state.total_processed.to_string(), Style::default().fg(Color::Green)),
        ]),
    ];

    let ops_paragraph = Paragraph::new(ops_text)
        .block(Block::default().title("Throughput").borders(Borders::ALL))
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Center);
    f.render_widget(ops_paragraph, right_chunks[0]);

    // Simple gauge for worker health
    let worker_ratio = if !state.workers.is_empty() {
        let active = state.workers.iter()
            .filter(|w| w.status == "idle" || w.status.to_lowercase() == "active")
            .count() as f64;
        active / state.workers.len() as f64
    } else {
        0.0
    };

    let gauge = Gauge::default()
        .block(Block::default().title("Worker Health").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Green))
        .percent((worker_ratio * 100.0) as u16)
        .label(Span::styled(
            format!("{:.0}%", worker_ratio * 100.0),
            Style::default().add_modifier(Modifier::BOLD),
        ));
    f.render_widget(gauge, right_chunks[1]);
}

fn draw_task_list_overlay(f: &mut Frame, state: &DashboardState, area: Rect) {
    if let Some(ref task_list) = state.task_list {
        let popup_width = 80.min(area.width.saturating_sub(4));
        let popup_height = area.height.saturating_sub(8);

        let x = (area.width.saturating_sub(popup_width)) / 2;
        let y = (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect {
            x: x.max(2),
            y: y.max(5),
            width: popup_width,
            height: popup_height,
        };

        // Clear area behind modal
        f.render_widget(Clear, popup_area);

        let title = format!(
            "Tasks: {} (Page {}/{} - {} tasks)",
            task_list.queue_name,
            task_list.page + 1,
            (task_list.total_available / task_list.page_size) + 1,
            task_list.total_available
        );

        let rows: Vec<Row> = task_list
            .tasks
            .iter()
            .enumerate()
            .map(|(i, task)| {
                let style = if i == task_list.selected_task {
                    Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                // Progress bar display
                let progress_str = if let Some(ref progress) = task.progress {
                    let bars = "==========";
                    let filled = (progress.current as f64 / 100.0 * 10.0) as usize;
                    let filled_bars = &bars[..filled.min(10)];
                    let empty_bars = &"          "[..(10 - filled.min(10))];
                    format!("[{}{}] {}%", filled_bars, empty_bars, progress.current)
                } else {
                    "N/A".to_string()
                };

                Row::new(vec![
                    Cell::from(shorten_id(&task.id, 10)),
                    Cell::from(task.task_type.clone()),
                    Cell::from(format!("{:?}", task.status)),
                    Cell::from(progress_str),
                    Cell::from(task.retry_cnt.to_string()),
                    Cell::from(format_timestamp(task.created_at)),
                ])
                .style(style)
            })
            .collect();

        let table = Table::new(rows, &[
            Constraint::Max(12),
            Constraint::Max(25),
            Constraint::Max(12),
            Constraint::Max(16),
            Constraint::Max(8),
            Constraint::Max(15),
        ])
        .header(
            Row::new(vec!["ID", "Type", "Status", "Progress", "Retry", "Created"])
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        )
        .block(Block::default().title(title).borders(Borders::ALL))
        .widths([
            Constraint::Percentage(18),
            Constraint::Percentage(30),
            Constraint::Percentage(12),
            Constraint::Percentage(18),
            Constraint::Percentage(10),
            Constraint::Percentage(12),
        ]);

        f.render_widget(table, popup_area);

        // Draw hint at bottom
        let hint = Paragraph::new("↑↓: Navigate | Esc: Back | r: Refresh")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        let hint_area = Rect {
            x: popup_area.x,
            y: popup_area.bottom().saturating_sub(1),
            width: popup_area.width,
            height: 1,
        };
        f.render_widget(hint, hint_area);
    }
}

fn draw_help_overlay(f: &mut Frame, area: Rect) {
    let popup_width = 60;
    let popup_height = 25;

    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect {
        x,
        y,
        width: popup_width,
        height: popup_height,
    };

    // Clear area behind modal
    f.render_widget(Clear, popup_area);

    let content = vec![
        Line::from(vec![
            Span::styled("Keyboard Shortcuts", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Navigation", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from("  ↑↓/←→    Navigate queue list"),
        Line::from("  Enter    View tasks in selected queue"),
        Line::from("  Esc      Return to main view / Clear status"),
        Line::from(""),
        Line::from(vec![
            Span::styled("Queue Operations", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from("  p        Pause selected queue"),
        Line::from("  r        Resume selected queue"),
        Line::from("  f        Flush selected queue"),
        Line::from(""),
        Line::from(vec![
            Span::styled("General", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from("  ?        Show/hide this help"),
        Line::from("  q        Quit dashboard"),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press any key to close", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let paragraph = Paragraph::new(content)
        .block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
        )
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);

    f.render_widget(paragraph, popup_area);
}

// ============================================================================
// Sparkline Chart Rendering
// ============================================================================

fn draw_sparkline_chart(
    f: &mut Frame,
    area: Rect,
    title: &str,
    data: &[u64],
    max_value: u64,
    color: Color,
    age_secs: u64,
) {
    let title_suffix = if age_secs > 0 {
        format!(" ({}s ago)", age_secs)
    } else {
        " (Collecting...)".to_string()
    };

    let sparkline = Sparkline::default()
        .block(
            Block::default()
                .title(format!("{}{}", title, title_suffix))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(color))
        )
        .data(data)
        .style(Style::default().fg(color))
        .max(max_value.max(1));

    f.render_widget(sparkline, area);
}

/// Draw history trends section in main view
fn draw_history_trends(f: &mut Frame, state: &DashboardState, area: Rect) {
    // Get the maximum value for each metric to scale the sparklines
    let pending_max = state.history.get_values("pending").iter().copied().max().unwrap_or(1);
    let active_max = state.history.get_values("active").iter().copied().max().unwrap_or(1);
    let completed_max = state.history.get_values("completed").iter().copied().max().unwrap_or(1);
    let dead_max = state.history.get_values("dead").iter().copied().max().unwrap_or(1);
    let error_rate_max = state.history.get_values("error_rate").iter().copied().max().unwrap_or(1);

    // Split into 3 columns for better use of space
    let chart_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(area);

    // Left column - Pending and Active
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(chart_chunks[0]);

    // Center column - Completed and Dead
    let center_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(chart_chunks[1]);

    // Right column - Error rate (full height)
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(100)])
        .split(chart_chunks[2]);

    // Pending tasks sparkline
    draw_sparkline_chart(
        f,
        left_chunks[0],
        "Pending",
        &state.history.get_values("pending"),
        pending_max,
        Color::Cyan,
        state.history.oldest_age_secs("pending"),
    );

    // Active tasks sparkline
    draw_sparkline_chart(
        f,
        left_chunks[1],
        "Active",
        &state.history.get_values("active"),
        active_max,
        Color::Green,
        state.history.oldest_age_secs("active"),
    );

    // Completed tasks sparkline
    draw_sparkline_chart(
        f,
        center_chunks[0],
        "Completed",
        &state.history.get_values("completed"),
        completed_max,
        Color::Blue,
        state.history.oldest_age_secs("completed"),
    );

    // Dead letter queue sparkline
    draw_sparkline_chart(
        f,
        center_chunks[1],
        "Dead",
        &state.history.get_values("dead"),
        dead_max,
        Color::Red,
        state.history.oldest_age_secs("dead"),
    );

    // Error rate sparkline (values are percentage * 10, so divide by 10 for display)
    let error_rate_values: Vec<u64> = state.history.get_values("error_rate");
    let error_rate_display: Vec<u64> = error_rate_values.iter().map(|v| v / 10).collect();
    let error_rate_max_display = (error_rate_max / 10).max(1);
    draw_sparkline_chart(
        f,
        right_chunks[0],
        "Error %",
        &error_rate_display,
        error_rate_max_display,
        Color::Yellow,
        state.history.oldest_age_secs("error_rate"),
    );
}

fn draw_status_message(f: &mut Frame, status: &OperationStatus, area: Rect) {
    let color = match status.status_type {
        StatusType::Success => Color::Green,
        StatusType::Error => Color::Red,
        StatusType::Info => Color::Cyan,
        StatusType::Warning => Color::Yellow,
    };

    let paragraph = Paragraph::new(status.message.as_str())
        .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center);

    // Draw at bottom of screen, above title bar
    let msg_area = Rect {
        x: 0,
        y: area.height.saturating_sub(1),
        width: area.width,
        height: 1,
    };
    f.render_widget(paragraph, msg_area);
}

// ============================================================================
// Utility Functions
// ============================================================================

fn style_for_count(count: u64) -> Style {
    if count > 1000 {
        Style::default().fg(Color::Red)
    } else if count > 100 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Green)
    }
}

fn shorten_id(id: &str, max_len: usize) -> String {
    if id.len() <= max_len {
        id.to_string()
    } else {
        format!("...{}", &id[id.len() - max_len + 3..])
    }
}

fn shorten_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

fn format_timestamp(timestamp: i64) -> String {
    let dt = Local::now();
    let now_timestamp = dt.timestamp();

    let duration = now_timestamp.saturating_sub(timestamp);

    if duration < 60 {
        format!("{}s ago", duration)
    } else if duration < 3600 {
        format!("{}m ago", duration / 60)
    } else if duration < 86400 {
        format!("{}h ago", duration / 3600)
    } else {
        let dt = Local.timestamp_opt(timestamp, 0).unwrap();
        dt.format("%m-%d %H:%M").to_string()
    }
}
