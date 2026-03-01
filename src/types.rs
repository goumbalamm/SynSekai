/// A row in the torrent table.
#[derive(Debug, Clone)]
pub struct TorrentRow {
    pub id: usize,
    pub name: String,
    pub total_bytes: u64,
    pub progress_pct: f32, // 0.0–100.0
    pub down_speed_bps: u64,
    pub peers_live: usize,
    pub peers_seen: usize,
    pub status: TorrentStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TorrentStatus {
    Initializing,
    Downloading,
    Seeding,
    Paused,
    Error(String),
}

impl TorrentStatus {
    pub fn as_str(&self) -> &str {
        match self {
            TorrentStatus::Initializing => "Init",
            TorrentStatus::Downloading => "DL",
            TorrentStatus::Seeding => "Seed",
            TorrentStatus::Paused => "Paused",
            TorrentStatus::Error(_) => "Error",
        }
    }
}

#[derive(Clone, Default, PartialEq, Debug)]
pub enum AppView {
    #[default]
    Downloader,
    Spoofer,
}

#[derive(Clone, Default, PartialEq, Debug)]
pub enum AppMode {
    #[default]
    Normal,
    AddDialog,
    ConfirmRemove {
        torrent_id: usize,
        delete_files: bool,
    },
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum ClientProfile {
    #[default]
    QBittorrent4_6,
    UTorrent3_3_2,
    Transmission2_92,
}

impl ClientProfile {
    pub fn peer_id_prefix(self) -> &'static str {
        match self {
            Self::QBittorrent4_6 => "-qB4600-",
            Self::UTorrent3_3_2 => "-UT3320-",
            Self::Transmission2_92 => "-TR292-",
        }
    }

    pub fn user_agent(self) -> &'static str {
        match self {
            Self::QBittorrent4_6 => "qBittorrent/4.6.0",
            Self::UTorrent3_3_2 => "uTorrent/3320(25302)",
            Self::Transmission2_92 => "Transmission/2.92",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::QBittorrent4_6 => "qBittorrent 4.6",
            Self::UTorrent3_3_2 => "uTorrent 3.3.2",
            Self::Transmission2_92 => "Transmission 2.92",
        }
    }

    pub fn all() -> &'static [ClientProfile] {
        &[
            Self::QBittorrent4_6,
            Self::UTorrent3_3_2,
            Self::Transmission2_92,
        ]
    }
}

#[derive(Clone, Debug, Default)]
pub struct SpooferSnapshot {
    pub uploaded: u64,
    pub downloaded: u64,
    pub seeders: Option<i64>,
    pub leechers: Option<i64>,
    pub interval_secs: u64,
    pub countdown_secs: u64,
    pub running: bool,
    pub last_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SpooferField {
    #[default]
    UploadRate,
    DownloadRate,
    TrackerUrl,
}

#[derive(Default, Debug, Clone)]
pub struct InputState {
    pub value: String,
    pub cursor: usize,
}

impl InputState {
    pub fn push(&mut self, c: char) {
        self.value.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        // Find the char boundary before cursor
        let mut idx = self.cursor - 1;
        while !self.value.is_char_boundary(idx) {
            idx -= 1;
        }
        self.value.remove(idx);
        self.cursor = idx;
    }

    pub fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut idx = self.cursor - 1;
        while !self.value.is_char_boundary(idx) {
            idx -= 1;
        }
        self.cursor = idx;
    }

