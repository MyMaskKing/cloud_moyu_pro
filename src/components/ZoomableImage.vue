<script setup lang="ts">
import { ref } from "vue";

defineProps<{ src: string; alt?: string; imgClass?: string }>();

const zoomed = ref(false);
function open() {
  zoomed.value = true;
}
function close() {
  zoomed.value = false;
}
</script>

<template>
  <img :src="src" :alt="alt" loading="lazy" :class="imgClass" @click="open" />
  <Teleport to="body">
    <div v-if="zoomed" class="zi-mask" @click.self="close">
      <img :src="src" :alt="alt" class="zi-big" @click="close" />
      <button class="zi-x" @click="close" title="关闭">✕</button>
    </div>
  </Teleport>
</template>

<style scoped>
img { cursor: zoom-in; }
.zi-mask {
  position: fixed;
  inset: 0;
  z-index: 30000;
  background: rgba(0, 0, 0, 0.82);
  display: flex;
  align-items: center;
  justify-content: center;
  backdrop-filter: blur(2px);
}
.zi-big {
  width: min(80vw, 520px);
  height: min(80vw, 520px);
  max-width: 92vw;
  max-height: 92vh;
  border-radius: 12px;
  background: #fff;
  cursor: zoom-out;
  object-fit: contain;
  box-shadow: 0 12px 48px rgba(0, 0, 0, 0.6);
}
.zi-x {
  position: absolute;
  top: 16px; right: 20px;
  background: rgba(255, 255, 255, 0.14);
  border: none;
  color: #fff;
  font-size: 18px;
  width: 36px;
  height: 36px;
  border-radius: 50%;
  cursor: pointer;
}
.zi-x:hover { background: rgba(255, 255, 255, 0.26); }
</style>
