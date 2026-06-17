use std::fmt;

/// 棋子颜色
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Color {
    Red,    // 红方
    Black,  // 黑方
}

impl Color {
    pub fn opposite(&self) -> Color {
        match self {
            Color::Red => Color::Black,
            Color::Black => Color::Red,
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Color::Red => write!(f, "red"),
            Color::Black => write!(f, "black"),
        }
    }
}

/// 棋子类型
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PieceType {
    King,     // 帅/将
    Advisor,  // 仕/士
    Bishop,   // 相/象
    Knight,   // 马
    Rook,     // 车
    Cannon,   // 炮
    Pawn,     // 兵/卒
}

impl PieceType {
    /// 棋子基础分值
    pub fn base_value(&self) -> i32 {
        match self {
            PieceType::King => 10000,
            PieceType::Rook => 600,
            PieceType::Cannon => 285,
            PieceType::Knight => 270,
            PieceType::Bishop => 120,
            PieceType::Advisor => 120,
            PieceType::Pawn => 30,
        }
    }

    /// 棋子中文名称
    pub fn chinese_name(&self, color: Color) -> &'static str {
        match (self, color) {
            (PieceType::King, Color::Red) => "帅",
            (PieceType::King, Color::Black) => "将",
            (PieceType::Advisor, Color::Red) => "仕",
            (PieceType::Advisor, Color::Black) => "士",
            (PieceType::Bishop, Color::Red) => "相",
            (PieceType::Bishop, Color::Black) => "象",
            (PieceType::Knight, _) => "马",
            (PieceType::Rook, _) => "车",
            (PieceType::Cannon, _) => "炮",
            (PieceType::Pawn, Color::Red) => "兵",
            (PieceType::Pawn, Color::Black) => "卒",
        }
    }

    /// 从 FEN 字符解析棋子类型
    pub fn from_fen_char(ch: char) -> Option<PieceType> {
        match ch {
            'K' | 'k' => Some(PieceType::King),
            'A' | 'a' => Some(PieceType::Advisor),
            'B' | 'b' => Some(PieceType::Bishop),
            'N' | 'n' => Some(PieceType::Knight),
            'R' | 'r' => Some(PieceType::Rook),
            'C' | 'c' => Some(PieceType::Cannon),
            'P' | 'p' => Some(PieceType::Pawn),
            _ => None,
        }
    }

    /// 转为 FEN 字符 (需配合颜色)
    pub fn to_fen_char(&self, color: Color) -> char {
        let ch = match self {
            PieceType::King => 'K',
            PieceType::Advisor => 'A',
            PieceType::Bishop => 'B',
            PieceType::Knight => 'N',
            PieceType::Rook => 'R',
            PieceType::Cannon => 'C',
            PieceType::Pawn => 'P',
        };
        if color == Color::Black {
            ch.to_ascii_lowercase()
        } else {
            ch
        }
    }
}

impl fmt::Display for PieceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PieceType::King => write!(f, "king"),
            PieceType::Advisor => write!(f, "advisor"),
            PieceType::Bishop => write!(f, "bishop"),
            PieceType::Knight => write!(f, "knight"),
            PieceType::Rook => write!(f, "rook"),
            PieceType::Cannon => write!(f, "cannon"),
            PieceType::Pawn => write!(f, "pawn"),
        }
    }
}

/// 棋子 = 颜色 + 类型
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Piece {
    pub color: Color,
    pub piece_type: PieceType,
}

impl Piece {
    pub fn new(color: Color, piece_type: PieceType) -> Self {
        Self { color, piece_type }
    }

    /// 基础分值
    pub fn base_value(&self) -> i32 {
        self.piece_type.base_value()
    }

    /// 从 FEN 字符解析棋子
    pub fn from_fen_char(ch: char) -> Option<Piece> {
        let color = if ch.is_ascii_uppercase() { Color::Red } else { Color::Black };
        let piece_type = PieceType::from_fen_char(ch)?;
        Some(Piece::new(color, piece_type))
    }

    /// 转为 FEN 字符
    pub fn to_fen_char(&self) -> char {
        self.piece_type.to_fen_char(self.color)
    }

    /// 中文名称
    pub fn chinese_name(&self) -> &'static str {
        self.piece_type.chinese_name(self.color)
    }
}

impl fmt::Display for Piece {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.color, self.piece_type)
    }
}

/// 走法
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Move {
    pub from: crate::board::Position,
    pub to: crate::board::Position,
}

