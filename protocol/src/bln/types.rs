use std::convert::From;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum BlnProtocolType {
    /// 设置位置请求
    SetPositionRsq(f32, f32),
    /// 设置位置响应 (第一阶段：通信确认)
    SetPositionRsp,
    /// 位置到达响应 (第二阶段：执行完成)
    PositionReached(f32, f32),
    /// 获取位置请求
    GetPositionRsq,
    /// 获取位置响应
    GetPositionRsp(f32, f32, u8),
    /// 错误响应，包含具体的错误原因。
    ErrorRsp(BlnErrorCause),
}

/// 表示 Bln 协议响应中的状态标志.
///
/// 这个枚举定义了响应是成功 (带数据或不带数据), 还是错误状态,
/// 或是未使用的/保留的值.
#[derive(Debug, PartialEq, Clone, Copy)]
#[repr(u8)]
pub enum BlnResponseStatus {
    /// 未使用或未指定的状态。
    Unused = 0x00,
    /// 操作成功，没有附加数据。
    Ok = 0x01,
    /// 操作成功，并附带了数据。
    OkWithData = 0x02,
    /// 操作失败，伴随错误原因码。
    Error = 0x03,
    /// 保留供将来使用的状态码。
    Reserved = 0x04,
}

impl From<u8> for BlnResponseStatus {
    /// 将 `u8` 值转换为 `BlnResponseStatus`.
    ///
    /// # 参数
    /// * `value`: 原始的 `u8` 状态码.
    ///
    /// # 返回
    /// 对应的 `BlnResponseStatus` 变体, 未知值会映射到 `Reserved`.
    fn from(value: u8) -> Self {
        match value {
            0x00 => Self::Unused,
            0x01 => Self::Ok,
            0x02 => Self::OkWithData,
            0x03 => Self::Error,
            _ => Self::Reserved,
        }
    }
}

impl From<BlnResponseStatus> for u8 {
    /// 将 `BlnResponseStatus` 转换为 `u8` 值.
    fn from(value: BlnResponseStatus) -> Self {
        value as u8
    }
}

/// 定义了 Bln 协议中各种错误的原因码.
///
/// 这些错误码通常在 `BlnResponseStatus::Error` 状态下,
/// 通过响应帧的 payload 来传递.
#[derive(Debug, PartialEq, Clone, Copy)]
#[repr(u8)]
pub enum BlnErrorCause {
    /// 表示操作成功完成，没有错误。
    Success = 0x00,
    /// 帧数据校验和不匹配。
    ChecksumError = 0x01,
    /// 接收到的参数不合法或超出范围。
    InvalidArgument = 0x02,
    /// 执行操作失败。
    OperationFailed = 0x03,
    /// 指定的配置项不存在。
    ConfigNotFound = 0x04,
    /// 设备内部发生未知错误。
    InternalError = 0x05,
    /// 设备当前状态不允许执行该操作。
    StateMismatch = 0x06,
    /// 没有可用的有效数据。
    NoValidData = 0x07,
    /// 未知的或未指定的错误。
    UnspecifiedError = 0xFF,
}

impl From<u8> for BlnErrorCause {
    /// 将 `u8` 值转换为 `BlnErrorCause`.
    ///
    /// # 参数
    /// * `value`: 原始的 `u8` 错误码.
    ///
    /// # 返回
    /// 对应的 `BlnErrorCause` 变体, 未知值会映射到 `UnspecifiedError`.
    fn from(value: u8) -> Self {
        match value {
            0x00 => Self::Success,
            0x01 => Self::ChecksumError,
            0x02 => Self::InvalidArgument,
            0x03 => Self::OperationFailed,
            0x04 => Self::ConfigNotFound,
            0x05 => Self::InternalError,
            0x06 => Self::StateMismatch,
            0x07 => Self::NoValidData,
            0xFF => Self::UnspecifiedError,
            _ => Self::UnspecifiedError,
        }
    }
}

impl From<BlnErrorCause> for u8 {
    /// 将 `BlnErrorCause` 转换为 `u8` 值.
    fn from(value: BlnErrorCause) -> Self {
        value as u8
    }
}
