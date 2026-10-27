#![no_std]
#![no_main]


// `alloc` 不在 prelude 里, 必须显式声明这一句
// 之后 `Vec`, `String`, `Box`, `format!` 才可见
extern crate alloc;


mod console;
mod logbuf;
mod heap;


use core::panic::PanicInfo;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use cortex_m_rt::entry;
use cortex_m_semihosting::debug;

use nostd_lib::{FixedString, HistoryBuffer};



/// 一个静态日志缓冲区: 不需要任何硬件就能记录信息
static LOG: logbuf::SharedLog<128> = logbuf::SharedLog::new();




/// panic 处理器: `no_std` 二进制**必须**提供, 且全程序只能有一个
///
/// 三条规则
///   1. 不能再 panic, 否则无限递归, 所以只用 `write!`, 不用 `format!`
///   2. 尽量不分配内存, 分配器本身可能会出问题
///   3. 把位置和消息都打出来, 方便定位
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!();
    println!("!!! kernal panic !!!");

    if let Some(location) = info.location(){
        println!(" at {}:{}:{}",location.file(),location.line(),location.column());
    }

    // `message()` 是 Rust 1.81 起稳定的接口, 返回实现了 `Display` 的 PanicMessage
    println!(" message: {}",info.message());
    println!("(exiting with failure)");

    debug::exit(debug::EXIT_FAILURE);
    loop{
        core::hint::spin_loop();
    }
}



/// 程序的入口点.
///
/// 注意这里没有汇编, 也没有 `#[unsafe(no_mangle)]`:
/// 栈指针、.bss 清零、.data 搬运这些事由 cortex-m-rt 的 Reset
/// 在调用这个函数之前做完. 在宿主操作系统上, 这些是 loader 的工作.
///
/// `#[entry]` 会把函数导出成 `main` 符号, 并保证签名是 `fn() -> !`.
#[entry]
fn kernel_main() -> ! {
    println!("==============================================");
    println!("  rust no_std tour / stage 3");
    println!("  target : thumbv7em-none-eabihf (Cortex-M)");
    println!("  running in QEMU lm3s6965evb");
    println!("==============================================");

    // 1. core 里的东西: 完全不需要堆
    let mut history: HistoryBuffer<&str, 3> = HistoryBuffer::new();
    for event in ["boot", "init", "mount", "net", "shell"] {
        history.push(event);
    }
    println!("[core]  last 3 events (no heap at all):");
    for event in &history {
        println!("          - {}", event);
    }

    let mut fs: FixedString<32> = FixedString::new();
    let _ = fs.push_str("fixed:");
    let _ = fs.push_u64(2026);
    println!("[core]  FixedString = {}", fs.as_str());

    // 2. alloc 里的东西: 现在有堆了
    let mut v: Vec<u32> = Vec::new();
    let (mut a, mut b) = (0u32, 1u32);
    for _ in 0..30 {
        v.push(a);
        let next = a.wrapping_add(b);
        a = b;
        b = next;
    }
    println!("[alloc] Vec<u32> = {:?}", v);

    let mut s: String = String::from("hello, ");
    s.push_str("no_std");
    s.push('!');
    println!("[alloc] String   = {}", s);

    let summary = format!(
        "{} fib numbers, sum = {}",
        v.len(),
        v.iter().map(|x| *x as u64).sum::<u64>()
    );
    println!("[alloc] format!  = {}", summary);

    let boxed: Box<u32> = Box::new(42);
    println!("[alloc] Box<u32> = {}", boxed);

    // 3. 体会这个分配器的代价
    println!();
    println!("[heap]  step 1: ask for 64 KiB, but the heap is only 16 KiB");
    let mut too_big: Vec<u8> = Vec::new();
    match too_big.try_reserve(64 * 1024) {
        Ok(()) => println!("        unexpectedly succeeded"),
        Err(e) => {
            println!("        try_reserve returned Err: {e:?}");
            println!("        ... and the program keeps running");
        }
    }

    println!("[heap]  step 2: allocate 2 KiB, then let it go out of scope");
    let before = heap::used();
    {
        let mut big: Vec<u8> = Vec::new();
        big.try_reserve(2 * 1024).ok();
        big.resize(2 * 1024, 7);
        println!("        while alive, used = {}", heap::used());
    } // <- 出了作用域, Vec 调用 dealloc
    println!(
        "        dropped it, but used is still {} (+{} bytes never came back)",
        heap::used(),
        heap::used() - before
    );

    println!("[heap]  step 3: allocate again, it can only start further ahead");
    let v2: Vec<u32> = (0..8).collect();
    println!("        v2 = {:?}", v2);
    println!("        now used = {}, peak = {}", heap::used(), heap::peak());

    println!();
    println!(
        "[heap]  capacity = {}, allocs = {}, used = {}, peak = {}",
        heap::capacity(),
        heap::alloc_count(),
        heap::used(),
        heap::peak()
    );

    // 4. 纯内存的 fmt::Write: 一个没有硬件的输出设备
    {
        use core::fmt::Write as _;
        // SAFETY: 单核单线程, 没有中断会碰它
        let log = unsafe { &mut *LOG.get() };
        let _ = write!(log, "boot ok; heap used {} bytes", heap::used());
        println!("[log]   LogBuffer = {}", log.as_str());
    }

    println!();
    println!("Done, exiting QEMU.");

    // 让 QEMU 直接结束进程(退出码 0), 不用手动 Ctrl-A X
    debug::exit(debug::EXIT_SUCCESS);

    // debug::exit 依赖调试器, 真机上不生效, 所以还是留一个停机循环
    loop {
        core::hint::spin_loop();
    }
}


