use thiserror::Error;

/// `Command` 定义了可以发送到设备的命令.
#[derive(Default, Debug)]
pub struct Command {
    /// 命令类型
    pub cmd_type: Vec<u8>,
    /// 命令负载
    pub payload: Option<Vec<u8>>,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("命令不适用于此协议")]
    CommandNotApplicable,
    #[error("此协议的命令类型无效")]
    InvalidCommandType,
    #[error("此命令的负载无效")]
    InvalidPayload,
}
