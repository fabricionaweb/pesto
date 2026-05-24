use crate::app::{App, AppState};
pub mod components;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph, Sparkline, Tabs},
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

    // Graceful degradation: terminal too small to render meaningfully
    if area.width < 40 || area.height < 10 {
        let msg = Paragraph::new(format!(
            "Terminal too small\n{}x{} — need 40x10",
            area.width, area.height
        ))
        .style(Style::default().fg(Color::Red))
        .block(Block::default().borders(Borders::ALL));
        f.render_widget(msg, area);
        return;
    }

    // Compact mode: skip header when height is tight
    let compact = area.height < 20;

    let constraints: Vec<Constraint> = if compact {
        vec![
            Constraint::Length(3), // Tabs only
            Constraint::Min(5),    // Main content
            Constraint::Length(1), // Status (slim)
        ]
    } else {
        vec![
            Constraint::Length(3), // Header
            Constraint::Length(3), // Tabs
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Status bar
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    if compact {
        draw_tabs(f, app, chunks[0]);
        draw_main(f, app, chunks[1]);
        // slim status: single line without borders
        let slim = Paragraph::new(app.status_bar.message.clone())
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(slim, chunks[2]);
    } else {
        draw_header(f, chunks[0]);
        draw_tabs(f, app, chunks[1]);
        draw_main(f, app, chunks[2]);
        draw_status_bar(f, app, chunks[3]);
    }
}

fn draw_header(f: &mut Frame, area: Rect) {
    let title = Line::from(vec![
        Span::styled(
            "UPA",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("PASTA "),
        Span::styled("v2", Style::default().fg(Color::Yellow)),
        Span::raw(" — Rust Edition"),
    ]);

    let header = Paragraph::new(title)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title(" Welcome to UpaPasta "),
        );

    f.render_widget(header, area);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles = vec![" Dashboard ", " Browser ", " History ", " Config "];
    let selected = match app.state {
        AppState::Dashboard => 0,
        AppState::Browser => 1,
        AppState::History => 2,
        AppState::Config => 3,
    };

    let tabs = Tabs::new(titles)
        .select(selected)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL).title(" Navigation "));

    f.render_widget(tabs, area);
}

fn draw_main(f: &mut Frame, app: &mut App, area: Rect) {
    match app.state {
        AppState::Browser => {
            app.file_tree.render(f, area, true);
        }
        AppState::Dashboard => {
            draw_dashboard(f, app, area);
        }
        AppState::History => {
            draw_history(f, app, area);
        }
        _ => {
            let content =
                Paragraph::new("Configuration screen (Phase 40d).\n\nPress Tab to cycle screens.")
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" Configuration "),
                    );
            f.render_widget(content, area);
        }
    }
}

fn draw_dashboard(f: &mut Frame, app: &mut App, area: Rect) {
    let mut constraints = vec![
        Constraint::Length(7), // Progress bar + sparkline (only when uploading)
        Constraint::Min(8),    // Main split (Queue + Logs)
    ];

    if !app.upload_in_progress {
        constraints.remove(0); // no progress bar when idle
    }

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let content_area = if app.upload_in_progress {
        // Draw progress bar at top
        draw_progress_section(f, app, main_chunks[0]);
        main_chunks[1]
    } else {
        main_chunks[0]
    };

    // Split remaining into Queue (left) + Logs (right)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(content_area);

    if app.upload_in_progress && !app.progress.files.is_empty() {
        draw_per_file_progress(f, app, chunks[0]);
    } else if !app.upload_queue.items.is_empty() {
        draw_upload_settings_summary(f, app, chunks[0]);
    } else {
        let idle = Paragraph::new(
            "No files in queue.\n\n\
             Go to Browser tab (press Tab) → navigate with j/k/Enter → add files with Enter.\n\
             Then come back here and press 'u' to start upload.",
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Dashboard — Ready "),
        );
        f.render_widget(idle, chunks[0]);
    }

    app.log_panel.render(f, chunks[1]);
}

