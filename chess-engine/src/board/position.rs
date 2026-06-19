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
        if ('a'..='i').contains(&ch) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let pos = Position::new(4, 5);
        assert_eq!(pos.col, 4);
        assert_eq!(pos.row, 5);
    }

    #[test]
    fn test_is_valid_in_bounds() {
        // All valid board positions
        assert!(Position::new(0, 0).is_valid());
        assert!(Position::new(8, 9).is_valid());
        assert!(Position::new(4, 5).is_valid());
        assert!(Position::new(0, 9).is_valid());
        assert!(Position::new(8, 0).is_valid());
    }

    #[test]
    fn test_is_valid_out_of_bounds() {
        assert!(!Position::new(9, 0).is_valid(), "col 9 is out of bounds");
        assert!(!Position::new(0, 10).is_valid(), "row 10 is out of bounds");
        assert!(!Position::new(9, 10).is_valid(), "both out of bounds");
        assert!(!Position::new(255, 255).is_valid(), "large values out of bounds");
    }

    #[test]
    fn test_col_to_char() {
        assert_eq!(Position::col_to_char(0), Some('a'));
        assert_eq!(Position::col_to_char(4), Some('e'));
        assert_eq!(Position::col_to_char(8), Some('i'));
        assert_eq!(Position::col_to_char(9), None, "col 9 should be out of bounds");
        assert_eq!(Position::col_to_char(255), None);
    }

    #[test]
    fn test_char_to_col() {
        assert_eq!(Position::char_to_col('a'), Some(0));
        assert_eq!(Position::char_to_col('e'), Some(4));
        assert_eq!(Position::char_to_col('i'), Some(8));
        assert_eq!(Position::char_to_col('j'), None, "'j' is out of bounds");
        assert_eq!(Position::char_to_col('A'), None, "uppercase should not match");
        assert_eq!(Position::char_to_col('0'), None, "digit should not match");
    }

    #[test]
    fn test_to_uci() {
        assert_eq!(Position::new(0, 0).to_uci(), "a0");
        assert_eq!(Position::new(4, 5).to_uci(), "e5");
        assert_eq!(Position::new(8, 9).to_uci(), "i9");
    }

    #[test]
    fn test_from_uci_valid() {
        assert_eq!(Position::from_uci("a0"), Some(Position::new(0, 0)));
        assert_eq!(Position::from_uci("e5"), Some(Position::new(4, 5)));
        assert_eq!(Position::from_uci("i9"), Some(Position::new(8, 9)));
    }

    #[test]
    fn test_from_uci_invalid() {
        assert_eq!(Position::from_uci(""), None, "empty string");
        assert_eq!(Position::from_uci("a"), None, "too short");
        assert_eq!(Position::from_uci("a01"), None, "too long");
        assert_eq!(Position::from_uci("j0"), None, "col j out of bounds");
        assert_eq!(Position::from_uci("a:"), None, "non-digit row");
        assert_eq!(Position::from_uci("A0"), None, "uppercase col");
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", Position::new(0, 0)), "a0");
        assert_eq!(format!("{}", Position::new(4, 9)), "e9");
    }
}
