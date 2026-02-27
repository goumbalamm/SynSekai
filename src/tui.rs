use std::sync::Arc;

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use futures::StreamExt;
use ratatui::{backend::Backend, Terminal};
use tokio::time::{timeout, Duration};

use crate::{
    app::App,
    engine::TorrentEngine,
    types::AppMode,
    ui,
};

/// Strip surrounding quotes and whitespace from a pasted/dropped path.
fn clean_path(s: &str) -> String {
    s.trim().trim_matches('\'').trim_matches('"').to_owned()
}

/// Read a file path from the system clipboard.
///
/// On macOS, Finder file copies (Cmd+C) put a file *reference* on the clipboard,
/// not plain text — arboard only sees the filename. We ask osascript to convert
/// the reference to a full POSIX path first, then fall back to arboard plain text.
fn read_clipboard() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("osascript")
            .args(["-e", "POSIX path of (the clipboard as alias)"])
            .output()
            .ok();
        if let Some(out) = out {
            if out.status.success() {
                if let Ok(path) = String::from_utf8(out.stdout) {
                    let path = path.trim().to_owned();
                    if !path.is_empty() {
                        return Some(path);
                    }
                }
            }
        }
    }
    arboard::Clipboard::new().ok()?.get_text().ok()
}

/// Engine-level actions produced by pure key handlers.
#[derive(Debug, PartialEq)]
pub enum Action {
    Quit,
    Pause(usize),
    Resume(usize),
    AddTorrent(String),
    Remove { id: usize, delete_files: bool },
}

/// Pure key handler for Normal mode.
pub fn key_normal(app: &mut App, event: Event) -> Option<Action> {
    if let Event::Key(key) = event {
        match key.code {
            KeyCode::Char('q') => return Some(Action::Quit),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(Action::Quit);
            }
            KeyCode::Char('a') => app.open_add_dialog(),
            KeyCode::Char('p') => {
                if let Some(id) = app.selected_torrent_id() {
                    let is_paused = app
                        .torrents
                        .iter()
                        .find(|t| t.id == id)
                        .map(|t| t.status == crate::types::TorrentStatus::Paused)
                        .unwrap_or(false);
                    return Some(if is_paused {
                        Action::Resume(id)
                    } else {
                        Action::Pause(id)
                    });
                }
            }
            KeyCode::Char('d') => app.open_confirm_remove(),
            KeyCode::Down | KeyCode::Char('j') => app.select_next(),
            KeyCode::Up | KeyCode::Char('k') => app.select_prev(),
            _ => {}
        }
    }
    None
}

/// Pure key handler for the Add dialog.
pub fn key_add_dialog(app: &mut App, event: Event) -> Option<Action> {
    match event {
        Event::Paste(text) => {
            let path = clean_path(&text);
            app.add_input.value = path.clone();
            app.add_input.cursor = path.len();
            app.status_message = None;
        }
        Event::Key(key) => match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(app.add_input.value.clone());
                }
            }
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(text) = read_clipboard() {
                    let path = clean_path(&text);
                    app.add_input.value = path.clone();
                    app.add_input.cursor = path.len();
                    app.status_message = None;
                }
            }
            KeyCode::Esc => {
                app.status_message = None;
                app.dismiss_dialog();
            }
            KeyCode::Enter => {
                let input = clean_path(&app.add_input.value);
                if !input.is_empty() {
                    // Don't dismiss yet — apply_action will dismiss on success,
                    // or keep the dialog open so the user can fix the path on error.
                    app.status_message = None;
                    return Some(Action::AddTorrent(input));
                }
            }
            KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.add_input.move_word_left();
            }
            KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.add_input.move_word_right();
            }
            KeyCode::Left => app.add_input.move_left(),
            KeyCode::Right => app.add_input.move_right(),
            KeyCode::Home => {
                app.add_input.move_to_start();
            }
            KeyCode::End => {
                app.add_input.move_to_end();
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.add_input.move_to_start();
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.add_input.move_to_end();
            }
            KeyCode::Backspace if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.status_message = None;
                app.add_input.delete_word_back();
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.status_message = None;
                app.add_input.delete_word_back();
            }
            KeyCode::Delete if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.status_message = None;
                app.add_input.delete_word_forward();
            }
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.status_message = None;
                app.add_input.delete_to_end();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.status_message = None;
                app.add_input.delete_to_start();
            }
            KeyCode::Backspace => {
                app.status_message = None;
                app.add_input.backspace();
            }
            KeyCode::Char(c) => {
                app.status_message = None;
                app.add_input.push(c);
            }
            _ => {}
        },
        _ => {}
    }
    None
}

