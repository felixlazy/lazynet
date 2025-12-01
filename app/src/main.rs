use app::app::LazyApp;
use color_eyre::Result;
use tracing_appender::{non_blocking, rolling};
use tracing_error::ErrorLayer;
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

#[tokio::main]
async fn main() -> Result<()> {
    // 安装 color_eyre 错误处理
    color_eyre::install()?;

    // -- 文件日志 --
    let file_appender = rolling::daily("logs", "lazy_net_log");
    let (non_blocking_writer, _guard) = non_blocking(file_appender);
    let file_layer = fmt::layer()
        .with_writer(non_blocking_writer)
        .with_ansi(false)
        .with_span_events(FmtSpan::CLOSE);

    // -- 控制台日志 (条件性) --
    // 检查环境变量 CONSOLE_LOG，如果设置，则启用控制台日志
    let console_layer = if std::env::var("CONSOLE_LOG").is_err() {
        None
    } else {
        Some(fmt::layer().with_writer(std::io::stdout).with_ansi(true))
    };

    // 初始化 tracing 订阅器
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("INFO")))
        .with(ErrorLayer::default())
        .with(file_layer)
        .with(console_layer)
        .init();

    let mut app = LazyApp::connect("192.168.1.103:5006").await?;
    app.start();
    app.run().await?;
    Ok(())
}
