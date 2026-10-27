# Rust OS 基础阶段：`no_std`、裸指针与内存布局学习笔记

> 对应练习：`mem_primitivies.rs`、`bump_allocator.rs`、`free_list_allocator.rs`  
> 范围：仅整理此前讨论的 Rust OS 内核开发相关知识点和语法，不包含练习代码的错误分析或修复。

## 1. 三个练习的知识脉络

| 练习 | 核心目标 | 关键知识 |
| --- | --- | --- |
| Memory Primitives | 操作已有的内存 | 裸指针、逐字节访问、重叠区域、C 风格字符串 |
| Bump Allocator | 从堆中分配新内存 | `Layout`、地址对齐、边界检查、原子 CAS |
| Free-List Allocator | 回收并复用内存块 | 侵入式链表、指针转换、内存块头部、对齐 |

可以把三者理解为三个层次：

1. **操作内存**：通过指针读取、写入指定地址。
2. **分配内存**：计算一个满足大小和对齐要求的可用地址。
3. **管理内存**：在释放后将内存块组织起来，供后续分配复用。

---

## 2. `no_std` 与条件编译

### 2.1 `#![no_std]` 的含义

`#![no_std]` 表示 crate 不自动依赖 Rust 标准库 `std`，通常以 `core` 为基础。`core` 提供许多不依赖操作系统的功能：

```rust
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use core::sync::atomic::{AtomicUsize, Ordering};
```

- `core`：基础类型、指针操作、内存布局、原子操作等。
- `alloc`：在提供分配器的环境下，可支持 `Box`、`Vec` 等堆分配类型。
- `std`：还包含文件、线程、网络等通常依赖操作系统的功能。

**`no_std` 不等于不能使用堆**；能否使用依赖堆的类型，取决于环境是否提供了相应的分配支持。

### 2.2 `cfg_attr` 与 `cfg`

三个练习采用：

```rust
#![cfg_attr(not(test), no_std)]
```

拆解：

- `#![...]`：内部属性，作用于所在 crate 或模块。
- `cfg_attr(条件, 属性)`：条件成立时附加该属性。
- `not(test)`：没有启用 `test` 配置时为真。

因此：非测试构建启用 `no_std`；测试构建可使用 `std`。

```rust
#[cfg(test)]
mod tests {
    // 仅在对应 test 配置下参与编译
}
```

两者的区别：

- `cfg`：控制代码是否参与编译。
- `cfg_attr`：控制是否附加某个属性。

**注意**：集成测试等情况下，库本身不一定启用 `cfg(test)`。

### 2.3 `UnsafeCell` 与 `Mutex`

Free List 练习中使用条件编译切换字段：

```rust
#[cfg(test)]
free_list: std::sync::Mutex<*mut FreeBlock>,

#[cfg(not(test))]
free_list: core::cell::UnsafeCell<*mut FreeBlock>,
```

- `Mutex`：通过互斥保护共享状态。
- `UnsafeCell<T>`：Rust 内部可变性的基础原语，允许经共享引用访问可变数据，但**自身不提供线程同步**。

即使读取和设置链表头分别使用互斥锁，也不代表整个“遍历、删除、插入”操作自动具有并发安全性。

---

## 3. 裸指针的基本概念

### 3.1 `*const T` 与 `*mut T`

```rust
let p1: *const u8;
let p2: *mut u8;
```

- `*const T`：通常用于只读访问。
- `*mut T`：通常用于可变访问。

裸指针不同于 `&T` 和 `&mut T`，不会自动保证：

- 非空；
- 地址有效；
- 满足目标类型的对齐；
- 目标内存已初始化；
- 不违反别名、生命周期和并发访问规则。

创建或传递裸指针不一定需要 `unsafe`；真正解引用、读写或执行具有额外前提的偏移操作通常需要。

### 3.2 常用方法速查

