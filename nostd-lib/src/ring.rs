use core::mem::MaybeUninit;
use core::ptr;

/// 固定容量, 满了顶掉最老元素的环形缓冲区
///
/// ```txt
///         head（下一个写入位置）
///          |
///   +---+---+---+---+
///   | 3 | 4 |   | 1 |
///   +---+---+---+---+
///              ^
///              tail（下一个读出位置）
///
///   len = 3，逻辑顺序: 1, 3, 4
/// ```
pub struct HistoryBuffer<T,const N:usize >{  // const N:usize 泛型类型常量语法，可以在编译期确定大小
    /// `MayBeUninit<T>` 表示这块内存是给 T 留的，但现在可能是未初始化的
    buf:[MaybeUninit<T>;N],
    /// 下一个写入位置
    head:usize,
    /// 下一个读出位置
    tail:usize,
    /// 当前元素个数
    len:usize
}

impl<T,const N:usize > HistoryBuffer<T,N>{
    pub const fn new()->Self{
        Self{
            buf:[const {MaybeUninit::uninit()};N],
            head:0,
            tail:0,
            len:0,
        }
        
    }

    pub const fn capacity(&self) -> usize {
        N
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool { 
        self.len==0
    }

    pub const fn is_full(&self) -> bool {
        self.len==N
    }

}


impl<T,const N:usize> HistoryBuffer<T,N>{
    /// #Safety
    /// `index` 必须小于 `N`
    #[inline]
    unsafe fn slot(&self,index:usize)-> * const T{
        debug_assert!(index<N);
        
        // `MaybeUninit<T>` 和 `T` 布局相同, 可以直接把指针转过去
        // 这个好理解，使用裸指针指向对应位置，返回对应元素的裸指针
        unsafe{
            self.buf.as_ptr().add(index).cast::<T>()
        }
    }


    /// #Safety
    /// `index` 必须小于 `N`
    #[inline]
    unsafe fn slot_mut(&mut self,index:usize) -> *mut T{
        debug_assert!(index<N);
        unsafe{
            self.buf.as_mut_ptr().add(index).cast::<T>()
        }
    }

    /// 推入一个元素，满了就丢掉最老的
    pub fn push(&mut self,value:T){
        if N==0 {
            return;    
        }

        // 先把最老的取出来丢掉，腾出 head 的位置
        if self.is_full(){
            drop(self.pop())
        }

        // SAFETY：head in [0,N)，且 len < N 时这个槽位一定是未初始化的
        unsafe{
            // 裸指针的 写 函数
            // 传入对应位置（head）的可变裸指针，以及对应值
            ptr::write(self.slot_mut(self.head),value);
        }

        self.head = (self.head+1) % N;
        self.len+=1;
    }

    /// 弹出最老的元素
    pub fn pop(&mut self)->Option<T>{
        if self.is_empty(){
            return None;
        }
        
        // SAFETY: tail in [0, N) 且该槽位已被 push 初始化过（len > 0）
        // 用 ptr::read 把值搬走, 此后该槽位逻辑上又变回未初始化
        // 所以下面不会重复 drop
        let value = unsafe{
            ptr::read(self.slot(self.tail))
        };

        self.tail = (self.tail+1) % N;
        self.len-=1;
        Some(value)
    }

    pub fn peek(&self)->Option<&T>{
        if self.is_empty(){
            return None;
        }

        Some(unsafe{&*self.slot(self.tail)})
    }

    // 按逻辑顺序遍历
    pub fn iter(&self) -> impl Iterator<Item=&T> +'_{
        (0..self.len).map(move|i|{
            let index = (self.tail+i) % N;
            
            // SAFETY: i < len ，所以这些槽位已经被初始化
            unsafe{
                &*self.slot(index)
            } 
        })
    }

    
}

// 别忘了 Drop
// 如果 T 不是 Copy (比如 Box<u8>、String), 不实现 Drop 就直接泄漏
impl<T, const N: usize> Drop for HistoryBuffer<T, N> {
    fn drop(&mut self) {
        // pop() 会把取出的值就地 drop 掉
        // 为什么这里可以发生析构呢？就是因为在 pop() 返回了 Option<T> 之后，Option<T> 的作用域结束
        // 因而，Option<T> 被 Rust 自动清理，所以 T 也被清理了
        while self.pop().is_some() {}

        // 也可以用下面的写法
        // 相当于是更加显式地表明析构含义
        // while let Some(value) = self.pop(){
        //     drop(value)
        // }
    }
}


/// 一个会记账的 Drop 类型, 用来验证 `HistoryBuffer` 有没有泄漏
#[allow(unused)]
struct DropCounter<'a> {
    counter: &'a core::cell::Cell<u32>,
}

impl Drop for DropCounter<'_> {
    fn drop(&mut self) {
        self.counter.set(self.counter.get() + 1);
    }
}

/// 顶掉老元素时, 那些元素必须被 drop, 而不是泄漏.
#[test]
fn history_buffer_drops_replaced_elements() {
    let dropped = core::cell::Cell::new(0);
    {
        let mut buf: HistoryBuffer<DropCounter<'_>, 4> = HistoryBuffer::new();
        // 塞 10 个进容量 4 的缓冲区: 前 6 个会被顶掉.
        for _ in 0..10 {
            buf.push(DropCounter { counter: &dropped });
        }
        assert_eq!(buf.len(), 4);
        assert_eq!(dropped.get(), 6);
    }
    // 出了作用域, 剩下的 4 个也必须被 drop. 一共 10 次, 没有泄漏.
    assert_eq!(dropped.get(), 10);
}






// ====== 实现迭代器相关 trait ======


impl<'a, T, const N: usize> IntoIterator for &'a HistoryBuffer<T, N> {
    type Item = &'a T;
    type IntoIter = HistoryIter<'a, T, N>;

    fn into_iter(self) -> Self::IntoIter {
        HistoryIter { buf: self, index: 0 }
    }
}

// 单独的一个 HistoryIter，内部是对 HistoryBuffer 的引用
// 那么这个迭代器就不可能拿走 Buffer 当中的所有权，不论是 into_iter() 还是 iter() 
pub struct HistoryIter<'a, T, const N: usize> {
    buf: &'a HistoryBuffer<T, N>,
    index: usize,
}

impl<'a, T, const N: usize> Iterator for HistoryIter<'a, T, N> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.buf.len {
            return None;
        }
        let slot = (self.buf.tail + self.index) % N;
        self.index += 1;
        // SAFETY: index < len, 槽位一定被初始化过
        Some(unsafe { &*self.buf.slot(slot) })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.buf.len - self.index;
        (remaining, Some(remaining))
    }
}

impl<T, const N: usize> ExactSizeIterator for HistoryIter<'_, T, N> {}



