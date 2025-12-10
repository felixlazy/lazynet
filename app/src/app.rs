use bytes::BytesMut;
use color_eyre::eyre::Result;
use protocol::traits::Protocol;
use stream::traits::{AsyncFrameReader, AsyncFrameWriter};
use tracing::instrument;

/// LazyApp 是一个封装了 Lazyclient 的应用结构体.
/// 它负责管理客户端连接的生命周期和数据收发循环.
pub struct LazyApp<'a, P, Io>
where
    P: Protocol + Sync,
    Io: AsyncFrameReader + AsyncFrameWriter + Send,
{
    /// 用于异步读写数据帧的网络流.
    stream: &'a mut Io,
    /// 用于应用层协议的解析和编码.
    protocol: &'a mut P,
    /// 控制主事件循环的标志, `true` 表示正在运行.
    running: bool,
    /// 用于从流中读取数据的缓冲区.
    buf: BytesMut,
}

impl<'a, P, Io> LazyApp<'a, P, Io>
where
    P: Protocol + Sync,
    Io: AsyncFrameReader + AsyncFrameWriter + Send,
{
    /// 创建一个新的 `LazyApp` 实例.
    ///
    /// # 参数
    /// - `stream`: 用于网络 I/O 的可变引用.
    /// - `protocol`: 处理应用层协议逻辑的可变引用.
    /// - `buf_capacity`: 内部缓冲区 `BytesMut` 的初始容量.
    ///
    /// # 返回
    /// 一个新的 `LazyApp` 实例.
    pub fn new(stream: &'a mut Io, protocol: &'a mut P, buf_capacity: usize) -> Self {
        Self {
            stream,
            running: false,
            protocol,
            buf: BytesMut::with_capacity(buf_capacity),
        }
    }

    /// 运行应用的主循环,持续从客户端读取数据并进行处理.
    /// 循环会一直运行,直到 `running` 标志被设置为 `false`.
    ///
    /// # 返回
    /// 如果循环正常退出或遇到错误,返回 `Result<()>`.
    #[instrument(skip(self), err)]
    pub async fn run(&mut self) -> Result<()> {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
        self.protocol
            .handle_periodic_send_action(self.stream)
            .await?;
        while self.running {
            // 使用 tokio::select! 宏异步等待客户端读取数据帧.
            // 当 read_frame 返回 Ok(len) 时,表示成功读取了 len 字节数据.
            tokio::select! {
                // 尝试从流中读取新的数据帧.
                // 如果成功读取,`len` 将是读取的字节数.
                // 此时可以处理接收到的数据 (例如,通过 protocol.parse_frame).
                Ok(len) = self.stream.read_frame(&mut self.buf) => {
                    // TODO: 在这里处理接收到的数据,例如解析帧并执行相应逻辑
                }
                // 每隔一定时间触发一次,可用于执行周期性任务,
                // 例如发送心跳包或检查连接状态.
                _ = interval.tick() => {
                    // TODO: 在这里执行周期性任务
                }
            }
        }
        Ok(())
    }

    /// 启动应用的主循环,将 `running` 标志设置为 `true`.
    pub fn start(&mut self) {
        self.running = true;
    }

    /// 停止应用的主循环,将 `running` 标志设置为 `false`.
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// 检查应用是否正在运行.
    ///
    /// # 返回
    /// 如果应用正在运行,返回 `true`; 否则返回 `false`.
    pub fn is_running(&self) -> bool {
        self.running
    }
}
