use nostd_lib::math;

use core::alloc::{GlobalAlloc,Layout}; // 全局分配的 trait 和 Layout 类型是在 core crate 的 alloc 模块 当中
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicUsize,Ordering};


/// 静态堆的大小，分配 16KB
const HEAP_SIZE:usize=16*1024;


/// 静态堆本体
/// 
/// - `UnsafeCell` 内部可变性
/// - `repr(align(16))` 保证 堆 起点至少 16 字节对齐
/// - `unsafe impl Sync` 下面所有偏移量访问都经过原子操作
///  所以并发调用 `alloc` 也不会拿到重叠区间
#[repr(align(16))]
struct Arena(UnsafeCell<[u8;HEAP_SIZE]>);

// SAFETY: 唯一会写这块内存的是 Bump::alloc,
//         它用 compare_exchange 保证不同 CPU 不会拿到重叠的区间
unsafe impl Sync for Arena {} 

/// 只增不减的分配器
struct Bump{
    arena:Arena,
    /// 下一个空闲字节相对 arena 起点的偏移
    offset:AtomicUsize,
    /// 统计分配次数    
    count:AtomicUsize,
    /// 统计历史最高水位
    peak:AtomicUsize
}


impl Bump{
    // const 函数
    const fn new() -> Self{
        Self{
            arena:Arena(UnsafeCell::new([0;HEAP_SIZE])),
            offset:AtomicUsize::new(0),
            count:AtomicUsize::new(0),
            peak:AtomicUsize::new(0),
        }
    }
}


unsafe impl GlobalAlloc for Bump{
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        
        // 如果请求分配的大小为 0 ，直接返回一个空指针
        // layout.size() == 0 是 UB
        // `Vec` 在 T 是零大小类型时不会走分配器, 但别的调用方可能会
        if layout.size()==0{
            return core::ptr::null_mut();
        }

        // 拿到堆本体的 基地址
        let base = self.arena.0.get().cast::<u8>() as usize;
        let align = layout.align().max(1);
        // 当前指向哪里
        let mut current = self.offset.load(Ordering::Relaxed);

        loop{

            // 1) 当前偏移对齐
            let start = match current.checked_add(align-1){
                Some(_)=>math::align_up(current, align),
                None=>return core::ptr::null_mut(),
            };

            // 2) 检查是否放得下
            let end = match start.checked_add(layout.size()){
                Some(end) if end <= HEAP_SIZE => end,

                // 放不下就返回空指针. **注意**: 这里不要 panic
                // 因为 `alloc` 的契约是 失败返回 null
                // 接下来是 panic 还是别的处理由 alloc crate 决定（相当于是让 alloc 这个 crate 决定后来怎么做）
                _=> return core::ptr::null_mut(),
            };

            // 3) 用 CAS 抢占 [start, end) 这段区间
            match self.offset.compare_exchange_weak(current, end, Ordering::Relaxed, Ordering::Relaxed){
                Ok(_) => {
                    self.count.fetch_add(1, Ordering::Relaxed);
                    self.peak.fetch_max(end, Ordering::Relaxed);
                    // SAFETY: [start, end) 落在 arena 内, 且刚被我们独占
                    return unsafe { (base as *mut u8).add(start) }
                },
                // 别人抢先改了 offset, 拿到新值重试
                Err(actual) => current = actual,
            }
        }

    }


    unsafe fn dealloc(&self,_ptr:*mut u8,_layout:Layout){
        // bump allocator 不复用堆内存
    }

}



/// 注册全局分配器，整个程序里 只能有一个
/// 因此是 static 的
#[global_allocator]
static ALLOCATOR:Bump = Bump::new();




/// 当前已被占走的字节数, 含对齐填充
pub fn used() -> usize {
    ALLOCATOR.offset.load(Ordering::Relaxed)
}

/// 历史最高水位
pub fn peak() -> usize {
    ALLOCATOR.peak.load(Ordering::Relaxed)
}

/// 分配次数
pub fn alloc_count() -> usize {
    ALLOCATOR.count.load(Ordering::Relaxed)
}

/// 堆的总容量
pub const fn capacity() -> usize {
    HEAP_SIZE
}