fn draw_progress_section(f: &mut Frame, app: &App, area: Rect) {
    let p = &app.progress;

    // Split the progress area: Gauge on top, Sparkline below
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Gauge
            Constraint::Length(3), // Sparkline + stats
        ])
        .split(area);

    let pct = p.progress_pct() as u16;

    let speed = if p.last_speed > 0.1 {
        format!("{:.1} MB/s", p.last_speed)
    } else {
        "calculating...".to_string()
    };

    let eta = if let Some(secs) = p.eta_seconds() {
        let m = secs / 60;
        let s = secs % 60;
        format!("ETA {}:{:02}", m, s)
    } else {
        "ETA --:--".to_string()
    };

    let label = format!(
        "{:.1}%  ({}/{} seg)  {}  {}",
        p.progress_pct(),
        p.done_segments,
        p.total_segments,
        speed,
        eta
    );

    let is_paused = app.upload_paused;

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(if is_paused {
            " Upload Progress — PAUSED "
        } else {
            " Upload Progress "
        }))
        .gauge_style(if is_paused {
            Style::default().fg(Color::Yellow).bg(Color::DarkGray)
        } else {
            Style::default().fg(Color::Green).bg(Color::DarkGray)
        })
        .percent(pct)
        .label(if is_paused {
            "PAUSED — Press 'p' to resume".to_string()
        } else {
            label
        });

    f.render_widget(gauge, chunks[0]);

    // Sparkline of recent throughput
    let spark_data: Vec<u64> = p
        .speed_history
        .iter()
        .map(|&s| (s * 10.0) as u64) // scale for visibility
        .collect();

    let spark_style = if is_paused {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let sparkline = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                .title(format!(
                    " Throughput History ({} samples) ",
                    spark_data.len()
                )),
        )
        .data(&spark_data)
        .style(spark_style);

    f.render_widget(sparkline, chunks[1]);
}

fn draw_per_file_progress(f: &mut Frame, app: &App, area: Rect) {
    use crate::app::FileStatus;

    let files = &app.progress.files;
    let n = files.len();

    // Outer block
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Files ({}) ", n));
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    if n == 0 || inner.height == 0 {
        return;
    }

    // Allocate up to 3 lines per file (name + gauge + gap), constrained by height
    let rows_available = inner.height as usize;
    let per_file = 2usize; // name line + gauge line
    let max_files = (rows_available / per_file).max(1);
    let shown = n.min(max_files);

    // Build constraints: alternating name (1) + gauge (1) rows
    let constraints: Vec<Constraint> = (0..shown)
        .flat_map(|_| [Constraint::Length(1), Constraint::Length(1)])
        .collect();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (i, fp) in files.iter().take(shown).enumerate() {
        let pct = if fp.total_segments > 0 {
            (fp.done_segments as f64 / fp.total_segments as f64 * 100.0).min(100.0) as u16
        } else {
            0
        };

        let (status_icon, icon_color) = match fp.status {
            FileStatus::Done => ("✓", Color::Green),
            FileStatus::Failed => ("✗", Color::Red),
            FileStatus::Active => ("▶", Color::Cyan),
            FileStatus::Pending => (" ", Color::DarkGray),
        };

        let name_row = rows[i * 2];
        let gauge_row = rows[i * 2 + 1];

        // Name line with status icon
        let max_name = (name_row.width as usize).saturating_sub(4);
        let short_name = if fp.name.len() > max_name && max_name > 3 {
            format!("{}…", &fp.name[..max_name - 1])
        } else {
            fp.name.clone()
        };
        let name_line = Line::from(vec![
            Span::styled(
                format!(" {} ", status_icon),
                Style::default().fg(icon_color),
            ),
            Span::raw(short_name),
        ]);
        f.render_widget(Paragraph::new(name_line), name_row);

        // Gauge
        let gauge_color = match fp.status {
            FileStatus::Done => Color::Green,
            FileStatus::Failed => Color::Red,
            FileStatus::Active => Color::Cyan,
            FileStatus::Pending => Color::DarkGray,
        };
        let label = if fp.total_segments > 0 {
            format!("{pct}%  {}/{}", fp.done_segments, fp.total_segments)
        } else {
            "waiting…".to_string()
        };
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(gauge_color).bg(Color::DarkGray))
            .percent(pct)
            .label(label);
        f.render_widget(gauge, gauge_row);
    }

    // If we couldn't show all files, show a summary line
    if shown < n {
        // There's no room; the outer block title already shows the count
    }
}

