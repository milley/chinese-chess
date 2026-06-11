# 中国象棋在线对战平台 — 从零实现开发指南

> 本文档详细记录了从零开始构建本项目每个子工程的完整步骤，包括架构设计、代码实现、数据库设计、前后端联调等所有环节。

---

## 目录

- [一、项目总览](#一项目总览)
- [二、开发环境准备](#二开发环境准备)
- [三、工程一：chess-engine（象棋引擎库）](#三工程一chess-engine象棋引擎库)
- [四、工程二：chess-server（后端服务）](#四工程二chess-server后端服务)
- [五、工程三：chess-client（前端客户端）](#五工程三chess-client前端客户端)
- [六、前后端联调与集成测试](#六前后端联调与集成测试)
- [七、项目目录结构总览](#七项目目录结构总览)

---

## 一、项目总览

### 1.1 项目简介

本项目是一个**中国象棋（Xiangqi）在线对战平台**，采用前后端分离架构，包含三个独立子工程：

| 子工程 | 技术栈 | 说明 |
|--------|--------|------|
| `chess-engine` | Rust (Library) | 象棋规则引擎 + AI 搜索算法 |
| `chess-server` | Rust (Axum + SQLx + PostgreSQL) | REST API + WebSocket 实时对战服务 |
| `chess-client` | Vue 3 + TypeScript + Vite | 单页面游戏客户端 |

### 1.2 整体架构

```
┌─────────────────────────────────────────────────────┐
│                    浏览器 (Vue 3 SPA)                │
│  ┌──────────┐  ┌──────────┐  ┌───────────────────┐  │
│  │  Pinia    │  │ Vue      │  │  Canvas 棋盘渲染   │  │
│  │  Stores   │  │ Router   │  │  (ChessBoard.vue) │  │
│  └────┬─────┘  └────┬─────┘  └────────┬──────────┘  │
│       │              │                 │              │
│  ┌────┴──────────────┴─────────────────┴──────────┐  │
│  │              API / WebSocket 层                  │  │
│  │  ┌──────────────┐  ┌────────────────────────┐  │  │
│  │  │  Axios REST  │  │  WebSocket Service     │  │  │
│  │  │  (api/index) │  │  (api/websocket)       │  │  │
│  │  └──────┬───────┘  └──────────┬─────────────┘  │  │
│  └─────────┼─────────────────────┼────────────────┘  │
└────────────┼─────────────────────┼───────────────────┘
             │ HTTP REST           │ WebSocket
             │ (Vite Proxy)        │ (Vite Proxy)
             ▼                     ▼
┌─────────────────────────────────────────────────────┐
│              Axum Server (Rust) :3000                │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────┐  │
│  │  REST     │  │ WebSocket│  │  JWT Auth        │  │
│  │  Handlers │  │ Handlers │  │  Middleware      │  │
│  └────┬──────┘  └────┬─────┘  └──────────────────┘  │
│       │              │                               │
│  ┌────┴──────────────┴──────────────────────────┐   │
│  │              AppState (共享状态)               │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────────┐  │   │
│  │  │UserRepo  │ │GameRepo  │ │RoomManager   │  │   │
│  │  └────┬─────┘ └────┬─────┘ └──────┬───────┘  │   │
│  └───────┼─────────────┼──────────────┼──────────┘   │
│          │             │              │               │
│          ▼             ▼              ▼               │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────┐   │
│  │PostgreSQL│  │PostgreSQL│  │  GameRoom         │   │
│  │ (users)  │  │ (games)  │  │  ┌────────────┐  │   │
│  └──────────┘  └──────────┘  │  │ chess-engine│  │   │
│                              │  │ (GameState) │  │   │
│                              │  └────────────┘  │   │
│                              └──────────────────┘   │
└─────────────────────────────────────────────────────┘
```

### 1.3 核心数据流（走棋）

1. 玩家点击棋盘 → `gameStore.selectSquare()` → REST 获取合法走法
2. 玩家点击目标格 → `gameStore.makeMove()` → REST `POST /api/games/{id}/move`
3. 服务端 Handler 委托给 `GameRoom.make_move()`（通过 RwLock 保证线程安全）
4. 走法由 chess-engine 规则验证，执行于 GameState
5. 服务端广播 `MoveMade` / `GameOver` 到房间内所有 WebSocket 客户端
6. 服务端更新数据库（FEN、走法历史、可能的游戏结果 + Elo 评分）
7. 客户端收到 WS 消息，更新本地状态，重绘棋盘

---

## 二、开发环境准备

### 2.1 必需工具

| 工具 | 版本要求 | 用途 |
|------|----------|------|
| Rust | Edition 2024 (1.85+) | 后端 + 引擎开发 |
| Node.js | 18+ | 前端开发 |
| PostgreSQL | 14+ | 数据库 |
| npm / pnpm | 最新版 | 前端包管理 |
| git | 最新版 | 版本控制 |

### 2.2 安装步骤

```bash
# 1. 安装 Rust (Windows: 下载 rustup-init.exe)
# https://rustup.rs/
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. 安装 Node.js
# https://nodejs.org/ 下载 LTS 版本

# 3. 安装 PostgreSQL
# https://www.postgresql.org/download/
# 安装后确保 psql 命令可用

# 4. 创建项目根目录
mkdir -p chinese-chess
cd chinese-chess
```

### 2.3 数据库初始化

```bash
# 登录 PostgreSQL
psql -U postgres

# 创建数据库
CREATE DATABASE chess;

# 退出
\q
```

---

## 三、工程一：chess-engine（象棋引擎库）

> 纯 Rust 库，不依赖任何 I/O 或网络框架。负责棋盘表示、走法生成、规则验证、游戏状态管理和 AI 搜索。

### 3.1 创建工程

```bash
cd chinese-chess
cargo new chess-engine --lib
cd chess-engine
```

### 3.2 配置 Cargo.toml

```toml
[package]
name = "chess-engine"
version = "0.1.0"
edition = "2024"

[dependencies]
serde = { version = "1", features = ["derive"], optional = true }

[features]
default = []
serde = ["dep:serde"]
```

> `serde` 设为可选依赖，引擎核心不需要序列化，仅在 server 端需要时启用。

### 3.3 模块架构

```
chess-engine/src/
├── lib.rs              # 库入口，导出公共类型
├── board/
│   ├── mod.rs          # 模块导出
│   ├── position.rs     # 坐标系统 (Position)
│   ├── board.rs        # 棋盘表示 (Board)
│   └── move_gen.rs     # 走法生成器
├── pieces/
│   ├── mod.rs
│   ├── types.rs        # 棋子类型、颜色、基础分值
│   └── movement.rs     # 棋子移动模式定义
├── rules/
│   ├── mod.rs
│   ├── check.rs        # 将军/将杀/困毙检测
│   └── validator.rs    # 走法合法性验证
├── game/
│   ├── mod.rs
│   └── state.rs        # 游戏状态管理 (GameState)
└── ai/
    ├── mod.rs
    ├── eval.rs         # 局面评估函数
    └── search.rs       # Alpha-Beta 搜索
```

### 3.4 实现步骤

#### 步骤 1：定义棋子类型 (`pieces/types.rs`)

```rust
/// 棋子颜色
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Color {
    Red,    // 红方
    Black,  // 黑方
}

/// 棋子类型
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PieceType {
    King,     // 帅/将
    Advisor,  // 仕/士
    Bishop,   // 相/象
    Knight,   // 马
    Rook,     // 车
    Cannon,   // 炮
    Pawn,     // 兵/卒
}

/// 棋子 = 颜色 + 类型
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Piece {
    pub color: Color,
    pub piece_type: PieceType,
}

/// 走法 (UCI 格式，如 "a0a1")
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub from: Position,
    pub to: Position,
}
```

**关键设计点：**
- 每种棋子类型有基础分值：King=10000, Rook=600, Cannon=285, Knight=270, Bishop=120, Advisor=120, Pawn=30（过河后 70-80）
- FEN 字符映射：红方大写 (K/A/B/N/R/C/P)，黑方小写 (k/a/b/n/r/c/p)
- 中文名称映射：帅/将/仕/士/相/象/马/车/炮/兵/卒

#### 步骤 2：定义坐标系统 (`board/position.rs`)

```rust
/// 棋盘坐标 (列 0-8, 行 0-9)
/// 行 0 = 黑方底线, 行 9 = 红方底线
/// 列 0 = 最左列 (a), 列 8 = 最右列 (i)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Position {
    pub col: u8,  // 0-8
    pub row: u8,  // 0-9
}

impl Position {
    /// 从 FEN 列字符和行号创建
    pub fn from_fen(col_char: char, row: u8) -> Option<Self> { ... }

    /// 转为 UCI 字符串 (如 "a0")
    pub fn to_uci(&self) -> String { ... }

    /// 从 UCI 字符串解析
    pub fn from_uci(s: &str) -> Option<Self> { ... }
}
```

**坐标系约定：**
- 列映射：a=0, b=1, c=2, d=3, e=4, f=5, g=6, h=7, i=8
- 行映射：0=黑方底线, 9=红方底线
- 红方九宫：col 3-5, row 7-9
- 黑方九宫：col 3-5, row 0-2
- 河界：row 4-5 之间

#### 步骤 3：实现棋盘表示 (`board/board.rs`)

```rust
pub struct Board {
    /// 棋子位置映射 (Position -> Piece)
    pieces: HashMap<Position, Piece>,
    /// 当前走子方
    side_to_move: Color,
    /// 红方物质分 (增量维护)
    red_material_score: i32,
    /// 黑方物质分 (增量维护)
    black_material_score: i32,
    /// 红方机动性 (合法走法数，增量维护)
    red_mobility: i32,
    /// 黑方机动性
    black_mobility: i32,
}
```

**核心方法：**

| 方法 | 功能 |
|------|------|
| `from_fen(fen: &str) -> Result<Self>` | 从 FEN 字符串解析棋盘 |
| `to_fen(&self) -> String` | 导出为 FEN 字符串 |
| `initial() -> Self` | 创建初始局面 |
| `piece_at(&self, pos: Position) -> Option<Piece>` | 获取某位置棋子 |
| `make_move(&mut self, m: Move) -> Option<Piece>` | 执行走法，返回被吃棋子 |
| `undo_move(&mut self, m: Move, captured: Option<Piece>)` | 撤销走法 |
| `is_in_bounds(pos: Position) -> bool` | 坐标是否在棋盘内 |

**FEN 格式说明：**
- 标准中国象棋 FEN：`rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1`
- 行间用 `/` 分隔，数字表示连续空格，`w` 表示红方走，`b` 表示黑方走

**增量维护设计：**
- `make_move` 时：减去被吃棋子分值，更新 `side_to_move`
- `undo_move` 时：恢复被吃棋子分值，恢复 `side_to_move`
- 机动性在走法生成后更新，用于 AI 评估

#### 步骤 4：实现走法生成 (`board/move_gen.rs`)

为每种棋子实现伪合法走法生成（不考虑被将），然后在 `generate_legal_moves` 中过滤掉导致自己被将的走法。

**各棋子走法规则：**

| 棋子 | 走法规则 |
|------|----------|
| 帅/将 | 九宫内一步直走；"飞将"规则（两将不能面对面） |
| 仕/士 | 九宫内一步斜走 |
| 相/象 | 走"田"字，不能过河，塞象眼不能走 |
| 马 | 走"日"字，蹩马腿不能走 |
| 车 | 直线任意距离，不能越子 |
| 炮 | 移动同车；吃子需隔一子（炮架） |
| 兵/卒 | 未过河只能前进一步；过河后可前进或左右一步 |

**关键实现：**

```rust
impl Board {
    /// 生成所有伪合法走法 (不检查是否导致自己被将)
    pub fn generate_pseudo_legal_moves(&self, color: Color) -> Vec<Move> {
        let mut moves = Vec::new();
        for (pos, piece) in &self.pieces {
            if piece.color == color {
                match piece.piece_type {
                    PieceType::King => self.gen_king_moves(*pos, color, &mut moves),
                    PieceType::Advisor => self.gen_advisor_moves(*pos, color, &mut moves),
                    PieceType::Bishop => self.gen_bishop_moves(*pos, color, &mut moves),
                    PieceType::Knight => self.gen_knight_moves(*pos, color, &mut moves),
                    PieceType::Rook => self.gen_rook_moves(*pos, color, &mut moves),
                    PieceType::Cannon => self.gen_cannon_moves(*pos, color, &mut moves),
                    PieceType::Pawn => self.gen_pawn_moves(*pos, color, &mut moves),
                }
            }
        }
        moves
    }

    /// 生成所有合法走法 (过滤掉导致自己被将的走法)
    pub fn generate_legal_moves(&self, color: Color) -> Vec<Move> {
        self.generate_pseudo_legal_moves(color)
            .into_iter()
            .filter(|&m| {
                let captured = self.piece_at(m.to);
                // 不能吃自己的子
                if let Some(p) = captured {
                    if p.color == color { return false; }
                }
                // 执行走法后检查是否被将
                let mut board = self.clone();
                board.make_move(m);
                !is_in_check(&board, color)
            })
            .collect()
    }
}
```

**性能优化要点：**
- 马的蹩腿检测：检查马走方向上的相邻格是否有棋子
- 象的塞眼检测：检查"田"字中心是否有棋子
- 炮的走法分两段：移动（同车）和吃子（需翻山）
- `generate_legal_moves` 使用 make_move/undo_move 模式过滤

#### 步骤 5：实现规则验证 (`rules/`)

**将军检测 (`rules/check.rs`)：**

```rust
/// 检测 color 方是否被将
pub fn is_in_check(board: &Board, color: Color) -> bool {
    // 找到 color 方的将/帅位置
    let king_pos = find_king(board, color);
    // 检查对方所有棋子是否能攻击到将/帅位置
    // 包含"飞将"规则：两将面对面
    ...
}

/// 检测是否将杀 (checkmate)
pub fn is_checkmate(board: &Board, color: Color) -> bool {
    is_in_check(board, color) && board.generate_legal_moves(color).is_empty()
}

/// 检测是否困毙 (stalemate)
/// 注意：中国象棋中困毙 = 输，不是和棋！
pub fn is_stalemate(board: &Board, color: Color) -> bool {
    !is_in_check(board, color) && board.generate_legal_moves(color).is_empty()
}
```

**走法验证 (`rules/validator.rs`)：**

```rust
pub enum MoveError {
    OutOfBounds,
    NoPieceAtFrom,
    WrongColor,
    IllegalMove,
    WouldBeInCheck,
    FlyingGeneral,  // 飞将
}

pub fn validate_move(board: &Board, m: Move, color: Color) -> Result<(), MoveError> { ... }
```

#### 步骤 6：实现游戏状态管理 (`game/state.rs`)

```rust
pub enum GameResult {
    RedWin,
    BlackWin,
    Draw,
}

pub enum GameEndReason {
    Checkmate,          // 将杀
    Stalemate,          // 困毙 (中国象棋中困毙=输)
    Resign(Color),      // 认输
    DrawAgreement,      // 协议和棋
    Timeout(Color),     // 超时
}

pub struct GameState {
    board: Board,
    history: Vec<(Move, Option<Piece>)>,  // (走法, 被吃棋子)
    result: Option<(GameResult, GameEndReason)>,
}

impl GameState {
    pub fn new() -> Self { ... }
    pub fn from_fen(fen: &str) -> Result<Self> { ... }

    /// 执行走法
    pub fn make_move(&mut self, m: Move) -> Result<(), MoveError> {
        // 1. 验证走法合法性
        // 2. 执行走法
        // 3. 记录历史
        // 4. 检查游戏是否结束 (将杀/困毙)
        ...
    }

    /// 撤销走法
    pub fn undo_move(&mut self) -> Option<Move> { ... }

    /// 认输
    pub fn resign(&mut self, color: Color) { ... }

    /// 和棋
    pub fn draw(&mut self) { ... }

    /// 生成中国象棋记谱法 (如 "炮二平五")
    pub fn generate_notation(&self, m: Move) -> String { ... }

    /// 检查游戏是否结束
    fn check_game_end(&mut self) { ... }
}
```

**中国象棋记谱法规则：**
- 格式：`棋子名 + 原始列号 + 动作(进/退/平) + 目标`
- 红方从右到左为一到九，黑方从右到左为 1 到 9
- 进/退：向前/向后移动；平：横向移动
- 同列两子用前/后区分

#### 步骤 7：实现 AI 评估函数 (`ai/eval.rs`)

```rust
/// 局面评估 (从红方视角)
pub fn evaluate(board: &Board) -> i32 {
    let mut score = 0i32;

    // 1. 物质分 (增量维护，直接读取)
    score += board.red_material_score() - board.black_material_score();

    // 2. 位置分 (查表)
    for (pos, piece) in board.all_pieces() {
        score += position_value(piece, pos);
    }

    // 3. 机动性分 (增量维护)
    score += (board.red_mobility() - board.black_mobility()) * MOBILITY_WEIGHT;

    score
}
```

**位置价值表设计：**

每种棋子在不同位置有不同价值，用 10×9 的二维数组表示：

- **兵/卒**：过河前价值低（30），过河后价值高（70-80），中路最高
- **马**：中心位置加分，边角减分
- **炮**：中心位置加分
- **车**：位置差异不大，主要靠物质分
- **帅/将**：九宫中心略优
- **仕/士**：九宫中心略优
- **相/象**：中路略优

#### 步骤 8：实现 AI 搜索 (`ai/search.rs`)

```rust
const MAX_DEPTH: u8 = 6;

/// Alpha-Beta 搜索入口
pub fn find_best_move(state: &GameState, depth: u8) -> Option<Move> {
    let mut best_move = None;
    let mut best_score = i32::MIN;
    let moves = state.board().generate_legal_moves(state.side_to_move());

    // MVV-LVA 走法排序 (Most Valuable Victim - Least Valuable Attacker)
    let mut sorted_moves = sort_moves(&moves, state.board());

    for m in sorted_moves {
        let mut new_state = state.clone();
        new_state.make_move(m).ok()?;
        let score = -alpha_beta(&new_state, depth - 1, i32::MIN, i32::MAX);
        if score > best_score {
            best_score = score;
            best_move = Some(m);
        }
    }
    best_move
}

/// Alpha-Beta 搜索 (Negamax 格式)
fn alpha_beta(state: &GameState, depth: u8, alpha: i32, beta: i32) -> i32 {
    if depth == 0 {
        return quiescence_search(state, alpha, beta, 4);
    }

    let moves = state.board().generate_legal_moves(state.side_to_move());
    if moves.is_empty() {
        // 将杀或困毙
        return if is_in_check(state.board(), state.side_to_move()) {
            -(10000 + depth as i32)  // 被将杀，越浅越差
        } else {
            -10000  // 困毙 = 输
        };
    }

    let mut alpha = alpha;
    let sorted_moves = sort_moves(&moves, state.board());

    for m in sorted_moves {
        let mut new_state = state.clone();
        new_state.make_move(m).ok()?;
        let score = -alpha_beta(&new_state, depth - 1, -beta, -alpha);
        if score >= beta { return beta; }  // Beta 剪枝
        if score > alpha { alpha = score; }
    }
    alpha
}

/// 静态搜索 (Quiescence Search)
/// 只搜索吃子走法，避免水平线效应
fn quiescence_search(state: &GameState, alpha: i32, beta: i32, depth: u8) -> i32 {
    let stand_pat = evaluate(state.board());
    if stand_pat >= beta { return beta; }
    if stand_pat > alpha { ... }

    // 只搜索吃子走法
    let captures = get_capture_moves(state);
    ...
}
```

**搜索优化技术：**
- **Negamax 格式**：统一红黑方评估，取负即可
- **Alpha-Beta 剪枝**：标准窗口剪枝
- **MVV-LVA 排序**：优先搜索高价值目标 + 低价值攻击者的走法
- **静态搜索**：在叶子节点继续搜索吃子走法，避免水平线效应
- **最大深度 6**：平衡搜索时间与棋力

#### 步骤 9：编写集成测试 (`tests/integration_tests.rs`)

```rust
#[test]
fn test_initial_position_fen() {
    let board = Board::initial();
    let fen = board.to_fen();
    assert!(fen.starts_with("rnbakabnr"));
}

#[test]
fn test_fen_roundtrip() {
    let fen = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w";
    let board = Board::from_fen(fen).unwrap();
    assert_eq!(board.to_fen(), fen);
}

#[test]
fn test_checkmate_detection() {
    // 用一个已知的将杀局面测试
    ...
}

#[test]
fn test_ai_finds_checkmate() {
    // 测试 AI 能否在几步内找到将杀
    ...
}

#[test]
fn test_piece_move_counts() {
    // 测试初始局面各棋子的走法数量
    let board = Board::initial();
    let red_moves = board.generate_legal_moves(Color::Red);
    assert_eq!(red_moves.len(), 44);  // 初始红方合法走法数
}
```

#### 步骤 10：完善 lib.rs 导出

```rust
pub mod board;
pub mod pieces;
pub mod rules;
pub mod game;
pub mod ai;

// 重导出常用类型
pub use board::{Board, Position};
pub use pieces::{Color, Piece, PieceType};
pub use game::{GameState, GameResult, GameEndReason, Move};
pub use rules::{is_checkmate, is_stalemate, is_in_check};
```

### 3.5 构建与测试

```bash
cd chess-engine
cargo build
cargo test
```

---

## 四、工程二：chess-server（后端服务）

> 基于 Axum 的异步 Web 服务，提供 REST API + WebSocket 实时对战，使用 PostgreSQL 持久化数据。

### 4.1 创建工程

```bash
cd chinese-chess
cargo new chess-server
cd chess-server
```

### 4.2 配置 Cargo.toml

```toml
[package]
name = "chess-server"
version = "0.1.0"
edition = "2024"

[dependencies]
# Web 框架
axum = { version = "0.8", features = ["ws", "macros"] }
tokio = { version = "1", features = ["full"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace"] }

# 数据库
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "uuid", "chrono", "migrate"] }

# 序列化
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# 认证
jsonwebtoken = "9"
argon2 = "0.5"

# 工具
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
dotenvy = "0.15"
futures = "0.3"
rand = "0.8"

# 象棋引擎 (本地路径依赖)
chess-engine = { path = "../chess-engine", features = ["serde"] }

[dev-dependencies]
reqwest = { version = "0.12", features = ["json"] }
```

### 4.3 模块架构

```
chess-server/src/
├── main.rs                 # 入口：配置加载、路由注册、服务启动
├── config/
│   └── mod.rs              # 环境变量配置 (AppConfig)
├── db/
│   ├── mod.rs              # 数据库模块导出
│   ├── models.rs           # 数据模型 (User, Game, DTO)
│   └── repositories/
│       ├── mod.rs
│       ├── user_repo.rs    # 用户数据访问
│       └── game_repo.rs    # 对局数据访问
├── handlers/
│   ├── mod.rs
│   ├── user_handler.rs     # 用户 API (注册/登录/CRUD)
│   ├── game_handler.rs     # 对局 API (创建/加入/列表)
│   ├── game_move_handler.rs # 走棋 API
│   ├── ai_handler.rs       # AI 走法 API
│   ├── move_handler.rs     # 合法走法查询 API
│   └── ws_handler.rs       # WebSocket 连接处理
├── middleware/
│   └── auth.rs             # JWT 认证中间件
├── websocket/
│   ├── mod.rs
│   ├── client.rs           # WS 客户端连接封装
│   ├── room.rs             # 游戏房间 (核心逻辑)
│   ├── manager.rs          # 房间管理器
│   └── message.rs          # WS 消息类型定义
├── utils/
│   ├── mod.rs
│   ├── auth.rs             # JWT 令牌生成/验证
│   └── password.rs         # Argon2id 密码哈希
├── services/
│   └── mod.rs              # 业务逻辑层 (预留)
└── error.rs                # 统一错误处理
```

### 4.4 数据库设计

#### 迁移 1：初始表结构 (`migrations/20250101000000_init.sql`)

```sql
-- 用户表
CREATE TABLE users (
    id UUID PRIMARY KEY,
    username VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    nickname VARCHAR(255),
    avatar_url VARCHAR(500),
    rating INTEGER DEFAULT 1500,       -- Elo 评分，初始 1500
    wins INTEGER DEFAULT 0,
    losses INTEGER DEFAULT 0,
    draws INTEGER DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- 对局表
CREATE TABLE games (
    id UUID PRIMARY KEY,
    red_player_id UUID REFERENCES users(id),
    black_player_id UUID REFERENCES users(id),
    status VARCHAR(50) NOT NULL DEFAULT 'waiting',  -- waiting/playing/finished
    result VARCHAR(50),                              -- red_win/black_win/draw
    fen TEXT NOT NULL,                               -- 当前局面 FEN
    move_history TEXT,                               -- 走法历史 (JSON 数组)
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    started_at TIMESTAMP WITH TIME ZONE,
    finished_at TIMESTAMP WITH TIME ZONE
);

-- 索引
CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_games_status ON games(status);
CREATE INDEX idx_games_players ON games(red_player_id, black_player_id);
```

#### 迁移 2：添加时间控制 (`migrations/20250102000000_add_time_control.sql`)

```sql
ALTER TABLE games ADD COLUMN time_control INTEGER;  -- 局时(秒)，NULL=不限时
ALTER TABLE games ADD COLUMN red_time INTEGER;       -- 红方剩余秒数
ALTER TABLE games ADD COLUMN black_time INTEGER;     -- 黑方剩余秒数
```

#### 迁移 3：添加步时限和读秒 (`migrations/20250103000000_add_increment_byoyomi.sql`)

```sql
ALTER TABLE games ADD COLUMN move_time_limit INTEGER;  -- 步时限(秒)，超时判负
ALTER TABLE games ADD COLUMN byoyomi INTEGER;          -- 读秒(秒)，局时用完后进入读秒
```

### 4.5 实现步骤

#### 步骤 1：配置模块 (`config/mod.rs`)

```rust
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub port: u16,           // 默认 3000
    pub host: String,        // 默认 "0.0.0.0"
    pub database_url: String,
    pub jwt_secret: String,  // 生产环境必须设置！
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| {
            if env::var("TEST_MODE").is_ok() {
                eprintln!("WARNING: Using default JWT secret in test mode");
                "test-secret-key-for-testing-only".to_string()
            } else {
                panic!("FATAL: JWT_SECRET not set. Set it in .env or environment.");
            }
        });
        Ok(Self {
            port: env::var("PORT").unwrap_or("3000".into()).parse()?,
            host: env::var("HOST").unwrap_or("0.0.0.0".into()),
            database_url: env::var("DATABASE_URL")
                .unwrap_or("postgres://postgres:postgres@localhost:5432/chess".into()),
            jwt_secret,
        })
    }
}
```

#### 步骤 2：环境配置文件 (`.env`)

```env
PORT=3000
DATABASE_URL=postgres://postgres:postgres@localhost:5432/chess
JWT_SECRET=dev-secret-key-change-before-production
TEST_MODE=1
```

#### 步骤 3：数据模型 (`db/models.rs`)

```rust
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::NaiveDateTime;

// === 数据库行模型 ===

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub nickname: Option<String>,
    pub avatar_url: Option<String>,
    pub rating: i32,
    pub wins: i32,
    pub losses: i32,
    pub draws: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Game {
    pub id: Uuid,
    pub red_player_id: Option<Uuid>,
    pub black_player_id: Option<Uuid>,
    pub status: String,
    pub result: Option<String>,
    pub fen: String,
    pub move_history: Option<String>,
    pub time_control: Option<i32>,
    pub move_time_limit: Option<i32>,
    pub byoyomi: Option<i32>,
    pub red_time: Option<i32>,
    pub black_time: Option<i32>,
    pub created_at: NaiveDateTime,
    pub started_at: Option<NaiveDateTime>,
    pub finished_at: Option<NaiveDateTime>,
}

// === 请求/响应 DTO ===

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserInfo,
}

#[derive(Deserialize)]
pub struct CreateUser {
    pub username: String,
    pub password: String,
    pub nickname: Option<String>,
}

#[derive(Serialize)]
pub struct UserInfo {
    pub id: Uuid,
    pub username: String,
    pub nickname: Option<String>,
    pub avatar_url: Option<String>,
    pub rating: i32,
    pub wins: i32,
    pub losses: i32,
    pub draws: i32,
}

#[derive(Serialize)]
pub struct GameInfo {
    pub id: Uuid,
    pub red_player: Option<UserInfo>,
    pub black_player: Option<UserInfo>,
    pub status: String,
    pub result: Option<String>,
    pub fen: String,
    pub time_control: Option<i32>,
    pub move_time_limit: Option<i32>,
    pub byoyomi: Option<i32>,
    pub red_time: Option<i32>,
    pub black_time: Option<i32>,
    pub created_at: NaiveDateTime,
}
```

#### 步骤 4：用户仓库 (`db/repositories/user_repo.rs`)

```rust
#[derive(Clone)]
pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    pub fn new(pool: PgPool) -> Self { Self { pool } }

    /// 创建用户
    pub async fn create(&self, username: &str, password_hash: &str, nickname: Option<&str>)
        -> Result<User> { ... }

    /// 按用户名查找
    pub async fn find_by_username(&self, username: &str) -> Result<Option<User>> { ... }

    /// 按 ID 查找
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<User>> { ... }

    /// 更新用户信息
    pub async fn update(&self, id: Uuid, nickname: Option<&str>, avatar_url: Option<&str>)
        -> Result<User> { ... }

    /// 删除用户
    pub async fn delete(&self, id: Uuid) -> Result<()> { ... }

    /// 列出用户 (分页)
    pub async fn list(&self, page: i64, page_size: i64) -> Result<Vec<User>> { ... }

    /// 更新 Elo 评分 (对局结束后调用)
    pub async fn update_rating(&self, id: Uuid, rating: i32, result: GameResultType) -> Result<()> {
        // result: Win -> wins += 1, Loss -> losses += 1, Draw -> draws += 1
        ...
    }
}
```

**Elo 评分算法：**

```rust
/// 计算新的 Elo 评分
/// K 因子 = 32
fn calculate_new_rating(rating: i32, opponent_rating: i32, score: f64) -> i32 {
    let expected = 1.0 / (1.0 + 10_f64.powi((opponent_rating - rating) / 400));
    let k = 32.0;
    (rating as f64 + k * (score - expected)).round() as i32
}
// score: 胜=1.0, 和=0.5, 负=0.0
```

#### 步骤 5：对局仓库 (`db/repositories/game_repo.rs`)

```rust
#[derive(Clone)]
pub struct GameRepository {
    pool: PgPool,
}

impl GameRepository {
    pub fn new(pool: PgPool) -> Self { Self { pool } }

    /// 创建对局
    pub async fn create(&self, red_player_id: Uuid, time_control: Option<i32>,
        move_time_limit: Option<i32>, byoyomi: Option<i32>) -> Result<Game> { ... }

    /// 按 ID 查找
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Game>> { ... }

    /// 加入对局 (设置黑方)
    pub async fn join_game(&self, id: Uuid, black_player_id: Uuid) -> Result<Game> { ... }

    /// 完成对局
    pub async fn finish_game(&self, id: Uuid, result: &str, fen: &str,
        move_history: &str) -> Result<Game> { ... }

    /// 更新时间
    pub async fn update_time(&self, id: Uuid, red_time: i32, black_time: i32) -> Result<()> { ... }

    /// 列出对局 (按状态过滤，分页)
    pub async fn list(&self, status: Option<&str>, page: i64, page_size: i64) -> Result<Vec<Game>> { ... }

    /// 删除对局
    pub async fn delete(&self, id: Uuid) -> Result<()> { ... }
}
```

#### 步骤 6：密码工具 (`utils/password.rs`)

```rust
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;

/// 使用 Argon2id 哈希密码
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

/// 验证密码
pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    let parsed = PasswordHash::new(hash)?;
    Ok(Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
}
```

#### 步骤 7：JWT 工具 (`utils/auth.rs`)

```rust
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,    // 用户 ID
    pub username: String,
    pub exp: usize,     // 过期时间 (24小时)
}

/// 生成 JWT 令牌
pub fn generate_token(user_id: &Uuid, username: &str, secret: &str) -> Result<String> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::hours(24))
        .unwrap()
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        exp: expiration,
    };

    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .map_err(|e| anyhow::anyhow!("Failed to generate token: {}", e))
}

/// 验证 JWT 令牌
pub fn verify_token(token: &str, secret: &str) -> Result<Claims> {
    decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default())
        .map(|data| data.claims)
        .map_err(|e| anyhow::anyhow!("Invalid token: {}", e))
}
```

#### 步骤 8：认证中间件 (`middleware/auth.rs`)

```rust
use axum::extract::FromRequestParts;

/// 认证用户提取器
/// 在需要认证的路由上使用 `AuthUser` 作为参数
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub username: String,
}

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // 1. 从请求扩展中获取 JWT secret (由中间件注入)
        let jwt_secret = parts.extensions
            .get::<String>()
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

        // 2. 从 Authorization header 提取 token
        let auth_header = parts.headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(StatusCode::UNAUTHORIZED)?;

        // 3. 验证 token
        let claims = verify_token(token, jwt_secret)
            .map_err(|_| StatusCode::UNAUTHORIZED)?;

        Ok(AuthUser {
            user_id: Uuid::parse_str(&claims.sub)
                .map_err(|_| StatusCode::UNAUTHORIZED)?,
            username: claims.username,
        })
    }
}
```

#### 步骤 9：统一错误处理 (`error.rs`)

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),

    #[error("Game error: {0}")]
    GameError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::Forbidden(_) => (StatusCode::FORBIDDEN, self.to_string()),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal error".into()),
            AppError::GameError(_) => (StatusCode::BAD_REQUEST, self.to_string()),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
```

#### 步骤 10：用户 Handler (`handlers/user_handler.rs`)

```rust
/// POST /api/users — 注册
pub async fn register(
    State(state): State<AppState>,
    Json(data): Json<CreateUser>,
) -> Result<Json<LoginResponse>, AppError> {
    // 1. 验证用户名 (3-20字符，字母开头，字母数字下划线)
    validate_username(&data.username)?;
    // 2. 验证密码 (6-100字符)
    validate_password(&data.password)?;
    // 3. 检查保留名 (root, admin, system, guest 等)
    check_reserved_names(&data.username)?;
    // 4. 检查用户名是否已存在
    if state.user_repo.find_by_username(&data.username).await?.is_some() {
        return Err(AppError::BadRequest("Username already exists".into()));
    }
    // 5. 哈希密码
    let hash = hash_password(&data.password)?;
    // 6. 创建用户
    let user = state.user_repo.create(&data.username, &hash, data.nickname.as_deref()).await?;
    // 7. 生成 JWT
    let token = generate_token(&user.id, &user.username, &state.jwt_secret)?;
    Ok(Json(LoginResponse { token, user: user.into() }))
}

/// POST /api/users/login — 登录
pub async fn login(
    State(state): State<AppState>,
    Json(data): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let user = state.user_repo.find_by_username(&data.username).await?
        .ok_or(AppError::Unauthorized("Invalid credentials".into()))?;
    if !verify_password(&data.password, &user.password_hash)? {
        return Err(AppError::Unauthorized("Invalid credentials".into()));
    }
    let token = generate_token(&user.id, &user.username, &state.jwt_secret)?;
    Ok(Json(LoginResponse { token, user: user.into() }))
}

/// GET /api/users/me — 获取当前用户 (需认证)
pub async fn get_current_user(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<UserInfo>, AppError> { ... }

/// PUT /api/users/me — 更新用户 (需认证)
pub async fn update_user(...) -> Result<Json<UserInfo>, AppError> { ... }

/// DELETE /api/users/me — 删除用户 (需认证)
pub async fn delete_user(...) -> Result<StatusCode, AppError> { ... }

/// GET /api/users/{id} — 获取指定用户
pub async fn get_user(...) -> Result<Json<UserInfo>, AppError> { ... }

/// GET /api/users — 列出用户
pub async fn list_users(...) -> Result<Json<Vec<UserInfo>>, AppError> { ... }
```

#### 步骤 11：对局 Handler (`handlers/game_handler.rs`)

```rust
/// POST /api/games — 创建对局 (需认证)
pub async fn create_game(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(data): Json<CreateGameRequest>,
) -> Result<Json<CreateGameResponse>, AppError> {
    // 1. 创建对局记录 (创建者为红方或黑方，由 player_color 决定)
    // 2. 设置时间控制参数
    // 3. 返回 game_id 和分配的颜色
    ...
}

/// POST /api/games/{id}/join — 加入对局 (需认证)
pub async fn join_game(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<GameInfo>, AppError> {
    // 1. 查找对局
    // 2. 检查状态是否为 waiting
    // 3. 检查是否已是对局玩家
    // 4. 设置为黑方 (或红方，取决于创建者选的颜色)
    // 5. 更新状态为 playing
    ...
}

/// GET /api/games — 列出对局 (支持状态过滤和分页)
pub async fn list_games(...) -> Result<Json<Vec<GameInfo>>, AppError> { ... }

/// GET /api/games/{id} — 获取对局详情
pub async fn get_game(...) -> Result<Json<GameInfo>, AppError> { ... }

/// DELETE /api/games/{id} — 删除对局 (需认证 + 权限检查)
pub async fn delete_game(...) -> Result<StatusCode, AppError> { ... }
```

#### 步骤 12：走棋 Handler (`handlers/game_move_handler.rs`)

```rust
/// POST /api/games/{id}/move — 执行走法 (需认证)
pub async fn make_move(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(data): Json<MakeMoveRequest>,
) -> Result<Json<MakeMoveResponse>, AppError> {
    // 1. 获取或创建 GameRoom
    // 2. 通过 RoomManager 执行走法 (线程安全)
    // 3. 返回新 FEN、是否将军、是否游戏结束
    ...
}

/// POST /api/moves/valid — 查询合法走法
pub async fn get_valid_moves(
    Json(data): Json<ValidMovesRequest>,
) -> Result<Json<ValidMovesResponse>, AppError> {
    // 1. 从 FEN 解析棋盘
    // 2. 生成指定位置的合法走法
    // 3. 返回目标位置列表
    ...
}
```

#### 步骤 13：AI Handler (`handlers/ai_handler.rs`)

```rust
/// POST /api/ai/move — 获取 AI 推荐走法
pub async fn get_ai_move(
    Json(data): Json<AiMoveRequest>,
) -> Result<Json<AiMoveResponse>, AppError> {
    let state = GameState::from_fen(&data.fen)?;
    let depth = data.depth.unwrap_or(4);
    let best = chess_engine::ai::search::find_best_move(&state, depth);
    Ok(Json(AiMoveResponse {
        best_move: best.map(|m| m.to_uci()),
        score: ...,
        depth,
    }))
}
```

#### 步骤 14：WebSocket 消息类型 (`websocket/message.rs`)

```rust
use serde::{Deserialize, Serialize};

// === 客户端 → 服务端 ===

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "auth")]
    Auth { token: String },
    #[serde(rename = "join_game")]
    JoinGame { game_id: String },
    #[serde(rename = "leave_game")]
    LeaveGame { game_id: String },
    #[serde(rename = "make_move")]
    MakeMove { game_id: String, from: String, to: String },
    #[serde(rename = "resign")]
    Resign { game_id: String },
    #[serde(rename = "offer_draw")]
    OfferDraw { game_id: String },
    #[serde(rename = "respond_draw")]
    RespondDraw { game_id: String, accept: bool },
    #[serde(rename = "ping")]
    Ping,
}

// === 服务端 → 客户端 ===

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type")]
pub enum ServerMessage {
    #[serde(rename = "joined_game")]
    JoinedGame { game_id: String, color: String, fen: String, ... },
    #[serde(rename = "opponent_joined")]
    OpponentJoined { game_id: String, opponent: UserInfo, ... },
    #[serde(rename = "move_made")]
    MoveMade { game_id: String, mv: MoveInfo, fen: String, is_check: bool },
    #[serde(rename = "illegal_move")]
    IllegalMove { game_id: String, reason: String },
    #[serde(rename = "game_over")]
    GameOver { game_id: String, result: String, reason: String },
    #[serde(rename = "opponent_disconnected")]
    OpponentDisconnected { game_id: String },
    #[serde(rename = "draw_offered")]
    DrawOffered { game_id: String },
    #[serde(rename = "draw_response")]
    DrawResponse { game_id: String, accepted: bool },
    #[serde(rename = "time_update")]
    TimeUpdate { game_id: String, red_time: i64, black_time: i64, active_color: String },
    #[serde(rename = "pong")]
    Pong,
    #[serde(rename = "error")]
    Error { message: String },
}
```

#### 步骤 15：WebSocket 客户端封装 (`websocket/client.rs`)

```rust
/// 封装 WebSocket 连接和用户信息
pub struct Client {
    pub user_id: Uuid,
    pub username: String,
    pub sender: mpsc::UnboundedSender<String>,  // 发送消息的通道
}
```

#### 步骤 16：游戏房间 (`websocket/room.rs`) — **核心模块**

这是整个实时对战的核心，负责管理一局对局的所有状态和逻辑。

```rust
pub struct GameRoom {
    /// 游戏状态 (来自 chess-engine)
    game_state: Arc<RwLock<GameState>>,
    /// 红方玩家
    red_player: Arc<RwLock<Option<Client>>>,
    /// 黑方玩家
    black_player: Arc<RwLock<Option<Client>>>,
    /// 观战者列表
    spectators: Arc<RwLock<Vec<Client>>>,
    /// 对局 ID
    game_id: Uuid,
    /// 时间控制参数
    time_control: Option<i32>,       // 局时(秒)
    move_time_limit: Option<i32>,    // 步时限(秒)
    byoyomi: Option<i32>,           // 读秒(秒)
    /// 剩余时间
    red_time: Arc<RwLock<i32>>,
    black_time: Arc<RwLock<i32>>,
    /// 当前走子方开始走棋的时间戳
    move_start_time: Arc<RwLock<Instant>>,
    /// 和棋请求状态
    draw_offer: Arc<RwLock<Option<Color>>>,
    /// 数据库仓库引用 (用于持久化)
    game_repo: GameRepository,
}

impl GameRoom {
    /// 从数据库记录创建房间
    pub fn from_game(game: &Game, game_repo: GameRepository) -> Self { ... }

    /// 玩家加入
    pub async fn join(&self, client: Client, color: Color) -> Result<(), String> { ... }

    /// 玩家离开
    pub async fn leave(&self, user_id: Uuid) -> Result<(), String> { ... }

    /// 执行走法 (核心方法)
    pub async fn make_move(&self, user_id: Uuid, from: &str, to: &str)
        -> Result<MoveResult, String> {
        // 1. 验证是否轮到该玩家走棋
        // 2. 解析走法
        // 3. 获取写锁
        // 4. 执行走法 (chess-engine)
        // 5. 切换计时器
        // 6. 检查游戏是否结束
        // 7. 广播走法/游戏结束消息
        // 8. 更新数据库
        // 9. 如果游戏结束，更新 Elo 评分
        ...
    }

    /// 认输
    pub async fn resign(&self, user_id: Uuid) -> Result<(), String> { ... }

    /// 提议和棋
    pub async fn offer_draw(&self, user_id: Uuid) -> Result<(), String> { ... }

    /// 回应和棋
    pub async fn respond_draw(&self, user_id: Uuid, accept: bool) -> Result<(), String> { ... }

    /// 切换计时器 (走法执行后调用)
    fn switch_timer(&self) {
        // 1. 计算走棋方已用时间
        // 2. 从剩余时间中扣除
        // 3. 检查是否超时
        // 4. 切换 move_start_time
        ...
    }

    /// 检查超时 (由后台任务每秒调用)
    pub async fn check_timeout(&self) -> Option<(GameResult, GameEndReason)> {
        // 1. 获取当前走子方
        // 2. 计算已用时间
        // 3. 检查局时是否用完
        // 4. 检查步时限是否超时
        // 5. 如果超时，结束游戏
        ...
    }

    /// 广播消息到房间内所有客户端
    async fn broadcast(&self, message: &ServerMessage) { ... }
}
```

**时间控制系统设计：**

```
┌─────────────────────────────────────────────┐
│              时间控制三层体系                  │
├─────────────────────────────────────────────┤
│  局时 (time_control)                         │
│  - 每位玩家的总时间 (如 15 分钟 = 900 秒)     │
│  - 用完后进入读秒                             │
├─────────────────────────────────────────────┤
│  步时限 (move_time_limit)                    │
│  - 每步棋的最大用时 (如 30 秒)                │
│  - 超过步时限直接判负                         │
├─────────────────────────────────────────────┤
│  读秒 (byoyomi)                              │
│  - 局时用完后，每步有 N 秒读秒时间            │
│  - 如 10 秒读秒：每步必须在 10 秒内走完       │
│  - 走完后读秒时间重置                         │
└─────────────────────────────────────────────┘
```

#### 步骤 17：房间管理器 (`websocket/manager.rs`)

```rust
pub struct RoomManager {
    /// 活跃房间映射 (game_id -> GameRoom)
    rooms: Arc<RwLock<HashMap<Uuid, Arc<GameRoom>>>>,
    /// 用户到房间的映射 (user_id -> game_id)
    user_rooms: Arc<RwLock<HashMap<Uuid, Uuid>>>,
    /// 数据库仓库引用
    game_repo: GameRepository,
}

impl RoomManager {
    pub fn with_game_repo(game_repo: GameRepository) -> Self { ... }

    /// 获取或创建房间 (懒加载，从数据库同步 FEN)
    pub async fn get_or_create_room(&self, game_id: Uuid) -> Result<Arc<GameRoom>, String> {
        // 1. 检查内存中是否已有房间
        // 2. 如果没有，从数据库加载对局记录
        // 3. 创建 GameRoom 并缓存
        ...
    }

    /// 执行走法 (REST 和 WS 两条路径的统一入口)
    pub async fn make_move(&self, game_id: Uuid, user_id: Uuid, from: &str, to: &str)
        -> Result<MoveResult, String> { ... }

    /// 启动超时检查器 (后台任务，每秒检查一次)
    pub fn start_timeout_checker(&self) {
        let rooms = self.rooms.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                // 遍历所有活跃房间，检查超时
                let rooms_guard = rooms.read().await;
                for (_, room) in rooms_guard.iter() {
                    if let Some((result, reason)) = room.check_timeout().await {
                        // 超时，结束游戏
                        room.handle_game_over(result, reason).await;
                    }
                }
            }
        });
    }
}
```

#### 步骤 18：WebSocket Handler (`handlers/ws_handler.rs`)

```rust
/// GET /ws — WebSocket 升级
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (sender, mut receiver) = socket.split();
    // 创建客户端发送通道
    let (tx, rx) = mpsc::unbounded_channel();
    // 启动发送任务
    tokio::spawn(send_task(rx, sender));
    // 接收消息循环
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let client_msg: ClientMessage = match serde_json::from_str(&text) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                handle_client_message(client_msg, &tx, &state).await;
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }
    // 清理：从房间移除客户端
    cleanup_client(&tx, &state).await;
}

