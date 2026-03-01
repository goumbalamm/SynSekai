use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    app::App,
    types::{ClientProfile, SpooferField},
};

fn format_bytes(n: u64) -> String {
    if n >= 1024 * 1024 * 1024 {
        format!("{:.2} GiB", n as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if n >= 1024 * 1024 {
        format!("{:.2} MiB", n as f64 / (1024.0 * 1024.0))
    } else if n >= 1024 {
        format!("{:.2} KiB", n as f64 / 1024.0)
    } else {
        format!("{n} B")
    }
}

fn format_countdown(secs: u64) -> String {
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let torrent_name = app
        .spoofer_torrent_name
        .as_deref()
        .unwrap_or("no torrent selected");

    let snapshot = app.spoofer_snapshot();
    let running = snapshot.as_ref().map(|s| s.running).unwrap_or(false);

    let title = format!(" Ratio Boost — {torrent_name} ");
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(block, area);

    // Inner area (inset 1 all around for the border)
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // upload rate
            Constraint::Length(3), // download rate
            Constraint::Length(3), // tracker URL
            Constraint::Length(1), // client
            Constraint::Length(1), // spacer
            Constraint::Length(4), // live stats
            Constraint::Length(1), // status line
        ])
        .split(inner);

    // ── config fields ──────────────────────────────────────────────────────────

    render_input_field(
        f,
        " Upload rate (KB/s): ",
        &app.spoofer_upload_input.value,
        app.spoofer_upload_input.cursor,
        app.spoofer_focused_field == Some(SpooferField::UploadRate),
        chunks[0],
    );

    render_input_field(
        f,
        " Download rate (KB/s): ",
        &app.spoofer_download_input.value,
        app.spoofer_download_input.cursor,
        app.spoofer_focused_field == Some(SpooferField::DownloadRate),
        chunks[1],
    );

    render_input_field(
        f,
        " Tracker URL: ",
        &app.spoofer_tracker_input.value,
        app.spoofer_tracker_input.cursor,
        app.spoofer_focused_field == Some(SpooferField::TrackerUrl),
        chunks[2],
    );

    let client = ClientProfile::all()
        .get(app.spoofer_client_idx % ClientProfile::all().len())
        .copied()
        .unwrap_or_default();
    let tracker_hint = if app.spoofer_tracker_urls.len() > 1 {
        "  [t] next tracker"
    } else {
        ""
    };
    let client_line = Line::from(vec![
        Span::styled(" Client: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("[ {} ]", client.label()),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(
            format!("  [c] next client{tracker_hint}"),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    f.render_widget(Paragraph::new(client_line), chunks[3]);

    // ── live stats ─────────────────────────────────────────────────────────────

    let snap = snapshot.as_ref();
    let (uploaded, downloaded, seeders, leechers, countdown) = if let Some(s) = snap {
        (
            s.uploaded,
            s.downloaded,
            s.seeders,
            s.leechers,
            s.countdown_secs,
        )
    } else {
        (0, 0, None, None, 0)
    };

    let stats_lines = vec![
        Line::from(vec![
            Span::styled("  Uploaded:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format_bytes(uploaded),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Downloaded: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format_bytes(downloaded),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Seeders: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                seeders.map(|n| n.to_string()).unwrap_or_else(|| "—".into()),
                Style::default().fg(Color::Green),
            ),
            Span::styled("  Leechers: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                leechers
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "—".into()),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Next announce in: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if running {
                    format_countdown(countdown)
                } else {
                    "—".into()
                },
                Style::default().fg(Color::White),
            ),
        ]),
    ];
    f.render_widget(Paragraph::new(stats_lines), chunks[5]);

    // ── status & error ─────────────────────────────────────────────────────────

    let (status_sym, status_color, status_text) = if running {
        ("●", Color::Green, "Running")
    } else {
        ("○", Color::DarkGray, "Stopped")
    };
    let mut status_spans = vec![
        Span::styled(format!(" {status_sym} "), Style::default().fg(status_color)),
        Span::styled(status_text, Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
    ];
    if let Some(err) = snap.and_then(|s| s.last_error.as_deref()) {
        status_spans.push(Span::styled(
            format!("  {err}"),
            Style::default().fg(Color::Red),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(status_spans)), chunks[6]);
}

fn render_input_field(
    f: &mut Frame,
    label: &str,
    value: &str,
    cursor: usize,
    focused: bool,
    area: Rect,
) {
    let border_color = if focused { Color::Yellow } else { Color::DarkGray };
    let inner_w = area.width.saturating_sub(2) as usize;
    let display_col = value[..cursor].chars().count();
    let scroll = display_col.saturating_sub(inner_w.saturating_sub(1));
    let visible: String = value.chars().skip(scroll).collect();

    let widget = Paragraph::new(visible.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(label)
                .style(Style::default().fg(border_color)),
        )
        .style(Style::default().fg(Color::White));
    f.render_widget(widget, area);

    if focused {
        f.set_cursor_position((area.x + 1 + (display_col - scroll) as u16, area.y + 1));
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        app::App,
        types::{AppView, TorrentRow, TorrentStatus},
    };
    use ratatui::{Terminal, backend::TestBackend};

    fn render_panel(app: &App) -> String {
        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| super::render(f, app, f.area()))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    fn app_with_torrent(id: usize, name: &str) -> App {
        let mut app = App::new();
        app.torrents = vec![TorrentRow {
            id,
            name: name.into(),
            total_bytes: 1_000_000,
            progress_pct: 0.0,
            down_speed_bps: 0,
            peers_live: 0,
            peers_seen: 0,
            status: TorrentStatus::Seeding,
        }];
        app
    }

    #[test]
    fn render_spoofer_panel_shows_torrent_name() {
        let mut app = app_with_torrent(0, "Ubuntu ISO");
        app.view = AppView::Spoofer;
        app.spoofer_torrent_name = Some("Ubuntu ISO".into());
        app.spoofer_info_hash = "a".repeat(40);
        let content = render_panel(&app);
        assert!(content.contains("Ubuntu"), "must show torrent name");
    }

    #[test]
    fn render_spoofer_panel_shows_stopped_when_not_running() {
        let mut app = app_with_torrent(0, "t");
        app.view = AppView::Spoofer;
        let content = render_panel(&app);
        assert!(content.contains("Stopped"), "must show Stopped status");
    }

    #[test]
    fn render_spoofer_panel_shows_no_torrent_label_when_standalone() {
        let mut app = App::new();
        app.view = AppView::Spoofer;
        let content = render_panel(&app);
        assert!(content.contains("no torrent"), "must show no-torrent label");
    }
}
