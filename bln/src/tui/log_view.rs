use ratatui::widgets::Borders;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::Line,
    widgets::{Block, List, ListState},
};
use std::fmt::Debug;
use ui::traits::RenderUi;

/// `BlnLogView` 是一个用于显示 BLN 协议解析后数据或日志的 TUI 组件.
/// 它维护一个固定容量的缓冲区, 以 FIFO (先进先出) 的方式管理条目,
/// 并以反向顺序渲染 (最新条目在底部), 类似于日志查看器.
#[derive(Debug)]
pub struct BlnLogView<'a> {
    /// 缓冲区最大容量, 超过此容量将移除最旧的条目.
    buffer_capacity: usize,
    /// 存储待显示的行数据.
    buf: Vec<Line<'a>>,
    /// 当前垂直滚动的偏移量, 用于控制列表的显示位置.
    vertical_scroll: usize,
    /// 列表的默认修饰符.
    modifier: Modifier,
    /// 列表项的默认前景色.
    fg: Color,
    /// 列表项的默认背景色.
    bg: Color,
    /// 选中项的前景色.
    highlight_fg: Color,
    /// 选中项的背景色.
    highlight_bg: Color,
    /// 选中项的修饰符.
    highlight_modifier: Modifier,
    /// 选中项的符号前缀.
    highlight_symbols: &'a str,
    /// 列表的标题.
    title: &'a str,
    /// 列表的边框样式.
    borders: Borders,
}

impl Default for BlnLogView<'_> {
    /// 创建一个带有默认配置的 `BlnLogView` 实例.
    ///
    /// 默认设置包括缓冲区容量, 初始滚动位置以及颜色和样式.
    fn default() -> Self {
        Self {
            buffer_capacity: Self::BUFFER_CAPACITY,
            buf: Vec::with_capacity(Self::BUFFER_CAPACITY),
            vertical_scroll: 0,
            fg: Color::Rgb(130, 170, 255),
            bg: Color::Rgb(34, 36, 54),
            modifier: Modifier::BOLD,
            highlight_fg: Color::Rgb(34, 36, 54),
            highlight_bg: Color::Rgb(130, 170, 255),
            highlight_modifier: Modifier::ITALIC,
            highlight_symbols: " ",
            title: "log_view",      // 默认标题
            borders: Borders::NONE, // 默认不显示边框
        }
    }
}

impl<'a> BlnLogView<'a> {
    /// 缓冲区容量常量, 定义了可存储的最大行数.
    const BUFFER_CAPACITY: usize = 20;

    /// 向 `BlnLogView` 的缓冲区添加一个新的行.
    ///
    /// 如果缓冲区已满, 将移除最旧的条目以保持容量. 新行总是添加到缓冲区顶部 (索引 0),
    /// 但在渲染时会反转显示, 所以新行在 UI 上会出现在底部.
    ///
    /// # 参数
    /// * `line` - 任何实现了 `Debug` trait 的类型, 其 `Debug` 输出将被格式化为一行.
    pub fn add_line<T>(&mut self, line: T)
    where
        T: Debug,
    {
        // 如果缓冲区已满, 移除最旧的条目. 由于新条目插入到开头, 最旧的在末尾.
        if self.buf.len() == self.buffer_capacity {
            self.buf.pop();
        }
        // 将新行插入到缓冲区开头, 这样在反转显示时它会出现在列表底部.
        self.buf.insert(0, Line::from(format!("{line:?}")));

        // 自动滚动到最新条目, 确保新行在列表底部可见.
        // `saturating_sub` 避免了在缓冲区为空或只有一项时出现负数.
        self.vertical_scroll = self.buf.len().saturating_sub(1);
    }

    /// 设置列表项的前景色.
    pub fn fg(mut self, fg: Color) -> Self {
        self.fg = fg;
        self
    }

    /// 设置列表项的背景色.
    pub fn bg(mut self, bg: Color) -> Self {
        self.bg = bg;
        self
    }

    /// 设置高亮选中项的前景色.
    pub fn highlight_fg(mut self, highlight_fg: Color) -> Self {
        self.highlight_fg = highlight_fg;
        self
    }

    /// 设置高亮选中项的背景色.
    pub fn highlight_bg(mut self, highlight_bg: Color) -> Self {
        self.highlight_bg = highlight_bg;
        self
    }

    /// 设置高亮选中项的修饰符.
    pub fn highlight_modifier(mut self, highlight_modifier: Modifier) -> Self {
        self.highlight_modifier = highlight_modifier;
        self
    }

    /// 设置高亮选中项的符号前缀.
    pub fn highlight_symbols(mut self, highlight_symbols: &'a str) -> Self {
        self.highlight_symbols = highlight_symbols;
        self
    }

    /// 设置列表的标题.
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    /// 设置列表的边框样式.
    pub fn borders(mut self, borders: Borders) -> Self {
        self.borders = borders;
        self
    }
}

impl<'a> RenderUi for BlnLogView<'a> {
    /// 在给定的 `Frame` 和 `Rect` 区域内渲染 `BlnLogView`.
    ///
    /// 此方法将缓冲区中的行反转, 以便最新的条目显示在列表底部.
    /// 它构建一个 `ratatui::widgets::List` 并使用内部状态进行滚动管理.
    ///
    /// # 参数
    /// * `frame` - `ratatui` 的绘制上下文.
    /// * `rect` - 渲染组件的屏幕区域.
    fn render(&self, frame: &mut Frame, rect: Rect) {
        // 反转缓冲区以便最新的条目 (在 `add_line` 中插入到 0 索引) 显示在列表底部.
        // `cloned()` 是因为 `Line` 不是 Copy, 需要克隆才能收集.
        let items = self.buf.iter().rev().cloned().collect::<Vec<_>>();

        // 配置 `ratatui::widgets::List` 的样式和行为.
        let list = List::new(items)
            .block(Block::bordered().title(self.title).borders(self.borders)) // 根据配置设置 Block 的标题和边框.
            .highlight_style(
                Style::default()
                    .add_modifier(self.highlight_modifier) // 应用高亮修饰符
                    .fg(self.highlight_fg) // 设置高亮前景色
                    .bg(self.highlight_bg), // 设置高亮背景色
            )
            .fg(self.fg) // 设置默认前景色
            .bg(self.bg) // 设置默认背景色
            .add_modifier(self.modifier) // 应用默认修饰符
            .highlight_symbol(self.highlight_symbols); // 设置高亮符号

        // 使用 `ListState` 来管理列表的滚动和选中状态.
        let mut list_state = ListState::default();
        // 设置选中项为 `vertical_scroll` 指向的索引, 通常是最新项.
        list_state.select(Some(self.vertical_scroll));
        // 渲染有状态的列表到帧中.
        frame.render_stateful_widget(list, rect, &mut list_state);
    }
}
