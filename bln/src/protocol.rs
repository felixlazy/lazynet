mod conversions;
mod types;
use protocol::traits::ProtocolSplit;

use crate::protocol::types::{BlnCommandDecode, BlnCommandEncoder};

#[derive(Default)]
pub struct BlnProtocol {
    /// 协议的编码器实例.
    encode: BlnCommandEncoder,
    /// 协议的解码器实例.
    decode: BlnCommandDecode,
}

impl ProtocolSplit for BlnProtocol {
    /// 定义 `BlnProtocol` 的编码器类型为 `BlnCommandEncoder`.
    type Encoder = BlnCommandEncoder;
    /// 定义 `BlnProtocol` 的解码器类型为 `BlnCommandDecode`.
    type Decode = BlnCommandDecode;

    /// 实现 `into_split`, 消耗 `BlnProtocol` 实例,
    /// 并返回其内部持有的编码器和解码器.
    fn into_split(self) -> (Self::Decode, Self::Encoder) {
        (self.decode, self.encode)
    }
}