impl Move {
    pub fn new(from: crate::board::Position, to: crate::board::Position) -> Self {
        Self { from, to }
    }

    /// 转为 UCI 字符串 (如 "a0a1")
    pub fn to_uci(&self) -> String {
        format!("{}{}", self.from.to_uci(), self.to.to_uci())
    }

    /// 从 UCI 字符串解析 (如 "a0a1")
    pub fn from_uci(s: &str) -> Option<Self> {
        if s.len() != 4 {
            return None;
        }
        let from = crate::board::Position::from_uci(&s[0..2])?;
        let to = crate::board::Position::from_uci(&s[2..4])?;
        Some(Self::new(from, to))
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_uci())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Color tests ===

    #[test]
    fn test_color_opposite() {
        assert_eq!(Color::Red.opposite(), Color::Black);
        assert_eq!(Color::Black.opposite(), Color::Red);
        assert_eq!(Color::Red.opposite().opposite(), Color::Red);
    }

    #[test]
    fn test_color_display() {
        assert_eq!(format!("{}", Color::Red), "red");
        assert_eq!(format!("{}", Color::Black), "black");
    }

    // === PieceType tests ===

    #[test]
    fn test_piece_type_base_values() {
        assert_eq!(PieceType::King.base_value(), 10000);
        assert_eq!(PieceType::Rook.base_value(), 600);
        assert_eq!(PieceType::Cannon.base_value(), 285);
        assert_eq!(PieceType::Knight.base_value(), 270);
        assert_eq!(PieceType::Bishop.base_value(), 120);
        assert_eq!(PieceType::Advisor.base_value(), 120);
        assert_eq!(PieceType::Pawn.base_value(), 30);
    }

    #[test]
    fn test_piece_type_chinese_name() {
        // Red pieces
        assert_eq!(PieceType::King.chinese_name(Color::Red), "帅");
        assert_eq!(PieceType::Advisor.chinese_name(Color::Red), "仕");
        assert_eq!(PieceType::Bishop.chinese_name(Color::Red), "相");
        assert_eq!(PieceType::Knight.chinese_name(Color::Red), "马");
        assert_eq!(PieceType::Rook.chinese_name(Color::Red), "车");
        assert_eq!(PieceType::Cannon.chinese_name(Color::Red), "炮");
        assert_eq!(PieceType::Pawn.chinese_name(Color::Red), "兵");
        // Black pieces
        assert_eq!(PieceType::King.chinese_name(Color::Black), "将");
        assert_eq!(PieceType::Advisor.chinese_name(Color::Black), "士");
        assert_eq!(PieceType::Bishop.chinese_name(Color::Black), "象");
        assert_eq!(PieceType::Knight.chinese_name(Color::Black), "马");
        assert_eq!(PieceType::Rook.chinese_name(Color::Black), "车");
        assert_eq!(PieceType::Cannon.chinese_name(Color::Black), "炮");
        assert_eq!(PieceType::Pawn.chinese_name(Color::Black), "卒");
    }

    #[test]
    fn test_piece_type_fen_char_roundtrip() {
        let all_types = [
            PieceType::King, PieceType::Advisor, PieceType::Bishop,
            PieceType::Knight, PieceType::Rook, PieceType::Cannon, PieceType::Pawn,
        ];
        for color in [Color::Red, Color::Black] {
            for &pt in &all_types {
                let ch = pt.to_fen_char(color);
                let piece = Piece::from_fen_char(ch).unwrap();
                assert_eq!(piece.color, color);
                assert_eq!(piece.piece_type, pt);
            }
        }
    }

    #[test]
    fn test_piece_type_fen_char_case() {
        // Red = uppercase, Black = lowercase
        assert!(PieceType::King.to_fen_char(Color::Red).is_uppercase());
        assert!(PieceType::King.to_fen_char(Color::Black).is_lowercase());
    }

    #[test]
    fn test_piece_type_from_invalid_fen() {
        assert!(PieceType::from_fen_char('x').is_none());
        assert!(PieceType::from_fen_char('1').is_none());
        assert!(PieceType::from_fen_char(' ').is_none());
    }

    // === Piece tests ===

    #[test]
    fn test_piece_new() {
        let piece = Piece::new(Color::Red, PieceType::King);
        assert_eq!(piece.color, Color::Red);
        assert_eq!(piece.piece_type, PieceType::King);
    }

    #[test]
    fn test_piece_base_value() {
        let rook = Piece::new(Color::Red, PieceType::Rook);
        assert_eq!(rook.base_value(), 600);
    }

