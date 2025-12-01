//! `bln` 模块定义了 Bln 协议相关的类型和解析逻辑.
//!
//! 它包括协议帧的类型定义,以及用于解析接收到的字节流的方法.

use bytes::{Buf, BufMut, BytesMut};
use std::convert::TryFrom;
use tracing::info;

use crate::{
    traits::{FrameGenerator, ParseProtocol},
    types::{Command, ProtocolError},
    utils::calculate_bcc,
};

/// `BlnProtocolType` 枚举定义了 Bln 协议中支持的命令类型.
///
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum BlnProtocolType {
    /// 设置位置请求
    SetPositionRsq = 0x31,
    /// 设置位置响应
    SetPositionRsp = 0x91,
    /// 获取位置请求
    GetPositionRsq = 0x33,
    /// 获取位置响应
    GetPositionRsp = 0x93,
}

/// 定义转换失败的错误类型
#[derive(Debug, PartialEq, Eq)]
pub struct InvalidProtocolType;

impl TryFrom<u8> for BlnProtocolType {
    type Error = InvalidProtocolType;
    /// 从 `u8` 值转换为 `BlnProtocolType`.
    ///
    /// # 参数
    /// * `value`: 代表协议类型的 `u8` 值.
    ///
    /// # 返回
    /// 对应的 `BlnProtocolType` 枚举变体,如果值不匹配则返回 `Err`.
    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0x31 => Ok(Self::SetPositionRsq),
            0x91 => Ok(Self::SetPositionRsp),
            0x33 => Ok(Self::GetPositionRsq),
            0x93 => Ok(Self::GetPositionRsp),
            _ => Err(InvalidProtocolType),
        }
    }
}

impl From<BlnProtocolType> for u8 {
    /// 从 `BlnProtocolType` 转换为 `u8` 值.
    /// # 参数
    /// * `value`: 要转换的 `BlnProtocolType` 枚举成员.
    ///
    /// # 返回
    /// 对应的 `u8` 值.
    fn from(value: BlnProtocolType) -> Self {
        value as u8
    }
}

/// `BlnProtocol` 结构体是 Bln 协议的实现,用于处理帧的解析.
pub struct BlnProtocol {}

impl BlnProtocol {
    /// 协议帧的头部同步字,通常用于标识帧的开始.
    const FRAME_HEAD: [u8; 2] = [0x55, 0xAA];
    /// 帧头同步字的长度.
    const FRAME_HEAD_LEN: u8 = 2;
    /// 协议帧的固定部分长度 (含帧头, 不含可变长的数据体和 BCC).
    const FRAME_FIXED_LEN: u8 = 9;
    /// 协议帧的块校验码 (BCC) 长度.
    const FRAME_BCC_LEN: u8 = 1;

    /// 在缓冲区中查找协议帧的头部同步字.
    ///
    /// 如果找到,则将缓冲区前进到同步字之后,并返回 `true`.
    /// 如果未找到,缓冲区状态不变,返回 `false`.
    ///
    /// # 参数
    /// * `buf`: 待搜索的 `BytesMut` 缓冲区.
    ///
    /// # 返回
    /// 如果找到帧头并前进缓冲区,返回 `true`; 否则返回 `false`.
    pub fn find_frame_head(&self, buf: &mut bytes::BytesMut) -> bool {
        if let Some(index) = buf
            .windows(Self::FRAME_HEAD_LEN as usize)
            .position(|f| f == Self::FRAME_HEAD)
        {
            // 丢弃头部之前的所有数据
            buf.advance(index);
            true
        } else {
            false
        }
    }

    /// 检查缓冲区中是否包含一个完整的协议帧.
    ///
    /// 该函数会根据协议固定长度和帧内编码的长度字段来判断帧是否完整.
    ///
    /// # 参数
    /// * `buf`: 待检查的 `BytesMut` 缓冲区.
    ///
    /// # 返回
    /// 如果缓冲区中包含一个完整的帧,返回帧的完整长度 `Some(usize)`;
    /// 否则返回 `None`,表示帧不完整或缓冲区数据不足以读取长度字段.
    fn is_frame_complete(&self, buf: &BytesMut) -> Option<usize> {
        // 协议的最小长度 (固定部分 + BCC)
        let min_len = (Self::FRAME_FIXED_LEN + Self::FRAME_BCC_LEN) as usize;
        if buf.len() < min_len {
            return None;
        }

        // 长度字段在偏移量为7的位置, 2字节, 小端序.
        // `try_into` 不会失败, 因为我们已经检查了 `buf.len()`.
        let len_bytes: [u8; 2] = buf[7..9].try_into().unwrap();
        let len_field = u16::from_le_bytes(len_bytes);

        // 帧协议定义了长度字段的高3位可能用于标志位, 这里我只取低13位作为实际的数据长度.
        // 0x1FFF 是一个掩码, 二进制为 0001 1111 1111 1111.
        let data_len = (len_field & 0x1FFF) as usize;
        // 帧的总长度 = 数据长度 + 最小长度 (包含BCC)
        let frame_len = data_len + min_len;

        // 检查缓冲区中是否包含整个帧 (包括数据体)
        if buf.len() < frame_len {
            return None; // 帧不完整
        }
        Some(frame_len) // 帧完整,返回其总长度
    }
}

impl ParseProtocol for BlnProtocol {
    /// 从缓冲区中解析并提取一个完整的 Bln 协议帧.
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
    fn parse_protocol_frame(&mut self, buf: &mut bytes::BytesMut) -> Option<BytesMut> {
        if self.find_frame_head(buf)
            && let Some(frame_len) = self.is_frame_complete(buf)
        {
            if calculate_bcc(&buf[2..frame_len - 1]) == buf[frame_len - 1] {
                return Some(buf.split_to(frame_len));
            } else {
                info!("BCC check failed for a frame, discarding it.");
                buf.advance(frame_len); // 丢弃整个损坏的帧
            }
        }
        None
    }
}

impl FrameGenerator for BlnProtocol {
    fn create_frame(&self, command: Command) -> Result<BytesMut, ProtocolError> {
        if command.cmd_type.len() != 1 {
            return Err(ProtocolError::InvalidCommandType);
        }
        let cmd_byte = command.cmd_type[0];
        if BlnProtocolType::try_from(cmd_byte).is_err() {
            return Err(ProtocolError::InvalidCommandType);
        }

        let data = command.payload.unwrap_or_default();
        let data_len = data.len();
        // 总长度 = 固定长度(9) + 数据长度 + BCC(1)
        let total_len = Self::FRAME_FIXED_LEN as usize + data_len + Self::FRAME_BCC_LEN as usize;
        let mut buf = BytesMut::with_capacity(total_len);

        buf.put_slice(&Self::FRAME_HEAD);
        // 将经过验证的协议命令类型写入缓冲区
        buf.put_u8(cmd_byte);
        buf.put_slice(&[0x00, 0x00, 0x00, 0x00]); // 保留字段
        buf.put_u16_le(data_len as u16);
        if !data.is_empty() {
            buf.put_slice(&data);
        }
        buf.put_u8(calculate_bcc(&buf[2..]));

        info!("BLN Frame Created: {:02X?}", buf.as_ref());
        Ok(buf)
    }
}
