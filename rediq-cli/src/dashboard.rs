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
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table, Wrap},
    Frame, Terminal,
};
use rediq::client::Client;
use std::time::{Duration, Instant};
use chrono::Local;

pub struct DashboardState {
    queues: Vec<QueueStats>,
    workers: Vec<WorkerInfo>,
    selected_queue: usize,
    last_update: Instant,
    total_processed: u64,
    ops_per_sec: f64,
}

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

pub async fn run(redis_url: &str, refresh_interval_ms: u64) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let client = Client::builder().redis_url(redis_url).build().await?;
    let inspector = client.inspector();

    let mut state = DashboardState {
        queues: Vec::new(),
        workers: Vec::new(),
        selected_queue: 0,
        last_update: Instant::now(),
        total_processed: 0,
        ops_per_sec: 0.0,
    };

    // Initial data fetch
    refresh_data(&inspector, &mut state).await;

    let refresh_duration = Duration::from_millis(refresh_interval_ms);
    let mut last_refresh = Instant::now();

    let result = run_dashboard(
        &mut terminal,
        &inspector,
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

async fn run_dashboard(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    inspector: &rediq::client::Inspector,
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
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        return Ok(());
                    }
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
                    KeyCode::Enter => {
                        // Could show detailed queue info
                    }
                    _ => {}
                }
            }
        }

        // Refresh data at interval
        if last_refresh.elapsed() >= refresh_duration {
            refresh_data(inspector, state).await;
            *last_refresh = Instant::now();
        }
    }
}

async fn refresh_data(inspector: &rediq::client::Inspector, state: &mut DashboardState) {
    let now = Instant::now();

    // Fetch queues
    if let Ok(queues) = inspector.list_queues().await {
        let mut new_queues = Vec::new();
        for queue in queues {
            if let Ok(stats) = inspector.queue_stats(&queue).await {
                new_queues.push(QueueStats {
                    name: queue,
                    pending: stats.pending,
                    active: stats.active,
                    delayed: stats.delayed,
                    retry: stats.retried,
                    dead: stats.dead,
                    completed: stats.completed,
                });
            }
        }
        state.queues = new_queues;
    }

    // Fetch workers
    if let Ok(workers) = inspector.list_workers().await {
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
}

fn ui(f: &mut Frame, state: &DashboardState) {
    let area = f.area();

    // Title bar with timestamp and total processed
    let time_str = Local::now().format("%H:%M:%S").to_string();
    let elapsed = state.last_update.elapsed().as_millis();
    let ago_str = if elapsed < 1000 {
        format!("{}ms ago", elapsed)
    } else {
        format!("{}s ago", elapsed / 1000)
    };

    let title = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Rediq", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" Dashboard "),
            Span::styled(format!("Updated: {} ({})", time_str, ago_str), Style::default().fg(Color::DarkGray)),
            Span::raw(" "),
            Span::styled(format!("Total Processed: {}", state.total_processed), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Press 'q' to quit | ↑↓ to select queue", Style::default().fg(Color::DarkGray)),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, Rect { x: 0, y: 0, width: area.width, height: 4 });

    // Main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(area.height - 12), Constraint::Length(8)].as_ref())
        .split(Rect { x: 0, y: 4, width: area.width, height: area.height - 4 });

    // Upper section - queues and workers side by side
    let upper_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
        .split(chunks[0]);

    draw_queues_panel(f, state, upper_chunks[0]);
    draw_workers_panel(f, state, upper_chunks[1]);

    // Lower section - stats
    draw_stats_panel(f, state, chunks[1]);
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
    .widths(&[
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
    .widths(&[
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
    // Workers are considered healthy if they're registered (status is "idle" or "active")
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
