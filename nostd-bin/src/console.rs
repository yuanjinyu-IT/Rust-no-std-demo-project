use core::fmt;


/// 一个"写到 semihosting stdout"的 `core::fmt::Write` 实现
pub struct HostWriter;


/// 实现 `core::fmt::Write`, 它只有一个必需方法，就是 write_str()
///
/// 只要实现了它, `write!`, `writeln!`, `format_args!` 就全部可用
impl fmt::Write for HostWriter{
    // 这个是必须实现的方法
    // 需要将其写到 semihosting 的 stdout
    // 因此使用对应 crate 提供的标准输出流就可以了
    fn write_str(&mut self, s: &str) -> fmt::Result {
        cortex_m_semihosting::hio::hstdout()
            .map_err(|_| fmt::Error)?
            .write_str(s)
            .map_err(|_| fmt::Error)
    }
}

// 写一个声明宏
#[macro_export]
macro_rules! println {
    () => {
        {
            use core::fmt::Write as _; // 引入 Write trait ，目的是为了使用相关的方法
            let _ = writeln!($crate::console::HostWriter); // 往这个结构体里写数据，因为实现了 Write 所以可以做到，并且写到 semihosting 的 stdout 当中
        }
    };
    ($($arg:tt)*)=>{
        {
            use core::fmt::Write as _;
            let _ = writeln!($crate::console::HostWriter,$($arg)*);
        }
    };
}