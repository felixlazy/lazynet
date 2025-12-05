use color_eyre::eyre::Result;
use net::client::{Lazyclient, connect};
use protocol::traits::{FrameGenerator, ParseProtocol};
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
    protocol: &'a mut P,
}

impl<'a, P> LazyApp<'a, P>
where
    P: ParseProtocol + FrameGenerator,
{
    /// 尝试连接到指定的地址,并返回一个 LazyApp 实例.
    ///
    /// # 参数
    /// * `addr`: 目标服务器的地址,例如 "127.0.0.1:8080".
    ///
    /// # 返回
    /// 如果连接成功,返回 `Result<Self>`,其中包含一个 LazyApp 实例;
    /// 否则返回 `Err`,包含连接失败的错误信息.
    #[instrument(skip(addr, protocol), err)]
    pub async fn connect(addr: impl AsRef<str>, protocol: &'a mut P) -> Result<Self> {
        // protocol 参数需要可变引用, 以匹配 LazyApp 结构体中的字段类型
        let client = connect(addr, 1024).await?;
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
            tokio::select! {
                _ret=self.client.read_frame()=> {
                    // 从客户端读取到数据后, 使用协议解析器尝试解析数据帧.
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
