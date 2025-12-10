use async_trait::async_trait;
use bytes::BufMut;
use color_eyre::Result;

/// 异步帧写入器
///
/// 定义了异步写入帧数据的方法
#[async_trait]
pub trait AsyncFrameWriter {
    /// 异步写入一个数据帧
    ///
    /// 将 `buf` 中的数据写入底层传输。
    /// 返回写入的字节数。
    async fn write_frame(&mut self, buf: &[u8]) -> Result<usize>;
}
/// 异步帧读取器
///
/// 定义了异步读取帧数据的方法
#[async_trait]
pub trait AsyncFrameReader {
    /// 异步读取一个数据帧
    ///
    /// 从底层传输中读取数据并填充到 `buf` 中。
    /// 返回读取的字节数。
    async fn read_frame<B>(&mut self, buf: &mut B) -> Result<usize>
    where
        B: BufMut + ?Sized + Send;
}
