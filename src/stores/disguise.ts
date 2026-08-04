import { defineStore } from "pinia";
import { ref, computed } from "vue";

export type DisguiseSkin = "vscode" | "excel" | "ppt";

const STORAGE_KEY = "moyu-disguise-v1";

interface Persisted {
  skin: DisguiseSkin;
}

function loadPersisted(): Persisted {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return { skin: "vscode", ...JSON.parse(raw) };
  } catch {
    /* ignore */
  }
  return { skin: "vscode" };
}

/**
 * M4 伪装皮肤:在主壳上覆盖仿真工作界面。
 * 激活/退出由本 store 统一管理,组件通过 active 控制覆盖层显隐,
 * App.vue 负责在激活时隐藏所有 web-tab(避免独立子窗穿帮)。
 */
export const useDisguiseStore = defineStore("disguise", () => {
  const persisted = loadPersisted();
  const skin = ref<DisguiseSkin>(persisted.skin);
  const active = ref(false);

  const isActive = computed(() => active.value);

  function setSkin(s: DisguiseSkin) {
    skin.value = s;
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ skin: s }));
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

  return { skin, active, isActive, setSkin, activate, deactivate, toggle };
});
