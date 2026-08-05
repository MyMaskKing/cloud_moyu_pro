import { defineStore } from "pinia";
import { ref, computed } from "vue";

export type DisguiseSkin = "vscode" | "excel" | "ppt";

/** 伪装模式:cover=遮蔽式(内容全盖住,防看最强) / reading=阅读式(内容仍可读,可继续摸鱼) */
export type DisguiseMode = "cover" | "reading";

const STORAGE_KEY = "moyu-disguise-v1";

interface Persisted {
  skin: DisguiseSkin;
  mode: DisguiseMode;
}

function loadPersisted(): Persisted {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return { skin: "vscode", mode: "cover", ...JSON.parse(raw) };
  } catch {
    /* ignore */
  }
  return { skin: "vscode", mode: "cover" };
}

/**
 * M4 伪装皮肤:在主壳上覆盖仿真工作界面。
 * 激活/退出由本 store 统一管理,组件通过 active 控制覆盖层显隐,
 * App.vue 负责在激活时隐藏所有 web-tab(避免独立子窗穿帮)。
 */
export const useDisguiseStore = defineStore("disguise", () => {
  const persisted = loadPersisted();
  const skin = ref<DisguiseSkin>(persisted.skin);
  const mode = ref<DisguiseMode>(persisted.mode);
  const active = ref(false);

  const isActive = computed(() => active.value);

  function persist() {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ skin: skin.value, mode: mode.value }));
  }

  function setSkin(s: DisguiseSkin) {
    skin.value = s;
    persist();
  }

  function setMode(m: DisguiseMode) {
    mode.value = m;
    persist();
  }

  function activate(s?: DisguiseSkin) {
    if (s) setSkin(s);
    active.value = true;
  }

  function deactivate() {
    active.value = false;
  }

  function toggle(s?: DisguiseSkin) {
    if (active.value) deactivate();
    else activate(s);
  }

  return { skin, mode, active, isActive, setSkin, setMode, activate, deactivate, toggle };
});