async fn handle_client_message(msg: ClientMessage, tx: &UnboundedSender<String>, state: &AppState) {
    match msg {
        ClientMessage::Auth { token } => {
            // 验证 JWT，创建 Client 对象
            let claims = verify_token(&token, &state.jwt_secret)?;
            // 存储用户信息到连接上下文
            ...
        }
        ClientMessage::JoinGame { game_id } => {
            // 加入房间
            let room = state.room_manager.get_or_create_room(game_id).await?;
            room.join(client, color).await?;
            // 发送 JoinedGame 消息
            ...
        }
        ClientMessage::MakeMove { game_id, from, to } => {
            // 通过 RoomManager 执行走法
            state.room_manager.make_move(game_id, user_id, &from, &to).await?;
        }
        ClientMessage::Resign { game_id } => { ... }
        ClientMessage::OfferDraw { game_id } => { ... }
        ClientMessage::RespondDraw { game_id, accept } => { ... }
        ClientMessage::Ping => { tx.send(r#"{"type":"pong"}"#.into()); }
    }
}
```

#### 步骤 19：组装 main.rs

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 加载 .env
    dotenvy::dotenv().ok();

    // 2. 初始化日志
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or("chess_server=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 3. 加载配置
    let config = AppConfig::from_env()?;

    // 4. 连接数据库
    let pool = PgPoolOptions::new().max_connections(10).connect(&config.database_url).await?;

    // 5. 运行迁移
    sqlx::migrate!("./migrations").run(&pool).await?;

    // 6. 创建应用状态
    let game_repo = GameRepository::new(pool.clone());
    let room_manager = RoomManager::with_game_repo(game_repo.clone());
    room_manager.start_timeout_checker();

    let state = AppState {
        user_repo: UserRepository::new(pool.clone()),
        game_repo,
        room_manager,
        jwt_secret: config.jwt_secret.clone(),
    };

    // 7. 构建路由
    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        // 用户路由
        .route("/api/users", post(user_handler::register))
        .route("/api/users/login", post(user_handler::login))
        .route("/api/users/me", get(user_handler::get_current_user))
        .route("/api/users/me", put(user_handler::update_user))
        .route("/api/users/me", delete(user_handler::delete_user))
        .route("/api/users/{id}", get(user_handler::get_user))
        .route("/api/users", get(user_handler::list_users))
        // 对局路由
        .route("/api/games", post(game_handler::create_game))
        .route("/api/games/{id}", get(game_handler::get_game))
        .route("/api/games/{id}", delete(game_handler::delete_game))
        .route("/api/games", get(game_handler::list_games))
        .route("/api/games/{id}/join", post(game_handler::join_game))
        // AI 和走法路由
        .route("/api/ai/move", post(ai_handler::get_ai_move))
        .route("/api/moves/valid", post(move_handler::get_valid_moves))
        .route("/api/games/{id}/move", post(game_move_handler::make_move))
        // WebSocket
        .route("/ws", get(ws_handler::ws_handler))
        // 中间件
        .layer(axum::middleware::from_fn_with_state(state.clone(), add_jwt_secret_to_extensions))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(state);

    // 8. 启动服务
    let addr = format!("{}:{}", config.host, config.port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
```

### 4.6 API 路由总览

| 方法 | 路径 | 认证 | 功能 |
|------|------|------|------|
| GET | `/health` | ❌ | 健康检查 |
| POST | `/api/users` | ❌ | 注册 |
| POST | `/api/users/login` | ❌ | 登录 |
| GET | `/api/users/me` | ✅ | 获取当前用户 |
| PUT | `/api/users/me` | ✅ | 更新用户信息 |
| DELETE | `/api/users/me` | ✅ | 删除用户 |
| GET | `/api/users/{id}` | ❌ | 获取指定用户 |
| GET | `/api/users` | ❌ | 列出用户 |
| POST | `/api/games` | ✅ | 创建对局 |
| GET | `/api/games/{id}` | ❌ | 获取对局详情 |
| DELETE | `/api/games/{id}` | ✅ | 删除对局 |
| GET | `/api/games` | ❌ | 列出对局 |
| POST | `/api/games/{id}/join` | ✅ | 加入对局 |
| POST | `/api/ai/move` | ❌ | AI 推荐走法 |
| POST | `/api/moves/valid` | ❌ | 查询合法走法 |
| POST | `/api/games/{id}/move` | ✅ | 执行走法 |
| GET | `/ws` | WS Auth | WebSocket 连接 |

### 4.7 构建与运行

```bash
cd chess-server

# 确保 PostgreSQL 已启动且 chess 数据库已创建
# 确保 .env 文件已配置

# 构建并运行 (自动执行数据库迁移)
cargo run

# 服务启动在 http://0.0.0.0:3000
```

---

## 五、工程三：chess-client（前端客户端）

> 基于 Vue 3 + TypeScript + Vite 的单页面应用，使用 HTML5 Canvas 渲染棋盘，Pinia 管理状态。

### 5.1 创建工程

```bash
cd chinese-chess
npm create vite@latest chess-client -- --template vue-ts
cd chess-client
npm install
```

### 5.2 安装依赖

```bash
# 核心依赖
npm install vue-router@4 pinia axios

# 开发依赖
npm install -D vitest @vue/test-utils
```

### 5.3 模块架构

```
chess-client/src/
├── main.ts                 # 入口：创建 Vue 应用，挂载 Pinia + Router
├── App.vue                 # 根组件 (RouterView)
├── style.css               # 全局样式
├── api/
│   ├── index.ts            # Axios REST 客户端 (认证拦截器)
│   └── websocket.ts        # WebSocket 单例服务 (重连/心跳)
├── stores/
│   ├── user.ts             # 用户状态 (登录/注册/Token 管理)
│   └── game.ts             # 游戏状态 (对局生命周期管理)
├── router/
│   └── index.ts            # 路由配置 + 认证守卫
├── types/
│   └── index.ts            # TypeScript 类型定义 (匹配后端 API)
├── components/
│   └── ChessBoard.vue      # Canvas 棋盘组件
├── views/
│   ├── LoginView.vue       # 登录页
│   ├── RegisterView.vue    # 注册页
│   ├── LobbyView.vue       # 大厅页 (对局列表/创建/加入)
│   ├── GameView.vue        # 对局页 (棋盘/计时器/操作)
│   └── ProfileView.vue     # 个人资料页
└── __tests__/
    ├── gameStore.test.ts
    ├── GameView.test.ts
    ├── LobbyView.test.ts
    └── userStore.test.ts
```

### 5.4 实现步骤

#### 步骤 1：配置 Vite (`vite.config.ts`)

```typescript
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

export default defineConfig({
  plugins: [vue()],
  server: {
    port: 5173,
    host: '0.0.0.0',
    proxy: {
      '/api': {
        target: 'http://localhost:3000',
        changeOrigin: true,
      },
      '/ws': {
        target: 'ws://localhost:3000',
        ws: true,
      },
    },
  },
});
```

> Vite 开发代理将 `/api` 和 `/ws` 请求转发到后端，避免跨域问题。

#### 步骤 2：TypeScript 类型定义 (`types/index.ts`)

定义与后端 API 完全匹配的类型接口，包括：

- `User`, `LoginResponse`, `CreateUser`, `LoginRequest`, `UpdateUser`
- `Game`, `GameStatus`, `GameResult`, `CreateGameRequest`, `CreateGameResponse`
- `AiMoveRequest`, `AiMoveResponse`
- WebSocket 消息类型：`WsClientMessage` (联合类型), `WsServerMessage` (联合类型)
- 各具体消息接口：`WsAuthMessage`, `WsJoinGameMessage`, `WsMakeMoveMessage`, `WsMoveMadeMessage`, `WsGameOverMessage` 等

> 详见 [三、工程二](#四工程二chess-server后端服务) 中的 WebSocket 消息类型定义，前端类型需与之完全对应。

#### 步骤 3：REST API 客户端 (`api/index.ts`)

```typescript
import axios, { type AxiosInstance } from 'axios';

const API_BASE_URL = import.meta.env.VITE_API_URL || '';

class ApiService {
  private client: AxiosInstance;

  constructor() {
    this.client = axios.create({
      baseURL: API_BASE_URL,
      headers: { 'Content-Type': 'application/json' },
    });

    // 请求拦截器：添加 Authorization header
    this.client.interceptors.request.use((config) => {
      const token = localStorage.getItem('token');
      if (token) {
        config.headers.Authorization = `Bearer ${token}`;
      }
      return config;
    });

    // 响应拦截器：401 时清除 token 并跳转登录
    this.client.interceptors.response.use(
      (response) => response,
      (error) => {
        if (error.response?.status === 401) {
          localStorage.removeItem('token');
          localStorage.removeItem('user');
          window.location.href = '/login';
        }
        return Promise.reject(error);
      }
    );
  }

  // 用户 API
  async register(data: CreateUser): Promise<LoginResponse> { ... }
  async login(data: LoginRequest): Promise<LoginResponse> { ... }
  async getCurrentUser(): Promise<User> { ... }
  async updateUser(data: UpdateUser): Promise<User> { ... }
  async deleteUser(): Promise<void> { ... }
  async getUser(id: string): Promise<User> { ... }
  async listUsers(page?: number, pageSize?: number): Promise<User[]> { ... }

  // 对局 API
  async createGame(data: CreateGameRequest): Promise<CreateGameResponse> { ... }
  async getGame(id: string): Promise<Game> { ... }
  async listGames(status?: string, page?: number, pageSize?: number): Promise<Game[]> { ... }
  async joinGame(id: string): Promise<Game> { ... }
  async deleteGame(id: string): Promise<void> { ... }

  // AI API
  async getAiMove(data: AiMoveRequest): Promise<AiMoveResponse> { ... }

  // 走法 API
  async getValidMoves(fen: string, from: string): Promise<string[]> { ... }
  async makeMove(gameId: string, from: string, to: string): Promise<...> { ... }
}

export const api = new ApiService();
```

#### 步骤 4：WebSocket 服务 (`api/websocket.ts`)

```typescript
class WebSocketService {
  private ws: WebSocket | null = null;
  private messageHandlers: Set<MessageHandler> = new Set();
  private connectHandlers: Set<ConnectionHandler> = new Set();
  private disconnectHandlers: Set<ConnectionHandler> = new Set();
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private reconnectDelay = 1000;
  private pingInterval: number | null = null;

  /// 连接 WebSocket 并发送认证消息
  connect(token: string): Promise<void> {
    return new Promise((resolve, reject) => {
      this.ws = new WebSocket(WS_URL);
      this.ws.onopen = () => {
        this.send({ type: 'auth', token });  // 首条消息必须是认证
        this.reconnectAttempts = 0;
        this.startPing();  // 启动 30 秒心跳
        resolve();
      };
      this.ws.onmessage = (event) => {
        const message: WsServerMessage = JSON.parse(event.data);
        this.messageHandlers.forEach(h => h(message));
      };
      this.ws.onclose = (event) => {
        this.stopPing();
        // 非正常关闭时自动重连
        if (event.code !== 1000 && this.reconnectAttempts < this.maxReconnectAttempts) {
          this.reconnectAttempts++;
          setTimeout(() => {
            const token = localStorage.getItem('token');
            if (token) this.connect(token);
          }, this.reconnectDelay * this.reconnectAttempts);
        }
      };
    });
  }

  // 便捷方法
  joinGame(gameId: string): void { this.send({ type: 'join_game', game_id: gameId }); }
  leaveGame(gameId: string): void { this.send({ type: 'leave_game', game_id: gameId }); }
  makeMove(gameId: string, from: string, to: string): void { ... }
  resign(gameId: string): void { ... }
  offerDraw(gameId: string): void { ... }
  respondDraw(gameId: string, accept: boolean): void { ... }

  // 事件注册 (返回取消函数)
  onMessage(handler: MessageHandler): () => void { ... }
  onConnect(handler: ConnectionHandler): () => void { ... }
  onDisconnect(handler: ConnectionHandler): () => void { ... }

  // 30 秒心跳
  private startPing(): void {
    this.pingInterval = setInterval(() => {
      if (this.ws?.readyState === WebSocket.OPEN) {
        this.ws.send(JSON.stringify({ type: 'ping' }));
      }
    }, 30000);
  }
}

export const wsService = new WebSocketService();
```

#### 步骤 5：用户 Store (`stores/user.ts`)

```typescript
export const useUserStore = defineStore('user', () => {
  const token = ref<string | null>(localStorage.getItem('token'));
  const user = ref<User | null>(null);
  const isLoggedIn = computed(() => !!token.value);

  /// 初始化 (从 localStorage 恢复)
  async function init() {
    if (token.value) {
      try {
        user.value = await api.getCurrentUser();
      } catch {
        logout();
      }
    }
  }

  /// 注册
  async function register(username: string, password: string, nickname?: string) {
    const res = await api.register({ username, password, nickname });
    token.value = res.token;
    user.value = res.user;
    localStorage.setItem('token', res.token);
    localStorage.setItem('user', JSON.stringify(res.user));
  }

  /// 登录
  async function login(username: string, password: string) {
    const res = await api.login({ username, password });
    token.value = res.token;
    user.value = res.user;
    localStorage.setItem('token', res.token);
    localStorage.setItem('user', JSON.stringify(res.user));
  }

  /// 登出
  function logout() {
    token.value = null;
    user.value = null;
    localStorage.removeItem('token');
    localStorage.removeItem('user');
    wsService.disconnect();
  }

  return { token, user, isLoggedIn, init, register, login, logout };
});
```

#### 步骤 6：游戏 Store (`stores/game.ts`) — **最复杂的 Store**

```typescript
export const useGameStore = defineStore('game', () => {
  // 状态
  const currentGame = ref<Game | null>(null);
  const playerColor = ref<'red' | 'black' | null>(null);
  const isSpectator = ref(false);
  const selectedSquare = ref<string | null>(null);
  const validMoves = ref<string[]>([]);
  const moveHistory = ref<string[]>([]);
  const drawOffered = ref(false);
  const drawOfferFrom = ref<'red' | 'black' | null>(null);

  // 计时器
  const redTime = ref<number>(0);
  const blackTime = ref<number>(0);
  const timerInterval = ref<number | null>(null);

  /// 创建对局
  async function createGame(data: CreateGameRequest) {
    const res = await api.createGame(data);
    playerColor.value = res.color;
    await loadGame(res.game_id);
    wsService.joinGame(res.game_id);
  }

  /// 加入对局
  async function joinGame(gameId: string) {
    const game = await api.joinGame(gameId);
    currentGame.value = game;
    // 确定玩家颜色
    playerColor.value = determineColor(game);
    wsService.joinGame(gameId);
  }

  /// 加载对局
  async function loadGame(gameId: string) {
    const game = await api.getGame(gameId);
    currentGame.value = game;
    redTime.value = game.red_time ?? 0;
    blackTime.value = game.black_time ?? 0;
  }

  /// 选择棋格
  async function selectSquare(position: string) {
    if (!currentGame.value || isSpectator.value) return;

    // 如果点击了合法走法目标，执行走法
    if (validMoves.value.includes(position) && selectedSquare.value) {
      await makeMove(selectedSquare.value, position);
      selectedSquare.value = null;
      validMoves.value = [];
      return;
    }

    // 选择己方棋子
    const fen = currentGame.value.fen;
    const moves = await api.getValidMoves(fen, position);
    if (moves.length > 0) {
      selectedSquare.value = position;
      validMoves.value = moves;
    } else {
      selectedSquare.value = null;
      validMoves.value = [];
    }
  }

  /// 执行走法
  async function makeMove(from: string, to: string) {
    if (!currentGame.value) return;
    const res = await api.makeMove(currentGame.value.id, from, to);
    // 更新 FEN
    if (currentGame.value) {
      currentGame.value.fen = res.fen;
    }
  }

  /// 处理 WebSocket 消息
  function handleWsMessage(message: WsServerMessage) {
    switch (message.type) {
      case 'joined_game':
        currentGame.value = { ... };
        startLocalTimer();
        break;
      case 'opponent_joined':
        // 对手加入，游戏开始
        break;
      case 'move_made':
        // 更新 FEN 和走法历史
        currentGame.value!.fen = message.fen;
        moveHistory.value.push(message.mv.notation);
        break;
      case 'game_over':
        // 游戏结束
        stopLocalTimer();
        currentGame.value!.status = 'finished';
        currentGame.value!.result = message.result as GameResult;
        break;
      case 'time_update':
        // 同步服务器时间
        redTime.value = message.red_time;
        blackTime.value = message.black_time;
        break;
      case 'draw_offered':
        drawOffered.value = true;
        drawOfferFrom.value = ...;
        break;
      case 'draw_response':
        if (message.accepted) { /* 和棋 */ }
        drawOffered.value = false;
        break;
      case 'illegal_move':
        // 显示错误提示
        break;
      case 'opponent_disconnected':
        // 显示对手断线提示
        break;
    }
  }

  /// 启动本地计时器 (1 秒间隔)
  function startLocalTimer() {
    stopLocalTimer();
    timerInterval.value = window.setInterval(() => {
      if (!currentGame.value || currentGame.value.status !== 'playing') {
        stopLocalTimer();
        return;
      }
      // 根据当前走子方扣减时间
      const active = parseFenSide(currentGame.value.fen);
      if (active === 'red') {
        redTime.value = Math.max(0, redTime.value - 1);
      } else {
        blackTime.value = Math.max(0, blackTime.value - 1);
      }
    }, 1000);
  }

  function stopLocalTimer() {
    if (timerInterval.value) {
      clearInterval(timerInterval.value);
      timerInterval.value = null;
    }
  }

  // 注册 WS 消息监听
  wsService.onMessage(handleWsMessage);

  return {
    currentGame, playerColor, isSpectator, selectedSquare, validMoves,
    moveHistory, drawOffered, redTime, blackTime,
    createGame, joinGame, loadGame, selectSquare, makeMove,
    resign, offerDraw, respondDraw,
  };
});
```

#### 步骤 7：路由配置 (`router/index.ts`)

```typescript
const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', redirect: '/lobby' },
    { path: '/login', name: 'Login', component: () => import('../views/LoginView.vue'), meta: { guest: true } },
    { path: '/register', name: 'Register', component: () => import('../views/RegisterView.vue'), meta: { guest: true } },
    { path: '/lobby', name: 'Lobby', component: () => import('../views/LobbyView.vue'), meta: { requiresAuth: true } },
    { path: '/game/:id', name: 'Game', component: () => import('../views/GameView.vue'), meta: { requiresAuth: true } },
    { path: '/profile', name: 'Profile', component: () => import('../views/ProfileView.vue'), meta: { requiresAuth: true } },
  ],
});

// 认证守卫
router.beforeEach((to, _from, next) => {
  const userStore = useUserStore();
  if (!userStore.token) userStore.init();

  if (to.meta.requiresAuth && !userStore.isLoggedIn) {
    next({ name: 'Login', query: { redirect: to.fullPath } });
  } else if (to.meta.guest && userStore.isLoggedIn) {
    next({ name: 'Lobby' });
  } else {
    next();
  }
});
```

#### 步骤 8：Canvas 棋盘组件 (`components/ChessBoard.vue`)

这是前端最核心的渲染组件，使用 HTML5 Canvas 绘制中国象棋棋盘。

**核心设计：**

```typescript
// 棋盘参数
const BOARD_COLS = 9;       // 9 列
const BOARD_ROWS = 10;      // 10 行
const CELL_SIZE = 60;       // 格子大小
const PADDING = 40;         // 边距
const PIECE_RADIUS = 25;    // 棋子半径

// 绘制流程
function draw() {
  clearCanvas();
  drawBoard();       // 棋盘线、九宫斜线、河界
  drawMarkers();     // 炮/兵起始位标记
  drawRiver();       // "楚河 汉界" 文字
  drawPieces();      // 所有棋子
  drawSelection();   // 选中高亮
  drawValidMoves();  // 合法走法指示
  drawLastMove();    // 上一步走法指示
}
```

**棋盘绘制细节：**

| 元素 | 说明 |
|------|------|
| 棋盘线 | 9×10 网格，河界处中间 7 列断开 |
| 九宫斜线 | 红方 (d7-f9) 和黑方 (d0-f2) 的对角线 |
| 河界文字 | "楚河" (左侧) 和 "汉界" (右侧) |
| 位置标记 | 炮位 (b2, h2, b7, h7) 和兵位 (a3, c3, e3, g3, i3, a6, c6, e6, g6, i6) 的十字标记 |
| 棋子 | 圆形，径向渐变填充，内圈边框，中文文字 |
| 红方棋子 | 红色文字：帅/仕/相/马/车/炮/兵 |
| 黑方棋子 | 黑色文字：将/士/象/马/车/炮/卒 |

**FEN 解析与棋子渲染：**

```typescript
function parseFen(fen: string): Map<string, { type: string; color: 'red' | 'black' }> {
  const pieces = new Map();
  const rows = fen.split(' ')[0].split('/');
  for (let row = 0; row < rows.length; row++) {
    let col = 0;
    for (const ch of rows[row]) {
      if (ch >= '1' && ch <= '9') {
        col += parseInt(ch);
      } else {
        const color = ch === ch.toUpperCase() ? 'red' : 'black';
        const type = fenCharToName(ch);
        pieces.set(`${col},${row}`, { type, color });
        col++;
      }
    }
  }
  return pieces;
}
```

**棋盘翻转：**

```typescript
// 黑方玩家视角：棋盘上下翻转
function getDisplayPosition(col: number, row: number, flip: boolean) {
  if (flip) {
    return { x: PADDING + (8 - col) * CELL_SIZE, y: PADDING + (9 - row) * CELL_SIZE };
  }
  return { x: PADDING + col * CELL_SIZE, y: PADDING + row * CELL_SIZE };
}
```

**点击处理：**

```typescript
function handleClick(event: MouseEvent) {
  const rect = canvas.getBoundingClientRect();
  const x = event.clientX - rect.left;
  const y = event.clientY - rect.top;

  // 像素坐标 → 棋盘坐标
  const col = Math.round((x - PADDING) / CELL_SIZE);
  const row = Math.round((y - PADDING) / CELL_SIZE);

  // 如果翻转，转换坐标
  const actualCol = flip ? 8 - col : col;
  const actualRow = flip ? 9 - row : row;

  // 转为 UCI 格式 (如 "a0")
  const position = String.fromCharCode('a'.charCodeAt(0) + actualCol) + actualRow;

  // 通知 gameStore
  gameStore.selectSquare(position);
}
```

#### 步骤 9：页面视图实现

**LoginView.vue：**
- 用户名 + 密码表单
- 登录成功后跳转到 redirect 参数或大厅
- 错误提示

**RegisterView.vue：**
- 用户名 + 密码 + 昵称表单
- 前端验证：用户名 3-20 字符、字母开头、字母数字下划线；密码 6-100 字符
- 保留名检查 (root, admin, system, guest 等)
- 注册成功自动登录

**LobbyView.vue：**
- 对局列表 (支持状态过滤：全部/等待中/进行中)
- 创建对局弹窗：选择颜色、设置时间控制
  - 局时：5-45 分钟
  - 步时限：可选
  - 读秒：可选
- 加入/观战/删除操作

**GameView.vue：**
- 棋盘组件 (ChessBoard)
- 双方计时器显示
- 走法历史列表
- 操作按钮：认输、提议和棋、回应和棋
- 观战模式 (不可走棋)
- 游戏结束弹窗

**ProfileView.vue：**
- 修改昵称
- 查看战绩统计 (胜/负/和、Elo 评分)
- 删除账号

#### 步骤 10：应用入口 (`main.ts`)

```typescript
import { createApp } from 'vue';
import { createPinia } from 'pinia';
import App from './App.vue';
import router from './router';
import './style.css';

const app = createApp(App);
app.use(createPinia());
app.use(router);
app.mount('#app');
```

### 5.5 构建与运行

```bash
cd chess-client

# 开发模式
npm run dev
# 访问 http://localhost:5173

# 生产构建
npm run build

# 预览生产构建
npm run preview

# 运行测试
npm run test
```

---

## 六、前后端联调与集成测试

### 6.1 启动顺序

```bash
# 1. 确保 PostgreSQL 运行中
#    数据库: chess
#    用户: postgres / 密码: postgres

# 2. 启动后端
cd chinese-chess/chess-server
cargo run
# 输出: Server starting on 0.0.0.0:3000

# 3. 启动前端 (新终端)
cd chinese-chess/chess-client
npm run dev
# 输出: Local: http://localhost:5173/
```

### 6.2 联调检查清单

| 序号 | 检查项 | 验证方法 |
|------|--------|----------|
| 1 | 后端健康检查 | `curl http://localhost:3000/health` → "OK" |
| 2 | 用户注册 | POST `/api/users` 创建新用户 |
| 3 | 用户登录 | POST `/api/users/login` 获取 JWT |
| 4 | 认证接口 | GET `/api/users/me` 带 Bearer token |
| 5 | 创建对局 | POST `/api/games` 创建等待中的对局 |
| 6 | 加入对局 | POST `/api/games/{id}/join` |
| 7 | WebSocket 连接 | 连接 `ws://localhost:3000/ws`，发送 auth |
| 8 | 实时走棋 | 通过 WS 发送 make_move，对手收到 move_made |
| 9 | 计时器同步 | WS time_update 消息与本地计时器一致 |
| 10 | 游戏结束 | 将杀/超时/认输后收到 game_over |
| 11 | Elo 评分更新 | 对局结束后检查用户评分变化 |
| 12 | 前端代理 | 浏览器访问 `http://localhost:5173` 能正常使用 |

### 6.3 双人对战测试流程

1. 打开两个浏览器窗口 (或无痕模式)
2. 窗口 A：注册用户 Alice，创建对局 (选红方)
3. 窗口 B：注册用户 Bob，在大厅加入 Alice 的对局
4. 双方交替走棋，验证：
   - 棋盘正确更新
   - 计时器正确倒计时
   - 走法历史正确记录
   - 将军提示正确显示
   - 将杀/困毙正确判定
5. 测试认输、和棋功能
6. 测试超时判负

### 6.4 常见问题排查

| 问题 | 原因 | 解决 |
|------|------|------|
| 数据库连接失败 | PostgreSQL 未启动或连接串错误 | 检查 .env 中的 DATABASE_URL |
| JWT 验证失败 | JWT_SECRET 不一致 | 确认 .env 中 JWT_SECRET 已设置 |
| WebSocket 连不上 | 后端未启动或代理配置错误 | 检查 Vite proxy 配置和后端端口 |
| 走法被拒绝 | FEN 不一致或走法不合法 | 检查前端发送的 FEN 是否与后端一致 |
| 计时器不准 | 本地与服务器时间不同步 | 依赖 WS time_update 消息同步 |
| CORS 错误 | 开发环境未配置 CORS | 后端已配置 `CorsLayer::new().allow_origin(Any)` |

---

## 七、项目目录结构总览

```
09_Chess/
├── DEV.md                          # 本文档
├── Readme.md                       # 开发笔记
├── kill_node_cargo.bat             # Windows 进程清理脚本
├── .gitignore
├── .claude/
│   └── settings.local.json
│
└── chinese-chess/
    │
    ├── chess-engine/               # 象棋引擎库 (Rust)
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs              # 库入口
    │       ├── board/
    │       │   ├── mod.rs
    │       │   ├── position.rs     # 坐标系统
    │       │   ├── board.rs        # 棋盘表示 + FEN
    │       │   └── move_gen.rs     # 走法生成
    │       ├── pieces/
    │       │   ├── mod.rs
    │       │   ├── types.rs        # 棋子类型/颜色/分值
    │       │   └── movement.rs     # 移动模式
    │       ├── rules/
    │       │   ├── mod.rs
    │       │   ├── check.rs        # 将军/将杀/困毙
    │       │   └── validator.rs    # 走法验证
    │       ├── game/
    │       │   ├── mod.rs
    │       │   └── state.rs        # 游戏状态管理
    │       └── ai/
    │           ├── mod.rs
    │           ├── eval.rs         # 评估函数
    │           └── search.rs       # Alpha-Beta 搜索
    │       └── tests/
    │           └── integration_tests.rs
    │
    ├── chess-server/               # 后端服务 (Rust/Axum)
    │   ├── Cargo.toml
    │   ├── .env                    # 环境变量
    │   ├── migrations/
    │   │   ├── 20250101000000_init.sql
    │   │   ├── 20250102000000_add_time_control.sql
    │   │   └── 20250103000000_add_increment_byoyomi.sql
    │   └── src/
    │       ├── main.rs             # 服务入口
    │       ├── config/
    │       │   └── mod.rs          # 配置加载
    │       ├── db/
    │       │   ├── mod.rs
    │       │   ├── models.rs       # 数据模型
    │       │   └── repositories/
    │       │       ├── mod.rs
    │       │       ├── user_repo.rs  # 用户仓库
    │       │       └── game_repo.rs  # 对局仓库
    │       ├── handlers/
    │       │   ├── mod.rs
    │       │   ├── user_handler.rs       # 用户 API
    │       │   ├── game_handler.rs       # 对局 API
    │       │   ├── game_move_handler.rs  # 走棋 API
    │       │   ├── ai_handler.rs         # AI API
    │       │   ├── move_handler.rs       # 合法走法 API
    │       │   └── ws_handler.rs         # WebSocket Handler
    │       ├── middleware/
    │       │   └── auth.rs         # JWT 认证
    │       ├── websocket/
    │       │   ├── mod.rs
    │       │   ├── client.rs       # WS 客户端封装
    │       │   ├── room.rs         # 游戏房间 (核心)
    │       │   ├── manager.rs      # 房间管理器
    │       │   └── message.rs      # WS 消息类型
    │       ├── utils/
    │       │   ├── mod.rs
    │       │   ├── auth.rs         # JWT 工具
    │       │   └── password.rs     # 密码哈希
    │       ├── services/
    │       │   └── mod.rs          # 业务逻辑层 (预留)
    │       └── error.rs            # 统一错误处理
    │
    └── chess-client/               # 前端客户端 (Vue 3)
        ├── package.json
        ├── vite.config.ts
        ├── tsconfig.json
        ├── index.html
        ├── .env.example
        └── src/
            ├── main.ts             # 应用入口
            ├── App.vue             # 根组件
            ├── style.css           # 全局样式
            ├── api/
            │   ├── index.ts        # REST 客户端
            │   └── websocket.ts    # WebSocket 服务
            ├── stores/
            │   ├── user.ts         # 用户 Store
            │   └── game.ts         # 游戏 Store
            ├── router/
            │   └── index.ts        # 路由配置
            ├── types/
            │   └── index.ts        # TypeScript 类型
            ├── components/
            │   └── ChessBoard.vue  # Canvas 棋盘
            ├── views/
            │   ├── LoginView.vue   # 登录
            │   ├── RegisterView.vue # 注册
            │   ├── LobbyView.vue   # 大厅
            │   ├── GameView.vue    # 对局
            │   └── ProfileView.vue # 个人资料
            └── __tests__/
                ├── gameStore.test.ts
                ├── GameView.test.ts
                ├── LobbyView.test.ts
                └── userStore.test.ts
```

---

## 附录 A：关键技术决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 棋盘表示 | HashMap<Position, Piece> | 快速查找，增量维护方便 |
| 走法格式 | UCI (如 "a0a1") | 简洁、无歧义、易于传输 |
| AI 算法 | Alpha-Beta + Quiescence | 平衡棋力与搜索时间 |
| 最大搜索深度 | 6 | 约 1-3 秒搜索时间 |
| 认证方案 | JWT (24h 过期) | 无状态，适合 REST + WS |
| 密码哈希 | Argon2id | 当前最安全的密码哈希算法 |
| 实时通信 | WebSocket | 双向通信，低延迟 |
| 走法双通道 | REST + WebSocket | REST 保证可靠性，WS 保证实时性 |
| 棋盘渲染 | HTML5 Canvas | 灵活绘制，性能好 |
| 状态管理 | Pinia | Vue 3 官方推荐，TypeScript 友好 |
| 数据库迁移 | SQLx embed migrate | 编译时检查 SQL，自动迁移 |
| Elo K 因子 | 32 | 标准值，适合业余级别 |
| 困毙规则 | 困毙 = 输 | 中国象棋标准规则 (不同于国际象棋) |

## 附录 B：中国象棋 FEN 初始局面

```
rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1
```

- `rnbakabnr` — 黑方底线：车马象士将士象马车
- `9` — 黑方炮行 (全空)
- `1c5c1` — 黑方炮位：空炮(5空)炮空
- `p1p1p1p1p` — 黑方卒行
- `9/9` — 河界 (两行全空)
- `P1P1P1P1P` — 红方兵行
- `1C5C1` — 红方炮位
- `9` — 红方炮行 (全空)
- `RNBAKABNR` — 红方底线
- `w` — 红方先走

## 附录 C：WebSocket 协议时序图

```
客户端A (红方)          服务端           客户端B (黑方)
     │                   │                   │
     │── auth ──────────>│                   │
     │<─ (认证成功) ─────│                   │
     │── join_game ─────>│                   │
     │<─ joined_game ────│                   │
     │   (color=red,     │                   │
     │    status=waiting)│                   │
     │                   │<──── auth ────────│
     │                   │<── join_game ─────│
     │<─ opponent_joined │── opponent_joined>│
     │                   │── joined_game ───>│
     │                   │   (color=black,   │
     │                   │    status=playing)│
     │                   │                   │
     │── make_move ─────>│                   │
     │   (a0a1)          │── move_made ─────>│
     │<─ move_made ──────│   (a0a1, fen=...) │
     │   (a0a1, fen=...) │                   │
     │                   │<── make_move ─────│
     │                   │   (b0c2)          │
     │<─ move_made ──────│── move_made ─────>│
     │   (b0c2, fen=...) │                   │
     │                   │                   │
     │  ... 交替走棋 ...  │                   │
     │                   │                   │
     │<─ game_over ──────│── game_over ─────>│
     │   (red_win,       │   (red_win,       │
     │    checkmate)     │    checkmate)     │
```
