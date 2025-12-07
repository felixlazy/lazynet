use color_eyre::eyre::Result;
use net::client::{Lazyclient, connect};
use protocol::traits::{FrameGenerator, ParseProtocol};
use std::time::Duration;
use tracing::instrument;

/// LazyApp 是一个封装了 Lazyclient 的应用结构体.
/// 它负责管理客户端连接的生命周期和数据收发循环.
pub struct LazyApp<'a, P>
where
    P: ParseProtocol + FrameGenerator,
{
    /// 内部的 Lazyclient 实例,用于处理网络通信.
    client: Lazyclient,
    /// 指示应用是否正在运行的标志.
    running: bool,
    /// 协议解析器和帧生成器,用于处理应用层协议逻辑.
    protocol: &'a mut P,
}

impl<'a, P> LazyApp<'a, P>
where
    P: ParseProtocol + FrameGenerator,
{
    /// 连接成功,返回 `Result<Self>`,其中包含一个 LazyApp 实例;
    /// 否则返回 `Err`,包含连接失败的错误信息.
    #[instrument(skip(addr, protocol), err)]
    pub async fn connect(
        addr: impl AsRef<str>,
        protocol: &'a mut P,
        timeout: Duration,
    ) -> Result<Self> {
        // protocol 参数需要可变引用, 以匹配 LazyApp 结构体中的字段类型
        let client = connect(addr, 1024, timeout).await?;
        Ok(Self {
            client,
            running: false,
            protocol,
        })
    }

    /// 运行应用的主循环,持续从客户端读取数据并进行处理.
    /// 循环会一直运行,直到 `running` 标志被设置为 `false`.
    ///
    /// # 返回
    /// 如果循环正常退出或遇到错误,返回 `Result<()>`.
    #[instrument(skip(self), err)]
    pub async fn run(&mut self) -> Result<()> {
        while self.running {
            // 使用 tokio::select! 宏异步等待客户端读取数据帧.
            // 当 read_frame 返回 Ok(len) 时,表示成功读取了 len 字节数据.
            tokio::select! {
                Ok(len)=self.client.read_frame()=> {
                    // parse_protocol_frame 负责处理缓冲区的具体内容, 例如识别完整的协议消息并从中提取信息.
                    self.protocol.parse_protocol_frame(self.client.get_bytes_mut());
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
    pub fn running_start(&self) -> bool {
        self.running
    }
}
