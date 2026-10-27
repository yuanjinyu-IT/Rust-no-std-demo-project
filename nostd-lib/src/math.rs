/// 向上取整到 `align` 的倍数, 写分配器时的必备工具
///
/// `align` 必须是 2 的幂
pub const fn align_up(addr: usize, align: usize) -> usize {
    // align 是 2 的幂, (align - 1) 是一串 1, 取反就是掩码
    (addr + align - 1) & !(align - 1)
}
