<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, nextTick, watch, computed } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { emit as emitEvent } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useMouseAutoHide } from "./composables/useMouseAutoHide";
import { useTabsStore } from "./stores/tabs";
import type { PipRatio } from "./stores/tabs";
import { useSitesStore } from "./stores/sites";
import SettingsView from "./views/SettingsView.vue";
import TabBar from "./components/TabBar.vue";
import { useAuthStore } from "./stores/auth";
import { useDisguiseStore } from "./stores/disguise";
import { usePetStore, PET_BUBBLE_H } from "./stores/pet";
import { useUserScriptsStore } from "./stores/userscripts";
import { fetchRemoteConfig, DASHANG_IMAGES, LOCAL_FORCE_UPDATE_VERSION } from "./config/api";
import { openUrl } from "@tauri-apps/plugin-opener";
import DisguiseOverlay from "./components/DisguiseOverlay.vue";
import DisguiseChrome from "./components/DisguiseChrome.vue";
import ZoomableImage from "./components/ZoomableImage.vue";

// ────── 窗口 / 全局设置 ──────
const win = getCurrentWindow();
const opacity = ref(1);
const showSettings = ref(false);
const isHidden = ref(false);
const tabs = useTabsStore();
const sitesStore = useSitesStore();
const auth = useAuthStore();
const disguise = useDisguiseStore();
const pet = usePetStore();
const userScripts = useUserScriptsStore();

// ────── 启动打赏弹窗(由远端配置 popDashang 控制) ──────
const DASHANG_HIDE_KEY = "moyu-dashang-hidden";
const dashangVisible = ref(false);
const dashangNeverRef = ref<HTMLInputElement | null>(null);
const dashangImages = DASHANG_IMAGES;
function dashangHiddenBefore(): boolean {
  return localStorage.getItem(DASHANG_HIDE_KEY) === "1";
}
function closeDashang(neverAgain: boolean) {
  if (neverAgain) localStorage.setItem(DASHANG_HIDE_KEY, "1");
  dashangVisible.value = false;
}

// ────── 更新弹窗(远端 lastUpdate.version 与本地不一致时,每次启动都弹) ──────
const forceUpdateVisible = ref(false);
// forcedUpdate=true = 强制:弹窗不可关闭,只能"立即更新";false = 可关,但下次启动仍弹
const forceUpdateClosable = ref(false);
const forceUpdateUrl = ref("");
const forceUpdateLines = ref<string[]>([]);
function openForceUpdate() {
  if (forceUpdateUrl.value) {
    openUrl(forceUpdateUrl.value).catch(() => {});
  }
}
function closeForceUpdate() {
  if (forceUpdateClosable.value) forceUpdateVisible.value = false;
}

// 更新窗优先显示;若同时需要弹打赏,更新窗关闭后再弹(强制更新窗不可关,打赏就不弹)
const dashangPending = ref(false);

// 启动弹窗(打赏/强制更新)显示时,子窗是独立 OS 窗口会盖在主窗弹窗之上,
// 必须一并隐藏;弹窗关闭后再把当前活跃 tab 恢复出来。
watch([dashangVisible, forceUpdateVisible], ([ds, fu], [prevDs, prevFu]) => {
  const becameVisible = (!prevDs && ds) || (!prevFu && fu);
  const becameHidden = (prevDs && !ds) || (prevFu && !fu);
  if (becameVisible) {
    invoke("set_all_tabs_hidden", { hidden: true }).catch(() => {});
  } else if (becameHidden) {
    // 更新窗关闭后若还需弹打赏,接着弹(保持子窗隐藏,不要先恢复再藏导致闪烁)
    if (dashangPending.value) {
      dashangPending.value = false;
      if (!dashangHiddenBefore()) {
        dashangVisible.value = true;
        return;
      }
    }
    if (!dashangVisible.value && !forceUpdateVisible.value) {
      const t = tabs.active;
      if (!t) return;
      if (t.mode === "inline" || t.mode === "fullscreen") {
        activateVisual(t.id);
      } else {
        invoke("set_web_tab_visible", { id: t.id, visible: true }).catch(() => {});
      }
    }
  }
});

const STORAGE_KEY = "moyu-settings-v1";
/** M5 检测到工作应用后执行的动作 */
type WorkAction = "disguise" | "hide";
/** M8 老板键触发时的动作 */
type BossAction = "hide" | "pet";
/** popout 独立窗口自绘标题栏主题 */
type PopoutTheme = "dark" | "light" | "system";
interface Settings {
  autoHide: boolean;
  autoHideDelay: number;
  videoAutoLandscape: boolean;
  bossKey: string;
  transparencyKey: string; // 空字符串 = 未设置
  /** M5:是否启用工作进程监测 */
  workMonitor: boolean;
  /** M5:命中后动作 —— 切伪装皮肤 / 直接隐藏 */
  workMonitorAction: WorkAction;
  /** M5:进程关键字(大小写不敏感子串匹配) */
  workMonitorKeywords: string[];
  /** M8:老板键动作 —— 直接隐藏 / 变成桌面宠物 */
  bossAction: BossAction;
  /** popout 独立窗口自绘标题栏主题:深色 / 浅色 / 跟随系统 */
  popoutTheme: PopoutTheme;
}
const DEFAULT_WORK_KEYWORDS = [
  "dingtalk", "钉钉",
  "wemeet", "txmeeting", "腾讯会议",
  "zoom",
  "feishu", "lark", "飞书",
  "teams",
  "wework", "企业微信",
];
const defaults: Settings = {
  autoHide: false,
  autoHideDelay: 1500,
  videoAutoLandscape: true,
  bossKey: "Ctrl+Alt+KeyQ",
  transparencyKey: "",
  workMonitor: false,
  workMonitorAction: "disguise",
  workMonitorKeywords: [...DEFAULT_WORK_KEYWORDS],
  bossAction: "hide",
  popoutTheme: "system",
};
const loaded: Settings = (() => {
  try {
    return { ...defaults, ...JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}") };
  } catch {
    return { ...defaults };
  }
})();
const autoHide = ref(loaded.autoHide);
const autoHideDelay = ref(loaded.autoHideDelay);
const videoAutoLandscape = ref(loaded.videoAutoLandscape);
const bossKey = ref(loaded.bossKey);
const transparencyKey = ref(loaded.transparencyKey);
const workMonitor = ref(loaded.workMonitor);
const workMonitorAction = ref<WorkAction>(loaded.workMonitorAction);
const workMonitorKeywords = ref<string[]>(loaded.workMonitorKeywords);
const bossAction = ref<BossAction>(loaded.bossAction);
const popoutTheme = ref<PopoutTheme>(loaded.popoutTheme);
/** 摸鱼主窗口是否置顶 */
const mainPinned = ref(false);
/** 透明度临时绕过:快捷键翻转;true = 强制不透明(方便操作),不改动 opacity 存的值 */
const bypassOpacity = ref(false);
/** 实际写到 CSS/子窗口的透明度值:bypass 时永远为 1;否则用 opacity */
const effectiveOpacity = computed(() => (bypassOpacity.value ? 1 : opacity.value));

/** 把 Tauri Shortcut 内部格式(Ctrl+Alt+KeyQ / Digit1 / F5 / Space)转成显示用 (Ctrl+Alt+Q / 1 / F5 / Space) */
function humanBossKey(sc: string): string {
  return sc.split("+").map((part) => {
    if (part.startsWith("Key") && part.length === 4) return part.slice(3);
    if (part.startsWith("Digit") && part.length === 6) return part.slice(5);
    return part;
  }).join("+");
}
const bossKeyLabel = computed(() => humanBossKey(bossKey.value));
const popoutTitle = (t: { title?: string; url: string }) =>
  `🐟 ${t.title || t.url}   ·   老板键 ${bossKeyLabel.value}`;

watch(
  [autoHide, autoHideDelay, videoAutoLandscape, bossKey, transparencyKey,
   workMonitor, workMonitorAction, workMonitorKeywords, bossAction, popoutTheme],
  () => {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        autoHide: autoHide.value,
        autoHideDelay: autoHideDelay.value,
        videoAutoLandscape: videoAutoLandscape.value,
        bossKey: bossKey.value,
        transparencyKey: transparencyKey.value,
        workMonitor: workMonitor.value,
        workMonitorAction: workMonitorAction.value,
        workMonitorKeywords: workMonitorKeywords.value,
        bossAction: bossAction.value,
        popoutTheme: popoutTheme.value,
      }),
    );
  },
  { deep: true },
);

// 用户脚本:把启用脚本全集下发到 Rust。脚本在子窗建窗时通过 initialization_script
// (WebView2 AddScriptToExecuteOnDocumentCreated)在真正的 document-start 注入——
// 这是让视频破解/播放器 hook 这类 @run-at document-start 脚本在页面脚本之前拿到执行权
// 的唯一可靠方式;运行时再 eval 最早也晚于页面脚本,对这类脚本无效。
// initialization_script 在建窗时固化、建窗后无法改,所以增删改/启停后,已打开的标签
// 必须"销毁子窗再按同一 URL/几何重建"才能用上新脚本(重建等价于一次彻底刷新,会丢失
// 该标签的滚动/SPA/表单状态,与刷新行为一致)。冷启动 onMounted 的首次同步不重建
// (标签即将首次建窗,建窗时即拿到最新脚本)。popout/pip 是独立浮窗,重建需另算 chrome
// 几何,暂不热更——它们下次被打开会自然用新脚本。
let isFirstUserScriptsSync = true;
async function syncUserScripts() {
  try {
    await invoke("set_user_scripts", {
      scripts: userScripts.scripts.map((s) => ({
        id: s.id,
        name: s.name,
        code: s.code,
        matches: s.matches,
        includes: s.includes,
        excludes: s.excludes,
        requires: s.requires,
        runAt: s.runAt,
        enabled: s.enabled,
      })),
    });
  } catch (e) {
    console.error("set_user_scripts failed", e);
    return;
  }
  if (isFirstUserScriptsSync) {
    isFirstUserScriptsSync = false;
    return;
  }
  // 收集已建窗的 inline/fullscreen 标签,按当前 holder 几何逐个销毁+重建
  if (!holder.value) return;
  // 遮罩层(设置/打赏/更新弹窗)开着时,重建的子窗必须建为隐藏,否则独立 OS 子窗
  // 会闪一下盖在遮罩层之上(白屏穿帮)。隐藏子窗仍会加载页面并在 document-start 注入脚本。
  const isMasked = showSettings.value || dashangVisible.value || forceUpdateVisible.value;
  const rect = holder.value.getBoundingClientRect();
  const scale = window.devicePixelRatio || 1;
  const outer = await win.outerPosition();
  const inner = await win.innerPosition();
  const chromeOffsetX = (outer.x - inner.x) / scale;
  const chromeOffsetY = (outer.y - inner.y) / scale;
  const x = inner.x / scale + rect.left + chromeOffsetX;
  const y = inner.y / scale + rect.top + chromeOffsetY;
  const width = rect.width;
  const height = rect.height;
  for (const t of tabs.tabs) {
    if (t.mode !== "inline" && t.mode !== "fullscreen") continue;
    const exists = await invoke<boolean>("web_tab_exists", { id: t.id }).catch(() => false);
    if (!exists) continue;
    await invoke("close_web_tab", { id: t.id }).catch(() => {});
    await invoke("open_web_tab", {
      id: t.id, url: t.url, x, y, width, height, show: !isMasked,
    }).catch(() => {});
    await invoke("set_web_tab_opacity", { id: t.id, opacity: effectiveOpacity.value }).catch(() => {});
    if (isMasked) {
      // 遮罩层期间:所有重建的子窗一律保持隐藏,不触发 activateVisual 的 show。
      await invoke("set_web_tab_visible", { id: t.id, visible: false }).catch(() => {});
    } else if (t.id !== tabs.activeId) {
      // open_web_tab 会 show()。非活跃标签立即隐藏,避免重建瞬间在屏幕上叠闪;
      // 最后由 activateVisual 统一把当前活跃标签摆正并显示。
      await invoke("set_web_tab_visible", { id: t.id, visible: false }).catch(() => {});
    }
  }
  // 遮罩层开着时不做任何 show;等遮罩关闭时 watch(showSettings) 会按活跃 tab 恢复。
  if (!isMasked && tabs.activeId) await activateVisual(tabs.activeId);
}
watch(() => userScripts.scripts, syncUserScripts, { deep: true });

