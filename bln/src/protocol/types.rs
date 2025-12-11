use std::convert::From;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use protocol::{
    traits::{FrameGenerator, ParseProtocol},
    types::{Command, ProtocolError},
    utils::calculate_bcc,
};
use tracing::{debug, info, instrument};

/// BLN 协议定义的特定指令类型.
///
/// 这个枚举列出了所有支持的协议命令, 并且可以方便地与 `u8` 类型进行转换,
/// 以便在协议帧中表示.
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

/// `BlnCommandEncoder` 是一个实现了 `FrameGenerator` trait 的具体编码器.
///
/// 它的唯一职责是将一个 `Command` 对象序列化成符合 BLN 协议规范的字节帧 (`Bytes`).
/// 这个结构体是无状态的.
#[derive(Default)]
pub struct BlnCommandEncoder;

impl BlnCommandEncoder {
    // BLN 协议帧结构中使用的常量
    /// 协议帧的头部同步字,用于标识帧的开始.
    const FRAME_HEAD: [u8; 2] = [0x55, 0xAA];
    /// 帧头同步字的长度.
    const FRAME_HEAD_LEN: usize = 2;
    /// 协议帧的固定部分长度 (含帧头, 不含可变长的数据体和 BCC).
    const FRAME_FIXED_LEN: usize = 9;
    /// 协议帧的块校验码 (BCC) 长度.
    const FRAME_BCC_LEN: usize = 1;
    /// 协议帧中的保留字段长度.
    const RESERVED_LEN: usize = 4;
}

impl FrameGenerator for BlnCommandEncoder {
    /// 实现 `create_frame`, 将 `Command` 编码为 `Bytes`.
    fn create_frame(&self, command: Command) -> Result<Bytes, ProtocolError> {
        if command.cmd_type.len() != 1 {
            return Err(ProtocolError::InvalidCommandType);
        }
        let cmd_byte = command.cmd_type[0];

        let data = command.payload.unwrap_or_default();
        let data_len = data.len();

        let total_len = Self::FRAME_FIXED_LEN + data_len + Self::FRAME_BCC_LEN;
        let mut buf = BytesMut::with_capacity(total_len);

        // 按照 BLN 协议格式组装帧
        buf.put_slice(&Self::FRAME_HEAD);
        buf.put_u8(cmd_byte);
        buf.put_slice(&[0x00; Self::RESERVED_LEN]); // 保留字段
        buf.put_u16(data_len as u16);
        if !data.is_empty() {
            buf.put_slice(&data);
        }
        // 计算并附加 BCC 校验码
        buf.put_u8(calculate_bcc(&buf[Self::FRAME_HEAD_LEN..]));

        info!("BLN Frame Created: {:02X?}", buf.as_ref());
        Ok(buf.freeze())
    }
}

/// `BlnCommandDecode` 是一个实现了 `ParseProtocol` trait 的具体解码器.
///
/// 它的唯一职责是从一个连续的字节流中解析出符合 BLN 协议规范的 `Command` 帧.
/// 这个结构体是无状态的, 所有的解析状态都通过传入的 `BytesMut` 缓冲区来管理.
#[derive(Default)]
pub struct BlnCommandDecode;

impl BlnCommandDecode {
    // BLN 协议帧结构中使用的常量
    /// 协议帧的头部同步字,用于标识帧的开始.
    const FRAME_HEAD: [u8; 2] = [0x55, 0xAA];
    /// 帧头同步字的长度.
    const FRAME_HEAD_LEN: usize = 2;
    /// 协议帧的固定部分长度 (含帧头, 不含可变长的数据体和 BCC).
    const FRAME_FIXED_LEN: usize = 9;
    /// 协议帧的块校验码 (BCC) 长度.
    const FRAME_BCC_LEN: usize = 1;
    /// 协议帧中的保留字段长度.
    const RESERVED_LEN: usize = 4;
    /// 协议帧中的命令类型字段长度.
    const TYPE_LEN: usize = 1;

    // 长度字段 (u16) 的位掩码常量, 用于分离数据长度和标志位
    const DATA_LENGTH_MASK: u16 = 0x1FFF; // 低 13 位用于实际数据长度
    const FLAGS_MASK: u16 = 0xE000; // 高 3 位用于标志位，如响应状态

    /// 在缓冲区中查找协议帧的头部同步字.
    /// 如果找到, 会丢弃头部之前的所有数据, 并返回 `true`.
    pub fn find_frame_head(&self, buf: &mut bytes::BytesMut) -> bool {
        if let Some(index) = buf
            .windows(Self::FRAME_HEAD_LEN)
            .position(|f| f == Self::FRAME_HEAD)
        {
            // 丢弃找到的帧头之前的所有无效数据
            buf.advance(index);
            true
        } else {
            false
        }
    }

    /// 检查缓冲区中是否包含一个完整的协议帧.
    /// 如果是, 返回整个帧的长度; 否则返回 `None`.
    fn is_frame_complete(&self, buf: &BytesMut) -> Option<usize> {
        let min_len = Self::FRAME_FIXED_LEN + Self::FRAME_BCC_LEN;
        if buf.len() < min_len {
            return None; // 数据不足, 无法判断
        }

        // 从帧的特定位置读取长度字段
        let len_bytes: [u8; 2] = buf[7..9].try_into().unwrap();
        let len_field = u16::from_be_bytes(len_bytes);

        // 解码出实际的数据负载长度
        let (data_len, _) = self.decode_length_flags(len_field);
        // 计算出完整的帧长度
        let frame_len = data_len + min_len;

        // 如果缓冲区的数据还不够一个完整帧的长度, 返回 None
        if buf.len() < frame_len {
            return None;
        }
        Some(frame_len)
    }

