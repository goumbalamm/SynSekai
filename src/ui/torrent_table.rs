use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Text,
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

use crate::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec![
        Cell::from(" # "),
        Cell::from("Name"),
        Cell::from("Size"),
        Cell::from("Progress"),
        Cell::from(format!("{:>10}", "Speed")),
        Cell::from("Peers"),
        Cell::from("Status"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD))
    .height(1);

    let rows: Vec<Row> = app
        .torrents
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let progress_bar = make_progress_bar(t.progress_pct);
            let size = ByteSize(t.total_bytes).to_string();
            let speed = format_speed(t.down_speed_bps);
            let is_selected = i == app.selected;

            let style = if is_selected {
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(format!(" {} ", t.id + 1)),
                Cell::from(t.name.clone()),
                Cell::from(size),
                Cell::from(Text::from(progress_bar)),
                Cell::from(speed),
                Cell::from(format!("{}/{}", t.peers_live, t.peers_seen)),
                Cell::from(t.status.as_str()),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Min(20),
        Constraint::Length(10),
        Constraint::Length(14),
        Constraint::Length(10),
        Constraint::Length(7),
        Constraint::Length(7),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(" Torrents "))
        .row_highlight_style(Style::default())
        .highlight_symbol(">> ");

    let mut state = TableState::default();
    if !app.torrents.is_empty() {
        state.select(Some(app.selected));
    }

    f.render_stateful_widget(table, area, &mut state);
}

fn make_progress_bar(pct: f32) -> String {
    let filled = ((pct / 100.0) * 8.0).round() as usize;
    let empty = 8usize.saturating_sub(filled);
    format!("{}{} {:5.1}%", "▓".repeat(filled), "░".repeat(empty), pct)
}

fn format_speed(bps: u64) -> String {
    format!("{:>10}", format!("{}/s", ByteSize(bps)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TorrentRow, TorrentStatus};
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn renders_without_panic_when_empty() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new();
        terminal.draw(|f| render(f, &app, f.area())).unwrap();
    }

    #[test]
    fn renders_non_selected_row_without_panic() {
        // Two torrents: index 0 selected, index 1 uses the non-selected style branch
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.torrents = vec![
            TorrentRow {
                id: 0, name: "First".into(), total_bytes: 100,
                progress_pct: 50.0, down_speed_bps: 0,
                peers_live: 0, peers_seen: 0,
                status: TorrentStatus::Downloading,
            },
            TorrentRow {
                id: 1, name: "Second".into(), total_bytes: 200,
                progress_pct: 0.0, down_speed_bps: 0,
                peers_live: 0, peers_seen: 0,
                status: TorrentStatus::Paused,
            },
        ];
        app.selected = 0; // index 1 is non-selected
        terminal.draw(|f| render(f, &app, f.area())).unwrap();
        let buf = terminal.backend().buffer().clone();
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(content.contains("Second"));
    }

    #[test]
    fn renders_torrent_name_in_table() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.torrents = vec![TorrentRow {
            id: 0,
            name: "Harry Potter".into(),
            total_bytes: 1_000_000,
            progress_pct: 50.0,
            down_speed_bps: 1024,
            peers_live: 3, peers_seen: 47,
            status: TorrentStatus::Downloading,
        }];
        terminal.draw(|f| render(f, &app, f.area())).unwrap();
        let buf = terminal.backend().buffer().clone();
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(content.contains("Harry Potter"));
    }
}
