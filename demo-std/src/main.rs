use std::collections::VecDeque;
use std::fmt;

pub struct HistoryBuffer<T> {
    inner: VecDeque<T>,
    capacity: usize,
}

impl<T> HistoryBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, value: T) {
        if self.inner.len() == self.capacity {
            self.inner.pop_front();
        }
        self.inner.push_back(value);
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.inner.iter()
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    NotANumber,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Empty => write!(f, "input is empty"),
            ParseError::NotANumber => write!(f, "input is not a number"),
        }
    }
}

impl std::error::Error for ParseError {}

/// 解析一行逗号分隔的十进制整数, 返回它们的和
///
/// 这里用了两种 std 才有的东西: `Vec` 收集中间结果, `parse()` 的错误转换
pub fn sum_csv(line: &str) -> Result<u64, ParseError> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err(ParseError::Empty);
    }

    let mut total: u64 = 0;
    let parts: Vec<&str> = trimmed.split(',').collect();
    for part in parts {
        let n: u64 = part.trim().parse().map_err(|_| ParseError::NotANumber)?;
        total += n;
    }
    Ok(total)
}

/// 生成一段给用户看的报告, 返回 `String`, 每调用一次就在堆上分配一次
pub fn report(values: &[u64]) -> String {
    let mut out = String::new();
    out.push_str("values: ");
    for (i, v) in values.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        // format! 内部又会分配一个临时 String
        out.push_str(&format!("{v}"));
    }
    out.push_str(&format!(" | sum = {}", values.iter().sum::<u64>()));
    out
}

pub fn divide(a: u64, b: u64) -> u64 {
    assert!(b != 0, "division by zero: {a} / {b}");
    a / b
}


fn main() {
    println!("== demo-std ==");

    let mut history: HistoryBuffer<&str> = HistoryBuffer::new(3);
    for event in ["boot", "init", "mount", "net", "shell"] {
        history.push(event);
    }
    println!("last 3 events: {:?}", history.iter().collect::<Vec<_>>());

    println!("sum_csv(\"1, 2, 3\") = {:?}", sum_csv("1, 2, 3"));
    println!("sum_csv(\"\")        = {:?}", sum_csv(""));
    println!("report             = {}", report(&[3, 1, 4, 1, 5]));
    println!("divide(42, 6)      = {}", divide(42, 6));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_keeps_only_the_latest() {
        let mut h = HistoryBuffer::new(3);
        for v in [1, 2, 3, 4, 5] {
            h.push(v);
        }
        assert_eq!(h.len(), 3);
        assert_eq!(h.iter().copied().collect::<Vec<_>>(), vec![3, 4, 5]);
    }

    #[test]
    fn csv_parsing_works() {
        assert_eq!(sum_csv("1, 2, 3"), Ok(6));
        assert_eq!(sum_csv(""), Err(ParseError::Empty));
        assert_eq!(sum_csv("1, x"), Err(ParseError::NotANumber));
    }
}


