use async_trait::async_trait;
use bytes::BufMut;
use color_eyre::eyre::{Result, eyre};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        TcpSocket,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    time,
};
use tracing::{info, instrument, trace};

use crate::traits::{AsyncFrameReader, AsyncFrameWriter};

/// Lazyclient结构体用于管理TCP客户端连接的读写半部以及一个用于接收数据的缓冲区.
/// `Lazyclient` 结构体用于管理TCP客户端连接的读写半部.
/// 它持有TCP流的读取和写入部分,以便进行异步数据传输.
pub struct Lazyclient {
    /// TCP连接的读取半部,用于接收数据.
    read_half: OwnedReadHalf,
    /// TCP连接的写入半部,用于发送数据.
    write_half: OwnedWriteHalf,
}

#[async_trait]
/// 为 `Lazyclient` 实现 `AsyncFrameWriter` trait, 提供异步写入帧数据的功能.
impl AsyncFrameWriter for Lazyclient {
    /// 异步写入一帧数据到TCP连接.
    ///
    /// 参数 `buf` 是要写入的数据切片.
    /// 返回写入的字节数.
    async fn write_frame(&mut self, buf: &[u8]) -> Result<usize> {
        let len = self.write_half.write(buf).await?;
        trace!("发送帧: {:X?}", buf);
        Ok(len)
    }
}

#[async_trait]
/// 为 `Lazyclient` 实现 `AsyncFrameReader` trait, 提供异步读取帧数据的功能.
impl AsyncFrameReader for Lazyclient {
    /// 异步读取一个数据帧到提供的缓冲区.
    ///
    /// 参数 `buf` 是一个实现了 `BufMut` trait 的可变缓冲区, 数据将被读取到其中.
    /// 返回读取的字节数.
    async fn read_frame<B>(&mut self, buf: &mut B) -> Result<usize>
    where
        B: BufMut + ?Sized + Send,
    {
        // 使用 `read_buf` 直接从读取半部读取数据并填充到缓冲区中.
        Ok(self.read_half.read_buf(buf).await?)
    }
}

/// 建立一个到指定地址的TCP连接.
///
/// 使用 `instrument` 宏进行tracing,并捕获连接过程中的错误.
///
/// # 参数
/// * `addr`: 实现 `AsRef<str>` trait 的类型, 表示要连接的目标地址 (例如 "127.0.0.1:8080").
/// * `timeout`: 连接尝试的超时时间.
///
/// # 返回值
/// `Result<Lazyclient>`: 如果连接成功, 返回一个 `Lazyclient` 实例; 如果发生错误 (如连接超时或连接失败), 则返回 `eyre::Error`.
///
/// # 内部实现
/// 1. 创建一个新的IPv4 TCP socket.
/// 2. 禁用Nagle算法 (`set_nodelay(true)`), 以减少数据传输延迟.
/// 3. 允许地址重用 (`set_reuseaddr(true)`), 方便快速重启服务.
/// 4. 尝试连接到远程地址, 并在指定的 `timeout` 内等待.
/// 5. 如果连接成功, 将TCP流拆分为独立的读写半部.
/// 6. 返回一个新的 `Lazyclient` 实例, 包含读写半部.
#[instrument(skip(addr))]
pub async fn connect(addr: impl AsRef<str>, timeout: Duration) -> Result<Lazyclient> {
    // 创建一个新的IPv4 TCP socket
    let socket = TcpSocket::new_v4()?;
    // 禁用Nagle算法,减少延迟
    socket.set_nodelay(true)?;
    // 允许地址重用
    socket.set_reuseaddr(true)?;

    let remote_addr = addr.as_ref().parse()?;

    // 连接到指定的地址,带超时
    let client = time::timeout(timeout, socket.connect(remote_addr))
        .await
        .map_err(|_| eyre!("连接 {} 超时", addr.as_ref()))?
        .map_err(|e| eyre!("连接 {} 失败: {}", addr.as_ref(), e))?;

    // 将TCP流拆分为读写两部分
    let (read_half, write_half) = client.into_split();
    info!("tcp连接到{}", addr.as_ref());
    // 返回一个新的Lazyclient实例
    Ok(Lazyclient {
        read_half,
        write_half,
    })
}
