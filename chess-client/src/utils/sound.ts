/**
 * Sound effects for chess moves using Web Audio API.
 * Generates tones programmatically — no audio files needed.
 * AudioContext is created lazily on first play to comply with
 * browser autoplay policies (user gesture required).
 */

let audioCtx: AudioContext | null = null;

function getAudioContext(): AudioContext {
  if (!audioCtx) {
    audioCtx = new AudioContext();
  }
  // Resume if suspended (browser autoplay policy)
  if (audioCtx.state === 'suspended') {
    audioCtx.resume();
  }
  return audioCtx;
}

function playTone(
  frequency: number,
  duration: number,
  type: OscillatorType = 'sine',
  volume: number = 0.3,
) {
  try {
    const ctx = getAudioContext();
    const oscillator = ctx.createOscillator();
    const gainNode = ctx.createGain();
    oscillator.type = type;
    oscillator.frequency.value = frequency;
    gainNode.gain.value = volume;
    gainNode.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + duration);
    oscillator.connect(gainNode);
    gainNode.connect(ctx.destination);
    oscillator.start(ctx.currentTime);
    oscillator.stop(ctx.currentTime + duration);
  } catch {
    // Silently fail — audio is non-essential
  }
}

/** Short click for a normal move */
export function playMove() {
  playTone(800, 0.08, 'sine', 0.2);
}

/** Lower, sharper thud for a capture */
export function playCapture() {
  playTone(400, 0.15, 'square', 0.2);
}

/** Double-beep alert for check */
export function playCheck() {
  playTone(1200, 0.1, 'sine', 0.25);
  setTimeout(() => playTone(1600, 0.1, 'sine', 0.25), 120);
}

/** Descending tone for game over (timeout/resign/checkmate) */
export function playGameOver() {
  playTone(600, 0.3, 'triangle', 0.25);
  setTimeout(() => playTone(400, 0.5, 'triangle', 0.15), 350);
}
