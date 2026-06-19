/**
 * Pure chess utility functions extracted from ChessBoard.vue for testability.
 */

// FEN char to Chinese name
const FEN_TO_NAME: Record<string, string> = {
  K: '帅', A: '仕', B: '相', N: '马', R: '车', C: '炮', P: '兵',
  k: '将', a: '士', b: '象', n: '马', r: '车', c: '炮', p: '卒',
};

/** Parse a FEN string into a map of piece positions. */
export function parseFen(fen: string): Map<string, { type: string; color: 'red' | 'black' }> {
  const pieces = new Map<string, { type: string; color: 'red' | 'black' }>();
  if (!fen) return pieces;
  const rows = fen.split(' ')[0].split('/');
  for (let row = 0; row < rows.length && row < 10; row++) {
    let col = 0;
    for (const ch of rows[row]) {
      if (ch >= '1' && ch <= '9') {
        col += parseInt(ch);
      } else {
        const color: 'red' | 'black' = ch === ch.toUpperCase() ? 'red' : 'black';
        const name = FEN_TO_NAME[ch] || ch;
        pieces.set(`${col},${row}`, { type: name, color });
        col++;
      }
    }
  }
  return pieces;
}

/** Find the king position for the given color from the FEN. */
export function findKing(fen: string, color: 'red' | 'black'): { col: number; row: number } | null {
  const pieces = parseFen(fen);
  const kingName = color === 'red' ? '帅' : '将';
  for (const [key, piece] of pieces) {
    if (piece.type === kingName && piece.color === color) {
      const [col, row] = key.split(',').map(Number);
      return { col, row };
    }
  }
  return null;
}

/** Parse a UCI position string (e.g., "a0", "e5") into col/row coordinates. */
export function parseUciPosition(uci: string): { col: number; row: number } | null {
  if (uci.length !== 2) return null;
  const col = uci.charCodeAt(0) - 'a'.charCodeAt(0);
  const row = parseInt(uci[1]);
  if (isNaN(row) || col < 0 || col > 8 || row < 0 || row > 9) return null;
  return { col, row };
}

/** Determine which side is to move from a FEN string. */
export function getSideToMove(fen: string): 'red' | 'black' {
  const parts = fen.split(' ');
  return parts[1] === 'w' ? 'red' : 'black';
}
