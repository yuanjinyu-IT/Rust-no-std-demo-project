/*
 * 这块板子的内存布局.
 *
 * cortex-m-rt 自带的 link.x 会 INCLUDE 这个文件, 并据此放置
 * 向量表、.text .data .bss 和栈.
 *
 * FLASH 的起始地址必须和 QEMU 机器匹配: lm3s6965evb 的 flash 从
 * 0x00000000 开始, 向量表必须落在 0 地址. 如果按别的板子写成 0x08000000,
 * QEMU 起跳时读不到向量表, 会直接 "Lockup: can't escalate 3 to HardFault".
 */
MEMORY
{
    FLASH : ORIGIN = 0x00000000, LENGTH = 256K
    RAM   : ORIGIN = 0x20000000, LENGTH = 64K
}