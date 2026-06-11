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