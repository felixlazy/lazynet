//! `bln` 模块定义了 Bln 协议相关的类型和解析逻辑.
//!
//! 它包括协议帧的类型定义,以及用于解析接收到的字节流的方法.

use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::convert::TryFrom;
use tracing::info;

use crate::{
    traits::{FrameGenerator, ParseProtocol},
    types::{Command, ProtocolError},
    utils::calculate_bcc,
};

/// `BlnProtocolType` 枚举定义了 Bln 协议中支持的命令类型.
///
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum BlnProtocolType {
    /// 设置位置请求
    SetPositionRsq(f32, f32),
    /// 设置位置响应
    SetPositionRsp,
    /// 获取位置请求
    GetPositionRsq,
    /// 获取位置响应
    GetPositionRsp(f32, f32, u8),
}

#[derive(Debug, PartialEq, Clone, Copy)]
#[repr(u8)]
pub enum BlnResponseStatus {
    Unused = 0x00,
    Ok = 0x01,
    OkWithData = 0x02,
    Error = 0x03,
    Reserved = 0x04,
}

impl From<u8> for BlnResponseStatus {
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
    fn from(value: BlnResponseStatus) -> Self {
        value as u8
    }
}

// 将通用的 `Command` 解析为具体的 `BlnProtocolType`.
impl TryFrom<Command> for BlnProtocolType {
    type Error = ProtocolError;

    fn try_from(value: Command) -> Result<Self, Self::Error> {
        let cmd_byte = *value
            .cmd_type
            .first()
            .ok_or(ProtocolError::InvalidCommandType)?;

        match cmd_byte {
            0x91 => {
                if value.payload.is_some() {
                    return Err(ProtocolError::InvalidPayload);
                }
                Ok(Self::SetPositionRsp)
            }
            0x93 => {
                let mut payload = value.payload.ok_or(ProtocolError::InvalidPayload)?;
                if payload.len() != 9 {
                    return Err(ProtocolError::InvalidPayload);
                }
                Ok(Self::GetPositionRsp(
                    payload.get_f32_ne(),
                    payload.get_f32_ne(),
                    payload.get_u8(),
                ))
            }
            _ => Err(ProtocolError::InvalidCommandType), // 未知的命令类型
        }
    }
}

// 将具体的 `BlnProtocolType` 转换为通用的 `Command` 以便后续生成字节帧.
impl TryFrom<BlnProtocolType> for Command {
    type Error = ProtocolError;

    fn try_from(value: BlnProtocolType) -> Result<Self, Self::Error> {
        match value {
            BlnProtocolType::SetPositionRsq(pos1, pos2) => {
                let mut cmd_type = BytesMut::with_capacity(1);
                cmd_type.put_u8(0x31);
                let mut load = BytesMut::with_capacity(8);
                load.put_f32_ne(pos1);
                load.put_f32_ne(pos2);
                Ok(Self {
                    cmd_type,
                    response_status: None, // 生成请求时，响应状态通常不适用或为默认值
                    payload: Some(load),
                })
            }
            BlnProtocolType::GetPositionRsq => {
                let mut cmd_type = BytesMut::with_capacity(1);
                cmd_type.put_u8(0x33);
                Ok(Self {
                    cmd_type,
                    response_status: None, // 生成请求时，响应状态通常不适用或为默认值
                    payload: None,
                })
            }
            _ => Err(ProtocolError::InvalidCommandType), // 其他类型不应由从机发送,因此视为错误
        }
    }
}

/// 定义转换失败的错误类型
#[derive(Debug, PartialEq, Eq)]
pub struct InvalidProtocolType;

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

    const RESERVED_LEN: usize = 4;

    const TYPE_LEN: usize = 1;

    // 长度字段 (u16) 的位掩码常量
    const DATA_LENGTH_MASK: u16 = 0x1FFF; // 低 13 位用于实际数据长度
    const FLAGS_MASK: u16 = 0xE000; // 高 3 位用于标志位，如响应状态

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
            .windows(Self::FRAME_HEAD_LEN)
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
        let min_len = Self::FRAME_FIXED_LEN + Self::FRAME_BCC_LEN;
        if buf.len() < min_len {
            return None;
        }

        // 长度字段在偏移量为7的位置, 2字节, 小端序.
        let len_bytes: [u8; 2] = buf[7..9].try_into().unwrap();
        let len_field = u16::from_le_bytes(len_bytes);

        let (data_len, _) = self.decode_length_flags(len_field);
        // 帧的总长度 = 数据长度 + 最小长度 (包含BCC)
        let frame_len = data_len + min_len;

        // 检查缓冲区中是否包含整个帧 (包括数据体)
        if buf.len() < frame_len {
            return None; // 帧不完整
        }
        Some(frame_len) // 帧完整,返回其总长度
    }

    /// 从 2 字节的长度字段中解码出实际数据长度和高位标志 (如响应状态).
    ///
    /// # 参数
    /// * `len_bytes`: 包含长度和标志的 2 字节数组.
    ///
    /// # 返回
    /// 一个元组 `(data_len, flags_u8)`, 分别代表低 13 位的实际数据长度 (usize)
    /// 和高 3 位的标志位 (u8).
    fn decode_length_flags(&self, len_bytes: u16) -> (usize, u8) {
        (
            (len_bytes & Self::DATA_LENGTH_MASK).into(), // 低 13 位为数据长度
            ((len_bytes & Self::FLAGS_MASK) >> 13) as u8, // 高 3 位为标志位
        )
    }
}