    #[test]
    fn test_piece_chinese_name() {
        let red_king = Piece::new(Color::Red, PieceType::King);
        assert_eq!(red_king.chinese_name(), "帅");
        let black_king = Piece::new(Color::Black, PieceType::King);
        assert_eq!(black_king.chinese_name(), "将");
    }

    #[test]
    fn test_piece_from_fen_all_chars() {
        let cases = [
            ('K', Color::Red, PieceType::King),
            ('A', Color::Red, PieceType::Advisor),
            ('B', Color::Red, PieceType::Bishop),
            ('N', Color::Red, PieceType::Knight),
            ('R', Color::Red, PieceType::Rook),
            ('C', Color::Red, PieceType::Cannon),
            ('P', Color::Red, PieceType::Pawn),
            ('k', Color::Black, PieceType::King),
            ('a', Color::Black, PieceType::Advisor),
            ('b', Color::Black, PieceType::Bishop),
            ('n', Color::Black, PieceType::Knight),
            ('r', Color::Black, PieceType::Rook),
            ('c', Color::Black, PieceType::Cannon),
            ('p', Color::Black, PieceType::Pawn),
        ];
        for (ch, color, pt) in cases {
            let piece = Piece::from_fen_char(ch).unwrap();
            assert_eq!(piece.color, color, "Failed for char '{}'", ch);
            assert_eq!(piece.piece_type, pt, "Failed for char '{}'", ch);
        }
    }

    #[test]
    fn test_piece_from_fen_invalid() {
        assert!(Piece::from_fen_char('x').is_none());
        assert!(Piece::from_fen_char('0').is_none());
    }

    // === Move tests ===

    #[test]
    fn test_move_uci_parsing() {
        let m = Move::from_uci("a0a1").unwrap();
        assert_eq!(m.from, crate::board::Position::new(0, 0));
        assert_eq!(m.to, crate::board::Position::new(0, 1));
    }

    #[test]
    fn test_move_uci_various() {
        let cases = [
            ("a0a0", 0, 0, 0, 0),
            ("e4e5", 4, 4, 4, 5),
            ("i9i8", 8, 9, 8, 8),
            ("b2c3", 1, 2, 2, 3),
        ];
        for (uci, fc, fr, tc, tr) in cases {
            let m = Move::from_uci(uci).unwrap();
            assert_eq!(m.from.col, fc, "from col for {}", uci);
            assert_eq!(m.from.row, fr, "from row for {}", uci);
            assert_eq!(m.to.col, tc, "to col for {}", uci);
            assert_eq!(m.to.row, tr, "to row for {}", uci);
            assert_eq!(m.to_uci(), uci);
        }
    }

    #[test]
    fn test_move_from_uci_invalid() {
        assert!(Move::from_uci("").is_none());
        assert!(Move::from_uci("abc").is_none());
        assert!(Move::from_uci("j0a0").is_none()); // 'j' is out of bounds
        assert!(Move::from_uci("a01").is_none());  // wrong length
    }

    #[test]
    fn test_move_display() {
        let m = Move::from_uci("b2c3").unwrap();
        assert_eq!(format!("{}", m), "b2c3");
    }

    #[test]
    fn test_move_equality() {
        let m1 = Move::new(crate::board::Position::new(0, 0), crate::board::Position::new(0, 1));
        let m2 = Move::new(crate::board::Position::new(0, 0), crate::board::Position::new(0, 1));
        let m3 = Move::new(crate::board::Position::new(0, 0), crate::board::Position::new(1, 0));
        assert_eq!(m1, m2);
        assert_ne!(m1, m3);
    }

    // === Display trait tests ===

    #[test]
    fn test_piece_type_display() {
        assert_eq!(format!("{}", PieceType::King), "king");
        assert_eq!(format!("{}", PieceType::Advisor), "advisor");
        assert_eq!(format!("{}", PieceType::Bishop), "bishop");
        assert_eq!(format!("{}", PieceType::Knight), "knight");
        assert_eq!(format!("{}", PieceType::Rook), "rook");
        assert_eq!(format!("{}", PieceType::Cannon), "cannon");
        assert_eq!(format!("{}", PieceType::Pawn), "pawn");
    }

    #[test]
    fn test_piece_display() {
        let red_king = Piece::new(Color::Red, PieceType::King);
        assert_eq!(format!("{}", red_king), "red king");
        let black_rook = Piece::new(Color::Black, PieceType::Rook);
        assert_eq!(format!("{}", black_rook), "black rook");
    }
}