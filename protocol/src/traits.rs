use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use color_eyre::Result;
use stream::traits::AsyncFrameWriter;

use crate::types::{Command, ProtocolError};

/// `ParseProtocol` trait 定义了一个通用的协议解析接口,
/// 用于从字节流中解析出完整的协议帧.
pub trait ParseProtocol {
    /// 从缓冲区中解析并提取所有完整的协议帧, 并将其转换为 `Command` 对象的列表.
    ///
    /// 此函数会循环尝试从给定的 `BytesMut` 缓冲区中查找帧头、检查帧完整性.
    /// 如果找到一个完整的帧, 它将进行块校验码 (BCC) 验证, 并将其解析为 `Command` 对象.
    ///
    /// - 如果 BCC 匹配, 则该帧被成功提取并解析.
    /// - 如果 BCC 不匹配, 说明帧已损坏, 该帧将被从缓冲区中丢弃, 并继续尝试解析后续数据.
    /// - 如果帧头未找到或帧不完整, 则返回已解析的命令或 `None`, 等待更多数据.
    ///
    /// # 参数
    /// * `buf`: 包含协议字节流的 `BytesMut` 缓冲区. 此缓冲区在函数调用后可能会被修改 (例如, 前进或切分).
    ///
    /// # 返回
    /// 如果成功解析并提取到至少一个有效的帧,返回包含 `Command` 对象的 `Option<Vec<Command>>`;
    /// 否则返回 `None`,表示缓冲区中没有足够的完整且有效的帧数据可供解析.
    fn parse_protocol_frame(&mut self, buf: &mut BytesMut) -> Option<Vec<Command>>;
}

/// 一个通用的帧生成器 trait, 用于将高级别的协议命令转换成字节帧.
///
/// 这个 trait 被设计为协议无关的, 只要给定的协议类型可以被转换成通用的 `Command` 结构体,
/// 就可以使用它来创建帧, 从而实现代码复用和良好的扩展性.
pub trait FrameGenerator {
    /// 根据给定的通用 `Command`, 创建一个完整的、可供发送的字节帧.
    ///
    /// # 返回
    /// 一个 `Result`, 成功时包含一个包含完整协议帧的 `Bytes`, 失败时包含一个 `ProtocolError`.
    fn create_frame(&self, command: Command) -> Result<Bytes, ProtocolError>;
}

#[async_trait]
/// `Protocol` 是一个组合 trait, 结合了 `ParseProtocol` 和 `FrameGenerator` 的功能,
/// 并提供了处理周期性发送动作的能力. 它是应用层协议的核心接口.
pub trait Protocol: ParseProtocol + FrameGenerator + Send {
    /// 处理周期性发送动作的异步方法,例如发送心跳包或保持连接活跃的消息.
    ///
    /// 此方法提供一个默认实现, 不执行任何操作并返回 `Ok(0)`.
    /// 具体的协议实现可以重写此方法, 以便在协议需要时发送周期性数据.
    ///
    /// # 参数
    /// * `_send`: 一个实现了 `AsyncFrameWriter` trait 的可变引用, 用于向流中写入数据.
    ///            在默认实现中未使用, 但在重写此方法的具体协议实现中可用于发送数据.
    ///
    /// # 返回
    /// 一个 `Result<usize>`, 表示成功发送的字节数, 或者在发送过程中遇到的错误.
    /// 默认实现总是返回 `Ok(0)`.
    async fn handle_periodic_send_action<W>(&self, _send: &mut W) -> Result<usize>
    where
        W: AsyncFrameWriter + Send,
    {
        std::future::pending().await
    }
}
