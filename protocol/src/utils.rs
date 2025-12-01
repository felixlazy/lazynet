/// 计算给定数据的块校验码 (BCC).
///
/// BCC 是通过对数据进行异或 (XOR) 累加计算得出的.
///
/// # 参数
/// * `data`: 需要计算校验码的数据切片.
///
/// # 返回
/// 计算出的 `u8` 校验码.
pub fn calculate_bcc(data: &[u8]) -> u8 {
    data.iter().fold(0, |bcc, &byte| bcc ^ byte)
}
