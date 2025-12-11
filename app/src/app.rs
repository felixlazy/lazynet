use bytes::BytesMut;
use color_eyre::eyre::Result;
use protocol::{
    traits::{FrameGenerator, ParseProtocol, ProtocolSplit},
    types::Command,
};
use stream::traits::{AsyncFrameReader, AsyncFrameWriter, AsyncStreamSplit};

use tokio::sync::mpsc;
use tracing::instrument;

/// `LazyApp` 是一个封装了应用核心逻辑的结构体.
///
/// 它现在作为一个高级别的"引导程序" (bootstrapper), 负责初始化网络流和协议,
/// 并通过其 `run` 方法启动核心的读/写异步任务.
pub struct LazyApp<P, Io>
where
    P: ProtocolSplit,
    Io: AsyncStreamSplit,
{
    /// 用于异步读写数据帧的网络流.
    stream: Io,
    /// 实现了 `ProtocolSplit` 的协议处理器.
    protocol: P,
}

impl<P, Io> LazyApp<P, Io>
where
    P: ProtocolSplit,
    Io: AsyncStreamSplit,
{
    /// 创建一个新的 `LazyApp` 实例.
    ///
    /// # 参数
    /// - `stream`: 一个实现了 `AsyncStreamSplit` 的网络流.
    /// - `protocol`: 一个实现了 `ProtocolSplit` 的协议处理器.
    ///
    /// # 返回
    /// 一个新的 `LazyApp` 实例, 准备好通过调用 `.run()` 来启动.
    pub fn new(stream: Io, protocol: P) -> Self {
        Self { stream, protocol }
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
    {
        // 1. 分离网络流和协议处理器
        let (mut stream_reader, _stream_writer) = self.stream.into_split();
        let (mut protocol_decoder, _protocol_encoder) = self.protocol.into_split();

        // 2. 创建用于外部与 Writer Task 通信的通道
        let (_command_sender, mut command_receiver) = mpsc::channel::<Command>(10);

        // --- Writer 任务 ---
        let writer_handle = tokio::spawn(async move {
            // 这个循环会一直运行, 直到 `command_receiver` 的所有发送端都被 drop
            while let Some(command) = command_receiver.recv().await {
                // TODO: 在这里使用 protocol_encoder 将 command 编码并用 stream_writer 发送出去
                println!("[Writer Task] Received command to send: {}", command)
            }
        });

        // --- Reader 任务 ---
        let reader_handle = tokio::spawn(async move {
            let mut buf = BytesMut::with_capacity(1024);
            // 这个循环会一直运行, 直到读取出错或连接关闭
            loop {
                // 从流中读取数据到缓冲区
                match stream_reader.read_frame(&mut buf).await {
                    Ok(_len) => {
                        // 尝试用解码器解析缓冲区中的数据
                        if let Some(commands) = protocol_decoder.parse_protocol_frame(&mut buf) {
                            // 成功解析出命令
                            commands.into_iter().for_each(|command| {
                                // TODO: 在这里处理解析出的 command, 例如通过另一个 channel 发送给 UI
                                println!("[Reader Task] Decoded a command: {:?}", command);
                            });
                        }
                    }
                    Err(e) => {
                        // 读取时发生错误
                        eprintln!("[Reader Task] Failed to read from stream: {}", e);
                        break;
                    }
                }
            }
        });

        // 3. 等待两个任务完成.
        // 在一个真实的应用中, 它们会一直运行, 直到发生错误或收到关闭信号.
        let _ = tokio::join!(reader_handle, writer_handle);

        Ok(())
    }
}
