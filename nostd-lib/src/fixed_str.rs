use core::fmt;
use core::str;


/// 固定容量的 UTF-8 字符串
pub struct FixedString<const N:usize >{
    /// 有效字节在 `buf[0..len]` 里，且保证是合法的 UTF-8
    buf:[u8;N],
    len:usize
}

/// 放不下的时候返回错误
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct CapacityError;


/* 注意 CapacityError 只实现了 fmt::Display 和 Debug, 没有 实现 std::error::Error, 因为后者只在 std 里. 
这是拆掉第四个依赖的方式: 少一层 trait 而已, 用户看到的错误消息一点没少. */
impl fmt::Display for CapacityError{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("fixed-capacity string is full")
    }
}


impl<const N:usize> FixedString<N>{
    // 同样的 const ，使得可以放在 static 当中
    // 也不是说，一定要这样做才对
    pub const fn new()->Self{
        Self{
            buf:[0;N], // 这里不用 const ，直接 0 即可
            len:0
        }
    }

    pub const fn len(&self) -> usize { self.len }
    pub const fn is_empty(&self) -> bool { self.len == 0 }
    pub const fn capacity(&self) -> usize { N }
    pub const fn remaining(&self) -> usize { N - self.len }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// 以 `&str` 的形式查看内容
    pub fn as_str(&self) -> &str{

        unsafe{
            str::from_utf8_unchecked(&self.buf[0..self.len])
        }
    }

    /// 追加一段 &str ，放不下时返回 Err 且 不做任何修改
    /// String::push_str() 是没有返回值的，但是这里必须返回 Result 
    /// 因为是固定容量，必须将可能发生的错误让调用者知道！
    pub fn push_str(&mut self,s:&str)->Result<(),CapacityError>{
        let bytes = s.as_bytes();
        if self.remaining() < bytes.len(){
            return Err(CapacityError);
        }

        // 直接 copy_from_slice 即可
        self.buf[self.len..self.len+bytes.len()].copy_from_slice(bytes);
        self.len+=bytes.len();
        Ok(())
    }


    /// 追加一个 `u64` 的十进制表示, 完全不分配
    /// 这个不用纠结，就是将 u64 转为对应的 ASCII 表示了
    pub fn push_u64(&mut self, mut n: u64) -> Result<(), CapacityError> {
        let mut digits = [0u8; 20];
        let mut i = digits.len();
        loop {
            i -= 1;
            digits[i] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        // SAFETY: digits 里全是 ASCII 数字, 必然是合法 UTF-8
        let s = unsafe { str::from_utf8_unchecked(&digits[i..]) };
        self.push_str(s)
    }


}


/// 实现 fmt::Write，这样的话，`write!(fs,"x ={} ",1)` 也能用
impl<const N:usize > fmt::Write for FixedString<N>{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s).map_err(|_| fmt::Error)
    }
}

/// 把格式化结果写进调用者提供的缓冲区。
///
/// 这是 `no_std` 世界里非常典型的函数签名, 内存由调用者提供（内存本身由调用者提供，C语言很多都是这样）
pub fn write_report<const N:usize>(
    out:&mut FixedString<N>,
    values:&[u64]
) -> Result<(),CapacityError>{
    use fmt::Write as _ ;

    let mut first = true;
    write!(out,"values: ").map_err(|_|CapacityError)?;
    for v in values{
        // 这里只借用了 out，没有产生任何临时 String
        write!(out,"{}{v}",if first {""} else {", "}).map_err(|_|CapacityError)?;
        first=false;
    }

    write!(out," | sum = {}",values.iter().sum::<u64>()).map_err(|_|CapacityError)
}