    /// 从 2 字节的长度字段中解码出实际数据长度和高位标志 (如响应状态).
    fn decode_length_flags(&self, len_field: u16) -> (usize, u8) {
        (
            (len_field & Self::DATA_LENGTH_MASK).into(),
            ((len_field & Self::FLAGS_MASK) >> 13) as u8,
        )
    }
}

impl ParseProtocol for BlnCommandDecode {
    #[instrument(skip(self, buf))]
    /// 实现 `parse_protocol_frame`, 尝试从缓冲区 `buf` 中解析出所有可能的 `Command` 帧.
    fn parse_protocol_frame(&mut self, buf: &mut bytes::BytesMut) -> Option<Vec<Command>> {
        let mut command_list = vec![];
        // 循环处理, 因为缓冲区中可能包含多个帧
        loop {
            // 首先找到帧头, 然后检查帧是否完整
            if self.find_frame_head(buf)
                && let Some(frame_len) = self.is_frame_complete(buf)
            {
                // 校验 BCC
                if calculate_bcc(&buf[Self::FRAME_HEAD_LEN..frame_len - Self::FRAME_BCC_LEN])
                    == buf[frame_len - Self::FRAME_BCC_LEN]
                {
                    // 校验成功, 解析帧内容
                    let mut frame_data = buf.split_to(frame_len);
                    frame_data.advance(Self::FRAME_HEAD_LEN); // 跳过帧头

                    let cmd_type_byte = frame_data.get_u8();
                    frame_data.advance(Self::RESERVED_LEN); // 跳过保留字段
                    let (data_len, flags) = self.decode_length_flags(frame_data.get_u16());

                    let payload = if data_len > 0 {
                        Some(frame_data.split_to(data_len))
                    } else {
                        None
                    };

                    let mut cmd_type = BytesMut::with_capacity(Self::TYPE_LEN);
                    cmd_type.put_u8(cmd_type_byte);

                    let cmd = Command {
                        cmd_type,
                        response_status: Some(flags),
                        payload,
                    };
                    debug!(%cmd);
                    command_list.push(cmd);
                } else {
                    // 校验失败, 丢弃这一个字节, 从下一个字节开始重新寻找帧头
                    info!("BCC check failed for a frame, discarding it.");
                    buf.advance(1);
                }
            } else {
                // 在当前缓冲区中再也找不到完整的帧了, 退出循环
                break;
            }
        }

        if command_list.is_empty() {
            None
        } else {
            Some(command_list)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bln_response_status_from_u8() {
        assert_eq!(BlnResponseStatus::from(0x00), BlnResponseStatus::Unused);
        assert_eq!(BlnResponseStatus::from(0x01), BlnResponseStatus::Ok);
        assert_eq!(BlnResponseStatus::from(0x02), BlnResponseStatus::OkWithData);
        assert_eq!(BlnResponseStatus::from(0x03), BlnResponseStatus::Error);
        assert_eq!(BlnResponseStatus::from(0x04), BlnResponseStatus::Reserved);
        assert_eq!(BlnResponseStatus::from(0x05), BlnResponseStatus::Reserved); // Unknown value
        assert_eq!(BlnResponseStatus::from(0xFF), BlnResponseStatus::Reserved); // Unknown value
    }

    #[test]
    fn test_bln_response_status_into_u8() {
        assert_eq!(u8::from(BlnResponseStatus::Unused), 0x00);
        assert_eq!(u8::from(BlnResponseStatus::Ok), 0x01);
        assert_eq!(u8::from(BlnResponseStatus::OkWithData), 0x02);
        assert_eq!(u8::from(BlnResponseStatus::Error), 0x03);
        assert_eq!(u8::from(BlnResponseStatus::Reserved), 0x04);
    }

    #[test]
    fn test_bln_error_cause_from_u8() {
        assert_eq!(BlnErrorCause::from(0x00), BlnErrorCause::Success);
        assert_eq!(BlnErrorCause::from(0x01), BlnErrorCause::ChecksumError);
        assert_eq!(BlnErrorCause::from(0x02), BlnErrorCause::InvalidArgument);
        assert_eq!(BlnErrorCause::from(0x03), BlnErrorCause::OperationFailed);
        assert_eq!(BlnErrorCause::from(0x04), BlnErrorCause::ConfigNotFound);
        assert_eq!(BlnErrorCause::from(0x05), BlnErrorCause::InternalError);
        assert_eq!(BlnErrorCause::from(0x06), BlnErrorCause::StateMismatch);
        assert_eq!(BlnErrorCause::from(0x07), BlnErrorCause::NoValidData);
        assert_eq!(BlnErrorCause::from(0xFF), BlnErrorCause::UnspecifiedError);
        assert_eq!(BlnErrorCause::from(0x08), BlnErrorCause::UnspecifiedError); // Unknown value
    }

    #[test]
    fn test_bln_error_cause_into_u8() {
        assert_eq!(u8::from(BlnErrorCause::Success), 0x00);
        assert_eq!(u8::from(BlnErrorCause::ChecksumError), 0x01);
        assert_eq!(u8::from(BlnErrorCause::InvalidArgument), 0x02);
        assert_eq!(u8::from(BlnErrorCause::OperationFailed), 0x03);
        assert_eq!(u8::from(BlnErrorCause::ConfigNotFound), 0x04);
        assert_eq!(u8::from(BlnErrorCause::InternalError), 0x05);
        assert_eq!(u8::from(BlnErrorCause::StateMismatch), 0x06);
        assert_eq!(u8::from(BlnErrorCause::NoValidData), 0x07);
        assert_eq!(u8::from(BlnErrorCause::UnspecifiedError), 0xFF);
    }
}
