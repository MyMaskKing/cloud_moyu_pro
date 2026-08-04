<script setup lang="ts">
import { computed } from "vue";
import { useAuthStore } from "../stores/auth";
import type { VipFeature } from "../services/auth";

/**
 * VIP 门控组件:
 * - 有权限 → 渲染默认插槽(真实功能)
 * - 无权限 → 渲染锁定占位,点击按钮派 'unlock' 事件,由父组件打开登录/开通面板
 *
 * M4/M5 功能直接用 <VipLock feature="disguise|workMonitor"> 包裹即可,
 * 功能实现与鉴权解耦。
 */
const props = withDefaults(defineProps<{
  feature: VipFeature;
  /** 锁定态按钮文案 */
  buttonText?: string;
  /** 锁定态标题(默认按 feature 生成) */
  title?: string;
  /** 紧凑模式:用于设置项行内,不占整行卡片 */
  compact?: boolean;
}>(), {
  buttonText: "开通 VIP",
  compact: false,
});

const emit = defineEmits<{ (e: "unlock"): void }>();

const auth = useAuthStore();
const allowed = computed(() => auth.canUse(props.feature));
const reason = computed(() => auth.denyReason(props.feature));

const defaultTitle = computed(() => {
  if (props.feature === "disguise") return "伪装皮肤 (VIP)";
  if (props.feature === "workMonitor") return "工作监测自动隐藏 (VIP)";
  return "VIP 功能";
});
const lockHint = computed(() => {
  if (reason.value === "not-logged-in") return "登录并开通 VIP 后可用";
  return "开通 VIP 后可用";
});
</script>

<template>
  <slot v-if="allowed" />
  <div v-else class="vip-lock" :class="{ compact }">
    <span class="lock-icon">🔒</span>
    <div class="lock-text">
      <div class="lock-title">{{ title || defaultTitle }}</div>
      <div v-if="!compact" class="lock-hint">{{ lockHint }}</div>
    </div>
    <button class="unlock-btn" @click="emit('unlock')">{{ buttonText }}</button>
  </div>
</template>

<style scoped>
.vip-lock {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 14px;
  background: rgba(255, 255, 255, 0.02);
  border: 1px dashed rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  color: #8b949e;
}
.vip-lock.compact { padding: 8px 12px; }
.lock-icon { font-size: 16px; opacity: 0.8; }
.lock-text { flex: 1; min-width: 0; }
.lock-title { font-size: 12px; color: #c9d1d9; }
.lock-hint { font-size: 11px; color: #6e7681; margin-top: 2px; }
.unlock-btn {
  height: 26px;
  padding: 0 12px;
  border: none;
  border-radius: 6px;
  background: linear-gradient(135deg, #d4a017, #f0c040);
  color: #1c1c1c;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
}
.unlock-btn:hover { filter: brightness(1.08); }
</style>
