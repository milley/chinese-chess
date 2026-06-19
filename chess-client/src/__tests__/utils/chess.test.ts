import { describe, it, expect } from 'vitest';
import { parseFen, findKing, parseUciPosition, getSideToMove, getDisplayPosition, pixelToPosition, PADDING, CELL_SIZE } from '../../utils/chess';

const INITIAL_FEN = 'rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1';

describe('parseFen', () => {
  it('initial position has 32 pieces', () => {
    const pieces = parseFen(INITIAL_FEN);
    expect(pieces.size).toBe(32);
  });

  it('empty string returns empty map', () => {
    const pieces = parseFen('');
    expect(pieces.size).toBe(0);
  });

  it('uppercase FEN chars map to red', () => {
    const pieces = parseFen('R9/9/9/9/9/9/9/9/9/4K4 w - - 0 1');
    const rook = pieces.get('0,0');
    expect(rook).toBeDefined();
    expect(rook!.color).toBe('red');
  });

  it('lowercase FEN chars map to black', () => {
    const pieces = parseFen('r9/9/9/9/9/9/9/9/9/4K4 w - - 0 1');
    const rook = pieces.get('0,0');
    expect(rook).toBeDefined();
    expect(rook!.color).toBe('black');
  });

  it('partial board returns correct count', () => {
    const pieces = parseFen('4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1');
    expect(pieces.size).toBe(2);
  });
});

describe('findKing', () => {
  it('finds red king in initial position', () => {
    const pos = findKing(INITIAL_FEN, 'red');
    expect(pos).toEqual({ col: 4, row: 9 });
  });

  it('finds black king in initial position', () => {
    const pos = findKing(INITIAL_FEN, 'black');
    expect(pos).toEqual({ col: 4, row: 0 });
  });

  it('returns null when king is missing', () => {
    const fen = 'rnb1kabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1';
    // This FEN has the red king but let's test a FEN with no red 帅
    const noRedKingFen = 'rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBA1ABNR w - - 0 1';
    const pos = findKing(noRedKingFen, 'red');
    expect(pos).toBeNull();
  });
});

describe('parseUciPosition', () => {
  it('parses valid positions', () => {
    expect(parseUciPosition('a0')).toEqual({ col: 0, row: 0 });
    expect(parseUciPosition('e5')).toEqual({ col: 4, row: 5 });
    expect(parseUciPosition('i9')).toEqual({ col: 8, row: 9 });
  });

  it('returns null for invalid length', () => {
    expect(parseUciPosition('a')).toBeNull();
    expect(parseUciPosition('abc')).toBeNull();
  });

  it('returns null for out-of-bounds', () => {
    expect(parseUciPosition('j0')).toBeNull(); // col 9 out of bounds
    expect(parseUciPosition('a10')).toBeNull(); // length 3
  });
});

describe('getSideToMove', () => {
  it('returns red for white side', () => {
    expect(getSideToMove(INITIAL_FEN)).toBe('red');
  });

  it('returns black for black side', () => {
    const blackFen = 'rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR b - - 0 1';
    expect(getSideToMove(blackFen)).toBe('black');
  });
});

