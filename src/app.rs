use crate::types::{AppMode, InputState, TorrentRow};

pub struct App {
    pub torrents: Vec<TorrentRow>,
    pub selected: usize,
    pub mode: AppMode,
    pub add_input: InputState,
    pub status_message: Option<String>,
    pub should_quit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            torrents: Vec::new(),
            selected: 0,
            mode: AppMode::default(),
            add_input: InputState::default(),
            status_message: None,
            should_quit: false,
        }
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_torrents(&mut self, rows: Vec<TorrentRow>) {
        let prev_len = self.torrents.len();
        self.torrents = rows;
        // Clamp selected index if list shrank
        if !self.torrents.is_empty() && self.selected >= self.torrents.len() {
            self.selected = self.torrents.len() - 1;
        }
        // Reset if list went from non-empty to empty
        if self.torrents.is_empty() {
            self.selected = 0;
        }
        let _ = prev_len;
    }

    pub fn select_next(&mut self) {
        if self.torrents.is_empty() {
            return;
        }
        if self.selected + 1 < self.torrents.len() {
            self.selected += 1;
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn selected_torrent_id(&self) -> Option<usize> {
        self.torrents.get(self.selected).map(|t| t.id)
    }

    pub fn open_add_dialog(&mut self) {
        self.add_input.clear();
        self.status_message = None;
        self.mode = AppMode::AddDialog;
    }

    pub fn open_confirm_remove(&mut self) {
        if let Some(id) = self.selected_torrent_id() {
            self.mode = AppMode::ConfirmRemove {
                torrent_id: id,
                delete_files: false,
            };
        }
    }

    pub fn toggle_delete_files(&mut self) {
        if let AppMode::ConfirmRemove {
            ref mut delete_files,
            ..
        } = self.mode
        {
            *delete_files = !*delete_files;
        }
    }

    pub fn dismiss_dialog(&mut self) {
        self.mode = AppMode::Normal;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn make_app_with_torrents(n: usize) -> App {
        let mut app = App::new();
        app.torrents = (0..n)
            .map(|i| TorrentRow {
                id: i,
                name: format!("torrent-{i}"),
                total_bytes: 1000,
                progress_pct: 0.0,
                down_speed_bps: 0,
                peers_live: 0,
                peers_seen: 0,
                status: TorrentStatus::Downloading,
            })
            .collect();
        app
    }

    #[test]
    fn select_next_on_empty_list_is_noop() {
        let mut app = App::new();
        app.select_next(); // should not panic, selected stays 0
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn select_next_clamps_at_end() {
        let mut app = make_app_with_torrents(3);
        app.selected = 2;
        app.select_next();
        assert_eq!(app.selected, 2); // does not wrap
    }

    #[test]
    fn select_prev_clamps_at_zero() {
        let mut app = make_app_with_torrents(3);
        app.selected = 0;
        app.select_prev();
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn selected_torrent_id_returns_none_when_empty() {
        let app = App::new();
        assert!(app.selected_torrent_id().is_none());
    }

    #[test]
    fn open_add_dialog_resets_input() {
        let mut app = make_app_with_torrents(1);
        app.add_input.push('x');
        app.open_add_dialog();
        assert_eq!(app.mode, AppMode::AddDialog);
        assert!(app.add_input.value.is_empty());
    }

    #[test]
    fn open_confirm_remove_captures_selected_id() {
        let mut app = make_app_with_torrents(3);
        app.selected = 1;
        app.open_confirm_remove();
        assert!(matches!(
            app.mode,
            AppMode::ConfirmRemove {
                torrent_id: 1,
                delete_files: false
            }
        ));
    }

    #[test]
    fn update_torrents_clamps_selected_when_list_shrinks() {
        let mut app = make_app_with_torrents(5);
        app.selected = 4;
        // shrink to 2
        let short: Vec<_> = (0..2)
            .map(|i| TorrentRow {
                id: i,
                name: format!("t{i}"),
                total_bytes: 0,
                progress_pct: 0.0,
                down_speed_bps: 0,
                peers_live: 0,
                peers_seen: 0,
                status: TorrentStatus::Downloading,
            })
            .collect();
        app.update_torrents(short);
        assert_eq!(app.selected, 1); // clamped to last index
    }

    #[test]
    fn update_torrents_resets_selected_when_list_becomes_empty() {
        let mut app = make_app_with_torrents(3);
        app.selected = 2;
        app.update_torrents(vec![]);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn selected_torrent_id_returns_correct_id() {
        let mut app = make_app_with_torrents(3);
        app.selected = 2;
        assert_eq!(app.selected_torrent_id(), Some(2));
    }

    #[test]
    fn dismiss_dialog_returns_to_normal() {
        let mut app = make_app_with_torrents(1);
        app.open_add_dialog();
        app.dismiss_dialog();
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn toggle_delete_files_flips_flag() {
        let mut app = make_app_with_torrents(1);
        app.selected = 0;
        app.open_confirm_remove();
        app.toggle_delete_files();
        assert!(matches!(
            app.mode,
            AppMode::ConfirmRemove {
                delete_files: true,
                ..
            }
        ));
        app.toggle_delete_files();
        assert!(matches!(
            app.mode,
            AppMode::ConfirmRemove {
                delete_files: false,
                ..
            }
        ));
    }
}