impl ParseProtocol for BlnProtocol {
    /// 从缓冲区中解析并提取所有完整的 Bln 协议帧, 并将其转换为 `Command` 对象的列表.
    ///
    /// 此函数使用递归方式, 循环尝试从给定的 `BytesMut` 缓冲区中查找帧头、检查帧完整性.
    /// 如果找到一个完整的帧, 它将进行块校验码 (BCC) 验证, 并将其解析为 `Command` 对象.
    ///
    /// - 如果 BCC 匹配, 则该帧被成功提取并解析, 添加到命令列表, 并继续递归解析缓冲区剩余部分.
    /// - 如果 BCC 不匹配, 说明帧已损坏, 该帧将被从缓冲区中丢弃, 并继续递归尝试解析后续数据.
    /// - 如果帧头未找到或帧不完整, 则停止当前层的解析, 返回已解析的命令列表.
    ///
    /// # 参数
    /// * `buf`: 包含协议字节流的 `BytesMut` 缓冲区. 此缓冲区在函数调用后会被修改 (例如, 前进或切分).
    ///
    /// # 返回
    /// 如果成功解析并提取到至少一个有效帧,返回包含 `Command` 对象的 `Option<Vec<Command>>`;
    /// 否则返回 `None`,表示缓冲区中没有足够的完整且有效的帧数据可供解析.
    fn parse_protocol_frame(&mut self, buf: &mut bytes::BytesMut) -> Option<Vec<Command>> {
        let mut command_list = vec![];
        loop {
            if self.find_frame_head(buf)
                && let Some(frame_len) = self.is_frame_complete(buf)
            {
                // 2. 初始化一个列表用于存放解析出的 Command

                // 3. 检查当前帧的 BCC 校验码
                if calculate_bcc(&buf[Self::FRAME_HEAD_LEN..frame_len - Self::FRAME_BCC_LEN])
                    == buf[frame_len - Self::FRAME_BCC_LEN]
                {
                    // 3.1 BCC 校验成功, 解析当前帧为 Command 对象
                    let mut cmd = Command {
                        cmd_type: BytesMut::with_capacity(Self::TYPE_LEN), // 命令类型字段
                        ..Default::default()
                    };
                    let mut frame_data = buf.split_to(frame_len); // 分割出当前帧的数据

                    frame_data.advance(Self::FRAME_HEAD_LEN); // 跳过帧头 (0x55AA)
                    cmd.cmd_type.put_u8(frame_data.get_u8()); // 读取命令字
                    frame_data.advance(Self::RESERVED_LEN); // 跳过保留字段 (4字节)
                    // 从帧数据中读取 2 字节的长度/标志字段 (小端序), 并解码出数据长度和状态标志
                    let (data_len, flags) = self.decode_length_flags(frame_data.get_u16_le());
                    cmd.response_status = Some(flags); // 将解析出的状态标志赋给 Command

                    if data_len != 0 {
                        cmd.payload = Some(frame_data.split_to(data_len)) // 读取负载数据
                    }

                    command_list.push(cmd); // 将解析出的当前 Command 添加到列表
                } else {
                    // 3.3 BCC 校验失败, 丢弃当前帧并继续解析
                    info!("BCC check failed for a frame, discarding it.");
                    buf.advance(frame_len); // 丢弃整个损坏的帧
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
        // 1. 尝试找到一个有效的帧头, 并确保缓冲区中有足够的数据来读取长度字段
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
        // 总长度 = 固定部分(9) + 数据长度 + BCC(1)
        let total_len = Self::FRAME_FIXED_LEN + data_len + Self::FRAME_BCC_LEN;
        let mut buf = BytesMut::with_capacity(total_len);

        // 按照 [帧头][命令字][保留位][长度][数据][BCC] 的顺序组装帧
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
