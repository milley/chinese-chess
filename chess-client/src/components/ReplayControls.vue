<template>
  <div class="replay-controls">
    <div class="replay-nav">
      <button class="btn btn-secondary" :disabled="isFirstStep" @click="$emit('go-first')" title="第一步">⏮</button>
      <button class="btn btn-secondary" :disabled="isFirstStep" @click="$emit('go-prev')" title="上一步">◀</button>
      <button class="btn btn-primary" @click="$emit('toggle-auto-play')" :title="isAutoPlaying ? '暂停' : '播放'">
        {{ isAutoPlaying ? '⏸' : '▶' }}
      </button>
      <button class="btn btn-secondary" :disabled="isLastStep" @click="$emit('go-next')" title="下一步">▶</button>
      <button class="btn btn-secondary" :disabled="isLastStep" @click="$emit('go-last')" title="最后一步">⏭</button>
    </div>
    <div class="replay-slider">
      <input
        type="range"
        :min="0"
        :max="totalSteps"
        :value="currentStep"
        @input="$emit('update:current-step', Number(($event.target as HTMLInputElement).value))"
        class="step-slider"
      />
      <span class="step-counter">{{ currentStep }} / {{ totalSteps }}</span>
    </div>
    <div class="replay-speed">
      <label>速度:</label>
      <select :value="autoPlaySpeed" @change="$emit('update:auto-play-speed', Number(($event.target as HTMLSelectElement).value))">
        <option :value="2000">0.5x</option>
        <option :value="1000">1x</option>
        <option :value="500">2x</option>
        <option :value="250">4x</option>
      </select>
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  currentStep: number;
  totalSteps: number;
  isFirstStep: boolean;
  isLastStep: boolean;
  isAutoPlaying: boolean;
  autoPlaySpeed: number;
}>();

defineEmits<{
  (e: 'go-first'): void;
  (e: 'go-prev'): void;
  (e: 'go-next'): void;
  (e: 'go-last'): void;
  (e: 'toggle-auto-play'): void;
  (e: 'update:current-step', step: number): void;
  (e: 'update:auto-play-speed', ms: number): void;
}>();
</script>

<style scoped>
.replay-controls {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 8px 0;
}
.replay-nav {
  display: flex;
  gap: 4px;
  justify-content: center;
  align-items: center;
}
.replay-nav .btn {
  min-width: 36px;
  padding: 4px 8px;
  font-size: 14px;
}
.replay-slider {
  display: flex;
  align-items: center;
  gap: 8px;
}
.step-slider {
  flex: 1;
}
.step-counter {
  font-size: 12px;
  color: #666;
  white-space: nowrap;
  min-width: 60px;
  text-align: right;
}
.replay-speed {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: #666;
}
.replay-speed select {
  padding: 2px 4px;
  font-size: 12px;
}
</style>