// 标题栏主题改动 → 立刻推给所有已打开的 popout/pip 子窗,无需重开
watch(popoutTheme, () => {
  for (const t of tabs.tabs) {
    if (t.mode === "popout" || t.mode === "pip") notifyTabMode(t.id);
  }
});

// bossKey 改动 → 刷新所有 pip/popout 的窗口标题(标题里显示当前老板键)
watch(bossKey, () => {
  for (const t of tabs.tabs) {
    if (t.mode === "pip") {
      // pip 也用自绘标题栏:无系统边框
      invoke("set_web_tab_chrome", {
        id: t.id,
        decorations: false,
        skipTaskbar: false,
        alwaysOnTop: true,
        title: popoutTitle(t),
      }).catch(() => {});
    } else if (t.mode === "popout") {
      // popout 用自绘标题栏:无系统边框(标题文字不可见,仅维持接口一致)
      invoke("set_web_tab_chrome", {
        id: t.id,
        decorations: false,
        skipTaskbar: false,
        alwaysOnTop: !!t.pinned,
        title: popoutTitle(t),
      }).catch(() => {});
    }
  }
});

useMouseAutoHide(autoHide, autoHideDelay, bypassOpacity);

async function minimize() { await win.minimize(); }
async function toggleMaximize() { await win.toggleMaximize(); }
async function close() { await win.close(); }

/** 切换主窗置顶 */
async function toggleMainPin() {
  const next = !mainPinned.value;
  await invoke("set_main_window_pinned", { pinned: next }).catch(() => {});
  mainPinned.value = next;
}

/** 切换 popout tab 置顶(pip 天然置顶,不处理;inline/fullscreen 无意义) */
async function togglePopoutPin(id: string) {
  const t = tabs.tabs.find((x) => x.id === id);
  if (!t || t.mode !== "popout") return;
  const next = !t.pinned;
  await invoke("set_web_tab_pinned", { id, pinned: next }).catch(() => {});
  tabs.setPinned(id, next);
  // set_always_on_top 会重算 WS_EX_STYLE 冲掉 WS_EX_LAYERED,补 apply 透明度
  await invoke("set_web_tab_opacity", { id, opacity: effectiveOpacity.value }).catch(() => {});
  await notifyTabMode(id);
}

/** 把 tab 当前的 mode + pinned + 标题栏主题推送给子 webview(用于自绘标题栏显隐/配色) */
async function notifyTabMode(id: string) {
  const t = tabs.tabs.find((x) => x.id === id);
  if (!t) return;
  await invoke("notify_web_tab_mode", {
    id,
    mode: t.mode,
    pinned: !!t.pinned,
    theme: popoutTheme.value,
  }).catch(() => {});
}

// 无边框窗口边缘 8 方向 resize 触发器
type ResizeDir = "North" | "South" | "East" | "West" | "NorthEast" | "NorthWest" | "SouthEast" | "SouthWest";
async function startResize(dir: ResizeDir, e: MouseEvent) {
  if (e.button !== 0) return;
  e.preventDefault();
  await win.startResizeDragging(dir);
}

function onOpacityInput(e: Event) {
  const v = Number((e.target as HTMLInputElement).value);
  opacity.value = v;
  // --web-tab-opacity 始终存"用户设定原始值"(useMouseAutoHide 恢复时会读它);
  // --shell-alpha 和子窗口透明度由下方 watch(effectiveOpacity) 集中 apply,受 bypass 影响
  document.body.style.setProperty("--web-tab-opacity", String(v));
  // 用户主动拖滑块视为"我要调透明度",退出快捷键的 bypass 临时态,让滑块立即生效
  if (bypassOpacity.value) bypassOpacity.value = false;
}

// 集中 apply 实际透明度:bypass 打开时立刻回到 1,不改滑块存的值
watch(effectiveOpacity, (v) => {
  document.body.style.setProperty("--shell-alpha", String(v));
  invoke("set_all_web_tabs_opacity", { opacity: v }).catch(() => {});
});
async function triggerBossKey() { await invoke("trigger_boss_key"); }

// ────── M4: 伪装皮肤 ──────
// 行为按 skin 推导:
//   vscode → 换壳式(chrome):主壳切成 DisguiseChrome,真实网页留在编辑区可继续浏览,
//            靠 set_reading_disguise 把网页染成暗色等宽观感;不关 tab、不 hide 子窗。
//   excel / ppt → 遮蔽式(cover):DisguiseOverlay 全屏盖住,必须隐藏所有 web-tab 子窗
//            (独立 OS 窗口不受 DOM 覆盖层遮挡,不隐藏会穿帮),并临时恢复不透明。
const disguiseAvailable = computed(() => auth.canUse("disguise"));
/** vscode 换壳伪装激活:主壳切成 DisguiseChrome 包住真实网页 */
const chromeActive = computed(
  () => disguiseAvailable.value && disguise.active && disguise.skin === "vscode",
);

async function enterDisguise() {
  if (!disguiseAvailable.value) return;
  const isChrome = disguise.skin === "vscode";
  // 换壳式只对 inline 子窗成立(子窗要对齐到编辑区);非 inline 先收回主窗
  if (isChrome && tabs.active && tabs.active.mode !== "inline") {
    await restoreInline(tabs.active.id);
  }
  disguise.activate();
  // 遮蔽式要把 tab 全收起,与进桌宠同理:先置 collapsing 再关设置页,
  // 否则关设置页的恢复链会和 set_all_tabs_hidden 并发,把 tab 亮回来。
  const willHideTabs = !isChrome;
  if (willHideTabs) collapsing.value = true;
  showSettings.value = false;
  try {
    if (isChrome) {
      await invoke("set_reading_disguise", { enabled: true }).catch(() => {});
    } else {
      await invoke("set_all_tabs_hidden", { hidden: true }).catch(() => {});
    }
    document.body.style.setProperty("--shell-alpha", "1");
  } finally {
    if (willHideTabs) collapsing.value = false;
  }
}

async function exitDisguise() {
  const wasChrome = disguise.skin === "vscode";
  disguise.deactivate();
  // 恢复用户设定的透明度
  document.body.style.setProperty("--shell-alpha", String(effectiveOpacity.value));
  if (wasChrome) {
    await invoke("set_reading_disguise", { enabled: false }).catch(() => {});
    // 模板切回普通布局后 holder 重建,ResizeObserver watch 会自动重新 sync;
    // 这里再主动 sync 一次,防止布局尚未稳定时子窗停在旧位置
    await nextTick();
    syncCurrentTab();
    return;
  }
  const cur = tabs.active;
  if (cur && cur.mode === "inline") {
    await activateVisual(cur.id);
  } else if (cur && (cur.mode === "pip" || cur.mode === "popout")) {
    await invoke("set_web_tab_visible", { id: cur.id, visible: true }).catch(() => {});
  }
}

function toggleDisguise() {
  if (disguise.active) exitDisguise();
  else enterDisguise();
}

// ────── M8: 桌面宠物 ──────
// 老板键动作可选"直接隐藏"(默认)或"变成桌面宠物"。
// 桌宠配置由这里预先下发给 Rust(set_pet_config),老板键按下时 Rust 直接开窗,
// 不依赖前端往返 —— 前端没响应也不能让老板键失效。
const petAvailable = computed(() => auth.canUse("pet"));

/** 桌宠窗口尺寸:高度要为气泡多留一段,否则气泡会被窗口边界裁掉 */
function petWindowSize() {
  return { width: pet.size, height: pet.size + PET_BUBBLE_H };
}

/** 把 pet store 配置拼成桌宠窗口的 URL query */
function buildPetQuery(): string {
  const q = new URLSearchParams();
  q.set("kind", pet.kind);
  q.set("size", String(pet.size));
  q.set("follow", pet.followMouse ? "1" : "0");
  q.set("wander", pet.wander ? "1" : "0");
  q.set("top", pet.alwaysOnTop ? "1" : "0");
  // URLSearchParams.set 自己会做编码,这里不能再套 encodeURIComponent(会双重编码)
  q.set("lines", JSON.stringify(pet.lines));
  return q.toString();
}