fn draw_upload_settings_summary(f: &mut Frame, app: &App, area: Rect) {
    let s = app.effective_upload_settings();

    let lines = vec![
        Line::from(" Obfuscation : ".to_string() + &s.obfuscate),
        Line::from(" Compression : ".to_string() + &s.compression),
        Line::from(" PAR2        : ".to_string() + &s.par2),
        Line::from(" Groups      : ".to_string() + &s.groups),
        Line::from(" From        : ".to_string() + &s.from),
        Line::from(" Article     : ".to_string() + &s.article_size),
        Line::from(" Verify      : ".to_string() + &s.verify),
    ];

    let title = if app.pesto_config.is_some() {
        " Effective Upload Settings (from config) "
    } else {
        " Effective Upload Settings (dry-run defaults) "
    };

    let para = Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(para, area);
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    // Dynamic help when upload is active
    if app.upload_in_progress {
        let pause_resume = if app.upload_paused {
            "p: resume"
        } else {
            "p: pause"
        };
        let help = format!(
            "{}  •  {}  •  x: cancel  •  Tab: switch  •  q: quit",
            app.status_bar.message, pause_resume
        );

        let status = Paragraph::new(help)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::TOP).title(" Status "));
        f.render_widget(status, area);
        return;
    }

    app.status_bar.render(f, area);
}

// ── History screen ─────────────────────────────────────────────────────────

fn draw_history(f: &mut Frame, app: &mut App, area: Rect) {
    if app.catalog.is_none() {
        let msg = Paragraph::new(
            "No catalog available.\n\nThe catalog could not be opened.\nCheck permissions for ~/.local/share/upapasta/",
        )
        .block(Block::default().borders(Borders::ALL).title(" History "));
        f.render_widget(msg, area);
        return;
    }

    // Layout: search bar on top, list left + detail right, stats at bottom
    let show_stats = app.history.show_stats;
    let mut constraints = vec![
        Constraint::Length(3), // search bar
        Constraint::Min(6),    // list + detail
    ];
    if show_stats {
        constraints.push(Constraint::Length(10)); // stats panel
    }

    let vchunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    draw_history_search(f, app, vchunks[0]);

    let hchunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(vchunks[1]);

    draw_history_list(f, app, hchunks[0]);
    draw_history_detail(f, app, hchunks[1]);

    if show_stats {
        draw_history_stats(f, app, vchunks[2]);
    }
}

fn draw_history_search(f: &mut Frame, app: &App, area: Rect) {
    let is_searching = app.history.searching;
    let query = &app.history.query;

    let content = if is_searching {
        format!(" /{}_", query)
    } else if query.is_empty() {
        " Press / to search, s for stats, Tab to switch tab".to_string()
    } else {
        format!(" Filter: {}  (/ to edit, Esc to clear)", query)
    };

    let border_style = if is_searching {
        Style::default().fg(Color::Yellow)
    } else if !query.is_empty() {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = format!(" History ({} records) ", app.history.rows.len());
    let para = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    );
    f.render_widget(para, area);
}

