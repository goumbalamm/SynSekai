#[cfg(test)]
mod tests {
    use crate::app::App;
    use ratatui::{Terminal, backend::TestBackend};

    fn buf_content(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn add_dialog_renders_without_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.open_add_dialog();
        terminal
            .draw(|f| super::render_add_dialog(f, &app, f.area()))
            .unwrap();
    }

    #[test]
    fn add_dialog_shows_typed_input() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.open_add_dialog();
        app.add_input.push('/');
        app.add_input.push('t');
        app.add_input.push('m');
        app.add_input.push('p');
        terminal
            .draw(|f| super::render_add_dialog(f, &app, f.area()))
            .unwrap();
        let content = buf_content(&terminal);
        assert!(content.contains("/tmp"));
    }

    #[test]
    fn confirm_remove_renders_without_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| super::render_confirm_remove(f, 0, false, f.area()))
            .unwrap();
    }

    #[test]
    fn confirm_remove_shows_torrent_number() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| super::render_confirm_remove(f, 2, false, f.area()))
            .unwrap();
        let content = buf_content(&terminal);
        assert!(content.contains('#'));
    }

    #[test]
    fn confirm_remove_delete_false_shows_no() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| super::render_confirm_remove(f, 0, false, f.area()))
            .unwrap();
        let content = buf_content(&terminal);
        assert!(content.contains("no"));
    }

    #[test]
    fn confirm_remove_delete_true_shows_yes() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| super::render_confirm_remove(f, 0, true, f.area()))
            .unwrap();
        let content = buf_content(&terminal);
        assert!(content.contains("YES"));
    }
}

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::App;

/// Center a rect of given dimensions within the parent.
fn centered_rect(width: u16, height: u16, parent: Rect) -> Rect {
    let x = parent.x + parent.width.saturating_sub(width) / 2;
    let y = parent.y + parent.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(parent.width),
        height: height.min(parent.height),
    }
}

pub fn render_add_dialog(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(60, 5, area);

    // Clear background
    f.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(1)])
        .split(popup_area);

    // Inner width of the input box (subtract 2 for left+right borders).
    let inner_w = chunks[0].width.saturating_sub(2) as usize;

    // Cursor column in display chars (byte offset → char count).
    let display_col = app.add_input.value[..app.add_input.cursor].chars().count();

    // Scroll offset in chars so the cursor is always in view.
    let scroll = display_col.saturating_sub(inner_w.saturating_sub(1));

    // Visible slice: skip `scroll` chars from the start.
    let visible: String = app.add_input.value.chars().skip(scroll).collect();

    let input_widget = Paragraph::new(visible.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Add Torrent (path or magnet) ")
                .style(Style::default().fg(Color::Yellow)),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(input_widget, chunks[0]);

    // Place terminal cursor at the scrolled position.
    f.set_cursor_position((
        chunks[0].x + 1 + (display_col - scroll) as u16,
        chunks[0].y + 1,
    ));

    let hint = Paragraph::new(Line::from(Span::styled(
        " [Enter] Add  [Esc] Cancel",
        Style::default().fg(Color::DarkGray),
    )));
    f.render_widget(hint, chunks[1]);
}

pub fn render_confirm_remove(f: &mut Frame, torrent_id: usize, delete_files: bool, area: Rect) {
    let popup_area = centered_rect(50, 6, area);
    f.render_widget(Clear, popup_area);

    let delete_style = if delete_files {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green)
    };

    let lines = vec![
        Line::from(format!(" Remove torrent #{}", torrent_id + 1)),
        Line::from(""),
        Line::from(vec![
            Span::raw(" Delete files: "),
            Span::styled(if delete_files { "YES" } else { "no" }, delete_style),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            " [Space] Toggle  [Enter] Confirm  [Esc] Cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let widget = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Confirm Remove ")
            .style(Style::default().fg(Color::Red)),
    );

    f.render_widget(widget, popup_area);
}
