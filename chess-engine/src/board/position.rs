use std::fmt;

/// 棋盘坐标 (列 0-8, 行 0-9)
/// 行 0 = 黑方底线, 行 9 = 红方底线
/// 列 0 = 最左列 (a), 列 8 = 最右列 (i)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Position {
    pub col: u8,  // 0-8
    pub row: u8,  // 0-9
}

impl Position {
    pub fn new(col: u8, row: u8) -> Self {
        Self { col, row }
    }

    /// 坐标是否在棋盘内
    pub fn is_valid(&self) -> bool {
        self.col <= 8 && self.row <= 9
    }

    /// 列映射字符: a=0, b=1, ..., i=8
    pub fn col_to_char(col: u8) -> Option<char> {
        if col <= 8 {
            Some((b'a' + col) as char)
        } else {
            None
        }
    }

    /// 字符映射到列号
    pub fn char_to_col(ch: char) -> Option<u8> {
        if ch >= 'a' && ch <= 'i' {
            Some((ch as u8) - b'a')
        } else {
            None
        }
    }

    /// 转为 UCI 字符串 (如 "a0")
    pub fn to_uci(&self) -> String {
        let col_char = Self::col_to_char(self.col).unwrap();
        format!("{}{}", col_char, self.row)
    }

    /// 从 UCI 字符串解析 (如 "a0")
    pub fn from_uci(s: &str) -> Option<Self> {
        let bytes = s.as_bytes();
        if bytes.len() != 2 {
            return None;
        }
        let col = Self::char_to_col(bytes[0] as char)?;
        let row = bytes[1].checked_sub(b'0')?;
        let pos = Self::new(col, row);
        if pos.is_valid() {
            Some(pos)
        } else {
            None
        }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_uci())
    }
}
