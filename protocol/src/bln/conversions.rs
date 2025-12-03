use super::types::{BlnErrorCause, BlnProtocolType, BlnResponseStatus};
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
            .ok_or(ProtocolError::InvalidPayload)? // 如果不存在则视为无效
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
            // ErrorRsp, SetPositionRsp, PositionReached 等响应类型不应由从机主动创建, 因此不实现转换为 Command
            _ => Err(ProtocolError::InvalidCommandType),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::bln::types::{BlnErrorCause, BlnProtocolType, BlnResponseStatus};
    use crate::types::{Command, ProtocolError};
    use bytes::{BufMut, BytesMut};

    // Helper to create a BytesMut from a slice
    fn b(s: &[u8]) -> BytesMut {
        let mut buf = BytesMut::new();
        buf.put_slice(s);
        buf
    }

    // --- Tests for TryFrom<Command> for BlnProtocolType ---

    #[test]
    fn test_parse_error_rsp_success() {
        let mut payload = BytesMut::new();
        payload.put_u8(0x01); // ChecksumError
        let command = Command {
            cmd_type: b(&[0x00]), // Dummy command type, doesn't matter for error
            response_status: Some(BlnResponseStatus::Error.into()),
            payload: Some(payload),
        };
        let result: Result<BlnProtocolType, ProtocolError> = command.try_into();
        assert_eq!(
            result,
            Ok(BlnProtocolType::ErrorRsp(BlnErrorCause::ChecksumError))
        );
    }

    #[test]
    fn test_parse_error_rsp_invalid_payload_len() {
        let mut payload = BytesMut::new();
        payload.put_slice(&[0x01, 0x02]); // Too long
        let command = Command {
            cmd_type: b(&[0x00]),
            response_status: Some(BlnResponseStatus::Error.into()),
            payload: Some(payload),
        };
        let result: Result<BlnProtocolType, ProtocolError> = command.try_into();
        assert_eq!(result, Err(ProtocolError::InvalidPayload));
    }

    #[test]
    fn test_parse_error_rsp_no_payload() {
        let command = Command {
            cmd_type: b(&[0x00]),
            response_status: Some(BlnResponseStatus::Error.into()),
            payload: None,
        };
        let result: Result<BlnProtocolType, ProtocolError> = command.try_into();
        assert_eq!(result, Err(ProtocolError::InvalidPayload));
    }

    #[test]
    fn test_parse_set_position_rsp_ok() {
        let command = Command {
            cmd_type: b(&[0x91]),
            response_status: Some(BlnResponseStatus::Ok.into()),
            payload: None,
        };
        let result: Result<BlnProtocolType, ProtocolError> = command.try_into();
        assert_eq!(result, Ok(BlnProtocolType::SetPositionRsp));
    }

    #[test]
    fn test_parse_set_position_rsp_ok_with_data_invalid_status() {
        let command = Command {
            cmd_type: b(&[0x91]),
            response_status: Some(BlnResponseStatus::Ok.into()), // Should be OkWithData
            payload: Some(b(&[0; 8])),
        };
        let result: Result<BlnProtocolType, ProtocolError> = command.try_into();
        assert_eq!(result, Err(ProtocolError::InvalidPayload)); // Status mismatch
    }

    #[test]
    fn test_parse_set_position_rsp_ok_with_data_success() {
        let mut payload = BytesMut::new();
        payload.put_f32_le(10.5);
        payload.put_f32_le(20.5);
        let command = Command {
            cmd_type: b(&[0x91]),
            response_status: Some(BlnResponseStatus::OkWithData.into()),
            payload: Some(payload),
        };
        let result: Result<BlnProtocolType, ProtocolError> = command.try_into();
        assert_eq!(result, Ok(BlnProtocolType::PositionReached(10.5, 20.5)));
    }

    #[test]
    fn test_parse_set_position_rsp_ok_with_data_invalid_payload_len() {
        let command = Command {
            cmd_type: b(&[0x91]),
            response_status: Some(BlnResponseStatus::OkWithData.into()),
            payload: Some(b(&[0; 7])), // Wrong length
        };
        let result: Result<BlnProtocolType, ProtocolError> = command.try_into();
        assert_eq!(result, Err(ProtocolError::InvalidPayload));
    }

    #[test]
    fn test_parse_get_position_rsp_success() {
        let mut payload = BytesMut::new();
        payload.put_f32_le(10.0);
        payload.put_f32_le(20.0);
        payload.put_u8(5); // Dummy u8
        let command = Command {
            cmd_type: b(&[0x93]),
            response_status: Some(BlnResponseStatus::OkWithData.into()),
            payload: Some(payload),
        };
        let result: Result<BlnProtocolType, ProtocolError> = command.try_into();
        assert_eq!(result, Ok(BlnProtocolType::GetPositionRsp(10.0, 20.0, 5)));
    }

    #[test]
    fn test_parse_get_position_rsp_invalid_status() {
        let command = Command {
            cmd_type: b(&[0x93]),
            response_status: Some(BlnResponseStatus::Ok.into()), // Should be OkWithData
            payload: Some(b(&[0; 9])),
        };
        let result: Result<BlnProtocolType, ProtocolError> = command.try_into();
        assert_eq!(result, Err(ProtocolError::InvalidPayload)); // Status mismatch
    }

    #[test]
    fn test_parse_get_position_rsp_invalid_payload_len() {
        let mut payload = BytesMut::new();
        payload.put_f32_le(10.0);
        payload.put_f32_le(20.0);
        // Missing last u8
        let command = Command {
            cmd_type: b(&[0x93]),
            response_status: Some(BlnResponseStatus::OkWithData.into()),
            payload: Some(payload),
        };
        let result: Result<BlnProtocolType, ProtocolError> = command.try_into();
        assert_eq!(result, Err(ProtocolError::InvalidPayload));
    }

    #[test]
    fn test_parse_invalid_cmd_type() {
        let command = Command {
            cmd_type: b(&[0xAA]), // Unknown command
            response_status: Some(BlnResponseStatus::Ok.into()),
            payload: None,
        };
        let result: Result<BlnProtocolType, ProtocolError> = command.try_into();
        assert_eq!(result, Err(ProtocolError::InvalidCommandType));
    }

    #[test]
    fn test_parse_empty_cmd_type() {
        let command = Command {
            cmd_type: BytesMut::new(), // Empty command type
            response_status: Some(BlnResponseStatus::Ok.into()),
            payload: None,
        };
        let result: Result<BlnProtocolType, ProtocolError> = command.try_into();
        assert_eq!(result, Err(ProtocolError::InvalidCommandType));
    }

    // --- Tests for TryFrom<BlnProtocolType> for Command ---

    #[test]
    fn test_create_set_position_rsq() {
        let bln_cmd = BlnProtocolType::SetPositionRsq(10.0, 20.0);
        let result: Result<Command, ProtocolError> = bln_cmd.try_into();
        assert!(result.is_ok());
        let command = result.unwrap();

        assert_eq!(command.cmd_type.as_ref(), &[0x31]);
        assert_eq!(command.response_status, None);

        let mut expected_payload = BytesMut::new();
        expected_payload.put_f32_le(10.0);
        expected_payload.put_f32_le(20.0);
        assert_eq!(command.payload.unwrap().as_ref(), expected_payload.as_ref());
    }

    #[test]
    fn test_create_get_position_rsq() {
        let bln_cmd = BlnProtocolType::GetPositionRsq;
        let result: Result<Command, ProtocolError> = bln_cmd.try_into();
        assert!(result.is_ok());
        let command = result.unwrap();

        assert_eq!(command.cmd_type.as_ref(), &[0x33]);
        assert_eq!(command.response_status, None);
        assert_eq!(command.payload, None);
    }

    #[test]
    fn test_create_unsupported_bln_type() {
        let bln_cmd = BlnProtocolType::SetPositionRsp; // Response type, cannot create
        let result: Result<Command, ProtocolError> = bln_cmd.try_into();
        assert_eq!(result, Err(ProtocolError::InvalidCommandType));
    }

    #[test]
    fn test_create_error_rsp_bln_type() {
        let bln_cmd = BlnProtocolType::ErrorRsp(BlnErrorCause::ChecksumError); // Response type, cannot create
        let result: Result<Command, ProtocolError> = bln_cmd.try_into();
        assert_eq!(result, Err(ProtocolError::InvalidCommandType));
    }
}