| 方法 | 含义 | 调用是否需要 `unsafe` |
| --- | --- | --- |
| `is_null()` | 检查是否为空指针 | 否 |
| `cast::<U>()` | 改变指针的目标类型 | 否 |
| `add(n)` | 向后移动 `n` 个元素 | 是 |
| `sub(n)` | 向前移动 `n` 个元素 | 是 |
| `offset(n)` | 按有符号元素偏移移动 | 是 |
| `wrapping_add(n)` | 计算可回绕的元素偏移 | 否 |
| `byte_add(n)` | 按字节偏移 | 是 |
| `read()` | 从地址读取一个 `T` | 是 |
| `write(value)` | 向地址写入一个 `T` | 是 |
| `replace(value)` | 替换并返回旧值 | 是 |
| `swap(other)` | 交换两个位置的值 | 是 |
| `read_unaligned()` | 从未对齐地址读取 | 是 |
| `write_unaligned(value)` | 向未对齐地址写入 | 是 |
| `read_volatile()` | volatile 读取 | 是 |
| `write_volatile(value)` | volatile 写入 | 是 |
| `addr()` | 获取指针地址的整数表示 | 否 |
| `copy_to(dst, count)` | 复制多个元素，允许区域重叠 | 是 |
| `copy_to_nonoverlapping(dst, count)` | 复制多个元素，区域不得重叠 | 是 |

`null_mut()` 是 `core::ptr` 中的**函数**，不是指针方法；`as *mut u8` 是类型转换语法。

### 3.3 指针偏移按“元素”计数

```rust
let values = [10u32, 20, 30, 40];
let p = values.as_ptr();

unsafe {
    assert_eq!(p.add(2).read(), 30);
}
```

`p.add(2)` 移动两个 `u32` 元素，通常是 8 字节，而不是 2 字节。

- `add(n)`：向后移动 `n` 个 `T`。
- `sub(n)`：向前移动 `n` 个 `T`。
- `offset(n)`：允许正、负元素偏移。
- `byte_add(n)`：按字节偏移。

这些偏移操作必须满足相应的内存对象范围要求。`wrapping_add()` 虽然不要求计算出的地址仍在对象范围内，但**不意味着该地址可被安全解引用**。

### 3.4 `cast()` 不会创建对象

```rust
let block: *mut FreeBlock = ptr.cast::<FreeBlock>();
```

`cast()` 只是改变指针类型，不改变地址，也不会：

- 分配内存；
- 初始化 `FreeBlock`；
- 检查内存大小或对齐。

后续访问必须由程序员保证目标内存满足 `FreeBlock` 的布局和有效性要求。

### 3.5 `read()`、`write()`、`replace()`、`swap()`

```rust
let mut value = 10u32;
let p = &mut value as *mut u32;

unsafe {
    let old = p.read();       // 读取一个值
    p.write(old + 1);         // 写入，不析构旧值
    let previous = p.replace(20); // 替换并取回旧值
    assert_eq!(previous, 11);
}
```

要点：

- `read()`：读取一个 `T`，不会清除原内存；对拥有资源的非 `Copy` 类型随意重复读取可能造成重复所有权。
- `write(value)`：直接写入，不读取或析构原值，适合初始化尚未包含有效 `T` 的内存。
- `replace(value)`：要求原位置已有有效的 `T`，返回原值。
- `swap(other)`：交换两个有效位置中的值。
- `(*ptr).field`：通过裸指针解引用后访问字段，与 `ptr.read()` 读取整个对象不同。

### 3.6 非空不等于有效

```rust
if !ptr.is_null() {
    // 还不能仅凭这一条件就断言 ptr.read() 安全
}
```

非空只是一个条件；还需要检查地址范围、对齐、初始化、访问权限、别名规则等。

### 3.7 未对齐读取与写入

普通的 `read()` / `write()` 要求地址满足 `T` 的对齐要求。

```rust
let bytes = [0u8; 8];
let p = unsafe { bytes.as_ptr().add(1).cast::<u32>() };
let value = unsafe { p.read_unaligned() };
```

`read_unaligned()` 放宽的是**对齐要求**，不是内存有效性或初始化要求。

