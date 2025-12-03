use bytes::BytesMut;
use thiserror::Error;

/// 一个通用的命令结构体,作为协议特定类型 (如 `BlnProtocolType`) 和通用帧生成/解析逻辑之间的中间层.
#[derive(Default, Debug)]
pub struct Command {
    /// 命令类型/ID, 通常是一个或多个字节.
    pub cmd_type: BytesMut,
    /// 响应状态标志, 从长度字段的高位解析而来, 可选.
    pub response_status: Option<u8>,
    /// 命令的可选负载数据.
    pub payload: Option<BytesMut>,
}

/// 定义了在协议处理过程中可能发生的通用错误.
///
/// 作为一个通用的错误枚举,它可以用于表示来自不同协议实现的错误,
#[derive(Error, Debug, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("命令不适用于此协议")]
    CommandNotApplicable,
    #[error("此协议的命令类型无效")]
    InvalidCommandType,
    #[error("此命令的负载无效")]
    InvalidPayload,
}
