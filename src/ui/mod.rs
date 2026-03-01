#[cfg(test)]
mod tests {
    use crate::{
        app::App,
        types::{AppView, TorrentRow, TorrentStatus},
    };
    use ratatui::{Terminal, backend::TestBackend};

    fn render_app(app: &App) -> String {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| super::render(f, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn full_render_empty_app() {
        let app = App::new();
        render_app(&app); // must not panic
    }

    #[test]
    fn full_render_with_torrent() {
        let mut app = App::new();
        app.torrents = vec![TorrentRow {
            id: 0,
            name: "Ubuntu".into(),
            total_bytes: 1_000_000,
            progress_pct: 50.0,
            down_speed_bps: 2048,
            peers_live: 0,
            peers_seen: 0,
            status: TorrentStatus::Downloading,
        }];
        let content = render_app(&app);
        assert!(content.contains("Ubuntu"));
    }

    #[test]
    fn full_render_add_dialog_mode() {
        let mut app = App::new();
        app.open_add_dialog();
        render_app(&app); // must not panic
    }

    #[test]
    fn full_render_spoofer_view() {
        let mut app = App::new();
        app.view = AppView::Spoofer;
        render_app(&app); // must not panic
    }

    #[test]
    fn full_render_confirm_remove_mode() {
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
        render_app(&app); // must not panic
    }
}

pub mod popups;
pub mod spoofer_panel;
pub mod status_bar;
pub mod tab_bar;
pub mod torrent_table;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

use crate::{app::App, types::AppView};

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tab bar
            Constraint::Min(0),    // content
            Constraint::Length(1), // status bar
        ])
        .split(area);

    tab_bar::render(f, app, chunks[0]);
    status_bar::render(f, app, chunks[2]);

    match app.view {
        AppView::Downloader => {
            torrent_table::render(f, app, chunks[1]);

            // Overlays rendered last so they appear on top
            match &app.mode {
                crate::types::AppMode::AddDialog => {
                    popups::render_add_dialog(f, app, chunks[1]);
                }
                crate::types::AppMode::ConfirmRemove {
                    torrent_id,
                    delete_files,
                } => {
                    popups::render_confirm_remove(f, *torrent_id, *delete_files, chunks[1]);
                }
                crate::types::AppMode::Normal => {}
            }
        }
        AppView::Spoofer => {
            spoofer_panel::render(f, app, chunks[1]);
        }
    }
}
