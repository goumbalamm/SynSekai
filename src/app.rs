use crate::{
    engine::TorrentMeta,
    spoofer::SpooferHandle,
    types::{AppMode, AppView, ClientProfile, InputState, SpooferField, SpooferSnapshot, TorrentRow},
};

#[derive(Default)]
pub struct App {
    pub torrents: Vec<TorrentRow>,
    pub selected: usize,
    pub view: AppView,
    pub mode: AppMode,
    pub add_input: InputState,
    pub status_message: Option<String>,
    pub should_quit: bool,

    // Spoofer state
    pub spoofer_handle: Option<SpooferHandle>,
    pub spoofer_upload_input: InputState,
    pub spoofer_download_input: InputState,
    pub spoofer_tracker_input: InputState,
    pub spoofer_tracker_urls: Vec<String>,
    pub spoofer_tracker_idx: usize,
    pub spoofer_client_idx: usize,
    pub spoofer_focused_field: Option<SpooferField>,
    pub spoofer_info_hash: String,
    pub spoofer_total_bytes: u64,
    pub spoofer_torrent_name: Option<String>,
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

    /// Switch to the Spoofer view, optionally pre-populating from a torrent.
    pub fn enter_spoofer_view(&mut self, torrent_id: Option<usize>, meta: Option<TorrentMeta>) {
        self.spoofer_upload_input.clear();
        self.spoofer_download_input.clear();
        self.spoofer_tracker_input.clear();
        self.spoofer_tracker_urls.clear();
        self.spoofer_tracker_idx = 0;
        self.spoofer_focused_field = None;

        self.spoofer_torrent_name = torrent_id
            .and_then(|id| self.torrents.iter().find(|t| t.id == id))
            .map(|t| t.name.clone());

        if let Some(m) = meta {
            self.spoofer_info_hash = m.info_hash_hex;
            self.spoofer_total_bytes = m.total_bytes;
            self.spoofer_tracker_urls = m.tracker_urls;
        } else {
            self.spoofer_info_hash.clear();
            self.spoofer_total_bytes = 0;
        }

        // Pre-fill the tracker URL field from the first known URL (if any)
        if let Some(url) = self.spoofer_tracker_urls.first() {
            let url = url.clone();
            self.spoofer_tracker_input.value = url.clone();
            self.spoofer_tracker_input.cursor = url.len();
        }

        self.view = AppView::Spoofer;
    }

    /// Toggle between Downloader and Spoofer views.
    pub fn toggle_view(&mut self) {
        self.view = match self.view {
            AppView::Downloader => AppView::Spoofer,
            AppView::Spoofer => AppView::Downloader,
        };
    }

    /// Cycle through `spoofer_tracker_urls` and update the tracker input field.
    pub fn cycle_spoofer_tracker(&mut self) {
        if self.spoofer_tracker_urls.is_empty() {
            return;
        }
        self.spoofer_tracker_idx =
            (self.spoofer_tracker_idx + 1) % self.spoofer_tracker_urls.len();
        let url = self.spoofer_tracker_urls[self.spoofer_tracker_idx].clone();
        self.spoofer_tracker_input.value = url.clone();
        self.spoofer_tracker_input.cursor = url.len();
    }

    /// Cycle through available `ClientProfile` variants.
    pub fn cycle_spoofer_client(&mut self) {
        self.spoofer_client_idx =
            (self.spoofer_client_idx + 1) % ClientProfile::all().len();
    }

    pub fn selected_spoofer_client(&self) -> ClientProfile {
        ClientProfile::all()[self.spoofer_client_idx % ClientProfile::all().len()]
    }

    /// Return a clone of the current spoofer snapshot (if a session is running).
    pub fn spoofer_snapshot(&self) -> Option<SpooferSnapshot> {
        let handle = self.spoofer_handle.as_ref()?;
        handle.snapshot.lock().ok().map(|s| s.clone())
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
