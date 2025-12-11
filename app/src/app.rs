use bytes::BytesMut;
use color_eyre::eyre::Result;
use protocol::{
    traits::{FrameGenerator, ParseProtocol, ProtocolSplit},
    types::Command,
};
use stream::traits::{AsyncFrameReader, AsyncFrameWriter, AsyncStreamSplit};

use tokio::sync::mpsc;
use tracing::{info, instrument};
use ui::traits::RenderUi;

/// `LazyApp` 是一个封装了应用核心逻辑的结构体.
///
/// 它现在作为一个高级别的"引导程序" (bootstrapper), 负责初始化网络流和协议,
/// 并通过其 `run` 方法启动核心的读/写异步任务.
pub struct LazyApp<P, Io, U>
where
    P: ProtocolSplit,
    Io: AsyncStreamSplit,
    U: RenderUi,
{
    /// 用于异步读写数据帧的网络流.
    stream: Io,
    /// 实现了 `ProtocolSplit` 的协议处理器.
    protocol: P,
    ui: U,
    interval: tokio::time::Interval,
}

impl<P, Io, U> LazyApp<P, Io, U>
where
    P: ProtocolSplit,
    Io: AsyncStreamSplit,
    // 在这里添加一个trait, 用于更新UI
    U: RenderUi + Send + 'static,
{
    /// 创建一个新的 `LazyApp` 实例.
    ///
    /// # 参数
    /// - `stream`: 一个实现了 `AsyncStreamSplit` 的网络流.
    /// - `protocol`: 一个实现了 `ProtocolSplit` 的协议处理器.
    ///
    /// # 返回
    /// 一个新的 `LazyApp` 实例, 准备好通过调用 `.run()` 来启动.
    pub fn new(stream: Io, protocol: P, ui: U, duration: tokio::time::Duration) -> Self {
        Self {
            stream,
            protocol,
            ui,
            interval: tokio::time::interval(duration),
        }
    }

    /// 运行应用的主循环.
    /// # 返回
    /// 如果 `tokio::join!` 正常返回 (即读写任务都已结束), 返回 `Ok(())`.
    /// 如果任务返回错误, 错误会向上传播.
    #[instrument(skip(self), err)]
    pub async fn run(self) -> Result<()>
    where
        // 为 `tokio::spawn` 约束生命周期和 `Send` trait
        Io::Writer: AsyncFrameWriter + Send + 'static,
        Io::Reader: AsyncFrameReader + Send + 'static,
        P::Encoder: FrameGenerator + Send + 'static,
        P::Decode: ParseProtocol + Send + 'static,
        // U: HandleCommand,
    {
        let mut terminal = ratatui::init();
        // 1. 分离网络流和协议处理器
        let (mut stream_reader, mut stream_writer) = self.stream.into_split();
        let (mut protocol_decoder, protocol_encoder) = self.protocol.into_split();
        let ui = self.ui;
        let mut interval = self.interval;

        // 2. 创建用于外部与 Writer Task 通信的通道
        let (_command_sender, mut command_receiver) = mpsc::channel::<Command>(10);
        let (ui_sender, mut ui_receiver) = mpsc::channel::<Command>(10);

        // --- Writer 任务 ---
        let writer_handle = tokio::spawn(async move {
            while let Some(command) = command_receiver.recv().await {
                match protocol_encoder.create_frame(command) {
                    Ok(frame) => {
                        if let Err(e) = stream_writer.write_frame(&frame).await {
                            info!("[Writer Task] Failed to write frame: {}", e);
                        }
                    }
                    Err(e) => info!("[Writer Task] Failed to create frame: {}", e),
                }
            }
        });

        // --- Reader 任务 ---
        let reader_handle = tokio::spawn(async move {
            let mut buf = BytesMut::with_capacity(1024);
            let sender = ui_sender.clone();
            // 这个循环会一直运行, 直到读取出错或连接关闭
            loop {
                // 从流中读取数据到缓冲区
                match stream_reader.read_frame(&mut buf).await {
                    Ok(_len) => {
                        if let Some(commands) = protocol_decoder.parse_protocol_frame(&mut buf) {
                            for command in commands {
                                let _ = sender.try_send(command).map_err(|f| {
                                    info!("[Reader Task] Failed to send command: {}", f);
                                });
                            }
                        }
                    }
                    Err(e) => {
                        info!("[Reader Task] Failed to read from stream: {}", e);
                        break;
                    }
                }
            }
        });
        let ui_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                     _ = interval.tick() => {
                        if let Err(e) = terminal.draw(|frame| {
                            ui.render(frame, frame.area());
                        }) {
                            info!("[UI Task] Failed to draw: {}", e);
                            break;
                        }
                    },
                    recv = ui_receiver.recv() => {
                        if let Some(_command)=recv{
                            //TODO: 增加ui的处理
                        }
                        else{
                            break;
                        }

                        // ui.add_line(command);
                    }
                }
            }
        });

        // 3. 等待两个任务完成.
        // 在一个真实的应用中, 它们会一直运行, 直到发生错误或收到关闭信号.
        let _ = tokio::join!(reader_handle, writer_handle, ui_handle);

        ratatui::restore();

        Ok(())
    }
}