describe('getDisplayPosition — board orientation', () => {
  it('red player: row 0 (black back rank) at top, row 9 (red back rank) at bottom', () => {
    const flip = false; // red player
    const top = getDisplayPosition(0, 0, flip);
    const bottom = getDisplayPosition(0, 9, flip);
    expect(top.y).toBe(PADDING);
    expect(bottom.y).toBe(PADDING + 9 * CELL_SIZE);
    expect(top.y).toBeLessThan(bottom.y);
  });

  it('black player: row 0 (black back rank) at bottom, row 9 (red back rank) at top', () => {
    const flip = true; // black player
    const top = getDisplayPosition(0, 9, flip);
    const bottom = getDisplayPosition(0, 0, flip);
    expect(top.y).toBe(PADDING);
    expect(bottom.y).toBe(PADDING + 9 * CELL_SIZE);
    expect(top.y).toBeLessThan(bottom.y);
  });

  it('red player: col 0 at left, col 8 at right', () => {
    const flip = false;
    const left = getDisplayPosition(0, 0, flip);
    const right = getDisplayPosition(8, 0, flip);
    expect(left.x).toBe(PADDING);
    expect(right.x).toBe(PADDING + 8 * CELL_SIZE);
  });

  it('black player: col 0 at right, col 8 at left (mirrored)', () => {
    const flip = true;
    const left = getDisplayPosition(8, 0, flip);
    const right = getDisplayPosition(0, 0, flip);
    expect(left.x).toBe(PADDING);
    expect(right.x).toBe(PADDING + 8 * CELL_SIZE);
  });

  it('red king (col=4, row=9) is at bottom for red player', () => {
    const pos = getDisplayPosition(4, 9, false);
    expect(pos.y).toBe(PADDING + 9 * CELL_SIZE);
  });

  it('red king (col=4, row=9) is at top for black player', () => {
    const pos = getDisplayPosition(4, 9, true);
    expect(pos.y).toBe(PADDING);
  });

  it('black king (col=4, row=0) is at top for red player', () => {
    const pos = getDisplayPosition(4, 0, false);
    expect(pos.y).toBe(PADDING);
  });

  it('black king (col=4, row=0) is at bottom for black player', () => {
    const pos = getDisplayPosition(4, 0, true);
    expect(pos.y).toBe(PADDING + 9 * CELL_SIZE);
  });
});

describe('pixelToPosition — click coordinate mapping', () => {
  it('red player: clicking top-left maps to a0', () => {
    const x = PADDING;
    const y = PADDING;
    expect(pixelToPosition(x, y, false)).toBe('a0');
  });

  it('red player: clicking bottom-right maps to i9', () => {
    const x = PADDING + 8 * CELL_SIZE;
    const y = PADDING + 9 * CELL_SIZE;
    expect(pixelToPosition(x, y, false)).toBe('i9');
  });

  it('black player: clicking top-left maps to i9 (flipped)', () => {
    const x = PADDING;
    const y = PADDING;
    expect(pixelToPosition(x, y, true)).toBe('i9');
  });

  it('black player: clicking bottom-right maps to a0 (flipped)', () => {
    const x = PADDING + 8 * CELL_SIZE;
    const y = PADDING + 9 * CELL_SIZE;
    expect(pixelToPosition(x, y, true)).toBe('a0');
  });

  it('red player: clicking red king area maps to e9', () => {
    // Red king is at col=4, row=9 → pixel (PADDING + 4*CELL_SIZE, PADDING + 9*CELL_SIZE)
    const x = PADDING + 4 * CELL_SIZE;
    const y = PADDING + 9 * CELL_SIZE;
    expect(pixelToPosition(x, y, false)).toBe('e9');
  });

  it('black player: clicking red king area (top of screen) maps to e9', () => {
    // For black player, red king (col=4, row=9) is at the top of the screen
    const x = PADDING + 4 * CELL_SIZE;
    const y = PADDING;
    expect(pixelToPosition(x, y, true)).toBe('e9');
  });

  it('returns null for clicks outside the board', () => {
    expect(pixelToPosition(0, 0, false)).toBeNull();
    expect(pixelToPosition(PADDING + 9 * CELL_SIZE, PADDING, false)).toBeNull();
  });

  it('round-trip: display position → pixel → UCI matches original', () => {
    // For red player
    const pos = getDisplayPosition(3, 5, false);
    const uci = pixelToPosition(pos.x, pos.y, false);
    expect(uci).toBe('d5');

    // For black player
    const posFlip = getDisplayPosition(3, 5, true);
    const uciFlip = pixelToPosition(posFlip.x, posFlip.y, true);
    expect(uciFlip).toBe('d5');
  });
});
