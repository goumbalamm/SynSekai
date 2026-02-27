#[cfg(test)]
mod tests {
    use crate::{
        app::App,
        types::{AppMode, TorrentRow, TorrentStatus},
    };
    use ratatui::{Terminal, backend::TestBackend};

    fn render_bar(app: &App) -> String {
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| super::render(f, app, f.area())).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn normal_mode_shows_keybindings() {
        let app = App::new();
        let content = render_bar(&app);
        assert!(content.contains("[q]") || content.contains("Quit"));
    }

    #[test]
    fn normal_mode_with_status_message_shows_message() {
        let mut app = App::new();
        app.status_message = Some("Torrent added.".into());
        let content = render_bar(&app);
        assert!(content.contains("Torrent added."));
    }

    #[test]
    fn add_dialog_mode_shows_hint() {
        let mut app = App::new();
        app.open_add_dialog();
        let content = render_bar(&app);
        assert!(content.contains("Enter") || content.contains("Esc"));
    }

    #[test]
    fn add_dialog_with_error_shows_error_and_cancel_hint() {
        let mut app = App::new();
        app.open_add_dialog();
        app.status_message = Some("Error: file not found".into());
        let content = render_bar(&app);
        assert!(content.contains("Error"));
        assert!(content.contains("Esc"));
    }

    #[test]
    fn normal_mode_with_status_message_still_shows_keybindings() {
        let mut app = App::new();
        app.status_message = Some("Torrent added.".into());
        let content = render_bar(&app);
        assert!(content.contains("Torrent added."));
        assert!(content.contains("[q]") || content.contains("Quit"));
    }

    #[test]
    fn confirm_remove_mode_shows_delete_flag() {
        let mut app = App::new();
        app.torrents = vec![TorrentRow {
            id: 0,
            name: "t".into(),
            total_bytes: 0,
            progress_pct: 0.0,
            down_speed_bps: 0,
            peers_live: 0,
            peers_seen: 0,
            status: TorrentStatus::Downloading,
        }];
        app.open_confirm_remove();
        let content = render_bar(&app);
        assert!(
            content.contains("delete files")
                || content.contains("Remove")
                || content.contains("no")
        );
    }

    #[test]
    fn confirm_remove_delete_files_true_shows_yes() {
        let mut app = App::new();
        app.mode = AppMode::ConfirmRemove {
            torrent_id: 0,
            delete_files: true,
        };
        let content = render_bar(&app);
        assert!(content.contains("YES"));
    }
}

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{app::App, types::AppMode};

const NORMAL_HINTS: &str = "[a] Add  [p] Pause/Resume  [d] Remove  [↑↓/jk] Nav  [q] Quit";

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let line = match &app.mode {
        AppMode::Normal => {
            if let Some(msg) = &app.status_message {
                let msg_color = if msg.starts_with("Error") {
                    Color::Red
                } else {
                    Color::Green
                };
                Line::from(vec![
                    Span::styled(format!(" {}  ", msg), Style::default().fg(msg_color)),
                    Span::styled("│  ", Style::default().fg(Color::DarkGray)),
                    Span::styled(NORMAL_HINTS, Style::default().fg(Color::White)),
                ])
            } else {
                Line::from(Span::styled(
                    format!(" {NORMAL_HINTS}"),
                    Style::default().fg(Color::White),
                ))
            }
        }
        AppMode::AddDialog => {
            if let Some(msg) = &app.status_message {
                Line::from(vec![
                    Span::styled(format!(" {}  ", msg), Style::default().fg(Color::Red)),
                    Span::styled("│  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("[Esc] Cancel", Style::default().fg(Color::Yellow)),
                ])
            } else {
                Line::from(Span::styled(
                    " Enter path or magnet link  [Enter] Add  [Esc] Cancel",
                    Style::default().fg(Color::Yellow),
                ))
            }
        }
        AppMode::ConfirmRemove { delete_files, .. } => {
            let delete_str = if *delete_files { "YES" } else { "no" };
            Line::from(Span::styled(
                format!(
                    " Remove torrent? [Space] delete files: {delete_str}  [Enter] Confirm  [Esc] Cancel"
                ),
                Style::default().fg(Color::Red),
            ))
        }
    };

    let para = Paragraph::new(line).style(Style::default().bg(Color::DarkGray));
    f.render_widget(para, area);
}
