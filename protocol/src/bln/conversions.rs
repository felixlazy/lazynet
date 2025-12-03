use super::types::{BlnProtocolType, BlnResponseStatus};
use crate::types::{Command, ProtocolError};
use bytes::{Buf, BufMut, BytesMut};
use std::convert::TryFrom;

// 将通用的 `Command` 解析为具体的 `BlnProtocolType`.
impl TryFrom<Command> for BlnProtocolType {
    type Error = ProtocolError;

    fn try_from(value: Command) -> Result<Self, Self::Error> {
        let cmd_byte = *value
            .cmd_type
            .first()
            .ok_or(ProtocolError::InvalidCommandType)?;

        let status: BlnResponseStatus = value
            .response_status
            .ok_or(ProtocolError::InvalidPayload)?
            .into();

        // 1. 首先，统一处理所有命令的“错误”状态
        if status == BlnResponseStatus::Error {
            // 如果是错误响应, 则 payload 应该包含 1 字节的错误原因
            let mut payload = value.payload.ok_or(ProtocolError::InvalidPayload)?;
            if payload.len() != 1 {
                return Err(ProtocolError::InvalidPayload);
            }
            return Ok(Self::ErrorRsp(payload.get_u8().into()));
        }

        // 2. 如果不是错误状态，再根据命令字处理各自的“成功”状态
        match cmd_byte {
            0x91 => match status {
                BlnResponseStatus::Ok => {
                    // 阶段1: 通信确认。payload 必须为空。
                    if value.payload.is_some() {
                        return Err(ProtocolError::InvalidPayload);
                    }
                    Ok(Self::SetPositionRsp)
                }
                BlnResponseStatus::OkWithData => {
                    // 阶段2: 执行完成确认。payload 必须为 8 字节 (f32 + f32)。
                    let mut payload = value.payload.ok_or(ProtocolError::InvalidPayload)?;
                    if payload.len() != 8 {
                        return Err(ProtocolError::InvalidPayload);
                    }
                    Ok(Self::PositionReached(
                        payload.get_f32_le(),
                        payload.get_f32_le(),
                    ))
                }
                // 对于 0x91 命令，不应该出现 Error 之外的其他状态
                _ => Err(ProtocolError::InvalidPayload),
            },
            0x93 => {
                // 校验：成功有数据的响应，其 status 必须是 OkWithData
                if status != BlnResponseStatus::OkWithData {
                    return Err(ProtocolError::InvalidPayload); // 状态与命令不符
                }
                // 校验：payload 必须为 9 字节
                let mut payload = value.payload.ok_or(ProtocolError::InvalidPayload)?;
                if payload.len() != 9 {
                    return Err(ProtocolError::InvalidPayload);
                }
                Ok(Self::GetPositionRsp(
                    payload.get_f32_le(),
                    payload.get_f32_le(),
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
                load.put_f32_le(pos1);
                load.put_f32_le(pos2);
                Ok(Command {
                    cmd_type,
                    response_status: None, // 生成请求时，响应状态通常不适用或为默认值
                    payload: Some(load),
                })
            }
            BlnProtocolType::GetPositionRsq => {
                let mut cmd_type = BytesMut::with_capacity(1);
                cmd_type.put_u8(0x33);
                Ok(Command {
                    cmd_type,
                    response_status: None, // 生成请求时，响应状态通常不适用或为默认值
                    payload: None,
                })
            }
            // ErrorRsp, SetPositionRsp 等响应类型不应由从机主动创建, 因此不实现转换
            _ => Err(ProtocolError::InvalidCommandType),
        }
    }
}