处理 `#[repr(packed)]` 字段时，不应先创建未对齐引用；应使用适当的原始地址获取方式（如 `addr_of!`）构造裸指针。

---

## 4. 内存对齐与 `Layout`（重点）

### 4.1 大小与对齐是两回事

- **Size**：对象占用多少字节。
- **Alignment**：对象起始地址需要满足的约束。

常见平台上的示例（具体以目标平台和 `size_of` / `align_of` 为准）：

| 类型 | 常见大小 | 常见对齐 |
| --- | ---: | ---: |
| `u8` | 1 B | 1 B |
| `u16` | 2 B | 2 B |
| `u32` | 4 B | 4 B |
| `u64` | 8 B | 8 B |

如果对齐要求是 8，则合法起始地址应满足：

```text
address % 8 == 0

例如：0x1000、0x1008、0x1010 ...
```

**对象的大小不一定等于对齐值，也不必仅凭 `Layout` 的要求就认为大小必须是对齐值的整数倍。**

### 4.2 为什么需要对齐

1. **硬件约束**：某些架构或指令不允许特定的未对齐访问。
2. **访问效率**：未对齐访问有时需要跨访问边界，可能增加成本。
3. **Rust 内存模型**：普通的类型化读取、写入和引用要求地址满足目标类型的对齐约束。

即使硬件支持未对齐访问，也不代表 Rust 的普通 `ptr.read()` 可以用于未对齐的 `T`。

### 4.3 `Layout` 是内存需求描述

```rust
let layout = Layout::from_size_align(24, 8).unwrap();
```

含义：

- 至少提供 24 字节；
- 返回地址满足 8 字节对齐。

`Layout` **不负责分配内存**；真正执行分配的是分配器。

```rust
let size = layout.size();
let align = layout.align();
```

也可以从类型推导：

```rust
let layout = Layout::new::<u64>();
```

`Layout` 要求对齐值为非零的 2 的幂，并满足布局有效性限制。

### 4.4 向上对齐公式

```rust
let start = (addr + align - 1) & !(align - 1);
```

其中 `align` 是 2 的幂。

例如：

```text
addr = 13
align = 8

13 + 7 = 20

20       = 0001_0100
!(8 - 1) = ...1111_1000
----------------------
start    = 0001_0000 = 16
```

它的作用是：找到**不小于当前地址**、且满足对齐要求的最小地址。

为什么先加 `align - 1`？因为要**向上取整**，而不是直接把低位清零后向下舍入。

### 4.5 地址计算与溢出

在分配器中，地址加法应注意 `usize` 溢出：

```rust
let Some(aligned_addr) = current.checked_add(align - 1) else {
    return null_mut();
};

let start = aligned_addr & !(align - 1);

let Some(end) = start.checked_add(layout.size()) else {
    return null_mut();
};
```

- `checked_add()` 返回 `Some(结果)` 或 `None`。
- 整数溢出检查与堆边界检查是**不同的约束**。
- 堆边界通常使用 `end > heap_end` 判断。

### 4.6 为什么 Bump 直接使用 `Layout`，Free List 要取最大值

**Bump Allocator** 只维护下一个可用位置：

```rust
let size = layout.size();
let align = layout.align();
```

它不需要在用户分配的内存块中写入链表管理信息。

**Free-List Allocator** 在释放后，会把原来的内存块开头作为 `FreeBlock`：

```rust
struct FreeBlock {
    size: usize,
    next: *mut FreeBlock,
}
```

所以同一块内存在不同阶段要满足两类要求：

| 阶段 | 大小要求 | 对齐要求 |
| --- | --- | --- |
| 分配给用户 | `layout.size()` | `layout.align()` |
| 释放后作为链表节点 | `size_of::<FreeBlock>()` | `align_of::<FreeBlock>()` |

因此：

```rust
let size = layout.size().max(core::mem::size_of::<FreeBlock>());
let align = layout.align().max(core::mem::align_of::<FreeBlock>());
```

