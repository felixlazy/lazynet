use bytes::{Bytes, BytesMut};
use color_eyre::Result;

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

/// `ProtocolSplit` trait 定义了一种将协议处理器分解为其编码和解码组件的标准方式.
///
/// 这个 trait 旨在替代一个单一、庞大的 `Protocol` trait, 遵循单一职责原则,
/// 使得协议的读取和写入逻辑可以被独立地处理和传递.
pub trait ProtocolSplit {
    /// 关联类型, 代表协议的编码器部分.
    /// 它应该实现 `FrameGenerator` trait.
    type Encoder;

    /// 关联类型, 代表协议的解码器/解析器部分.
    /// 它应该实现 `ParseProtocol` trait.
    type Decode;

    /// 消耗协议对象本身, 并返回其分离的编码器和解码器组件.
    ///
    /// 这种 "消耗并返回部分" 的模式在 Rust 的并发编程中非常有用,
    /// 因为它允许我们将不同的组件 (如编码器和解码器) 的所有权
    /// 移动到不同的异步任务中 (例如,一个专门用于写入,一个专门用于读取),
    /// 这有助于满足 `tokio::spawn` 的 `'static` 生命周期要求.
    fn into_split(self) -> (Self::Decode, Self::Encoder);
}