async function enterPetMode() {
  if (!petAvailable.value) return;
  // 必须在关设置页之前置位:关设置页的 watcher 会去恢复 tab 显示,
  // 那条链与 Rust 的批量 hide() 并发,不让路就会把 tab 亮回来。
  collapsing.value = true;
  showSettings.value = false;
  try {
    const { width, height } = petWindowSize();
    await invoke("enter_pet_mode", { width, height, query: buildPetQuery() });
    pet.setActive(true);
    isHidden.value = true;
    // 建窗时一律 always_on_top(true),这里按用户配置校正
    if (!pet.alwaysOnTop) {
      await invoke("set_pet_always_on_top", { on: false }).catch(() => {});
    }
    // Rust 已 hide 全部 tab,但设置页 watcher 那条链可能已抢先 show 过某个 tab。
    // 这里以"桌宠已开"为准兜一遍,保证屏幕上只剩宠物。
    for (const t of tabs.tabs) {
      if (t.mode !== "pip") {
        await invoke("set_web_tab_visible", { id: t.id, visible: false }).catch(() => {});
      }
    }
  } catch (e) {
    // 开窗失败不能让用户"既没隐藏也没宠物"地暴露在屏幕上:
    // 退化为直接隐藏,保证老板键始终有效。
    console.error("[moyu] enter_pet_mode failed:", e);
    const hidden = await invoke<boolean>("is_hidden").catch(() => false);
    if (!hidden) await invoke("trigger_boss_key").catch(() => {});
  } finally {
    collapsing.value = false;
  }
}

// 老板键动作与桌宠配置都预先下发给 Rust:
// 老板键按下时由 Rust 直接开窗,不依赖前端往返(前端没响应也不能让老板键失效)。
// 无权益时下发 pet:false,自动降级为"直接隐藏"(与 M5 降级策略一致)。
watch([bossAction, petAvailable], () => {
  invoke("set_boss_action_pet", {
    pet: bossAction.value === "pet" && petAvailable.value,
  }).catch(() => {});
}, { immediate: true });

watch(
  [
    () => pet.kind, () => pet.size, () => pet.followMouse,
    () => pet.wander, () => pet.alwaysOnTop, () => pet.lines,
  ],
  () => {
    const { width, height } = petWindowSize();
    invoke("set_pet_config", { width, height, query: buildPetQuery() }).catch(() => {});
  },
  { deep: true, immediate: true },
);

// 打字监控开关下发:关掉后 Rust 探测线程完全不扫键盘。
// 无权益时按关处理 —— 键盘探测是敏感能力,没买会员就不该跑。
watch([() => pet.typingReact, petAvailable], () => {
  invoke("set_pet_typing", { on: pet.typingReact && petAvailable.value }).catch(() => {});
}, { immediate: true });

// 桌宠已开着时,设置页改尺寸/置顶要立刻生效,不然得关掉重开才看得到。
// 尺寸走事件让桌宠窗自己改(它要同步内部布局);置顶直接调命令。
// 注意:桌宠窗自己改完也会回写 store 触发这里,但值相同时两侧都会提前返回,不会循环。
watch(() => pet.size, (n) => {
  if (!pet.active) return;
  emitEvent("pet-size-push", n).catch(() => {});
});
watch(() => pet.alwaysOnTop, (on) => {
  if (!pet.active) return;
  invoke("set_pet_always_on_top", { on }).catch(() => {});
});

/**
 * 从"全部隐藏"状态恢复后重整 tab 显隐。
 * Rust 恢复时会对 WEB_TABS 全部 show(),会露出非活跃的 inline tab;
 * 这里接管:让当前 active(如果是 inline)显示,其余 inline 再隐藏;popout/pip 保持原样。
 */
async function restoreTabsVisual() {
  // 设置页开着时它占满整个主壳,任何 tab 显示出来都会盖在它上面。
  // 启动弹窗(打赏/强制更新)期间同理,子窗会穿帮盖住弹窗,必须保持隐藏。
  // Rust 恢复时对 WEB_TABS 全部 show() 了,这里必须把它们全收回去。
  if (showSettings.value || dashangVisible.value || forceUpdateVisible.value) {
    for (const t of tabs.tabs) {
      if (t.mode !== "pip") {
        await invoke("set_web_tab_visible", { id: t.id, visible: false }).catch(() => {});
      }
    }
    return;
  }
  const cur = tabs.active;
  if (cur && cur.mode === "inline") {
    await activateVisual(cur.id);
  } else {
    for (const other of tabs.tabs) {
      if (other.mode === "inline") {
        await invoke("set_web_tab_visible", { id: other.id, visible: false }).catch(() => {});
      }
    }
  }
}

// ────── M5: 工作进程监测 ──────
// 把配置下发给 Rust 轮询线程;开关/关键字变化时立即同步。
function pushWorkMonitorConfig() {
  const enabled = workMonitor.value && auth.canUse("workMonitor");
  invoke("set_work_monitor_config", {
    enabled,
    keywords: workMonitorKeywords.value,
  }).catch(() => {});
}
watch([workMonitor, workMonitorKeywords, () => auth.canUse("workMonitor")], pushWorkMonitorConfig, {
  deep: true,
});
/** M5 状态指示:当前是否检测到工作类应用在运行 */
const workAppPresent = ref(false);


/**
 * 「正在收起一切」标志(进桌宠 / 进隐藏)。
 *
 * 这类动作会先关掉设置页,而关设置页的 watcher 默认要"恢复活跃 tab 显示"——
 * 它和 Rust 侧的批量 hide() 是两条并发链,show 落在 hide 之后 tab 就又露出来了。
 * 收起期间置 true,让恢复逻辑整体让路。
 */
const collapsing = ref(false);

// 设置打开时全部 tab 隐藏,关闭时按当前活跃恢复
watch(showSettings, async (v) => {
  if (v) {
    for (const t of tabs.tabs) {
      if (t.mode !== "pip") await invoke("set_web_tab_visible", { id: t.id, visible: false }).catch(() => {});
    }
    // 上面逐个 await 期间可能有 activateVisual 并发把某个 tab 又 show 出来。
    // 设置页是全屏视图,这里再兜一遍,确保它永远是最后的权威。
    if (showSettings.value) {
      for (const t of tabs.tabs) {
        if (t.mode !== "pip") await invoke("set_web_tab_visible", { id: t.id, visible: false }).catch(() => {});
      }
    }
  } else if (!collapsing.value && tabs.active && tabs.active.mode === "inline") {
    await syncCurrentTab();
    // 上面 await 期间可能已经开始收起(进桌宠/隐藏),再判一次才不会把 tab 亮回来
    if (collapsing.value) return;
    await invoke("set_web_tab_visible", { id: tabs.active.id, visible: true }).catch(() => {});
  }
});

// ────── 内置站点 ──────
// 站点数据已迁到 useSitesStore,支持增删

// ────── WebView 承载 ──────
const address = ref("");
const loading = ref(false);
const holder = ref<HTMLDivElement | null>(null);

const activeTab = computed(() => tabs.active);

/** 把 holder 的屏幕坐标同步给当前活跃 tab(inline 模式) */
async function syncCurrentTab() {
  const t = tabs.active;
  // inline / fullscreen 都把当前子窗贴进 holder(fullscreen 时 holder 由 .fs-content 铺满标题栏下方)
  if (!t || (t.mode !== "inline" && t.mode !== "fullscreen") || !holder.value) return;
  const rect = holder.value.getBoundingClientRect();
  const scale = window.devicePixelRatio || 1;
  const outer = await win.outerPosition();
  const inner = await win.innerPosition();
  const chromeOffsetX = (outer.x - inner.x) / scale;
  const chromeOffsetY = (outer.y - inner.y) / scale;

  const x = inner.x / scale + rect.left + chromeOffsetX;
  const y = inner.y / scale + rect.top + chromeOffsetY;
  await invoke("resize_web_tab", {
    id: t.id, x, y, width: rect.width, height: rect.height,
  });
}

/** 视觉激活:先把位置摆好、再显示、最后 focus——顺序错任何一个都会有 bug */
async function activateVisual(id: string) {
  const t = tabs.tabs.find((x) => x.id === id);
  if (!t || t.mode === "pip" || t.mode === "popout") return;
  // 冷启动兜底:tabs store 是持久化的,但子 webview 每次重启后都要重建
  const exists = await invoke<boolean>("web_tab_exists", { id }).catch(() => true);
  if (!exists && holder.value) {
    const rect = holder.value.getBoundingClientRect();
    const scale = window.devicePixelRatio || 1;
    const outer = await win.outerPosition();
    const inner = await win.innerPosition();
    const chromeOffsetX = (outer.x - inner.x) / scale;
    const chromeOffsetY = (outer.y - inner.y) / scale;
    const x = inner.x / scale + rect.left + chromeOffsetX;
    const y = inner.y / scale + rect.top + chromeOffsetY;
    await invoke("open_web_tab", { id, url: t.url, x, y, width: rect.width, height: rect.height }).catch(() => {});
    await invoke("set_web_tab_opacity", { id, opacity: effectiveOpacity.value }).catch(() => {});
  }
  // 1) 先把当前 tab 摆到 holder 位置(此时可能还没显示,不影响)
  await syncCurrentTab();
  // 2) 只对 inline/fullscreen 的 tab 做隐藏切换;popout/pip 是脱离主壳的浮动窗口,不动
  //    注意:上面几个 await 期间用户可能已经打开了设置页(它占满主壳),
  //    此时任何 show 都会盖在设置页上,所以这里必须重新判定而不能只信入口状态。
  //    启动弹窗(打赏/强制更新)期间同理:子窗是独立 OS 窗口会盖在弹窗之上,必须保持隐藏。
  const shouldShow =
    !showSettings.value && !dashangVisible.value && !forceUpdateVisible.value;
  for (const other of tabs.tabs) {
    if (other.mode === "pip" || other.mode === "popout") continue;
    const visible = shouldShow && other.id === id;
    await invoke("set_web_tab_visible", { id: other.id, visible }).catch(() => {});
  }
  if (!shouldShow) return;
  // 3) 再 sync 一次,防止 show 触发的 layout 抖动导致位置飘
  await syncCurrentTab();
  // 4) focus 让子窗口接收键鼠事件(Windows 上 show 不自动 focus)
  await invoke("focus_web_tab", { id }).catch(() => {});
  // 5) 换壳伪装进行中:新建/切换的 tab 需补注入暗色样式(命令对已注入的窗口幂等)
  if (disguise.active && disguise.skin === "vscode") {
    await invoke("set_reading_disguise", { enabled: true }).catch(() => {});
  }
}

async function openSite(url: string, hint?: { name: string; icon: string }) {
  if (!holder.value) return;
  const rect = holder.value.getBoundingClientRect();
  const scale = window.devicePixelRatio || 1;
  const outer = await win.outerPosition();
  const inner = await win.innerPosition();
  const chromeOffsetX = (outer.x - inner.x) / scale;
  const chromeOffsetY = (outer.y - inner.y) / scale;
  const x = inner.x / scale + rect.left + chromeOffsetX;
  const y = inner.y / scale + rect.top + chromeOffsetY;

  const existed = tabs.tabs.find((t) => t.url === url);
  const id = tabs.openOrFocus(url, hint?.name, hint?.icon, videoAutoLandscape.value);
  address.value = url;

  loading.value = true;
  try {
    if (!existed) {
      await invoke("open_web_tab", { id, url, x, y, width: rect.width, height: rect.height });
      await invoke("set_web_tab_opacity", { id, opacity: effectiveOpacity.value });
    }
    await activateVisual(id);
  } catch (e) {
    console.error("open_web_tab failed", e);
  } finally {
    loading.value = false;
  }
}

