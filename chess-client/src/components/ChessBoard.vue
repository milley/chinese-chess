<template>
  <canvas
    ref="canvasRef"
    :width="canvasWidth * dpr"
    :height="canvasHeight * dpr"
    :style="{ width: canvasWidth + 'px', height: canvasHeight + 'px' }"
    @click="handleClick"
    @mousedown="handleMouseDown"
    @mousemove="handleMouseMove"
    @mouseup="handleMouseUp"
    @mouseleave="handleMouseUp"
    @touchstart.prevent="handleTouchStart"
    @touchmove.prevent="handleTouchMove"
    @touchend.prevent="handleTouchEnd"
  />
</template>

<script setup lang="ts">
import { ref, watch, onMounted, computed } from 'vue';
import { parseFen, findKing, parseUciPosition, getSideToMove, getDisplayPosition as getDisplayPositionUtil, pixelToPosition as pixelToPositionUtil, BOARD_COLS, BOARD_ROWS, CELL_SIZE, PADDING } from '../utils/chess';

const props = defineProps<{
  fen: string;
  playerColor: 'red' | 'black' | null;
  selectedSquare: string | null;
  validMoves: string[];
  isCheck?: boolean;
  lastMove?: { from: string; to: string } | null;
}>();

const emit = defineEmits<{
  (e: 'squareClick', position: string): void;
}>();

const PIECE_RADIUS = 25;

const canvasRef = ref<HTMLCanvasElement | null>(null);
const dpr = ref(window.devicePixelRatio || 1);

// Drag-and-drop state
const isDragging = ref(false);
const dragFrom = ref<string | null>(null);
const dragPiece = ref<{ type: string; color: 'red' | 'black' } | null>(null);
const dragX = ref(0);
const dragY = ref(0);

const canvasWidth = computed(() => (BOARD_COLS - 1) * CELL_SIZE + PADDING * 2);
const canvasHeight = computed(() => (BOARD_ROWS - 1) * CELL_SIZE + PADDING * 2);

const flip = computed(() => props.playerColor === 'black');

// FEN char to Chinese name (kept for drawing — the util version uses it internally)
const FEN_TO_NAME: Record<string, string> = {
  K: '帅', A: '仕', B: '相', N: '马', R: '车', C: '炮', P: '兵',
  k: '将', a: '士', b: '象', n: '马', r: '车', c: '炮', p: '卒',
};

function getDisplayPosition(col: number, row: number): { x: number; y: number } {
  return getDisplayPositionUtil(col, row, flip.value);
}

