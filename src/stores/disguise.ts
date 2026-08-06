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
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<{ skin: unknown; mode: unknown }>;
      // 旧版本带 mode(cover/reading):仅采纳合法 skin,忽略已下线的 mode 字段
      if (parsed.skin === "vscode" || parsed.skin === "excel" || parsed.skin === "ppt") {
        return { skin: parsed.skin };
      }
    }
  } catch {
    /* ignore */
  }
  return { skin: "vscode" };
}

/**
 * M4 伪装皮肤:在主壳上覆盖仿真工作界面。
 * 行为按 skin 自动推导,不再有独立的模式开关:
 *   - vscode → 换壳式(chrome):主壳换成 VSCode 外壳包住真实网页,可继续浏览
 *   - excel / ppt → 遮蔽式(cover):全屏静态假界面盖住内容
 * 激活/退出由本 store 统一管理,组件通过 active 控制外壳/覆盖层显隐,
 * App.vue 负责 web-tab 的隐藏与样式注入联动。
 */
export const useDisguiseStore = defineStore("disguise", () => {
  const persisted = loadPersisted();
  const skin = ref<DisguiseSkin>(persisted.skin);
  const active = ref(false);

  const isActive = computed(() => active.value);
  /** vscode 走换壳沉浸式;excel/ppt 走全屏遮蔽 */
  const isChrome = computed(() => active.value && skin.value === "vscode");

  function persist() {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ skin: skin.value }));
  }

  function setSkin(s: DisguiseSkin) {
    skin.value = s;
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

  return { skin, active, isActive, isChrome, setSkin, activate, deactivate, toggle };
});