async function go() {
  let url = address.value.trim();
  if (!url) return;
  if (!/^https?:\/\//i.test(url)) url = "https://" + url;
  await openSite(url);
}

async function switchTab(id: string) {
  tabs.activate(id);
  address.value = tabs.active?.url ?? "";
  if (tabs.active && tabs.active.mode !== "pip") {
    await activateVisual(id);
  }
}

function closeTabById(id: string) {
  tabs.closeTab(id);
  if (tabs.active) {
    address.value = tabs.active.url;
    activateVisual(tabs.active.id);
  } else {
    // 关到空了:兜底把所有可能残留的 web-tab 全部关闭(hide+close),不留幽灵画面
    address.value = "";
    invoke("close_all_web_tabs").catch(() => {});
  }
}

// ────── 收藏当前 URL / 删除快捷站点 ──────
const canFav = computed(() => {
  const url = tabs.active?.url || address.value.trim();
  return !!url && !sitesStore.hasUrl(url);
});
function favCurrent() {
  const t = tabs.active;
  const url = t?.url || address.value.trim();
  if (!url) return;
  sitesStore.add({
    name: t?.title || new URL(url.startsWith("http") ? url : "https://" + url).hostname,
    icon: t?.icon || sitesStore.guessIcon(url),
    url: url.startsWith("http") ? url : "https://" + url,
  });
}
function removeSite(url: string, ev: MouseEvent) {
  ev.stopPropagation();
  sitesStore.remove(url);
}

// 站点快捷栏拖拽排序(HTML5 原生拖拽)
const dragFromUrl = ref<string | null>(null);
const dragOverUrl = ref<string | null>(null);
function onSiteDragStart(ev: DragEvent, url: string) {
  // 改名中不允许拖
  if (editingUrl.value) { ev.preventDefault(); return; }
  dragFromUrl.value = url;
  ev.dataTransfer?.setData("text/plain", url);
  if (ev.dataTransfer) ev.dataTransfer.effectAllowed = "move";
}
function onSiteDragOver(ev: DragEvent, url: string) {
  if (!dragFromUrl.value || dragFromUrl.value === url) return;
  ev.preventDefault();
  if (ev.dataTransfer) ev.dataTransfer.dropEffect = "move";
  dragOverUrl.value = url;
}
function onSiteDrop(ev: DragEvent, url: string) {
  ev.preventDefault();
  ev.stopPropagation();
  if (dragFromUrl.value) sitesStore.reorder(dragFromUrl.value, url);
  dragFromUrl.value = null;
  dragOverUrl.value = null;
}
function onSiteDragEnd() {
  dragFromUrl.value = null;
  dragOverUrl.value = null;
}

// 行内改名:双击/铅笔按钮进入编辑,回车/失焦提交,Esc 取消
const editingUrl = ref<string | null>(null);
const editingBuffer = ref("");
function startRenameSite(url: string, currentName: string, ev: Event) {
  ev.stopPropagation();
  editingUrl.value = url;
  editingBuffer.value = currentName;
  nextTick(() => {
    const input = document.querySelector<HTMLInputElement>('.site-name-input');
    input?.focus();
    input?.select();
  });
}
function commitRenameSite() {
  if (editingUrl.value) sitesStore.rename(editingUrl.value, editingBuffer.value);
  editingUrl.value = null;
  editingBuffer.value = "";
}
function cancelRenameSite() {
  editingUrl.value = null;
  editingBuffer.value = "";
}

// ────── 导航 ──────
async function goBack() {
  if (!tabs.active) return;
  await invoke("web_tab_go_back", { id: tabs.active.id }).catch(() => {});
}
async function goForward() {
  if (!tabs.active) return;
  await invoke("web_tab_go_forward", { id: tabs.active.id }).catch(() => {});
}
async function reload() {
  if (!tabs.active) return;
  await invoke("web_tab_reload", { id: tabs.active.id }).catch(() => {});
}

// ────── 右键菜单(独立 Tauri 窗口) ──────
interface CtxItem { key: string; label: string; icon?: string; divider?: boolean; disabled?: boolean; }
let pendingCtxTargetId: string | null = null;

function ctxItemsForTab(tabId: string | null): CtxItem[] {
  const t = tabId ? tabs.tabs.find((x) => x.id === tabId) : tabs.active;
  if (!t) return [];
  const items: CtxItem[] = [
    { key: "back",       label: "◂ 后退" },
    { key: "forward",    label: "▸ 前进" },
    { key: "reload",     label: "⟳ 刷新" },
    { key: "d1",         label: "",  divider: true },
    { key: "popout",     label: t.mode === "popout"     ? "回到摸鱼窗口" : "独立窗口",     icon: "⧉" },
    { key: "pip",        label: t.mode === "pip"        ? "回到摸鱼窗口" : "画中画",       icon: "⛶" },
    { key: "fullscreen", label: t.mode === "fullscreen" ? "退出全屏"     : "应用内全屏",   icon: "⛶" },
  ];
  if (t.mode === "popout") {
    items.push({ key: "pin", label: t.pinned ? "取消置顶" : "窗口置顶", icon: "📌" });
  }
  items.push(
    { key: "d2",    label: "",  divider: true },
    { key: "close", label: "关闭标签", icon: "✕" },
  );
  return items;
}

/** 打开独立菜单窗口(能盖住 web-tab) */
async function openContextMenu(e: MouseEvent, tabId?: string) {
  e.preventDefault();
  const targetId = tabId ?? tabs.active?.id ?? null;
  if (!targetId) return;
  pendingCtxTargetId = targetId;

  const items = ctxItemsForTab(targetId);
  // 尺寸预算:每项 32px + padding 8,菜单宽 180
  const w = 180;
  const h = items.length * 32 + 8;

  // 转成屏幕物理坐标(菜单是独立窗口,坐标以屏幕为基准)
  const scale = window.devicePixelRatio || 1;
  const outer = await win.outerPosition();
  const inner = await win.innerPosition();
  const chromeOffsetX = (outer.x - inner.x) / scale;
  const chromeOffsetY = (outer.y - inner.y) / scale;

  const screenX = inner.x / scale + chromeOffsetX + e.clientX;
  const screenY = inner.y / scale + chromeOffsetY + e.clientY;

  // 边界翻转(菜单不超出屏幕)
  const maxX = window.screen.availWidth - w - 4;
  const maxY = window.screen.availHeight - h - 4;
  const finalX = Math.min(screenX, maxX);
  const finalY = Math.min(screenY, maxY);

  await invoke("show_context_menu", {
    x: finalX,
    y: finalY,
    width: w,
    height: h,
    data: encodeURIComponent(JSON.stringify(items)),
  });
}

/** 标题栏"⋯"按钮:菜单在按钮正下方弹出 */
async function openTitlebarMenu(e: MouseEvent) {
  if (!tabs.active) return;
  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
  // 构造一个假的 MouseEvent-like,复用 openContextMenu 逻辑
  const fakeE = {
    preventDefault() {},
    clientX: rect.left,
    clientY: rect.bottom + 4,
  } as MouseEvent;
  await openContextMenu(fakeE, tabs.active.id);
}

async function pickContextMenu(key: string) {
  // 桌宠窗口的菜单也走同一个独立菜单窗口,pick 事件是全局广播的;
  // pet: 前缀那批由桌宠自己处理,主窗必须放过,否则会被当成 tab 操作。
  if (key.startsWith("pet:")) return;
  const targetId = pendingCtxTargetId ?? tabs.active?.id ?? null;
  pendingCtxTargetId = null;
  if (!targetId) return;
  const t = tabs.tabs.find((x) => x.id === targetId);
  if (!t) return;

  switch (key) {
    case "back":       return goBack();
    case "forward":    return goForward();
    case "reload":     return reload();
    case "close":      return closeTabById(targetId);
    case "popout":
      if (t.mode === "popout") return restoreInline(targetId);
      return popOut(targetId);
    case "pip":
      if (t.mode === "pip") return restoreInline(targetId);
      { const size = pipDefaultSize(t.pipRatio);
        const x = window.screen.availWidth - size.w - 24;
        const y = window.screen.availHeight - size.h - 24;
        await invoke("enter_pip", { id: targetId, x, y, width: size.w, height: size.h });
        await invoke("set_web_tab_resizable", { id: targetId, resizable: true });
        await invoke("set_web_tab_chrome", {
          id: targetId,
          decorations: false,
          skipTaskbar: false,
          alwaysOnTop: true,
          title: popoutTitle(t),
        });
        await invoke("set_web_tab_owner", { id: targetId, owner: false }).catch(() => {});
        tabs.setMode(targetId, "pip");
        await invoke("set_web_tab_opacity", { id: targetId, opacity: effectiveOpacity.value }).catch(() => {});
        await notifyTabMode(targetId);
      }
      return;
    case "fullscreen":
      if (t.mode === "fullscreen") return restoreInline(targetId);
      return fullscreenInApp(targetId);
    case "pin":
      return togglePopoutPin(targetId);
  }
}

// ────── PiP / Popout / Fullscreen 三种脱离 mode ──────

/** 视频站默认 480×270(16:9),其它 400×560(4:3-ish) */
function pipDefaultSize(ratio: PipRatio): { w: number; h: number } {
  if (ratio === "16:9") return { w: 480, h: 270 };
  if (ratio === "4:3") return { w: 400, h: 300 };
  return { w: 420, h: 320 };
}

/** 标题栏 ⛶ 按钮:fullscreen 下退出全屏,其它状态走画中画切换 */
async function onPipBtnClick() {
  const t = tabs.active;
  if (!t) return;
  if (t.mode === "fullscreen") return restoreInline(t.id);
  return togglePip();
}

async function togglePip() {
  const t = tabs.active;
  if (!t) return;
  if (t.mode === "pip") {
    await restoreInline(t.id);
  } else {
    const size = pipDefaultSize(t.pipRatio);
    const x = window.screen.availWidth - size.w - 24;
    const y = window.screen.availHeight - size.h - 24;
    await invoke("enter_pip", { id: t.id, x, y, width: size.w, height: size.h });
    await invoke("set_web_tab_resizable", { id: t.id, resizable: true });
    // pip 也用自绘标题栏:无系统边框,可拖动/最小化/收回
    await invoke("set_web_tab_chrome", {
      id: t.id,
      decorations: false,
      skipTaskbar: false,
      alwaysOnTop: true,
      title: popoutTitle(t),
    });
    // 断开 owner,主窗最小化时它保持浮着
    await invoke("set_web_tab_owner", { id: t.id, owner: false }).catch(() => {});
    tabs.setMode(t.id, "pip");
    // decorations 切换会重设 Windows 的 EX_STYLE,重新 apply 透明度
    await invoke("set_web_tab_opacity", { id: t.id, opacity: effectiveOpacity.value }).catch(() => {});
    await notifyTabMode(t.id);
  }
}

/** 独立浮窗:大尺寸+可缩放+自绘标题栏(无系统边框,深/浅/跟随系统) */
async function popOut(id?: string) {
  const t = id ? tabs.tabs.find((x) => x.id === id) : tabs.active;
  if (!t) return;
  const w = 900, h = 640;
  const x = Math.round((window.screen.availWidth - w) / 2);
  const y = Math.round((window.screen.availHeight - h) / 2);
  await invoke("enter_pip", { id: t.id, x, y, width: w, height: h });
  await invoke("set_web_tab_resizable", { id: t.id, resizable: true });
  // popout:无系统标题栏,改用注入脚本里的自绘标题栏(可拖/置顶/最小化/收回)
  await invoke("set_web_tab_chrome", {
    id: t.id,
    decorations: false,
    skipTaskbar: false,
    alwaysOnTop: false,
    title: popoutTitle(t),
  });
  // 断开与主窗的 owner 关系,主窗最小化时它不再被拖走
  await invoke("set_web_tab_owner", { id: t.id, owner: false }).catch(() => {});
  tabs.setMode(t.id, "popout");
  // decorations 切换后重新 apply 透明度
  await invoke("set_web_tab_opacity", { id: t.id, opacity: effectiveOpacity.value }).catch(() => {});
  await notifyTabMode(t.id);
}

/** 应用内全屏:webview 占满整个 shell 位置(把地址栏/tabbar 遮住) */
async function fullscreenInApp(id?: string) {
  const t = id ? tabs.tabs.find((x) => x.id === id) : tabs.active;
  if (!t) return;
  // 先把 mode 改掉,DOM 更新后再 sync 位置(遮住 tabbar/toolbar)
  tabs.setMode(t.id, "fullscreen");
  await nextTick();
  await syncFullscreenTab(t.id);
  // 只对 inline/fullscreen tab 做切换,popout/pip 不动
  for (const other of tabs.tabs) {
    if (other.mode === "pip" || other.mode === "popout") continue;
    await invoke("set_web_tab_visible", { id: other.id, visible: other.id === t.id }).catch(() => {});
  }
  await invoke("focus_web_tab", { id: t.id }).catch(() => {});
  // 保险:fullscreen 前若从 pip/popout 过来,decorations 可能被切过,重新 apply
  await invoke("set_web_tab_opacity", { id: t.id, opacity: effectiveOpacity.value }).catch(() => {});
  await notifyTabMode(t.id);
}

/** fullscreen 时把子窗口拉满整个 shell 内容区(标题栏下方)。
 *  复用 syncCurrentTab 的 holder 矩形算法:fullscreen 下 .fs-content padding:0,
 *  holder 正好铺满标题栏下方区域,且走同一套 resize_web_tab(Win32 SetWindowPos 按外框),
 *  避免手算 innerSize-36 在右/下产生度量不一致导致溢出。 */
async function syncFullscreenTab(_id: string) {
  await syncCurrentTab();
}

/** 回到嵌入模式 */
async function restoreInline(id: string) {
  const t = tabs.tabs.find((x) => x.id === id);
  // fullscreen 模式从未改过 chrome / owner / resizable / always_on_top,
  // 退出时不要跑那一串"恢复"调用——多余不说,某些平台上还会因窗口样式重算导致抖动
  const fromFullscreen = t?.mode === "fullscreen";
  if (fromFullscreen) {
    // 应用内全屏是靠子页面的 fake fullscreen(套 position:fixed + body overflow:hidden)撑起来的。
    // 光重置窗口几何不够,必须让子页面执行 exitFullscreen 还原样式,
    // 否则退出后页面无法上下滚动、滚动条消失。
    await invoke("exit_web_tab_fullscreen", { id }).catch(() => {});
  }
  if (!fromFullscreen) {
    await invoke("exit_pip", { id }).catch(() => {});
    await invoke("set_web_tab_resizable", { id, resizable: false });
    // 恢复:关系统标题栏,退出任务栏,回到主窗口子级外观
    await invoke("set_web_tab_chrome", {
      id,
      decorations: false,
      skipTaskbar: true,
      alwaysOnTop: false,
      title: null,
    }).catch(() => {});
    // 恢复 owner 关系,让主窗能控制它
    await invoke("set_web_tab_owner", { id, owner: true }).catch(() => {});
  }
  tabs.setMode(id, "inline");
  await nextTick();
  // 只有被收回的是当前 active 才显示;否则显式隐藏,避免旧 popout 位置留下空浮壳
  if (tabs.activeId === id) {
    await activateVisual(id);
  } else {
    await invoke("set_web_tab_visible", { id, visible: false }).catch(() => {});
  }
  // decorations 恢复后重新 apply 透明度(fullscreen 分支没改 decorations,这里保险重新应用一次也无副作用)
  await invoke("set_web_tab_opacity", { id, opacity: effectiveOpacity.value }).catch(() => {});
  // 从 popout/pip 回 inline:tabs.pinned 状态本身不清零(用户可能再切出去),
  // 但当前既然回主窗,先撤掉子窗的 always_on_top,防止依附主窗后依然置顶
  if (!fromFullscreen) {
    await invoke("set_web_tab_pinned", { id, pinned: false }).catch(() => {});
    tabs.setPinned(id, false);
  }
  await notifyTabMode(id);
}

// ────── 生命周期 ──────
let unlistenMove: (() => void) | undefined;
let unlistenResize: (() => void) | undefined;
let unlistenBoss: (() => void) | undefined;
let unlistenCtx: (() => void) | undefined;
let unlistenCloseReq: (() => void) | undefined;
let unlistenVideoFs: (() => void) | undefined;
let unlistenPinSignal: (() => void) | undefined;
let unlistenVideoPlay: (() => void) | undefined;
let unlistenTranspToggle: (() => void) | undefined;
let unlistenWorkApp: (() => void) | undefined;
let unlistenWorkAppGone: (() => void) | undefined;
let unlistenPetExited: (() => void) | undefined;
let unlistenPetSize: (() => void) | undefined;
let unlistenPetTop: (() => void) | undefined;
let ro: ResizeObserver | undefined;

onMounted(async () => {
  await nextTick();
  // 鉴权状态初始化(异步拉取,不阻塞主窗口)
  auth.init().catch(() => {});
  // 把已持久化的用户脚本下发到 Rust(冷启动恢复后)。
  // 必须 await:set_user_scripts 会预拉取 @require 依赖,完成后才写 USER_SCRIPTS。
  // 若不等它就建子窗,build_userscript_boot 会读到空脚本集,导致冷启动后插件不运行
  // (热加载时数据已在内存,无此竞态)。首次同步只下发不重建,见 isFirstUserScriptsSync。
  await syncUserScripts();
  // 拉取远端启动配置:
  //  - forcedUpdate=true 且 lastUpdate.version 与本地不一致 → 弹强制更新(优先,不可关闭,不处理打赏)
  //  - 否则 popDashang=true 且未选"不再提示"时弹打赏窗
  // 网络失败/超时静默忽略,不影响启动。
  fetchRemoteConfig().then((cfg) => {
    if (!cfg) return;
    // 更新弹窗:lastUpdate.version 与本地不一致 → 每次启动都弹。
    // forcedUpdate=true 时弹窗不可关闭(只能去更新);false 时可关,下次启动仍会再弹。
    const fu = cfg.lastUpdate;
    if (
      fu &&
      fu.url &&
      String(fu.version).trim() !== "" &&
      String(fu.version) !== String(LOCAL_FORCE_UPDATE_VERSION)
    ) {
      forceUpdateUrl.value = fu.url;
      forceUpdateClosable.value = cfg.forcedUpdate !== true;
      // message 按逗号或换行分割成多段
      forceUpdateLines.value = String(fu.message ?? "")
        .split(/[,\n]/)
        .map((s) => s.trim())
        .filter((s) => s.length > 0);
      forceUpdateVisible.value = true;
      // 更新窗优先;打赏若也需要,等更新窗关闭后再弹
      if (cfg.popDashang && !dashangHiddenBefore()) {
        dashangPending.value = true;
      }
      return;
    }
    if (cfg.popDashang && !dashangHiddenBefore()) {
      dashangVisible.value = true;
    }
  });
  // 主窗口移动/缩放 → 根据当前 tab mode 同步
  const onWinChange = () => {
    const t = tabs.active;
    if (!t) return;
    if (t.mode === "fullscreen") syncFullscreenTab(t.id);
    else if (t.mode === "inline") syncCurrentTab();
    // pip / popout 独立浮动,不跟主窗口
  };
  unlistenMove = await win.onMoved(onWinChange);
  unlistenResize = await win.onResized(onWinChange);
  // holder 会随换壳伪装(fullscreen)等 v-if 分支重建,用 watch 把 RO 重连到当前节点;
  // 只在 onMounted 里 observe 一次会在分支切换后丢失监听,子窗不再随编辑区尺寸重对齐。
  ro = new ResizeObserver(() => {
    if (tabs.active?.mode === "inline") syncCurrentTab();
  });
  watch(holder, (el, old) => {
    if (old) ro?.unobserve(old);
    if (el) {
      ro?.observe(el);
      // 分支刚切完布局可能还没稳,补一帧再 sync,防止子窗停在旧位置
      nextTick(() => {
        if (tabs.active?.mode === "inline") syncCurrentTab();
      });
    }
  }, { immediate: true });
  unlistenBoss = await win.listen<boolean>("boss-key-toggled", (evt) => {
    isHidden.value = evt.payload;
    // 恢复时 Windows ShowWindow 可能重置了 WS_EX_LAYERED,重新 apply 透明度
    if (!evt.payload) {
      // 伪装态下按老板键恢复:一并退出伪装,回到真实摸鱼界面
      if (disguise.active) {
        exitDisguise();
        return;
      }
      invoke("set_all_web_tabs_opacity", { opacity: effectiveOpacity.value }).catch(() => {});
      document.body.style.setProperty("--shell-alpha", String(effectiveOpacity.value));
      restoreTabsVisual();
    }
  });
  // 独立菜单窗口选中项后广播
  unlistenCtx = await win.listen<string>("ctx-menu-pick", (evt) => {
    pickContextMenu(evt.payload);
  });
  // pip / popout / fullscreen 按 ✕ = 收回摸鱼窗口;inline ✕(理论不会触发) = 真关
  unlistenCloseReq = await win.listen<{ id: string; label: string }>(
    "web-tab-close-requested",
    async (evt) => {
      const t = tabs.tabs.find((x) => x.id === evt.payload.id);
      if (!t) return;
      if (t.mode !== "inline") {
        // 自绘 ✕ 不像系统标题栏按钮那样会自动激活 owner 主窗;
        // 若主窗被压在其它窗口下(或最小化),收回的子窗会飘在主窗位置却不在主窗里。
        // 先把主窗拉回前台再恢复。
        try { await win.unminimize(); } catch { /* 未最小化时无操作 */ }
        try { await win.setFocus(); } catch { /* 跨平台置前失败可忽略 */ }
        await restoreInline(t.id);
      } else {
        tabs.closeTab(t.id);
      }
    },
  );
  // 子 webview 的视频 requestFullscreen → 走"应用内全屏"(浏览器原生全屏被 init 脚本拦掉了)
  unlistenVideoFs = await win.listen<{ id: string; entering: boolean }>(
    "web-tab-video-fullscreen",
    async (evt) => {
      const { id, entering } = evt.payload;
      const t = tabs.tabs.find((x) => x.id === id);
      if (!t) return;
      // popout / pip 独立窗口:视频已通过 CSS 撑满 webview(等于窗口内全屏),
      // 不把它拉回主窗口做"应用内全屏"
      if (t.mode === "popout" || t.mode === "pip") return;
      if (entering) {
        if (t.mode !== "fullscreen") await fullscreenInApp(id);
      } else {
        if (t.mode === "fullscreen") await restoreInline(id);
      }
    },
  );
  // 视频 play → 若开启"自动横屏",让当前 tab 里的视频进入"视频全屏"
  // 走 requestFullscreen 而非直接 fullscreenInApp:
  //   requestFullscreen 会被子 webview 的 init_script hook,
  //   给 video 元素套 position:fixed 撑满 webview,视频画面才是真填满
  //   fake fullscreen 触发后会 signal 回来,由 web-tab-video-fullscreen 监听接管应用内全屏
  unlistenVideoPlay = await win.listen<string>(
    "web-tab-video-play",
    async (evt) => {
      if (!videoAutoLandscape.value) return;
      const id = evt.payload;
      const t = tabs.tabs.find((x) => x.id === id);
      if (!t || t.mode === "fullscreen" || t.mode === "pip" || t.mode === "popout") return;
      await invoke("request_video_fullscreen", { id }).catch(() => {});
    },
  );
  // 全局"取消透明"快捷键:翻转 bypassOpacity;为 true 时所有 apply 走 1
  unlistenTranspToggle = await win.listen("transparency-toggle-requested", () => {
    bypassOpacity.value = !bypassOpacity.value;
  });
  // M5: 检测到工作类应用启动 → 按配置切伪装或直接隐藏
  unlistenWorkApp = await win.listen("work-app-detected", async () => {
    workAppPresent.value = true;
    if (!workMonitor.value || !auth.canUse("workMonitor")) return;
    if (workMonitorAction.value === "hide") {
      // 已经隐藏就不重复触发(trigger_boss_key 是 toggle 语义)
      const hidden = await invoke<boolean>("is_hidden").catch(() => false);
      if (!hidden) await invoke("trigger_boss_key").catch(() => {});
    } else if (auth.canUse("disguise")) {
      // 动作为伪装但无伪装权益时退化为隐藏,保证监测始终有效
      if (!disguise.active) await enterDisguise();
    } else {
      const hidden = await invoke<boolean>("is_hidden").catch(() => false);
      if (!hidden) await invoke("trigger_boss_key").catch(() => {});
    }
  });
  // M5: 工作应用退出 → 不自动恢复(避免老板还在旁边时突然弹回摸鱼界面),
  //     仅通知前端更新状态指示,由用户手动退出伪装。
  unlistenWorkAppGone = await win.listen("work-app-gone", () => {
    workAppPresent.value = false;
  });
  // M8: 桌宠双击退出 → Rust 已恢复主窗,前端重整 tab 显隐与透明度
  unlistenPetExited = await win.listen("pet-exited", async () => {
    pet.setActive(false);
    isHidden.value = false;
    invoke("set_all_web_tabs_opacity", { opacity: effectiveOpacity.value }).catch(() => {});
    document.body.style.setProperty("--shell-alpha", String(effectiveOpacity.value));
    await restoreTabsVisual();
  });
  // M8: 桌宠窗口内改了尺寸/置顶(滚轮、右键菜单)→ 回写 store 持久化,
  //     并重新下发配置,下次开桌宠沿用。窗口本身已自行改好,这里不再调命令。
  unlistenPetSize = await win.listen<number>("pet-size-changed", (evt) => {
    pet.setSize(evt.payload);
  });
  unlistenPetTop = await win.listen<boolean>("pet-top-changed", (evt) => {
    pet.setAlwaysOnTop(evt.payload);
  });
  // 冷启动:把已保存的 M5 配置下发给 Rust 轮询线程
  pushWorkMonitorConfig();
  // 冷启动:如已保存"取消透明"快捷键,注册回 Rust
  if (transparencyKey.value) {
    invoke("update_transparency_shortcut", { shortcut: transparencyKey.value }).catch(() => {});
  }
  // popout 内浮动 📌 按钮点击 → 同步到主窗状态
  unlistenPinSignal = await win.listen<{ id: string; pinned: boolean }>(
    "web-tab-pin-toggle",
    async (evt) => {
      const { id, pinned } = evt.payload;
      const t = tabs.tabs.find((x) => x.id === id);
      if (!t || t.mode !== "popout") return;
      await invoke("set_web_tab_pinned", { id, pinned }).catch(() => {});
      tabs.setPinned(id, pinned);
      // 同上:always_on_top 会冲掉 WS_EX_LAYERED,补 apply 透明度
      await invoke("set_web_tab_opacity", { id, opacity: effectiveOpacity.value }).catch(() => {});
    },
  );
  isHidden.value = await invoke<boolean>("is_hidden");

  if (tabs.active) {
    address.value = tabs.active.url;
    // 冷启动:tabs 已从 localStorage 恢复,但子 webview 未建 → 主动激活当前 tab 触发懒建
    if (tabs.active.mode === "inline") await activateVisual(tabs.active.id);
  }
});

onBeforeUnmount(() => {
  unlistenMove?.();
  unlistenResize?.();
  unlistenBoss?.();
  unlistenCtx?.();
  unlistenCloseReq?.();
  unlistenVideoFs?.();
  unlistenVideoPlay?.();
  unlistenTranspToggle?.();
  unlistenPinSignal?.();
  unlistenWorkApp?.();
  unlistenWorkAppGone?.();
  unlistenPetExited?.();
  unlistenPetSize?.();
  unlistenPetTop?.();
  ro?.disconnect();
});
</script>

<template>
  <div class="shell" :class="{ 'is-fullscreen': activeTab?.mode === 'fullscreen' }" @contextmenu="openContextMenu">
    <!-- 顶部自定义标题栏(换壳伪装时由 DisguiseChrome 提供自己的标题栏) -->
    <header v-if="!chromeActive" class="titlebar" data-tauri-drag-region>
      <span class="brand" data-tauri-drag-region>
        <svg class="brand-logo" viewBox="0 0 128 128" width="16" height="16" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
          <rect x="4" y="4" width="120" height="120" rx="28" fill="#2f6ff0"/>
          <path fill="#fff" d="M64 32 C56 32 49 35 45 41 C41 40 33 41 29 48 C22 50 16 58 16 68 C16 82 26 90 42 89 C50 91 57 92 64 92 C71 92 78 91 86 89 C102 90 112 82 112 68 C112 58 106 50 99 48 C95 41 87 40 83 41 C79 35 72 32 64 32 Z"/>
          <path fill="#0a3289" d="M82 61 L97 53 L90 66 L97 79 L81 72 Z"/>
          <path fill="#1f6ef0" d="M35 56 C40 47 57 44 73 55 C82 61 85 65 83 69 C80 74 66 80 50 75 C41 72 37 66 35 60 Z"/>
          <circle cx="42" cy="55" r="3.3" fill="#fff"/>
          <circle cx="43" cy="54.3" r="1.4" fill="#0c2a6b"/>
        </svg>
        云摸鱼
      </span>
      <div class="drag-spacer" data-tauri-drag-region></div>
      <div class="titlebar-actions">
        <label class="opacity" v-if="activeTab?.mode !== 'fullscreen'">
          <span data-tauri-drag-region>透明度</span>
          <input type="range" min="0.2" max="1" step="0.05" :value="opacity" @input="onOpacityInput" />
        </label>
        <button
          class="btn"
          :class="{ toggled: activeTab && activeTab.mode !== 'inline' }"
          :disabled="!activeTab"
          @click="onPipBtnClick"
          :title="activeTab?.mode === 'fullscreen'
            ? '退出应用内全屏'
            : activeTab?.mode === 'pip' ? '退出画中画' : '画中画(小窗置顶)'"
        >⛶</button>
        <button
          class="btn"
          :disabled="!activeTab"
          @click="openTitlebarMenu"
          title="当前 tab 操作菜单(后退/前进/独立窗口/全屏…)"
        >⋯</button>
        <button class="btn" @click="triggerBossKey" title="老板键">🚨</button>
        <button
          v-if="disguiseAvailable"
          class="btn"
          @click="toggleDisguise"
          title="伪装皮肤(VIP) · 进入后按 Esc 退出"
        >🎭</button>
        <button
          class="btn"
          :class="{ toggled: mainPinned }"
          @click="toggleMainPin"
          :title="mainPinned ? '取消置顶' : '窗口置顶(Always on top)'"
        >📌</button>
        <button class="btn" @click="showSettings = !showSettings" title="设置">⚙</button>
        <button class="btn" @click="minimize" title="最小化">—</button>
        <button class="btn" @click="toggleMaximize" title="最大化 / 还原">▢</button>
        <button class="btn close" @click="close" title="关闭">✕</button>
      </div>
    </header>

    <!-- 独立设置视图 -->
    <SettingsView
      v-if="showSettings"
      :auto-hide="autoHide"
      :auto-hide-delay="autoHideDelay"
      :video-auto-landscape="videoAutoLandscape"
      :boss-key="bossKey"
      :transparency-key="transparencyKey"
      :bypass-opacity="bypassOpacity"
      :work-monitor="workMonitor"
      :work-monitor-action="workMonitorAction"
      :work-monitor-keywords="workMonitorKeywords"
      :work-app-present="workAppPresent"
      :boss-action="bossAction"
      :popout-theme="popoutTheme"
      @update:auto-hide="autoHide = $event"
      @update:auto-hide-delay="autoHideDelay = $event"
      @update:video-auto-landscape="videoAutoLandscape = $event"
      @update:boss-key="bossKey = $event"
      @update:transparency-key="transparencyKey = $event"
      @update:work-monitor="workMonitor = $event"
      @update:work-monitor-action="workMonitorAction = $event"
      @update:work-monitor-keywords="workMonitorKeywords = $event"
      @update:boss-action="bossAction = $event"
      @preview-disguise="enterDisguise"
      @preview-pet="enterPetMode"
      @close="showSettings = false"
    />

    <!-- fullscreen 模式 / 换壳伪装下隐藏地址栏和 tabbar -->
    <template v-if="activeTab?.mode !== 'fullscreen' && !chromeActive">
      <!-- 地址栏 + 站点预设 -->
      <div class="toolbar">
        <div class="address">
          <button class="btn" @click="goBack" :disabled="!activeTab" title="后退">◂</button>
          <button class="btn" @click="goForward" :disabled="!activeTab" title="前进">▸</button>
          <button class="btn" @click="reload" :disabled="!activeTab" title="刷新">⟳</button>
          <input v-model="address" placeholder="输入网址或从下方选站点开摸" @keydown.enter="go" />
          <button
            class="btn"
            :disabled="!canFav"
            @click="favCurrent"
            :title="canFav ? '收藏到快捷栏' : '已在快捷栏或无有效网址'"
          >⭐</button>
          <button class="btn primary" @click="go" :disabled="loading">
            {{ loading ? "…" : "打开" }}
          </button>
        </div>
        <div class="sites">
          <div
            v-for="s in sitesStore.sites"
            :key="s.url"
            class="site"
            :class="{
              active: activeTab?.url === s.url,
              editing: editingUrl === s.url,
              'dragging': dragFromUrl === s.url,
              'drag-over': dragOverUrl === s.url,
            }"
            draggable="true"
            @click="editingUrl !== s.url && openSite(s.url, s)"
            @dblclick.stop="startRenameSite(s.url, s.name, $event)"
            @dragstart="onSiteDragStart($event, s.url)"
            @dragover="onSiteDragOver($event, s.url)"
            @drop="onSiteDrop($event, s.url)"
            @dragend="onSiteDragEnd"
            :title="s.url + ' · 双击改名 · 拖动排序'"
          >
            <span class="icon">{{ s.icon }}</span>
            <input
              v-if="editingUrl === s.url"
              class="site-name-input"
              v-model="editingBuffer"
              @click.stop
              @keydown.enter.prevent="commitRenameSite"
              @keydown.esc.prevent="cancelRenameSite"
              @blur="commitRenameSite"
            />
            <span v-else class="name">{{ s.name }}</span>
            <span
              v-if="editingUrl !== s.url"
              class="site-edit"
              @click="(e) => startRenameSite(s.url, s.name, e)"
              title="改名"
            >✎</span>
            <span
              v-if="editingUrl !== s.url"
              class="site-x"
              @click="(e) => removeSite(s.url, e)"
              title="从快捷栏删除"
            >×</span>
          </div>
        </div>
      </div>

      <!-- 多标签栏 -->
      <TabBar @switch="switchTab" @close="closeTabById" @contextmenu="(e, id) => openContextMenu(e, id)" />
    </template>

    <!-- WebView 承载区 -->
    <!-- 换壳伪装:用 VSCode 外壳包住真实网页承载区,坐标同步自动贴齐编辑区 -->
    <DisguiseChrome
      v-if="chromeActive"
      @exit="exitDisguise"
      @switch="switchTab"
      @close="closeTabById"
      @open-site="(u) => openSite(u)"
      @minimize="minimize"
      @toggle-maximize="toggleMaximize"
      @close-win="close"
    >
      <div ref="holder" class="web-holder"></div>
    </DisguiseChrome>

    <main v-else class="content" :class="{ 'fs-content': activeTab?.mode === 'fullscreen' }">
      <div ref="holder" class="web-holder">
        <div v-if="!activeTab || activeTab.mode === 'pip' || activeTab.mode === 'popout'" class="hero">
          <svg class="hero-logo" viewBox="0 0 128 128" width="84" height="84" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
            <defs>
              <linearGradient id="hl-tile" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#5aa3ff"/><stop offset=".55" stop-color="#2f6ff0"/><stop offset="1" stop-color="#163fbf"/></linearGradient>
              <linearGradient id="hl-cloud" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff"/><stop offset="1" stop-color="#e4eefc"/></linearGradient>
              <linearGradient id="hl-fish" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#3b86ff"/><stop offset="1" stop-color="#0c3a9e"/></linearGradient>
            </defs>
            <rect x="4" y="4" width="120" height="120" rx="28" fill="url(#hl-tile)"/>
            <path fill="url(#hl-cloud)" stroke="#cfe0f6" stroke-width="1" d="M64 32 C56 32 49 35 45 41 C41 40 33 41 29 48 C22 50 16 58 16 68 C16 82 26 90 42 89 C50 91 57 92 64 92 C71 92 78 91 86 89 C102 90 112 82 112 68 C112 58 106 50 99 48 C95 41 87 40 83 41 C79 35 72 32 64 32 Z"/>
            <path fill="#0a3289" d="M82 61 L97 53 L90 66 L97 79 L81 72 Z"/>
            <path fill="url(#hl-fish)" d="M35 56 C40 47 57 44 73 55 C82 61 85 65 83 69 C80 74 66 80 50 75 C41 72 37 66 35 60 Z"/>
            <path d="M42 55 C52 50 65 52 74 58" fill="none" stroke="#a9ccff" stroke-width="2.2" stroke-linecap="round" opacity=".7"/>
            <circle cx="42" cy="55" r="3.3" fill="#fff"/>
            <circle cx="43" cy="54.3" r="1.4" fill="#0c2a6b"/>
          </svg>
          <h1>云摸鱼</h1>
          <p v-if="activeTab?.mode === 'pip'" class="tag">当前 tab 已进入画中画,浮在屏幕右下</p>
          <p v-else-if="activeTab?.mode === 'popout'" class="tag">当前 tab 已独立浮窗</p>
          <p v-else class="tag">选一个站点开摸,或粘贴网址到地址栏</p>
          <p class="tip">
            <template v-if="activeTab && activeTab.mode !== 'inline' && activeTab.mode !== 'fullscreen'">
              点下方按钮收回到摸鱼窗口
            </template>
            <template v-else>
              ✨ 提示:右键任何位置打开菜单 · Ctrl+Alt+Q 老板键 · ⚙ 打开设置
            </template>
          </p>
          <button
            v-if="activeTab && (activeTab.mode === 'pip' || activeTab.mode === 'popout')"
            class="btn primary big"
            @click="restoreInline(activeTab.id)"
          >↩ 回到摸鱼窗口</button>
        </div>
      </div>
    </main>

    <!-- Fullscreen 时的浮动返回按钮(右上角,永远置顶) -->
    <button
      v-if="activeTab?.mode === 'fullscreen'"
      class="fs-back"
      @click="restoreInline(activeTab.id)"
      title="退出全屏"
    >↩ 退出全屏</button>

    <!-- 右键菜单已改为独立 Tauri 窗口(能盖住 web-tab),不在 DOM 里渲染 -->

    <!-- M4 伪装皮肤覆盖层:仅 excel/ppt 遮蔽式渲染(vscode 走换壳 chrome) -->
    <DisguiseOverlay v-if="disguise.active && disguise.skin !== 'vscode'" @exit="exitDisguise" />

    <!-- 启动打赏弹窗 -->
    <div v-if="dashangVisible" class="ds-mask" @click.self="closeDashang(false)">
      <div class="ds-dialog">
        <button class="ds-x" @click="closeDashang(false)" title="关闭">✕</button>
        <h3 class="ds-title">支持云摸鱼 ☕</h3>
        <p class="ds-sub">如果这个小工具让你摸鱼愉快,欢迎请作者喝杯咖啡</p>
        <div class="ds-qrs">
          <div v-for="img in dashangImages" :key="img.url" class="ds-qr">
            <ZoomableImage :src="img.url" :alt="img.name" img-class="ds-qr-img" />
            <span>{{ img.name }}</span>
          </div>
        </div>
        <label class="ds-never">
          <input type="checkbox" ref="dashangNeverRef" />
          <span>以后不再弹出</span>
        </label>
        <button class="btn primary ds-close" @click="closeDashang(!!dashangNeverRef?.checked)">
          关闭
        </button>
      </div>
    </div>

    <!-- 更新弹窗:version 与本地不一致时每次启动都弹;forcedUpdate=true 时不可关闭 -->
    <div v-if="forceUpdateVisible" class="ds-mask" @click.self="closeForceUpdate">
      <div class="ds-dialog fu-dialog">
        <button v-if="forceUpdateClosable" class="ds-x" @click="closeForceUpdate" title="关闭">✕</button>
        <h3 class="ds-title">发现新版本</h3>
        <p class="ds-sub">
          {{ forceUpdateClosable ? '有新版本可用,建议更新' : '当前版本过低,请更新后继续使用' }}
        </p>
        <ul v-if="forceUpdateLines.length" class="fu-notes">
          <li v-for="(line, i) in forceUpdateLines" :key="i">{{ line }}</li>
        </ul>
        <button class="btn primary fu-update-btn" @click="openForceUpdate">立即更新</button>
        <button
          v-if="forceUpdateClosable"
          class="btn fu-later-btn"
          @click="closeForceUpdate"
        >稍后再说</button>
      </div>
    </div>
  </div>

  <!-- 无边框窗口 8 方向拉伸手柄:放在 .shell 外部,用 fixed 定位到窗口边缘。
       .shell 有 border-radius+overflow:hidden 会裁剪子元素命中区域,
       导致右下角等圆角处点不到;移出后不受裁剪。fullscreen 时不显示。 -->
  <template v-if="activeTab?.mode !== 'fullscreen'">
    <div class="resize-edge n"  @mousedown="(e) => startResize('North', e)"></div>
    <div class="resize-edge s"  @mousedown="(e) => startResize('South', e)"></div>
    <div class="resize-edge w"  @mousedown="(e) => startResize('West', e)"></div>
    <div class="resize-edge e"  @mousedown="(e) => startResize('East', e)"></div>
    <div class="resize-corner nw" @mousedown="(e) => startResize('NorthWest', e)"></div>
    <div class="resize-corner ne" @mousedown="(e) => startResize('NorthEast', e)"></div>
    <div class="resize-corner sw" @mousedown="(e) => startResize('SouthWest', e)"></div>
    <div class="resize-corner se" @mousedown="(e) => startResize('SouthEast', e)"></div>
  </template>