function draw() {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const ctx = canvas.getContext('2d');
  if (!ctx) return;

  ctx.save();
  ctx.scale(dpr.value, dpr.value);

  // Clear
  ctx.fillStyle = '#f0d9b5';
  ctx.fillRect(0, 0, canvasWidth.value, canvasHeight.value);

  // Board background
  ctx.fillStyle = '#e8c888';
  ctx.fillRect(PADDING - 10, PADDING - 10, (BOARD_COLS - 1) * CELL_SIZE + 20, (BOARD_ROWS - 1) * CELL_SIZE + 20);

  // Grid lines
  ctx.strokeStyle = '#4a3520';
  ctx.lineWidth = 1;

  // Horizontal lines
  for (let row = 0; row < BOARD_ROWS; row++) {
    const y = PADDING + row * CELL_SIZE;
    ctx.beginPath();
    ctx.moveTo(PADDING, y);
    ctx.lineTo(PADDING + (BOARD_COLS - 1) * CELL_SIZE, y);
    ctx.stroke();
  }

  // Vertical lines - full for border columns, split at river for inner columns
  for (let col = 0; col < BOARD_COLS; col++) {
    const x = PADDING + col * CELL_SIZE;
    if (col === 0 || col === 8) {
      // Border columns - full height
      ctx.beginPath();
      ctx.moveTo(x, PADDING);
      ctx.lineTo(x, PADDING + (BOARD_ROWS - 1) * CELL_SIZE);
      ctx.stroke();
    } else {
      // Inner columns - split at river (between row 4 and row 5)
      ctx.beginPath();
      ctx.moveTo(x, PADDING);
      ctx.lineTo(x, PADDING + 4 * CELL_SIZE);
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(x, PADDING + 5 * CELL_SIZE);
      ctx.lineTo(x, PADDING + 9 * CELL_SIZE);
      ctx.stroke();
    }
  }

  // Palace diagonals
  ctx.lineWidth = 1;
  // Black palace (row 0-2, col 3-5)
  drawPalaceDiagonals(ctx, 3, 0, 5, 2);
  // Red palace (row 7-9, col 3-5)
  drawPalaceDiagonals(ctx, 3, 7, 5, 9);

  // River text
  ctx.fillStyle = '#4a3520';
  ctx.font = '20px serif';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  const riverY = PADDING + 4.5 * CELL_SIZE;
  if (flip.value) {
    ctx.fillText('汉界', PADDING + 2 * CELL_SIZE, riverY);
    ctx.fillText('楚河', PADDING + 6 * CELL_SIZE, riverY);
  } else {
    ctx.fillText('楚河', PADDING + 2 * CELL_SIZE, riverY);
    ctx.fillText('汉界', PADDING + 6 * CELL_SIZE, riverY);
  }

  // Last-move highlight (yellow background on from/to squares)
  if (props.lastMove) {
    for (const uci of [props.lastMove.from, props.lastMove.to]) {
      const pos = parseUciPosition(uci);
      if (pos) {
        const { x, y } = getDisplayPosition(pos.col, pos.row);
        ctx.fillStyle = 'rgba(255, 255, 0, 0.35)';
        ctx.beginPath();
        ctx.arc(x, y, PIECE_RADIUS + 4, 0, Math.PI * 2);
        ctx.fill();
      }
    }
  }

  // Determine which king is in check for the check highlight
  const checkedKingPos = props.isCheck ? findKing(props.fen, getSideToMove(props.fen)) : null;

  // Pieces
  const pieces = parseFen(props.fen);
  for (const [key, piece] of pieces) {
    const [col, row] = key.split(',').map(Number);

    // Skip drawing the dragged piece at its original position (it follows the cursor)
    if (isDragging.value && dragFrom.value === String.fromCharCode('a'.charCodeAt(0) + col) + row) {
      continue;
    }

    const { x, y } = getDisplayPosition(col, row);

    // Check highlight — red glow behind the checked king
    if (checkedKingPos && col === checkedKingPos.col && row === checkedKingPos.row) {
      ctx.save();
      ctx.shadowColor = '#ff0000';
      ctx.shadowBlur = 15;
      ctx.fillStyle = 'rgba(255, 0, 0, 0.3)';
      ctx.beginPath();
      ctx.arc(x, y, PIECE_RADIUS + 5, 0, Math.PI * 2);
      ctx.fill();
      ctx.restore();
    }

    drawPiece(ctx, x, y, piece.type, piece.color);
  }

  // Draw the dragged piece at cursor position (on top of everything)
  if (isDragging.value && dragPiece.value) {
    drawPiece(ctx, dragX.value, dragY.value, dragPiece.value.type, dragPiece.value.color);
  }

  // Selection highlight
  if (props.selectedSquare) {
    const pos = parseUciPosition(props.selectedSquare);
    if (pos) {
      const { x, y } = getDisplayPosition(pos.col, pos.row);
      ctx.strokeStyle = '#ff6600';
      ctx.lineWidth = 3;
      ctx.beginPath();
      ctx.arc(x, y, PIECE_RADIUS + 2, 0, Math.PI * 2);
      ctx.stroke();
    }
  }

  // Valid move indicators
  for (const move of props.validMoves) {
    const pos = parseUciPosition(move);
    if (pos) {
      const { x, y } = getDisplayPosition(pos.col, pos.row);
      const hasPiece = pieces.has(`${pos.col},${pos.row}`);
      if (hasPiece) {
        // Capture indicator - ring
        ctx.strokeStyle = 'rgba(255, 0, 0, 0.5)';
        ctx.lineWidth = 3;
        ctx.beginPath();
        ctx.arc(x, y, PIECE_RADIUS + 2, 0, Math.PI * 2);
        ctx.stroke();
      } else {
        // Move indicator - small dot
        ctx.fillStyle = 'rgba(0, 150, 0, 0.5)';
        ctx.beginPath();
        ctx.arc(x, y, 8, 0, Math.PI * 2);
        ctx.fill();
      }
    }
  }

  ctx.restore();
}

/// Get the side to move from FEN ('w' = red, 'b' = black).
function drawPalaceDiagonals(ctx: CanvasRenderingContext2D, c1: number, r1: number, c2: number, r2: number) {
  const p1 = getDisplayPosition(c1, r1);
  const p2 = getDisplayPosition(c2, r2);
  const p3 = getDisplayPosition(c2, r1);
  const p4 = getDisplayPosition(c1, r2);
  ctx.beginPath();
  ctx.moveTo(p1.x, p1.y);
  ctx.lineTo(p2.x, p2.y);
  ctx.stroke();
  ctx.beginPath();
  ctx.moveTo(p3.x, p3.y);
  ctx.lineTo(p4.x, p4.y);
  ctx.stroke();
}