/// Pure key handler for the Confirm-Remove dialog.
pub fn key_confirm_remove(
    app: &mut App,
    event: Event,
    torrent_id: usize,
    delete_files: bool,
) -> Option<Action> {
    if let Event::Key(key) = event {
        match key.code {
            KeyCode::Esc => app.dismiss_dialog(),
            KeyCode::Char(' ') => app.toggle_delete_files(),
            KeyCode::Enter => {
                app.dismiss_dialog();
                return Some(Action::Remove {
                    id: torrent_id,
                    delete_files,
                });
            }
            _ => {}
        }
    }
    None
}

/// Main event loop. Generic over backend and event source for testability.
pub async fn event_loop<B, S>(
    terminal: &mut Terminal<B>,
    engine: Arc<TorrentEngine>,
    mut events: S,
) -> Result<()>
where
    B: Backend,
    S: futures::Stream<Item = std::io::Result<Event>> + Unpin,
{
    let mut app = App::new();

    while !app.should_quit {
        let rows = {
            let e = Arc::clone(&engine);
            tokio::task::spawn_blocking(move || e.list_torrents()).await?
        };
        app.update_torrents(rows);

        terminal.draw(|f| ui::render(f, &app))?;

        match timeout(Duration::from_millis(200), events.next()).await {
            Ok(Some(Ok(event))) => {
                let action = match app.mode.clone() {
                    AppMode::Normal => key_normal(&mut app, event),
                    AppMode::AddDialog => key_add_dialog(&mut app, event),
                    AppMode::ConfirmRemove { torrent_id, delete_files } => key_confirm_remove(&mut app, event, torrent_id, delete_files),
                };
                apply_action(&mut app, &engine, action).await;
            }
            Ok(Some(Err(e))) => return Err(e.into()),
            Ok(None) => break,
            Err(_) => {} // timeout — redraw with fresh data
        }
    }

    Ok(())
}

