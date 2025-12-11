mod log_view;

use ratatui::{Frame, prelude::Rect, style::Modifier, widgets::Borders};

use ui::{theme::Theme, traits::RenderUi};

use crate::tui::log_view::BlnLogView;

pub struct BlnTui<'a> {
    log_view: Option<BlnLogView<'a>>,
    theme: Theme,
}

impl<'a> RenderUi for BlnTui<'a> {
    fn render(&self, frame: &mut Frame, rect: Rect) {
        if let Some(ref log_view) = self.log_view {
            log_view.render(frame, rect);
        }
    }
}

impl<'a> Default for BlnTui<'a> {
    fn default() -> Self {
        let theme = Theme::default();
        Self {
            log_view: Some(
                BlnLogView::default()
                    .fg(theme.fg)
                    .bg(theme.bg)
                    .highlight_bg(theme.bg_highlight)
                    .highlight_fg(theme.fg)
                    .highlight_symbols(" ")
                    .highlight_modifier(Modifier::ITALIC)
                    .borders(Borders::NONE),
            ),
            theme,
        }
    }
}