function drawPiece(ctx: CanvasRenderingContext2D, x: number, y: number, name: string, color: 'red' | 'black') {
  // Background circle
  const gradient = ctx.createRadialGradient(x - 4, y - 4, 2, x, y, PIECE_RADIUS);
  gradient.addColorStop(0, '#fff8e8');
  gradient.addColorStop(1, '#d4b87a');
  ctx.fillStyle = gradient;
  ctx.beginPath();
  ctx.arc(x, y, PIECE_RADIUS, 0, Math.PI * 2);
  ctx.fill();

  // Border
  ctx.strokeStyle = color === 'red' ? '#c0392b' : '#2c3e50';
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.arc(x, y, PIECE_RADIUS, 0, Math.PI * 2);
  ctx.stroke();

  // Inner ring
  ctx.strokeStyle = color === 'red' ? '#c0392b' : '#2c3e50';
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.arc(x, y, PIECE_RADIUS - 4, 0, Math.PI * 2);
  ctx.stroke();

  // Text
  ctx.fillStyle = color === 'red' ? '#c0392b' : '#2c3e50';
  ctx.font = 'bold 22px "Noto Sans SC", "Microsoft YaHei", "SimHei", serif';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(name, x, y + 1);
}

function pixelToPosition(clientX: number, clientY: number): string | null {
  const canvas = canvasRef.value;
  if (!canvas) return null;

  const rect = canvas.getBoundingClientRect();
  const x = clientX - rect.left;
  const y = clientY - rect.top;

  return pixelToPositionUtil(x, y, flip.value);
}

function handleClick(event: MouseEvent) {
  // If a drag just completed, don't fire click
  if (isDragging.value) return;
  const position = pixelToPosition(event.clientX, event.clientY);
  if (position) emit('squareClick', position);
}

// --- Drag-and-drop handlers ---

function handleMouseDown(event: MouseEvent) {
  const position = pixelToPosition(event.clientX, event.clientY);
  if (!position) return;

  // Check if there's a piece at this position
  const pieces = parseFen(props.fen);
  const pos = parseUciPosition(position);
  if (!pos) return;

  const piece = pieces.get(`${pos.col},${pos.row}`);
  if (!piece) return;

  // Start drag
  isDragging.value = true;
  dragFrom.value = position;
  dragPiece.value = piece;

  const canvas = canvasRef.value;
  if (canvas) {
    const rect = canvas.getBoundingClientRect();
    dragX.value = event.clientX - rect.left;
    dragY.value = event.clientY - rect.top;
  }

  // Also emit squareClick to select the piece (shows valid moves)
  emit('squareClick', position);
}

function handleMouseMove(event: MouseEvent) {
  if (!isDragging.value) return;

  const canvas = canvasRef.value;
  if (canvas) {
    const rect = canvas.getBoundingClientRect();
    dragX.value = event.clientX - rect.left;
    dragY.value = event.clientY - rect.top;
  }

  draw();
}

function handleMouseUp(event: MouseEvent) {
  if (!isDragging.value) return;

  const position = pixelToPosition(event.clientX, event.clientY);

  // If dropped on a valid move target, emit click to execute the move
  if (position && position !== dragFrom.value) {
    emit('squareClick', position);
  }

  // End drag
  isDragging.value = false;
  dragFrom.value = null;
  dragPiece.value = null;

  draw();
}

/// Touch event handlers for mobile drag support.
function handleTouchStart(event: TouchEvent) {
  if (event.touches.length === 0) return;
  const touch = event.touches[0];
  const position = pixelToPosition(touch.clientX, touch.clientY);
  if (!position) return;

  // Check if there's a piece at this position
  const pieces = parseFen(props.fen);
  const pos = parseUciPosition(position);
  if (!pos) return;

  const piece = pieces.get(`${pos.col},${pos.row}`);
  if (!piece) {
    // No piece — just emit click (for valid move targets)
    emit('squareClick', position);
    return;
  }

  // Start drag
  isDragging.value = true;
  dragFrom.value = position;
  dragPiece.value = piece;

  const canvas = canvasRef.value;
  if (canvas) {
    const rect = canvas.getBoundingClientRect();
    dragX.value = touch.clientX - rect.left;
    dragY.value = touch.clientY - rect.top;
  }

  // Also emit click to select the piece
  emit('squareClick', position);
}

function handleTouchMove(event: TouchEvent) {
  if (!isDragging.value || event.touches.length === 0) return;

  const touch = event.touches[0];
  const canvas = canvasRef.value;
  if (canvas) {
    const rect = canvas.getBoundingClientRect();
    dragX.value = touch.clientX - rect.left;
    dragY.value = touch.clientY - rect.top;
  }

  draw();
}

function handleTouchEnd(event: TouchEvent) {
  if (!isDragging.value) return;

  // Use the last known drag position to determine drop target
  const canvas = canvasRef.value;
  if (canvas) {
    const position = pixelToPositionUtil(dragX.value, dragY.value, flip.value);
    if (position && position !== dragFrom.value) {
      emit('squareClick', position);
    }
  }

  // End drag
  isDragging.value = false;
  dragFrom.value = null;
  dragPiece.value = null;

  draw();
}

// Watch for changes and redraw
watch(() => [props.fen, props.selectedSquare, props.validMoves, props.isCheck, props.lastMove], draw, { deep: true });

onMounted(() => {
  draw();
});
</script>
