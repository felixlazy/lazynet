use crate::{
    traits::{FrameGenerator, ParseProtocol},
    types::{Command, ProtocolError},
    utils::calculate_bcc,
};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use tracing::info;

/// `BlnProtocol` 结构体是 Bln 协议的实现,用于处理帧的解析.
pub struct BlnProtocol {}

impl BlnProtocol {
    /// 协议帧的头部同步字,通常用于标识帧的开始.
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

    // 长度字段 (u16) 的位掩码常量
    const DATA_LENGTH_MASK: u16 = 0x1FFF; // 低 13 位用于实际数据长度
    const FLAGS_MASK: u16 = 0xE000; // 高 3 位用于标志位，如响应状态

    /// 在缓冲区中查找协议帧的头部同步字.
    pub fn find_frame_head(&self, buf: &mut bytes::BytesMut) -> bool {
        if let Some(index) = buf
            .windows(Self::FRAME_HEAD_LEN)
            .position(|f| f == Self::FRAME_HEAD)
        {
            buf.advance(index);
            true
        } else {
            false
        }
    }

    /// 检查缓冲区中是否包含一个完整的协议帧.
    fn is_frame_complete(&self, buf: &BytesMut) -> Option<usize> {
        let min_len = Self::FRAME_FIXED_LEN + Self::FRAME_BCC_LEN;
        if buf.len() < min_len {
            return None;
        }

        let len_bytes: [u8; 2] = buf[7..9].try_into().unwrap();
        let len_field = u16::from_le_bytes(len_bytes);

        let (data_len, _) = self.decode_length_flags(len_field);
        let frame_len = data_len + min_len;

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

impl ParseProtocol for BlnProtocol {
    fn parse_protocol_frame(&mut self, buf: &mut bytes::BytesMut) -> Option<Vec<Command>> {
        let mut command_list = vec![];
        loop {
            if self.find_frame_head(buf)
                && let Some(frame_len) = self.is_frame_complete(buf)
            {
                if calculate_bcc(&buf[Self::FRAME_HEAD_LEN..frame_len - Self::FRAME_BCC_LEN])
                    == buf[frame_len - Self::FRAME_BCC_LEN]
                {
                    let mut frame_data = buf.split_to(frame_len);

                    frame_data.advance(Self::FRAME_HEAD_LEN);
                    let cmd_type_byte = frame_data.get_u8();
                    frame_data.advance(Self::RESERVED_LEN);
                    let (data_len, flags) = self.decode_length_flags(frame_data.get_u16_le());

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
                    command_list.push(cmd);
                } else {
                    info!("BCC check failed for a frame, discarding it.");
                    buf.advance(frame_len);
                }
            } else {
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

impl FrameGenerator for BlnProtocol {
    fn create_frame<T>(&self, protocol_type: T) -> Result<Bytes, ProtocolError>
    where
        T: TryInto<Command>,
        ProtocolError: From<T::Error>,
    {
        let command = protocol_type.try_into()?;
        if command.cmd_type.len() != 1 {
            return Err(ProtocolError::InvalidCommandType);
        }
        let cmd_byte = command.cmd_type[0];

        let data = command.payload.unwrap_or_default();
        let data_len = data.len();

        let total_len = Self::FRAME_FIXED_LEN + data_len + Self::FRAME_BCC_LEN;
        let mut buf = BytesMut::with_capacity(total_len);

        buf.put_slice(&Self::FRAME_HEAD);
        buf.put_u8(cmd_byte);
        buf.put_slice(&[0x00, 0x00, 0x00, 0x00]); // 保留字段
        buf.put_u16_le(data_len as u16);
        if !data.is_empty() {
            buf.put_slice(&data);
        }
        buf.put_u8(calculate_bcc(&buf[Self::FRAME_HEAD_LEN..]));

        info!("BLN Frame Created: {:02X?}", buf.as_ref());
        Ok(buf.freeze())
    }
}