fn draw_history_list(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = &app.history.rows;

    let items: Vec<ListItem> = rows
        .iter()
        .map(|r| {
            let date = r.uploaded_at.format("%Y-%m-%d").to_string();
            let size = r
                .size_bytes
                .map(|b| format_bytes(b as u64))
                .unwrap_or_else(|| "—".to_string());
            let cat_color = category_color(&r.category);
            let short_name = if r.original_name.len() > 34 {
                format!("{}…", &r.original_name[..33])
            } else {
                r.original_name.clone()
            };
            let line = Line::from(vec![
                Span::styled(format!("{} ", date), Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:<35}", short_name),
                    Style::default().fg(Color::White),
                ),
                Span::styled(format!("{:<8}", r.category), Style::default().fg(cat_color)),
                Span::styled(size, Style::default().fg(Color::Cyan)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let selected = app.history.selected;
    let mut state = ListState::default();
    if !rows.is_empty() {
        state.select(Some(selected));
    }

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Uploads (j/k to navigate) "),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut state);
}

fn draw_history_detail(f: &mut Frame, app: &App, area: Rect) {
    let rows = &app.history.rows;
    if rows.is_empty() || app.history.selected >= rows.len() {
        let msg = Paragraph::new(" No record selected.")
            .block(Block::default().borders(Borders::ALL).title(" Detail "));
        f.render_widget(msg, area);
        return;
    }

    let r = &rows[app.history.selected];
    let date = r.uploaded_at.format("%Y-%m-%d %H:%M UTC").to_string();
    let size = r
        .size_bytes
        .map(|b| format_bytes(b as u64))
        .unwrap_or_else(|| "unknown".to_string());
    let dur = r
        .upload_duration_s
        .map(|s| {
            let m = s as u64 / 60;
            let sec = s as u64 % 60;
            if m > 0 {
                format!("{}m {:02}s", m, sec)
            } else {
                format!("{:.1}s", s)
            }
        })
        .unwrap_or_else(|| "—".to_string());
    let group = r.usenet_group.as_deref().unwrap_or("—");

    let lines = vec![
        Line::from(vec![
            Span::styled(" Name    ", Style::default().fg(Color::DarkGray)),
            Span::raw(r.original_name.clone()),
        ]),
        Line::from(vec![
            Span::styled(" Date    ", Style::default().fg(Color::DarkGray)),
            Span::raw(date),
        ]),
        Line::from(vec![
            Span::styled(" Category", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(" {}", r.category),
                Style::default().fg(category_color(&r.category)),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Size    ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!(" {}", size)),
        ]),
        Line::from(vec![
            Span::styled(" Duration", Style::default().fg(Color::DarkGray)),
            Span::raw(format!(" {}", dur)),
        ]),
        Line::from(vec![
            Span::styled(" Group   ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!(" {}", group)),
        ]),
    ];

    let para =
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" Detail "));
    f.render_widget(para, area);
}

fn draw_history_stats(f: &mut Frame, app: &App, area: Rect) {
    let Some(ref stats) = app.history.stats else {
        let msg = Paragraph::new(" Loading stats…")
            .block(Block::default().borders(Borders::ALL).title(" Stats "));
        f.render_widget(msg, area);
        return;
    };

    let total_gb = stats.total_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
    let mut lines = vec![
        Line::from(format!(
            " Total: {} uploads  |  {:.2} GB",
            stats.total_uploads, total_gb
        )),
        Line::from(""),
    ];

    // Categories
    let cats: Vec<String> = stats
        .by_category
        .iter()
        .map(|(cat, n)| format!("{}: {}", cat, n))
        .collect();
    lines.push(Line::from(format!(" By category — {}", cats.join("  "))));

    // Monthly bytes
    if !stats.bytes_by_month.is_empty() {
        lines.push(Line::from(""));
        let month_strs: Vec<String> = stats
            .bytes_by_month
            .iter()
            .map(|(m, b)| format!("{}: {:.1}GB", m, *b as f64 / 1024.0 / 1024.0 / 1024.0))
            .collect();
        lines.push(Line::from(format!(" Monthly — {}", month_strs.join("  "))));
    }

    let para = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Catalog Stats "),
    );
    f.render_widget(para, area);
}

// ── helpers ────────────────────────────────────────────────────────────────

fn format_bytes(b: u64) -> String {
    if b >= 1_073_741_824 {
        format!("{:.1}GB", b as f64 / 1_073_741_824.0)
    } else if b >= 1_048_576 {
        format!("{:.0}MB", b as f64 / 1_048_576.0)
    } else if b >= 1024 {
        format!("{:.0}KB", b as f64 / 1024.0)
    } else {
        format!("{}B", b)
    }
}

fn category_color(cat: &str) -> Color {
    match cat {
        "Movie" => Color::Magenta,
        "TV" => Color::Blue,
        "Anime" => Color::Yellow,
        _ => Color::DarkGray,
    }
}
