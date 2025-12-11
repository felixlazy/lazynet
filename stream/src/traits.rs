use async_trait::async_trait;
use bytes::BufMut;

use color_eyre::eyre::Result;

/// `AsyncFrameWriter` trait 定义了异步写入一个完整数据帧的功能.
///
/// 这个 trait 抽象了向底层I/O (如 TCP 流) 写入字节数据的操作,
/// 适用于面向帧的协议.
#[async_trait]
pub trait AsyncFrameWriter {
    /// 异步地将一个 `Bytes` 帧写入到底层流中.
    ///
    /// # 参数
    /// * `frame`: 一个 `Bytes` 对象, 包含了要写入的完整数据帧.
    ///
    /// # 返回
    /// * `Ok(usize)`: 如果写入成功, 返回写入的字节数.
    /// * `Err(eyre::Report)`: 如果在写入过程中发生错误.
    async fn write_frame(&mut self, frame: &[u8]) -> Result<usize>;
}

/// `AsyncFrameReader` trait 定义了异步读取一个完整数据帧的功能.
///
/// 它负责处理从底层 I/O 读取数据, 并将其存入缓冲区的逻辑.
/// 具体的帧边界检测和解析应由协议解析器完成.
#[async_trait]
pub trait AsyncFrameReader {
    /// 异步地从底层流中读取数据, 并将其追加到 `buf` 缓冲区中.
    ///
    /// # 参数
    /// * `buf`: 一个实现了 `BufMut` 的可变缓冲区引用, 用于存放读取到的数据.
    ///
    /// # 返回
    /// * `Ok(Some(usize))`: 如果成功读取了非零字节, 返回读取的字节数.
    /// * `Ok(None)`: 如果流已经关闭 (EOF), 返回 `None`.
    /// * `Err(eyre::Report)`: 如果在读取过程中发生错误.
    async fn read_frame<B>(&mut self, buf: &mut B) -> Result<usize>
    where
        B: BufMut + ?Sized + Send;
}

/// `AsyncStreamSplit` trait 定义了一种将异步流分解为其读取和写入组件的标准方式.
pub trait AsyncStreamSplit {
    /// 关联类型, 代表流的读取器部分.
    /// 它应该实现 `AsyncFrameReader` trait.
    type Reader;

    /// 关联类型, 代表流的写入器部分.
    /// 它应该实现 `AsyncFrameWriter` trait.
    type Writer;

    /// 消耗流对象本身, 并返回其分离的、拥有所有权的读取器和写入器.
    fn into_split(self) -> (Self::Reader, Self::Writer);
}
