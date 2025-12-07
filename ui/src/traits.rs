use ratatui::{Frame, layout::Rect};

/// 可在 TUI 中渲染的组件的 trait。
pub trait RenderUi {
    /// 在给定的框架和区域中渲染组件。
    fn render(&self, frame: &mut Frame, rect: Rect);
}