**本质原因**：Free List 使用**侵入式链表（Intrusive Linked List）**，释放后的用户内存本身被用来存储链表节点。

在常见 64 位平台上，`FreeBlock` 的两个字段通常共占 16 字节、对齐为 8 字节，但应以实际目标平台为准。

---

## 5. Bump Allocator 的基础机制

Bump Allocator 维护一个表示下一可用地址的指针或整数：

```rust
next: AtomicUsize
```

分配过程：

1. 读取 `next`；
2. 根据 `Layout` 对齐起始地址；
3. 计算结束地址；
4. 检查溢出和堆边界；
5. 使用 CAS 尝试更新 `next`；
6. CAS 失败则重试，成功则返回起始地址。

### 5.1 CAS：Compare-And-Swap

```rust
let mut current = self.next.load(Ordering::SeqCst);

loop {
    // 根据 current 计算 start 和 end

    match self.next.compare_exchange_weak(
        current,
        end,
        Ordering::SeqCst,
        Ordering::Relaxed,
    ) {
        Ok(_) => return start as *mut u8,
        Err(actual) => current = actual,
    }
}
```

CAS 表示：只有当前值仍等于预期值时，才把它更新为新值。

- `compare_exchange()`：强 CAS，不允许伪失败。
- `compare_exchange_weak()`：弱 CAS，允许伪失败。
- **两者都可能因为并发竞争失败，都可能需要重试循环。**

`Ordering::SeqCst` 提供较强的原子内存顺序；`Relaxed` 不提供额外的跨操作同步顺序。

### 5.2 `dealloc` 与 `reset`

Bump Allocator 不逐个回收对象，`dealloc` 可以为空实现。

`reset()` 把 `next` 移回堆起点，相当于整体重新使用堆区域。调用时必须确保旧分配不再被使用，并避免与并发分配发生冲突。

---

## 6. Free List 与侵入式链表

Free List 使用链表保存可复用的内存块：

```text
free_list -> [size | next] -> [size | next] -> null
```

每个空闲块的起始位置存放一个 `FreeBlock`。

核心概念：

- **First-fit**：从链表头开始，寻找第一个满足大小和对齐要求的空闲块。
- **Head**：链表头指针，可能为空。
- **Previous / Current**：遍历链表时的前驱和当前节点。
- **Intrusive list**：节点信息直接存储在被管理的内存块内部。

释放时的核心内存操作：

```rust
let block = ptr.cast::<FreeBlock>();

unsafe {
    block.write(FreeBlock {
        size,
        next: head,
    });
}
```

这里：

1. `cast` 只转换指针类型。
2. `write` 才真正把链表头部写入内存。
3. 释放后的内存不再作为用户对象使用，而成为分配器的管理节点。

---

## 7. Rust 2024 的 `unsafe` 语法

### 7.1 `unsafe fn` 与 `unsafe {}`

```rust
pub unsafe fn read_byte(ptr: *const u8) -> u8 {
    unsafe { ptr.read() }
}
```

两种 `unsafe` 含义不同：

- `unsafe fn`：**调用者**必须满足额外的安全前提。
- `unsafe {}`：**实现者**在此处承担不安全操作正确性的证明责任。

Rust 2024 中，`unsafe_op_in_unsafe_fn` 默认会对在 `unsafe fn` 内未显式使用 `unsafe` 块的不安全操作给出警告。

### 7.2 `unsafe impl`

```rust
unsafe impl GlobalAlloc for BumpAllocator {
    // ...
}
```

表示实现者承诺满足 `GlobalAlloc` 这一 unsafe trait 的安全契约。

它不表示实现中的每个操作都自动安全。

### 7.3 `pub unsafe extern "C" fn`

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_memcpy(
    dst: *mut u8,
    src: *const u8,
    n: usize,
) -> *mut u8 {
    // ...
}
```

| 语法 | 含义 |
| --- | --- |
| `pub` | 对模块外可见 |
| `unsafe fn` | 调用有额外安全前提 |
| `extern "C"` | 使用目标平台的 C ABI |
| `#[unsafe(no_mangle)]` | 不修饰导出的符号名，并显式承担符号安全责任 |