    pub fn move_right(&mut self) {
        if self.cursor >= self.value.len() {
            return;
        }
        let mut idx = self.cursor + 1;
        while idx < self.value.len() && !self.value.is_char_boundary(idx) {
            idx += 1;
        }
        self.cursor = idx;
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    pub fn move_to_start(&mut self) {
        self.cursor = 0;
    }

    pub fn move_to_end(&mut self) {
        self.cursor = self.value.len();
    }

    /// Move cursor left by one word (bash/readline Ctrl+Left).
    /// Skips trailing non-word chars then skips the word.
    pub fn move_word_left(&mut self) {
        let s = &self.value[..self.cursor];
        let chars: Vec<(usize, char)> = s.char_indices().collect();
        let mut i = chars.len();
        while i > 0 && !chars[i - 1].1.is_alphanumeric() {
            i -= 1;
        }
        while i > 0 && chars[i - 1].1.is_alphanumeric() {
            i -= 1;
        }
        self.cursor = if i == 0 { 0 } else { chars[i].0 };
    }

    /// Move cursor right by one word (bash/readline Ctrl+Right).
    pub fn move_word_right(&mut self) {
        let s = &self.value[self.cursor..];
        let chars: Vec<(usize, char)> = s.char_indices().collect();
        let mut i = 0;
        // skip non-alphanumeric
        while i < chars.len() && !chars[i].1.is_alphanumeric() {
            i += 1;
        }
        // skip word chars
        while i < chars.len() && chars[i].1.is_alphanumeric() {
            i += 1;
        }
        let offset = if i < chars.len() { chars[i].0 } else { s.len() };
        self.cursor += offset;
    }

    /// Delete one word backwards (Ctrl+Backspace / Ctrl+W).
    pub fn delete_word_back(&mut self) {
        let old_cursor = self.cursor;
        self.move_word_left();
        self.value.drain(self.cursor..old_cursor);
    }

    /// Delete one word forwards (Ctrl+Delete).
    pub fn delete_word_forward(&mut self) {
        let start = self.cursor;
        let s = &self.value[start..];
        let chars: Vec<(usize, char)> = s.char_indices().collect();
        let mut i = 0;
        // skip non-alphanumeric
        while i < chars.len() && !chars[i].1.is_alphanumeric() {
            i += 1;
        }
        // skip word chars
        while i < chars.len() && chars[i].1.is_alphanumeric() {
            i += 1;
        }
        let end_offset = if i < chars.len() { chars[i].0 } else { s.len() };
        self.value.drain(start..start + end_offset);
    }

    /// Delete from cursor to end of line (Ctrl+K).
    pub fn delete_to_end(&mut self) {
        self.value.truncate(self.cursor);
    }

    /// Delete from start of line to cursor (Ctrl+U).
    pub fn delete_to_start(&mut self) {
        self.value.drain(..self.cursor);
        self.cursor = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_state_push_and_backspace() {
        let mut s = InputState::default();
        s.push('h');
        s.push('i');
        assert_eq!(s.value, "hi");
        s.backspace();
        assert_eq!(s.value, "h");
    }

    #[test]
    fn input_state_clear() {
        let mut s = InputState {
            value: "hello".into(),
            cursor: 5,
        };
        s.clear();
        assert_eq!(s.value, "");
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn torrent_status_as_str() {
        assert_eq!(TorrentStatus::Initializing.as_str(), "Init");
        assert_eq!(TorrentStatus::Downloading.as_str(), "DL");
        assert_eq!(TorrentStatus::Seeding.as_str(), "Seed");
        assert_eq!(TorrentStatus::Paused.as_str(), "Paused");
        assert_eq!(TorrentStatus::Error("oops".into()).as_str(), "Error");
    }

    #[test]
    fn input_state_push_multibyte() {
        let mut s = InputState::default();
        s.push('é'); // 2-byte UTF-8
        assert_eq!(s.value, "é");
        assert_eq!(s.cursor, 2);
        s.backspace();
        assert_eq!(s.value, "");
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn backspace_on_empty_is_noop() {
        let mut s = InputState::default();
        s.backspace(); // should not panic
        assert_eq!(s.value, "");
    }

    #[test]
    fn move_left_and_right() {
        let mut s = InputState::default();
        s.push('a');
        s.push('b');
        s.push('c');
        assert_eq!(s.cursor, 3);
        s.move_left();
        assert_eq!(s.cursor, 2);
        s.move_right();
        assert_eq!(s.cursor, 3);
    }

    #[test]
    fn move_left_clamps_at_zero() {
        let mut s = InputState::default();
        s.move_left();
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn move_right_clamps_at_end() {
        let mut s = InputState::default();
        s.push('x');
        s.move_right(); // already at end
        assert_eq!(s.cursor, 1);
    }

    #[test]
    fn move_left_right_multibyte() {
        let mut s = InputState::default();
        s.push('é'); // 2 bytes
        assert_eq!(s.cursor, 2);
        s.move_left();
        assert_eq!(s.cursor, 0);
        s.move_right();
        assert_eq!(s.cursor, 2);
    }

    #[test]
    fn move_to_start_and_end() {
        let mut s = InputState::default();
        s.push('a');
        s.push('b');
        s.push('c');
        s.move_to_start();
        assert_eq!(s.cursor, 0);
        s.move_to_end();
        assert_eq!(s.cursor, 3);
    }

    #[test]
    fn move_word_right_skips_word() {
        let mut s = InputState {
            value: "hello world".into(),
            cursor: 0,
        };
        s.move_word_right();
        assert_eq!(s.cursor, 5); // after "hello"
        s.move_word_right();
        assert_eq!(s.cursor, 11); // after "world"
    }

    #[test]
    fn move_word_left_skips_word() {
        let mut s = InputState {
            value: "hello world".into(),
            cursor: 11,
        };
        s.move_word_left();
        assert_eq!(s.cursor, 6); // start of "world"
        s.move_word_left();
        assert_eq!(s.cursor, 0); // start of "hello"
    }

    #[test]
    fn move_word_left_from_middle_of_word() {
        let mut s = InputState {
            value: "hello".into(),
            cursor: 3,
        };
        s.move_word_left();
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn move_word_right_clamps_at_end() {
        let mut s = InputState {
            value: "hello".into(),
            cursor: 5,
        };
        s.move_word_right();
        assert_eq!(s.cursor, 5);
    }

    #[test]
    fn move_word_left_clamps_at_start() {
        let mut s = InputState {
            value: "hello".into(),
            cursor: 0,
        };
        s.move_word_left();
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn delete_word_back_removes_word() {
        let mut s = InputState {
            value: "hello world".into(),
            cursor: 11,
        };
        s.delete_word_back();
        assert_eq!(s.value, "hello ");
        assert_eq!(s.cursor, 6);
    }

    #[test]
    fn delete_word_back_skips_spaces_first() {
        let mut s = InputState {
            value: "hello   ".into(),
            cursor: 8,
        };
        s.delete_word_back();
        assert_eq!(s.value, "");
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn delete_word_forward_removes_word() {
        let mut s = InputState {
            value: "hello world".into(),
            cursor: 0,
        };
        s.delete_word_forward();
        assert_eq!(s.value, " world");
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn delete_word_forward_skips_spaces_first() {
        let mut s = InputState {
            value: "   world".into(),
            cursor: 0,
        };
        s.delete_word_forward();
        assert_eq!(s.value, "");
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn delete_to_end_truncates() {
        let mut s = InputState {
            value: "hello world".into(),
            cursor: 5,
        };
        s.delete_to_end();
        assert_eq!(s.value, "hello");
        assert_eq!(s.cursor, 5);
    }

    #[test]
    fn delete_to_start_clears_before_cursor() {
        let mut s = InputState {
            value: "hello world".into(),
            cursor: 6,
        };
        s.delete_to_start();
        assert_eq!(s.value, "world");
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn delete_word_back_on_empty_is_noop() {
        let mut s = InputState::default();
        s.delete_word_back();
        assert_eq!(s.value, "");
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn delete_word_forward_on_empty_is_noop() {
        let mut s = InputState::default();
        s.delete_word_forward();
        assert_eq!(s.value, "");
    }
}