</template>

<style>
:root {
  --shell-alpha: 1;
  --web-tab-opacity: 1;
  color-scheme: dark;
}

html, body, #app {
  height: 100%;
  margin: 0;
  padding: 0;
  background: transparent;
  color: #e6edf3;
  font-family: -apple-system, "Segoe UI", "Microsoft YaHei", sans-serif;
  overflow: hidden;
  user-select: none;
}

.shell {
  position: relative;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: rgba(22, 27, 34, calc(var(--shell-alpha) * 0.85));
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  border-radius: 12px;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.06);
}

.titlebar {
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px 0 14px;
  background: rgba(13, 17, 23, calc(var(--shell-alpha) * 0.6));
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  flex-shrink: 0;
}
.brand { font-size: 13px; letter-spacing: 0.5px; opacity: 0.9; display: inline-flex; align-items: center; gap: 6px; }
.brand-logo { display: block; flex-shrink: 0; border-radius: 3px; }
.titlebar-actions { display: flex; align-items: center; gap: 10px; }
.drag-spacer { flex: 1; height: 100%; min-width: 40px; }
.opacity {
  display: flex; align-items: center; gap: 6px;
  font-size: 11px; opacity: 0.65;
}
.opacity input[type="range"] { width: 90px; accent-color: #58a6ff; }

.btn {
  min-width: 28px;
  height: 26px;
  padding: 0 8px;
  border: none;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.06);
  color: #c9d1d9;
  cursor: pointer;
  font-size: 12px;
  transition: background 0.15s, color 0.15s;
}
.btn:hover:not(:disabled) { background: rgba(255, 255, 255, 0.12); }
.btn:disabled { opacity: 0.35; cursor: not-allowed; }
.btn.close:hover { background: #da3633; color: #fff; }
.btn.primary { background: #1f6feb; color: #fff; }
.btn.primary:hover:not(:disabled) { background: #388bfd; }
.btn.toggled {
  background: rgba(31, 111, 235, 0.25);
  color: #58a6ff;
  border: 1px solid rgba(88, 166, 255, 0.4);
}

.toolbar {
  padding: 8px 12px 10px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  background: rgba(13, 17, 23, calc(var(--shell-alpha) * 0.4));
  flex-shrink: 0;
}
.address {
  display: flex; gap: 6px; margin-bottom: 8px;
}
.address input {
  flex: 1; height: 30px; padding: 0 10px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.04);
  color: #e6edf3; font-size: 12px; outline: none;
  transition: border-color 0.15s;
}
.address input:focus { border-color: #58a6ff; }

.sites {
  display: flex; gap: 6px; overflow-x: auto; overflow-y: hidden;
  padding-bottom: 4px;
  scrollbar-width: thin;
  scrollbar-color: rgba(255, 255, 255, 0.18) transparent;
}
.sites::-webkit-scrollbar { height: 6px; }
.sites::-webkit-scrollbar-track { background: transparent; }
.sites::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.14);
  border-radius: 3px;
}
.sites::-webkit-scrollbar-thumb:hover { background: rgba(255, 255, 255, 0.28); }
.site {
  position: relative;
  display: flex; align-items: center; gap: 5px;
  padding: 5px 40px 5px 10px;
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.03);
  color: #c9d1d9;
  cursor: grab;
  font-size: 12px; white-space: nowrap;
  transition: all 0.15s;
}
.site:active { cursor: grabbing; }
.site.dragging { opacity: 0.4; }
.site.drag-over {
  border-color: #1f6feb;
  background: rgba(31, 111, 235, 0.15);
  transform: translateY(-1px);
}
.site:hover { background: rgba(255, 255, 255, 0.08); }
.site.active {
  background: rgba(31, 111, 235, 0.2);
  border-color: #1f6feb;
  color: #58a6ff;
}
.site .icon { font-size: 14px; }
.site-x {
  position: absolute;
  right: 4px;
  top: 50%;
  transform: translateY(-50%);
  width: 14px; height: 14px;
  line-height: 12px;
  text-align: center;
  border-radius: 50%;
  font-size: 12px;
  color: #6e7681;
  opacity: 0;
  transition: opacity 0.15s, background 0.15s;
}
.site:hover .site-x { opacity: 1; }
.site:hover .site-edit { opacity: 1; }
.site.editing { padding: 0; }
.site-name-input {
  width: 64px;
  height: 24px;
  margin: 2px 4px;
  padding: 0 6px;
  border: 1px solid #58a6ff;
  border-radius: 10px;
  background: rgba(13, 17, 23, 0.8);
  color: #e6edf3;
  font-size: 12px;
  outline: none;
}
.site-edit {
  position: absolute;
  right: 22px;
  top: 50%;
  transform: translateY(-50%);
  width: 14px; height: 14px;
  line-height: 12px;
  text-align: center;
  border-radius: 3px;
  font-size: 11px;
  color: #6e7681;
  opacity: 0;
  transition: opacity 0.15s, color 0.15s, background 0.15s;
}
.site-edit:hover {
  background: rgba(88, 166, 255, 0.25);
  color: #58a6ff;
}
.site-x:hover {
  background: rgba(255, 100, 100, 0.3);
  color: #f85149;
}

