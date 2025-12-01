use bytes::BytesMut;

use crate::types::{Command, ProtocolError};

/// `LazyParseProtocol` trait 定义了一个通用的协议解析接口,
/// 用于从字节流中惰性地解析出完整的协议帧.
pub trait ParseProtocol {
    /// 从缓冲区中解析并提取一个完整的协议帧.
    ///
    /// 此函数会尝试从给定的 `BytesMut` 缓冲区中查找帧头、检查帧完整性.
    /// 如果找到一个完整的帧, 它将进行块校验码 (BCC) 验证.
    /// - 如果 BCC 匹配, 则该帧被成功提取并返回.
    /// - 如果 BCC 不匹配, 说明帧已损坏, 该帧将被从缓冲区中丢弃, 并返回 `None`.
    /// - 如果帧头未找到或帧不完整, 则返回 `None`, 等待更多数据.
    ///
    /// # 参数
    /// * `buf`: 包含协议字节流的 `BytesMut` 缓冲区. 此缓冲区在函数调用后可能会被修改 (例如, 前进或切分).
    ///
    /// # 返回
    /// 如果成功解析并提取到一个有效的帧,返回包含帧数据的 `Option<BytesMut>`;
    /// 否则返回 `None`,表示未找到帧头、帧不完整或 BCC 校验失败.
    fn parse_protocol_frame(&mut self, buf: &mut BytesMut) -> Option<BytesMut>;
}

/// `BlnFrameGenerator` trait 定义了用于创建 Bln 协议命令帧的接口.
pub trait FrameGenerator {
    /// 根据给定的 `BlnCommand` 创建一个协议帧.
    ///
    /// # 参数
    /// * `command`: 要编码的 `Command`.
    ///
    /// # 返回
    /// 一个包含完整协议帧的 `BytesMut` 缓冲区.
    fn create_frame(&self, command: Command) -> Result<BytesMut, ProtocolError>;
}