pub async fn apply_action(app: &mut App, engine: &Arc<TorrentEngine>, action: Option<Action>) {
    match action {
        Some(Action::Quit) => app.should_quit = true,
        Some(Action::Pause(id)) => {
            if let Err(e) = engine.pause(id).await {
                app.status_message = Some(format!("Error: {e}"));
            } else {
                app.status_message = None;
            }
        }
        Some(Action::Resume(id)) => {
            if let Err(e) = engine.resume(id).await {
                app.status_message = Some(format!("Error: {e}"));
            } else {
                app.status_message = None;
            }
        }
        Some(Action::AddTorrent(input)) => {
            app.status_message = Some("Adding…".into());
            match engine.add_torrent(&input).await {
                Ok(_) => {
                    app.dismiss_dialog();
                    app.status_message = Some("Torrent added.".into());
                }
                Err(e) => {
                    // Keep dialog open so user can fix the path
                    app.status_message = Some(format!("Error: {e}"));
                }
            }
        }
        Some(Action::Remove { id, delete_files }) => {
            match engine.remove(id, delete_files).await {
                Ok(_) => app.status_message = Some("Torrent removed.".into()),
                Err(e) => app.status_message = Some(format!("Error: {e}")),
            }
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        engine::TorrentEngine,
        types::{TorrentRow, TorrentStatus},
    };
    use crossterm::event::{KeyEvent, KeyEventKind, KeyEventState};
    use ratatui::{backend::TestBackend, Terminal};
    use std::path::PathBuf;
    use tempfile::TempDir;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn fixture_path() -> String {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/harry-potter.torrent")
            .to_str()
            .unwrap()
            .to_owned()
    }

    async fn make_engine() -> (Arc<TorrentEngine>, TempDir) {
        let dir = TempDir::new().unwrap();
        let engine = TorrentEngine::new_with_opts(dir.path().to_owned(), true)
            .await
            .unwrap();
        (Arc::new(engine), dir)
    }

    fn make_terminal() -> Terminal<TestBackend> {
        Terminal::new(TestBackend::new(80, 24)).unwrap()
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }

    fn ctrl(code: KeyCode) -> Event {
        Event::Key(KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }

    fn app_with_torrents(n: usize) -> App {
        let mut app = App::new();
        app.torrents = (0..n)
            .map(|i| TorrentRow {
                id: i,
                name: format!("t{i}"),
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

    // ── event_loop integration ────────────────────────────────────────────────
    // Smoke-tests only: the unit tests for key_* and apply_action cover individual
    // branches; these verify the loop wires up and terminates correctly.

    #[tokio::test]
    async fn event_loop_quits_on_q() {
        let (engine, _dir) = make_engine().await;
        let events = futures::stream::iter(vec![Ok::<Event, std::io::Error>(key(KeyCode::Char('q')))]);
        event_loop(&mut make_terminal(), engine, events).await.unwrap();
    }

    #[tokio::test]
    async fn event_loop_breaks_when_stream_ends() {
        let (engine, _dir) = make_engine().await;
        let events = futures::stream::empty::<std::io::Result<Event>>();
        event_loop(&mut make_terminal(), engine, events).await.unwrap();
    }

    #[tokio::test]
    async fn event_loop_propagates_io_error() {
        let (engine, _dir) = make_engine().await;
        let events = futures::stream::iter(vec![Err::<Event, std::io::Error>(std::io::Error::other("test error"))]);
        assert!(event_loop(&mut make_terminal(), engine, events).await.is_err());
    }

    // ── apply_action ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn apply_action_add_torrent_success() {
        let (engine, _dir) = make_engine().await;
        let mut app = App::new();
        app.open_add_dialog();
        apply_action(&mut app, &engine, Some(Action::AddTorrent(fixture_path()))).await;
        assert_eq!(app.status_message.as_deref(), Some("Torrent added."));
        assert_eq!(app.mode, AppMode::Normal); // dismissed on success
    }

    #[tokio::test]
    async fn apply_action_add_torrent_bad_path_sets_error() {
        let (engine, _dir) = make_engine().await;
        let mut app = App::new();
        app.open_add_dialog();
        apply_action(
            &mut app,
            &engine,
            Some(Action::AddTorrent("/nonexistent.torrent".into())),
        )
        .await;
        assert!(app.status_message.as_deref().unwrap_or("").starts_with("Error"));
        assert_eq!(app.mode, AppMode::AddDialog); // dialog stays open to fix the path
    }

    #[tokio::test]
    async fn apply_action_pause_success() {
        let (engine, _dir) = make_engine().await;
        engine.add_torrent(&fixture_path()).await.unwrap();
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if engine.list_torrents()[0].status != TorrentStatus::Initializing {
                break;
            }
        }
        let id = engine.list_torrents()[0].id;
        let mut app = App::new();
        apply_action(&mut app, &engine, Some(Action::Pause(id))).await;
        assert!(app.status_message.is_none());
    }

    #[tokio::test]
    async fn apply_action_pause_invalid_id_sets_error() {
        let (engine, _dir) = make_engine().await;
        let mut app = App::new();
        apply_action(&mut app, &engine, Some(Action::Pause(999))).await;
        assert!(app.status_message.as_deref().unwrap_or("").starts_with("Error"));
    }

    #[tokio::test]
    async fn apply_action_resume_success() {
        let (engine, _dir) = make_engine().await;
        engine.add_torrent(&fixture_path()).await.unwrap();
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if engine.list_torrents()[0].status != TorrentStatus::Initializing {
                break;
            }
        }
        let id = engine.list_torrents()[0].id;
        engine.pause(id).await.unwrap();
        let mut app = App::new();
        apply_action(&mut app, &engine, Some(Action::Resume(id))).await;
        assert!(app.status_message.is_none());
    }

    #[tokio::test]
    async fn apply_action_resume_invalid_id_sets_error() {
        let (engine, _dir) = make_engine().await;
        let mut app = App::new();
        apply_action(&mut app, &engine, Some(Action::Resume(999))).await;
        assert!(app.status_message.as_deref().unwrap_or("").starts_with("Error"));
    }

    #[tokio::test]
    async fn apply_action_remove_keep_files() {
        let (engine, _dir) = make_engine().await;
        engine.add_torrent(&fixture_path()).await.unwrap();
        let id = engine.list_torrents()[0].id;
        let mut app = App::new();
        apply_action(&mut app, &engine, Some(Action::Remove { id, delete_files: false })).await;
        assert_eq!(app.status_message.as_deref(), Some("Torrent removed."));
        assert!(engine.list_torrents().is_empty());
    }

    #[tokio::test]
    async fn apply_action_remove_delete_files() {
        let (engine, _dir) = make_engine().await;
        engine.add_torrent(&fixture_path()).await.unwrap();
        let id = engine.list_torrents()[0].id;
        let mut app = App::new();
        apply_action(&mut app, &engine, Some(Action::Remove { id, delete_files: true })).await;
        assert_eq!(app.status_message.as_deref(), Some("Torrent removed."));
    }

    #[tokio::test]
    async fn apply_action_remove_invalid_id_sets_error() {
        let (engine, _dir) = make_engine().await;
        let mut app = App::new();
        apply_action(&mut app, &engine, Some(Action::Remove { id: 999, delete_files: false })).await;
        assert!(app.status_message.as_deref().unwrap_or("").starts_with("Error"));
    }

    // ── key_normal ────────────────────────────────────────────────────────────

    #[test]
    fn normal_q_quits() {
        let mut app = App::new();
        assert_eq!(key_normal(&mut app, key(KeyCode::Char('q'))), Some(Action::Quit));
    }

    #[test]
    fn normal_ctrl_c_quits() {
        let mut app = App::new();
        assert_eq!(key_normal(&mut app, ctrl(KeyCode::Char('c'))), Some(Action::Quit));
    }

    #[test]
    fn normal_a_opens_add_dialog() {
        let mut app = App::new();
        assert_eq!(key_normal(&mut app, key(KeyCode::Char('a'))), None);
        assert_eq!(app.mode, AppMode::AddDialog);
    }

    #[test]
    fn normal_d_opens_confirm_remove() {
        let mut app = app_with_torrents(1);
        assert_eq!(key_normal(&mut app, key(KeyCode::Char('d'))), None);
        assert!(matches!(app.mode, AppMode::ConfirmRemove { .. }));
    }

    #[test]
    fn normal_j_moves_down() {
        let mut app = app_with_torrents(3);
        assert_eq!(key_normal(&mut app, key(KeyCode::Char('j'))), None);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn normal_k_moves_up() {
        let mut app = app_with_torrents(3);
        app.selected = 2;
        assert_eq!(key_normal(&mut app, key(KeyCode::Char('k'))), None);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn normal_down_moves_down() {
        let mut app = app_with_torrents(2);
        key_normal(&mut app, key(KeyCode::Down));
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn normal_up_moves_up() {
        let mut app = app_with_torrents(2);
        app.selected = 1;
        key_normal(&mut app, key(KeyCode::Up));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn normal_p_on_downloading_returns_pause() {
        let mut app = app_with_torrents(1);
        assert_eq!(key_normal(&mut app, key(KeyCode::Char('p'))), Some(Action::Pause(0)));
    }

    #[test]
    fn normal_p_on_paused_returns_resume() {
        let mut app = app_with_torrents(1);
        app.torrents[0].status = TorrentStatus::Paused;
        assert_eq!(key_normal(&mut app, key(KeyCode::Char('p'))), Some(Action::Resume(0)));
    }

    #[test]
    fn normal_p_with_no_torrents_returns_none() {
        let mut app = App::new();
        assert_eq!(key_normal(&mut app, key(KeyCode::Char('p'))), None);
    }

    #[test]
    fn normal_unknown_key_returns_none() {
        let mut app = App::new();
        assert_eq!(key_normal(&mut app, key(KeyCode::F(1))), None);
    }

    #[test]
    fn normal_ignores_non_key_event() {
        let mut app = App::new();
        assert_eq!(key_normal(&mut app, Event::FocusGained), None);
    }

    // ── key_add_dialog ────────────────────────────────────────────────────────

    #[test]
    fn add_dialog_esc_dismisses() {
        let mut app = App::new();
        app.open_add_dialog();
        assert_eq!(key_add_dialog(&mut app, key(KeyCode::Esc)), None);
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn add_dialog_enter_with_input_returns_add_torrent() {
        let mut app = App::new();
        app.open_add_dialog();
        app.add_input.value = "magnet:?xt=test".into();
        app.add_input.cursor = app.add_input.value.len();
        let action = key_add_dialog(&mut app, key(KeyCode::Enter));
        assert_eq!(action, Some(Action::AddTorrent("magnet:?xt=test".into())));
        // Dialog stays open — apply_action dismisses it on success
        assert_eq!(app.mode, AppMode::AddDialog);
    }

    #[test]
    fn add_dialog_enter_with_empty_input_does_nothing() {
        let mut app = App::new();
        app.open_add_dialog();
        assert_eq!(key_add_dialog(&mut app, key(KeyCode::Enter)), None);
        assert_eq!(app.mode, AppMode::AddDialog);
    }

    #[test]
    fn add_dialog_char_appends() {
        let mut app = App::new();
        app.open_add_dialog();
        key_add_dialog(&mut app, key(KeyCode::Char('x')));
        assert_eq!(app.add_input.value, "x");
    }

    #[test]
    fn add_dialog_left_moves_cursor() {
        let mut app = App::new();
        app.open_add_dialog();
        app.add_input.push('a');
        app.add_input.push('b');
        key_add_dialog(&mut app, key(KeyCode::Left));
        assert_eq!(app.add_input.cursor, 1);
    }

    #[test]
    fn add_dialog_right_moves_cursor() {
        let mut app = App::new();
        app.open_add_dialog();
        app.add_input.push('a');
        app.add_input.push('b');
        key_add_dialog(&mut app, key(KeyCode::Left));
        key_add_dialog(&mut app, key(KeyCode::Right));
        assert_eq!(app.add_input.cursor, 2);
    }

    #[test]
    fn add_dialog_backspace_removes_char() {
        let mut app = App::new();
        app.open_add_dialog();
        app.add_input.push('h');
        app.add_input.push('i');
        key_add_dialog(&mut app, key(KeyCode::Backspace));
        assert_eq!(app.add_input.value, "h");
    }

    #[test]
    fn add_dialog_ctrl_c_does_not_panic() {
        let mut app = App::new();
        app.open_add_dialog();
        app.add_input.push('x');
        // Clipboard may not be available in CI — must not panic either way
        key_add_dialog(&mut app, ctrl(KeyCode::Char('c')));
    }

    #[test]
    fn add_dialog_ctrl_v_does_not_panic() {
        let mut app = App::new();
        app.open_add_dialog();
        // Clipboard may not be available in CI — must not panic either way
        key_add_dialog(&mut app, ctrl(KeyCode::Char('v')));
    }

    #[test]
    fn add_dialog_paste_sets_input() {
        let mut app = App::new();
        app.open_add_dialog();
        app.add_input.push('x'); // pre-existing content replaced by paste
        assert_eq!(key_add_dialog(&mut app, Event::Paste("/tmp/my file.torrent".into())), None);
        assert_eq!(app.add_input.value, "/tmp/my file.torrent");
        assert_eq!(app.add_input.cursor, "/tmp/my file.torrent".len());
    }

    #[test]
    fn add_dialog_paste_trims_whitespace() {
        let mut app = App::new();
        app.open_add_dialog();
        key_add_dialog(&mut app, Event::Paste("  /tmp/test.torrent\n".into()));
        assert_eq!(app.add_input.value, "/tmp/test.torrent");
    }

    #[test]
    fn add_dialog_paste_strips_single_quotes() {
        let mut app = App::new();
        app.open_add_dialog();
        key_add_dialog(&mut app, Event::Paste("'/Users/goumbala/Downloads/harry-potter.torrent'".into()));
        assert_eq!(app.add_input.value, "/Users/goumbala/Downloads/harry-potter.torrent");
    }

    #[test]
    fn add_dialog_paste_strips_double_quotes() {
        let mut app = App::new();
        app.open_add_dialog();
        key_add_dialog(&mut app, Event::Paste("\"/tmp/my torrent.torrent\"".into()));
        assert_eq!(app.add_input.value, "/tmp/my torrent.torrent");
    }

    fn ctrl_code(code: KeyCode) -> Event {
        Event::Key(KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }

    #[test]
    fn add_dialog_ctrl_left_moves_word_left() {
        let mut app = App::new();
        app.open_add_dialog();
        app.add_input.value = "hello world".into();
        app.add_input.cursor = 11;
        key_add_dialog(&mut app, ctrl_code(KeyCode::Left));
        assert_eq!(app.add_input.cursor, 6);
    }

    #[test]
    fn add_dialog_ctrl_right_moves_word_right() {
        let mut app = App::new();
        app.open_add_dialog();
        app.add_input.value = "hello world".into();
        app.add_input.cursor = 0;
        key_add_dialog(&mut app, ctrl_code(KeyCode::Right));
        assert_eq!(app.add_input.cursor, 5);
    }

    #[test]
    fn add_dialog_home_moves_to_start() {
        let mut app = App::new();
        app.open_add_dialog();
        app.add_input.value = "hello".into();
        app.add_input.cursor = 5;
        key_add_dialog(&mut app, key(KeyCode::Home));
        assert_eq!(app.add_input.cursor, 0);
    }

    #[test]
    fn add_dialog_end_moves_to_end() {
        let mut app = App::new();
        app.open_add_dialog();
        app.add_input.value = "hello".into();
        app.add_input.cursor = 0;
        key_add_dialog(&mut app, key(KeyCode::End));
        assert_eq!(app.add_input.cursor, 5);
    }

    #[test]
    fn add_dialog_ctrl_a_moves_to_start() {
        let mut app = App::new();
        app.open_add_dialog();
        app.add_input.value = "hello".into();
        app.add_input.cursor = 5;
        key_add_dialog(&mut app, ctrl(KeyCode::Char('a')));
        assert_eq!(app.add_input.cursor, 0);
    }

    #[test]
    fn add_dialog_ctrl_e_moves_to_end() {
        let mut app = App::new();
        app.open_add_dialog();
        app.add_input.value = "hello".into();
        app.add_input.cursor = 0;
        key_add_dialog(&mut app, ctrl(KeyCode::Char('e')));
        assert_eq!(app.add_input.cursor, 5);
    }

    #[test]
    fn add_dialog_ctrl_backspace_deletes_word_back() {
        let mut app = App::new();
        app.open_add_dialog();
        app.add_input.value = "hello world".into();
        app.add_input.cursor = 11;
        key_add_dialog(&mut app, ctrl_code(KeyCode::Backspace));
        assert_eq!(app.add_input.value, "hello ");
    }

    #[test]
    fn add_dialog_ctrl_w_deletes_word_back() {
        let mut app = App::new();
        app.open_add_dialog();
        app.add_input.value = "hello world".into();
        app.add_input.cursor = 11;
        key_add_dialog(&mut app, ctrl(KeyCode::Char('w')));
        assert_eq!(app.add_input.value, "hello ");
    }

    #[test]
    fn add_dialog_ctrl_delete_deletes_word_forward() {
        let mut app = App::new();
        app.open_add_dialog();
        app.add_input.value = "hello world".into();
        app.add_input.cursor = 0;
        key_add_dialog(&mut app, ctrl_code(KeyCode::Delete));
        assert_eq!(app.add_input.value, " world");
    }

    #[test]
    fn add_dialog_ctrl_k_deletes_to_end() {
        let mut app = App::new();
        app.open_add_dialog();
        app.add_input.value = "hello world".into();
        app.add_input.cursor = 5;
        key_add_dialog(&mut app, ctrl(KeyCode::Char('k')));
        assert_eq!(app.add_input.value, "hello");
    }

    #[test]
    fn add_dialog_ctrl_u_deletes_to_start() {
        let mut app = App::new();
        app.open_add_dialog();
        app.add_input.value = "hello world".into();
        app.add_input.cursor = 6;
        key_add_dialog(&mut app, ctrl(KeyCode::Char('u')));
        assert_eq!(app.add_input.value, "world");
        assert_eq!(app.add_input.cursor, 0);
    }

    #[test]
    fn add_dialog_ignores_non_key_event() {
        let mut app = App::new();
        app.open_add_dialog();
        assert_eq!(key_add_dialog(&mut app, Event::FocusGained), None);
    }

    #[test]
    fn add_dialog_unknown_key_returns_none() {
        let mut app = App::new();
        app.open_add_dialog();
        assert_eq!(key_add_dialog(&mut app, key(KeyCode::F(2))), None);
    }

    // ── key_confirm_remove ────────────────────────────────────────────────────

    #[test]
    fn confirm_esc_dismisses() {
        let mut app = app_with_torrents(1);
        app.open_confirm_remove();
        assert_eq!(key_confirm_remove(&mut app, key(KeyCode::Esc), 0, false), None);
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn confirm_space_toggles_delete_files() {
        let mut app = app_with_torrents(1);
        app.open_confirm_remove();
        assert_eq!(key_confirm_remove(&mut app, key(KeyCode::Char(' ')), 0, false), None);
        assert!(matches!(app.mode, AppMode::ConfirmRemove { delete_files: true, .. }));
    }

    #[test]
    fn confirm_enter_returns_remove_keep_files() {
        let mut app = app_with_torrents(1);
        app.open_confirm_remove();
        let action = key_confirm_remove(&mut app, key(KeyCode::Enter), 0, false);
        assert_eq!(action, Some(Action::Remove { id: 0, delete_files: false }));
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn confirm_enter_returns_remove_delete_files() {
        let mut app = app_with_torrents(1);
        app.open_confirm_remove();
        let action = key_confirm_remove(&mut app, key(KeyCode::Enter), 0, true);
        assert_eq!(action, Some(Action::Remove { id: 0, delete_files: true }));
    }

    #[test]
    fn confirm_ignores_non_key_event() {
        let mut app = app_with_torrents(1);
        app.open_confirm_remove();
        assert_eq!(key_confirm_remove(&mut app, Event::FocusGained, 0, false), None);
    }

    #[test]
    fn confirm_unknown_key_returns_none() {
        let mut app = app_with_torrents(1);
        app.open_confirm_remove();
        assert_eq!(key_confirm_remove(&mut app, key(KeyCode::F(3)), 0, false), None);
    }
}