.content {
  flex: 1; padding: 6px 10px 10px; min-height: 0;
}
.content.fs-content {
  /* 全屏也留窄边:inline 靠 content 的 10px padding 露出主窗边缘做 WM_NCHITTEST 缩放,
     全屏铺满会让独立子窗盖到边缘、主窗收不到命中。右/下/左留 8px(≥8 物理像素缩放带),
     顶部靠 36px 标题栏(可拖动)。 */
  padding: 0 8px 8px 8px;
}
.shell.is-fullscreen .web-holder {
  border-radius: 0;
  border: none;
}
.web-holder {
  width: 100%; height: 100%;
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.25);
  border: 1px solid rgba(255, 255, 255, 0.04);
  display: flex; align-items: center; justify-content: center;
  overflow: hidden;
}
.hero { text-align: center; }
.hero-logo { display: block; margin: 0 auto 14px; border-radius: 20px; box-shadow: 0 14px 36px -12px rgba(47,111,240,.6); }
.hero h1 { margin: 0 0 8px; font-size: 32px; font-weight: 600; }
.tag { margin: 0 0 6px; color: #8b949e; font-size: 13px; }
.tip { margin: 12px 0 16px; color: #6e7681; font-size: 11px; }

.btn.big {
  height: 36px;
  padding: 0 20px;
  font-size: 13px;
  border-radius: 8px;
}

.fs-back {
  position: absolute;
  top: 44px;
  right: 12px;
  z-index: 20;
  height: 30px;
  padding: 0 14px;
  border: none;
  border-radius: 15px;
  background: rgba(31, 111, 235, 0.85);
  color: #fff;
  cursor: pointer;
  font-size: 12px;
  font-weight: 500;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
  backdrop-filter: blur(8px);
  transition: background 0.15s;
}
.fs-back:hover { background: #388bfd; }

/* 无边框窗口边缘拉伸手柄(fixed 定位在 .shell 外,避免被 shell 圆角+overflow 裁剪命中区)
   背景给极低 alpha:主窗口 transparent+border-radius,完全透明像素在某些环境下命中不稳,
   近乎不可见的底色保证整块区域稳定接收鼠标事件。 */
.resize-edge, .resize-corner {
  position: fixed;
  z-index: 9999;
  background: rgba(255, 255, 255, 0.02);
  pointer-events: auto;
}
.resize-edge.n  { top: 0; left: 8px; right: 8px; height: 6px; cursor: n-resize; }
.resize-edge.s  { bottom: 0; left: 8px; right: 8px; height: 6px; cursor: s-resize; }
.resize-edge.w  { left: 0; top: 8px; bottom: 8px; width: 6px; cursor: w-resize; }
.resize-edge.e  { right: 0; top: 8px; bottom: 8px; width: 6px; cursor: e-resize; }
.resize-corner.nw { top: 0; left: 0; width: 14px; height: 14px; cursor: nw-resize; }
.resize-corner.ne { top: 0; right: 0; width: 14px; height: 14px; cursor: ne-resize; }
.resize-corner.sw { bottom: 0; left: 0; width: 14px; height: 14px; cursor: sw-resize; }
.resize-corner.se { bottom: 0; right: 0; width: 14px; height: 14px; cursor: se-resize; }

/* 启动打赏弹窗 */
.ds-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.55);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 10000;
  backdrop-filter: blur(2px);
}
.ds-dialog {
  position: relative;
  width: 420px;
  max-width: calc(100% - 32px);
  background: #1b2230;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 14px;
  padding: 22px 22px 18px;
  box-shadow: 0 18px 60px rgba(0, 0, 0, 0.5);
  color: #e6edf3;
}
.ds-x {
  position: absolute;
  top: 12px; right: 12px;
  background: transparent;
  border: none;
  color: #8b949e;
  font-size: 15px;
  cursor: pointer;
  line-height: 1;
  padding: 4px;
}
.ds-x:hover { color: #f0f6fc; }
.ds-title { margin: 0 0 6px; font-size: 18px; text-align: center; }
.ds-sub { margin: 0 0 18px; font-size: 12px; color: #8b949e; text-align: center; }
.ds-qrs {
  display: flex;
  gap: 18px;
  justify-content: center;
}
.ds-qr {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: #c9d1d9;
}
.ds-qr img {
  width: 150px;
  height: 150px;
  border-radius: 8px;
  background: #fff;
  object-fit: contain;
}
.ds-never {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 16px 0 12px;
  font-size: 12px;
  color: #8b949e;
  cursor: pointer;
}
.ds-never input { accent-color: #58a6ff; }
.ds-close { width: 100%; padding: 8px; }

/* 更新弹窗按钮:flex 居中,避免文字偏移 */
.fu-dialog .fu-update-btn,
.fu-dialog .fu-later-btn {
  width: 100%;
  height: 34px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  margin-top: 10px;
}

/* 更新弹窗 */
.fu-dialog { width: 380px; }
.fu-notes {
  margin: 0 0 18px;
  padding-left: 20px;
  font-size: 13px;
  color: #c9d1d9;
  line-height: 1.7;
}
.fu-notes li { list-style: disc; }
</style>
