import { describe, it, expect } from 'vitest';
import { parseFen, findKing, parseUciPosition, getSideToMove } from '../../utils/chess';

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