**ABI**（应用程序二进制接口）约定参数传递、返回值、寄存器使用等细节，有助于 Rust 与 C、汇编或其他底层代码交互。

Rust 2024 还要求外部函数声明块使用 `unsafe extern`：

```rust
unsafe extern "C" {
    fn external_function();
}
```

它与**定义函数**时使用的 `pub unsafe extern "C" fn` 不是同一种语法场景。

---

## 8. `volatile` 语义与 MMIO

### 8.1 `volatile` 是什么

**Volatile 访问表示：读写动作本身具有可观察意义，编译器不能仅因其结果看似无用，就像普通内存操作那样删除或合并这些访问。**

Rust 裸指针提供：

```rust
ptr.read_volatile();
ptr.write_volatile(value);
```

### 8.2 为什么硬件寄存器需要它

在内存映射 I/O（MMIO）中，特定地址对应硬件寄存器。

例如，连续向设备控制寄存器写入不同值，可能分别表示：

```text
写入 1 -> 启动
写入 2 -> 重置
写入 3 -> 停止
```

即使最终值是 3，前两次写入也可能触发过真实硬件动作，不能因为“后来被覆盖”就删除。

轮询设备状态也常使用 volatile：

```rust
const STATUS_REG: *const u32 = 0x1000_0000 as *const u32;

unsafe {
    loop {
        let status = STATUS_REG.read_volatile();

        if status & 1 != 0 {
            break;
        }
    }
}
```

地址仅用于示意；实际 MMIO 地址、映射、访问宽度和顺序应以目标硬件要求为准。

### 8.3 `volatile`、`Atomic`、`Mutex` 的区别

| 机制 | 解决的问题 | 是否提供线程同步 |
| --- | --- | --- |
| 普通读写 | 访问一般内存数据 | 否 |
| `volatile` | 保留具有可观察意义的读写动作 | 否 |
| `AtomicUsize` | 原子访问、CAS、内存序 | 对原子对象提供相应保证 |
| `Mutex` | 保护共享数据的临界区 | 正确使用时可以 |

特别注意：

- `volatile` **不是**原子操作。
- `volatile` **不是**线程同步机制。
- `volatile` **不是**通用 CPU 内存屏障。
- `no_std` **不意味着**所有内存访问都应使用 volatile。

Bump Allocator 使用 CAS，是为了处理并发更新；设备寄存器使用 volatile，是为了保留对硬件有意义的访问。两者目的不同。

---

## 9. 知识点速记

1. **`Layout`**：描述内存大小与对齐要求，本身不分配内存。
2. **Alignment**：限制对象起始地址；普通类型化访问必须满足相应对齐。
3. **Bump**：只推进下一可用地址，因此直接依据用户 `Layout` 分配。
4. **Free List**：释放后把内存块改作链表节点，因此还要满足 `FreeBlock` 的大小和对齐要求。
5. **`add(n)`**：按元素偏移，不是默认按字节偏移。
6. **`cast()`**：只改变指针类型，不初始化内存。
7. **`write()`**：写入值，不先读取或析构旧值。
8. **`read_unaligned()`**：只放宽对齐要求，不放宽其他安全前提。
9. **`unsafe fn`**：约束调用者；**`unsafe {}`**：标记实现中的不安全操作。
10. **`extern "C"`**：指定 C ABI；**`#[unsafe(no_mangle)]`**：控制导出符号名。
11. **CAS**：通过比较并交换来原子更新共享状态。
12. **Volatile**：保证相关读写具有可观察性，不等于 atomic、mutex 或内存屏障。

**整体思维模型：`Layout` 规定内存需要满足的条件，Allocator 决定从哪里提供内存，裸指针用于访问内存，而 `unsafe` 要求程序员证明访问满足 Rust 的安全契约。**
