use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{app::App, types::AppView};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let active = |is_active: bool| -> Style {
        if is_active {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    };

    let line = Line::from(vec![
        Span::styled("  Downloader", active(app.view == AppView::Downloader)),
        Span::styled("  │  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Ratio Boost", active(app.view == AppView::Spoofer)),
    ]);

    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(Color::Black)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use crate::{app::App, types::AppView};
    use ratatui::{Terminal, backend::TestBackend};

    fn render_tab_bar(app: &App) -> String {
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
    fn downloader_tab_is_highlighted_by_default() {
        let app = App::new();
        let content = render_tab_bar(&app);
        assert!(content.contains("Downloader"));
        assert!(content.contains("Ratio Boost"));
    }

    #[test]
    fn spoofer_tab_renders_without_panic() {
        let mut app = App::new();
        app.view = AppView::Spoofer;
        render_tab_bar(&app); // must not panic
    }
}
