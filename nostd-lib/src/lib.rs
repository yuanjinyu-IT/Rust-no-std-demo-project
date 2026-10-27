#![no_std]
// #![cfg_attr(not(test), no_std)] // 条件编译

pub mod ring;
pub mod fixed_str;
pub mod math;

pub use fixed_str::{CapacityError,FixedString};
pub use ring::HistoryBuffer;