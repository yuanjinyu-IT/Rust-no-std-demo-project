//! 一个**没有任何硬件依赖**的 `core::fmt::Write` 实现: 往内存里的缓冲区写日志
//! 
//! `console.rs` 实际上是依赖于 cortex_m_semihosting 提供的 模拟主机 上的输入输出流的


use core::cell::UnsafeCell; // 内部可变性，提供的是裸指针
use core::fmt;


/// 固定容量的日志缓冲区
pub struct LogBuffer<const N:usize>{
    buf:[u8;N],
    len:usize,
    /// 因为放不下而丢掉了多少字节, 静默截断是不可接受的
    /// 调用者至少要知道, 这里丢过东西
    dropped:usize
}

#[allow(unused)]
impl<const N: usize> LogBuffer<N> {
    pub const fn new() -> Self {
        Self { buf: [0; N], len: 0, dropped: 0 }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    /// 已经丢掉的字节数
    pub const fn dropped(&self) -> usize {
        self.dropped
    }

    /// 以 `&str` 的形式查看内容
    pub fn as_str(&self) -> &str {
        // SAFETY: 由类型不变量保证
        unsafe { core::str::from_utf8_unchecked(&self.buf[..self.len]) }
    }
}

impl<const N:usize> fmt::Write for LogBuffer<N>{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        // min(能放多少，需要多少)
        let n  = core::cmp::min(bytes.len(),N-self.len);
        self.buf[self.len..self.len+n].copy_from_slice(&bytes[..n]);
        self.len+=n;
        // 放不下的部分丢掉
        // 注意：这里不能返回 Err 日志缓冲区满了，不应该让程序崩溃
        self.dropped+=bytes.len()-n;
        Ok(())
    }
}


impl<const N: usize> Default for LogBuffer<N> {
    fn default() -> Self {
        Self::new()
    }
}


/// 把 `LogBuffer` 变成可以放进 `static` 的类型
/// 需要变成可以放进 static 的类型是因为，全局只有一份日志缓冲区，都往一个缓冲区当中写日志（考虑到了现实一点的实现）
#[repr(transparent)]
pub struct SharedLog<const N: usize>(UnsafeCell<LogBuffer<N>>);


// SAFETY: 调用方必须保证不会真正并发地访问它
// 怪不得说不要手动实现这个 Sync trait ，因为可能引发 UB

/// static 的变量，Rust 当中要求必须实现 Sync 这个 trait 
/// Unsafe 本身没有实现，所以又在外面包了一层，然后 unsafe 实现，需要在程序的逻辑语义上确保不会发生并发访问，否则就是 UB
/// unsafe 本身不会做任何 unsafe 的检查，只是提供了进行 unsafe 操作的能力，程序员自己要保证不会出现问题
unsafe impl<const N: usize> Sync for SharedLog<N> {}


impl<const N: usize> SharedLog<N> {
    pub const fn new() -> Self {
        Self(UnsafeCell::new(LogBuffer::new()))
    }

    /// 拿到内部缓冲区的裸指针
    /// get() 得到的是 裸指针
    /// get_mut() 得到的是 &mut T
    pub const fn get(&self) -> *mut LogBuffer<N> {
        self.0.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::fmt::Write;

    #[test]
    fn formats_without_any_allocation() {
        let mut log = LogBuffer::<64>::new();
        write!(log, "hello {}, answer = {}", "world", 42).unwrap();
        assert_eq!(log.as_str(), "hello world, answer = 42");
    }

    #[test]
    fn overflow_is_counted_not_silently_dropped() {
        let mut log = LogBuffer::<8>::new();
        write!(log, "0123456789").unwrap();
        assert_eq!(log.as_str(), "01234567");
        assert_eq!(log.dropped(), 2);
    }
}

