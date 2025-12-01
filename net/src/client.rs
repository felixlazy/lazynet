use bytes::BytesMut;
use color_eyre::eyre::Result;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        TcpSocket,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
};
use tracing::{info, instrument, trace};

/// Lazyclient结构体用于管理TCP客户端连接的读写半部以及一个用于接收数据的缓冲区.
pub struct Lazyclient {
    /// TCP连接的读取半部
    read_half: OwnedReadHalf,
    /// TCP连接的写入半部
    write_half: OwnedWriteHalf,
    /// 用于接收数据的缓冲区
    buf: bytes::BytesMut,
}

impl Lazyclient {
    /// 异步读取一帧数据到内部缓冲区.
    /// 返回读取到的字节数.
    pub async fn read_frame(&mut self) -> Result<usize> {
        let len = self.read_half.read_buf(&mut self.buf).await?;
        trace!("接收到的帧{:X?}", &self.buf[..len]);
        Ok(len)
    }

    /// 异步写入一帧数据到TCP连接.
    /// 参数 `buf` 是要写入的数据切片.
    /// 返回写入的字节数.
    pub async fn write_frame(&mut self, buf: &[u8]) -> Result<usize> {
        let len = self.write_half.write(buf).await?;
        trace!("发送的帧{:X?}", buf);
        Ok(len)
    }

    /// 获取内部缓冲区的不可变引用.
    pub fn buf(&self) -> &[u8] {
        &self.buf
    }

    /// 获取内部可变缓冲区 `BytesMut` 的可变引用.
    /// 允许外部代码直接操作底层的缓冲区.
    pub fn get_bytes_mut(&mut self) -> &mut BytesMut {
        &mut self.buf
    }

    /// 从内部缓冲区中分割出指定长度的数据并以十六进制打印.
    /// `len` 参数表示要分割并打印的字节长度.
    /// 分割出的数据将从缓冲区中移除,缓冲区剩余部分不变.
    pub fn into_print(&mut self, len: usize) {
        println!("{:02X?}", self.buf.split_to(len).as_ref());
    }
}

/// 建立一个到指定地址的TCP连接.
/// 使用 `instrument` 宏进行tracing,并捕获错误.
/// `addr` 参数是要连接的地址, `buf_len` 是内部缓冲区的初始容量.
#[instrument(skip(addr))]
pub async fn connect(addr: impl AsRef<str>, buf_len: usize) -> Result<Lazyclient> {
    // 创建一个新的IPv4 TCP socket
    let socket = TcpSocket::new_v4()?;
    // 禁用Nagle算法,减少延迟
    socket.set_nodelay(true)?;
    // 允许地址重用
    socket.set_reuseaddr(true)?;
    // 连接到指定的地址
    let client = socket.connect(addr.as_ref().parse()?).await?;
    // 将TCP流拆分为读写两部分
    let (read_half, write_half) = client.into_split();
    info!("tcp连接到{}", addr.as_ref());
    // 返回一个新的Lazyclient实例
    Ok(Lazyclient {
        read_half,
        write_half,
        buf: bytes::BytesMut::with_capacity(buf_len),
    })
}
