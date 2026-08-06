<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount, computed } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { listen, emit as emitGlobal, type UnlistenFn } from "@tauri-apps/api/event";
import { PET_BUBBLE_H, clampPetSize } from "../stores/pet";

import lopcatMain from "../images/奶白垂耳猫.png";
import lopcat01 from "../images/奶白垂耳猫01.png";
import lopcat02 from "../images/奶白垂耳猫02.png";
import lopcat03 from "../images/奶白垂耳猫03.png";
import lopcat04 from "../images/奶白垂耳猫04.png";
import lopcat05 from "../images/奶白垂耳猫05.png";

import psyduckMain from "../images/黄色可达鸭.png";
import psyduck01 from "../images/黄色可达鸭01.png";
import psyduck02 from "../images/黄色可达鸭02.png";
import psyduck03 from "../images/黄色可达鸭03.png";
import psyduck04 from "../images/黄色可达鸭04.png";
import psyduck05 from "../images/黄色可达鸭05.png";

import hamsterMain from "../images/雾灰蓝仓鼠.png";
import hamster01 from "../images/雾灰蓝仓鼠01.png";
import hamster02 from "../images/雾灰蓝仓鼠02.png";
import hamster03 from "../images/雾灰蓝仓鼠03.png";
import hamster04 from "../images/雾灰蓝仓鼠04.png";
import hamster05 from "../images/雾灰蓝仓鼠05.png";

import corgiMain from "../images/焦糖柯基.png";
import corgi01 from "../images/焦糖柯基01.png";
import corgi02 from "../images/焦糖柯基02.png";
import corgi03 from "../images/焦糖柯基03.png";
import corgi04 from "../images/焦糖柯基04.png";
import corgi05 from "../images/焦糖柯基05.png";

/**
 * M8 桌面宠物(独立窗口 ?view=pet)。
 * 本窗口不挂 pinia:配置由主窗通过 URL query 传入,保持窗口轻量;
 * 窗口内改动(缩放/置顶)通过全局事件回传主窗持久化。
 *
 * 窗口高度 = 本体边长 + PET_BUBBLE_H:
 * 顶部那段留白专门给气泡用 —— 窗口只有本体大时气泡会被窗口边界裁掉。
 *
 * 互动:
 *   - 左键按住拖动 → startDragging,放到哪停在哪(不自动归位)
 *   - 单击 → 轮流播放 跳跃 / 压扁回弹 / 左右抖动,并随机说一句
 *   - 双击 → 退出桌宠、恢复摸鱼窗口
 *   - 右键 → 独立菜单窗口:调整大小 / 置顶开关 / 退出
 *   - 滚轮 → 连续缩放(底边中点为锚点,由 Rust 侧完成)
 *   - 鼠标移动 → 仅视线跟随,绝不移动窗口
 *   - 打字反应 → 主进程 pet-typing 驱动(仅 Windows,只知道"有没有在打字")
 */

type PetKind = "lopcat" | "psyduck" | "hamster" | "corgi";
/** 持续态 */
type Mood = "idle" | "sleep" | "typing";
/** 瞬时动作:点击时按序轮换 */
type PetAction = "" | "jump" | "squash" | "shake";
type Facing = "left" | "right";

const win = getCurrentWindow();
const params = new URLSearchParams(window.location.search);

const KIND_LIST: PetKind[] = ["lopcat", "psyduck", "hamster", "corgi"];
const rawKind = (params.get("kind") || "lopcat") as PetKind;
// 归一化:旧版本 kind(cat/capy/shiba/duck...)经 HMR 残留在窗口里时,
// 新模板的 v-if 链不认识它们会渲染空白 → 一律回退默认角色,保证不透明。
const kind: PetKind = KIND_LIST.includes(rawKind) ? rawKind : "lopcat";
const followMouse = params.get("follow") === "1";
const wander = params.get("wander") === "1";
/** 本体边长;滚轮/右键菜单会改它,窗口尺寸同步跟着变 */
const bodySize = ref(clampPetSize(Number(params.get("size") || 140)));
const onTop = ref(params.get("top") !== "0");
const lines: string[] = (() => {
  try {
    const raw = params.get("lines");
    if (!raw) return [];
    // params.get 已完成一次解码,不能再 decodeURIComponent(台词含中文会解坏)
    const arr = JSON.parse(raw);
    return Array.isArray(arr) ? arr.filter((x) => typeof x === "string" && x.trim()) : [];
  } catch {
    return [];
  }
})();

const mood = ref<Mood>("idle");
const action = ref<PetAction>("");
const facing = ref<Facing>("right");
const bubble = ref("");
const isDragging = ref(false);
/** 鼠标靠近时为 true:垂耳猫会歪头,其它角色用来放大眨眼等反应 */
const near = ref(false);


/** 点击动作轮换顺序 —— 每次点击取下一个,而不是随机,让人感觉"逗它有反应" */
const CLICK_ACTIONS: Exclude<PetAction, "">[] = ["jump", "squash", "shake"];
const ACTION_MS: Record<Exclude<PetAction, "">, number> = { jump: 620, squash: 520, shake: 520 };
let clickIdx = 0;

const PET_IMAGES: Record<PetKind, { main: string; exprs: string[] }> = {
  lopcat: {
    main: lopcatMain,
    exprs: [lopcat01, lopcat02, lopcat03, lopcat04, lopcat05],
  },
  psyduck: {
    main: psyduckMain,
    exprs: [psyduck01, psyduck02, psyduck03, psyduck04, psyduck05],
  },
  hamster: {
    main: hamsterMain,
    exprs: [hamster01, hamster02, hamster03, hamster04, hamster05],
  },
  corgi: {
    main: corgiMain,
    exprs: [corgi01, corgi02, corgi03, corgi04, corgi05],
  },
};

const activeExprIdx = ref<number>(-1);
const randomIdleExprIdx = ref<number>(-1);
let idleExprTimer: number | undefined;

// ── 图片双层渐变切换系统 ──
function getTargetSrc(): string {
  const data = PET_IMAGES[kind] || PET_IMAGES.lopcat;
  if (mood.value === "sleep") return data.exprs[4] || data.main;
  if (mood.value === "typing") return data.exprs[1] || data.main;
  if (isDragging.value) return data.exprs[2] || data.main;
  if (action.value === "jump") return data.exprs[0] || data.main;
  if (action.value === "squash") return data.exprs[2] || data.main;
  if (action.value === "shake") return data.exprs[3] || data.main;
  if (activeExprIdx.value >= 0 && data.exprs[activeExprIdx.value]) return data.exprs[activeExprIdx.value];
  if (near.value && data.exprs[0]) return data.exprs[0];
  if (randomIdleExprIdx.value >= 0 && data.exprs[randomIdleExprIdx.value]) return data.exprs[randomIdleExprIdx.value];
  return data.main;
}

const _initSrc = getTargetSrc();
const imgSrcA = ref(_initSrc);
const imgSrcB = ref(_initSrc);
const topLayer = ref<0 | 1>(0);

function switchTo(newSrc: string) {
  const curSrc = topLayer.value === 0 ? imgSrcA.value : imgSrcB.value;
  if (curSrc === newSrc) return;
  if (topLayer.value === 0) {
    imgSrcB.value = newSrc;
    window.requestAnimationFrame(() => { topLayer.value = 1; });
  } else {
    imgSrcA.value = newSrc;
    window.requestAnimationFrame(() => { topLayer.value = 0; });
  }
}

watch([mood, action, isDragging, activeExprIdx, randomIdleExprIdx, near], () => {
  switchTo(getTargetSrc());
});

let bubbleTimer: number | undefined;
let actionTimer: number | undefined;
let wanderTimer: number | undefined;
let idleTimer: number | undefined;
let imgSwitchTimer: number | undefined;
/** 拖动判定:按下位置,位移超过阈值才算拖动而非点击 */
let downAt: { x: number; y: number } | null = null;
const DRAG_THRESHOLD = 4;
/** 滚轮缩放节流:窗口 set_size 是 IPC,连续滚会堆积 */
let wheelPending = false;

const unlistens: UnlistenFn[] = [];

const rootStyle = computed(() => ({
  width: bodySize.value + "px",
  height: bodySize.value + PET_BUBBLE_H + "px",
}));
const stageStyle = computed(() => ({
  width: bodySize.value + "px",
  height: bodySize.value + "px",
}));



/** 气泡单条上限:留白区约能排四行,超长的自定义台词截断而非硬裁一半 */
const BUBBLE_MAX_CHARS = 26;

function say(text: string, ms = 2600) {
  if (!text) return;
  const t = text.length > BUBBLE_MAX_CHARS ? text.slice(0, BUBBLE_MAX_CHARS - 1) + "…" : text;
  bubble.value = t;
  if (bubbleTimer) window.clearTimeout(bubbleTimer);
  bubbleTimer = window.setTimeout(() => { bubble.value = ""; }, ms);
}

/** 播放一段瞬时动作;结束后自动清空,不影响 idle/typing/sleep 持续态 */
function playAction(a: Exclude<PetAction, "">) {
  if (actionTimer) window.clearTimeout(actionTimer);
  if (imgSwitchTimer) window.clearTimeout(imgSwitchTimer);
  // 同一个动作连点时先清空一帧,否则 CSS 动画不会重新触发
  action.value = "";
  window.requestAnimationFrame(() => {
    action.value = a;
    actionTimer = window.setTimeout(() => {
      action.value = "";
      // 动作结束后稍作延迟再回到主图,避免表情闪烁
      imgSwitchTimer = window.setTimeout(() => {
        activeExprIdx.value = -1;
      }, 280);
    }, ACTION_MS[a]);
  });
}

/** 空闲久了打瞌睡;任何互动都会重置 */
function resetIdle() {
  if (idleTimer) window.clearTimeout(idleTimer);
  if (mood.value === "sleep") mood.value = "idle";
  idleTimer = window.setTimeout(() => {
    if (mood.value === "idle") mood.value = "sleep";
  }, 45000);
}

function onPointerDown(e: PointerEvent) {
  if (e.button !== 0) return;
  downAt = { x: e.screenX, y: e.screenY };
  isDragging.value = false;
  resetIdle();
}

function onPointerMove(e: PointerEvent) {
  if (!downAt || isDragging.value) return;
  const moved =
    Math.abs(e.screenX - downAt.x) > DRAG_THRESHOLD ||
    Math.abs(e.screenY - downAt.y) > DRAG_THRESHOLD;
  if (moved) {
    isDragging.value = true;
    // 交给系统拖动:此后本 webview 不再收到 move/up,收尾在 pointercancel/blur
    win.startDragging().catch(() => {});
  }
}

/** 拖动结束:位置保持用户松手处,不做任何自动归位 */
function endDrag() {
  if (isDragging.value) {
    isDragging.value = false;
    downAt = null;
    playAction("jump");
    return;
  }
  downAt = null;
}

function onClick() {
  if (isDragging.value) return;
  resetIdle();
  playAction(CLICK_ACTIONS[clickIdx % CLICK_ACTIONS.length]);
  activeExprIdx.value = clickIdx % 5;
  clickIdx++;
  if (lines.length) {
    say(lines[Math.floor(Math.random() * lines.length)]);
  }
}

async function onDblClick() {
  await invoke("exit_pet_mode").catch(() => {});
}

/** 应用新尺寸:先改窗口(Rust 保持底边中点不动),再改内部布局 */
async function applySize(next: number) {
  const n = clampPetSize(next);
  if (n === bodySize.value) return;
  bodySize.value = n;
  await invoke("set_pet_size", { width: n, height: n + PET_BUBBLE_H }).catch(() => {});
  // 回传主窗持久化,下次开桌宠沿用这个尺寸
  await emitGlobal("pet-size-changed", n).catch(() => {});
}

function onWheel(e: WheelEvent) {
  e.preventDefault();
  if (wheelPending) return;
  wheelPending = true;
  window.requestAnimationFrame(() => { wheelPending = false; });
  // 向上滚放大;每格约 10px,一档一档来手感比按比例缩放稳
  const step = e.deltaY < 0 ? 10 : -10;
  resetIdle();
  applySize(bodySize.value + step);
}

/** 右键菜单:复用主窗已验证的独立菜单窗口(键名加 pet: 前缀,免得被主窗当成 tab 操作) */
async function onContextMenu(e: MouseEvent) {
  // 不阻止的话会弹出 webview 自带菜单,立刻穿帮
  e.preventDefault();
  resetIdle();
  const items = [
    { key: "pet:size:96",  label: bodySize.value === 96  ? "● 小" : "小",  icon: "▫" },
    { key: "pet:size:140", label: bodySize.value === 140 ? "● 中" : "中",  icon: "▫" },
    { key: "pet:size:190", label: bodySize.value === 190 ? "● 大" : "大",  icon: "▫" },
    { key: "pet:d1", label: "", divider: true },
    { key: "pet:top", label: onTop.value ? "取消置顶" : "窗口置顶", icon: "📌" },
    { key: "pet:d2", label: "", divider: true },
    { key: "pet:exit", label: "退出桌宠", icon: "✕" },
  ];
  const w = 150;
  // 尺寸预算要按 ContextMenuStandalone 的实际样式算:普通项 28px + 间隙,divider 9px
  const h = items.reduce((s, it) => s + (it.divider ? 9 : 30), 0) + 10;
  try {
    // 桌宠窗无边框,outer 与 inner 重合,直接用 innerPosition 换算屏幕坐标
    const scale = window.devicePixelRatio || 1;
    const pos = await win.innerPosition();
    const screenX = pos.x / scale + e.clientX;
    const screenY = pos.y / scale + e.clientY;
    const maxX = window.screen.availWidth - w - 4;
    const maxY = window.screen.availHeight - h - 4;
    await invoke("show_context_menu", {
      x: Math.max(0, Math.min(screenX, maxX)),
      y: Math.max(0, Math.min(screenY, maxY)),
      width: w,
      height: h,
      data: encodeURIComponent(JSON.stringify(items)),
    });
  } catch {
    /* 菜单开不出来不影响其它互动 */
  }
}

async function onMenuPick(key: string) {
  if (!key.startsWith("pet:")) return;
  if (key === "pet:exit") {
    await invoke("exit_pet_mode").catch(() => {});
    return;
  }
  if (key === "pet:top") {
    onTop.value = !onTop.value;
    await invoke("set_pet_always_on_top", { on: onTop.value }).catch(() => {});
    await emitGlobal("pet-top-changed", onTop.value).catch(() => {});
    say(onTop.value ? "我盯着最上面" : "我躲后面去了", 1800);
    return;
  }
  const m = /^pet:size:(\d+)$/.exec(key);
  if (m) await applySize(Number(m[1]));
}

/** 自主漫步:只在原地转个向,不移动窗口(位置由用户决定) */
function tickWander() {
  if (!wander || isDragging.value || mood.value === "sleep") return;
  if (Math.random() > 0.3) return;
  facing.value = facing.value === "right" ? "left" : "right";
}

/**
 * 由 pet-cursor 驱动:更新朝向与鼠标靠近状态,切换对应表情。
 * 位置完全由用户拖拽决定 —— 宠物自己乱跑会挡住桌面操作。
 */
async function applyEyeTrack(cx: number, cy: number) {
  if (!followMouse) return;
  try {
    const scale = window.devicePixelRatio || 1;
    const pos = await win.innerPosition();
    const centerX = pos.x / scale + bodySize.value / 2;
    const centerY = pos.y / scale + PET_BUBBLE_H + bodySize.value / 2;
    const dx = cx / scale - centerX;
    const dy = cy / scale - centerY;
    if (dx > 12) facing.value = "right";
    else if (dx < -12) facing.value = "left";
    const dist = Math.hypot(dx, dy);
    near.value = dist < bodySize.value * 0.6;
  } catch {
    /* ignore */
  }
}

function startIdleExprLoop() {
  idleExprTimer = window.setInterval(() => {
    if (mood.value === "idle" && !near.value && !action.value && !isDragging.value) {
      randomIdleExprIdx.value = Math.floor(Math.random() * 4);
      setTimeout(() => {
        randomIdleExprIdx.value = -1;
      }, 2200);
    }
  }, 8500);
}

onMounted(async () => {
  resetIdle();
  wanderTimer = window.setInterval(tickWander, 6000);
  startIdleExprLoop();

  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", endDrag);
  window.addEventListener("pointercancel", endDrag);
  window.addEventListener("blur", endDrag);
  window.addEventListener("wheel", onWheel, { passive: false });

  unlistens.push(
    await listen<{ x: number; y: number }>("pet-cursor", (e) => {
      applyEyeTrack(e.payload.x, e.payload.y);
    }),
  );
  unlistens.push(
    await listen<boolean>("pet-typing", (e) => {
      if (e.payload) {
        resetIdle();
        mood.value = "typing";
      } else if (mood.value === "typing") {
        mood.value = "idle";
      }
    }),
  );
  unlistens.push(
    await listen<string>("pet-say", (e) => {
      resetIdle();
      say(e.payload);
    }),
  );
  unlistens.push(await listen<string>("ctx-menu-pick", (e) => { onMenuPick(e.payload); }));
  // 设置页改了尺寸 → 桌宠已开着时即时生效(否则要重开才看得到)
  unlistens.push(
    await listen<number>("pet-size-push", (e) => {
      const n = clampPetSize(e.payload);
      if (n === bodySize.value) return;
      bodySize.value = n;
      invoke("set_pet_size", { width: n, height: n + PET_BUBBLE_H }).catch(() => {});
    }),
  );
});

onBeforeUnmount(() => {
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", endDrag);
  window.removeEventListener("pointercancel", endDrag);
  window.removeEventListener("blur", endDrag);
  window.removeEventListener("wheel", onWheel);
  if (bubbleTimer) window.clearTimeout(bubbleTimer);
  if (actionTimer) window.clearTimeout(actionTimer);
  if (idleTimer) window.clearTimeout(idleTimer);
  if (wanderTimer) window.clearInterval(wanderTimer);
  if (imgSwitchTimer) window.clearTimeout(imgSwitchTimer);
  if (idleExprTimer) window.clearInterval(idleExprTimer);
  unlistens.forEach((u) => u());
});
</script>

<template>
  <div
    class="pet-root"
    :style="rootStyle"
    :class="[`m-${mood}`, `f-${facing}`, `k-${kind}`,
      action && `a-${action}`, { 'is-drag': isDragging, 'is-near': near }]"
    @pointerdown="onPointerDown"
    @click="onClick"
    @dblclick="onDblClick"
    @contextmenu="onContextMenu"
  >
    <!-- 气泡:画在顶部留白区,不与本体重叠 -->
    <div v-if="bubble" class="pet-bubble">{{ bubble }}</div>

    <div class="pet-stage" :style="stageStyle">
      <div class="pet-body">
        <!-- 地面阴影 -->
        <div class="pet-shadow-el"></div>

        <!-- 双层图片渐变切换:避免表情切换时闪烁 -->
        <div class="pet-img-wrap">
          <img :src="imgSrcA" :alt="kind" class="pet-img" :class="{ 'img-top': topLayer === 0 }" />
          <img :src="imgSrcB" :alt="kind" class="pet-img" :class="{ 'img-top': topLayer === 1 }" />
        </div>

        <!-- 睡觉 zzz 标记 -->
        <div v-if="mood === 'sleep'" class="zzz-box">
          <span class="zzz">z</span>
          <span class="zzz z2">z</span>
        </div>

        <!-- ══════ 打字特效:键盘 + 交替拍打的爪子(所有宠物共用) ══════ -->
        <svg viewBox="0 0 100 100" class="pet-typing-fx">
          <g class="fx-kb">
            <!-- 键盘本体:横在身前,压得比脸低,不挡表情 -->
            <path d="M18 88 h64 l5 8 h-74 Z" fill="#4a5563" stroke="#2f3743" stroke-width="1.8" stroke-linejoin="round" />
            <g fill="#8fa0b3">
              <rect x="24" y="90" width="7" height="3" rx="1.2" />
              <rect x="34" y="90" width="7" height="3" rx="1.2" />
              <rect x="44" y="90" width="7" height="3" rx="1.2" />
              <rect x="54" y="90" width="7" height="3" rx="1.2" />
              <rect x="64" y="90" width="7" height="3" rx="1.2" />
            </g>
            <!-- 敲击闪光 -->
            <circle class="fx-spark s1" cx="36" cy="88" r="2.6" fill="#ffe27a" />
            <circle class="fx-spark s2" cx="64" cy="88" r="2.6" fill="#7fe0ff" />
          </g>
          <!-- 两只爪子交替拍(柔和无粗黑边) -->
          <ellipse class="fx-paw paw-l" cx="36" cy="86" rx="5.8" ry="3.8" stroke="rgba(0, 0, 0, 0.15)" stroke-width="0.8" />
          <ellipse class="fx-paw paw-r" cx="64" cy="86" rx="5.8" ry="3.8" stroke="rgba(0, 0, 0, 0.15)" stroke-width="0.8" />
          <!-- 飞出的音符,表示手速飞快 -->
          <text class="fx-note n1" x="16" y="58" font-size="12" fill="#ffcf47">♪</text>
          <text class="fx-note n2" x="80" y="52" font-size="10" fill="#7fd8f0">✦</text>
          <text class="fx-note n3" x="76" y="70" font-size="9" fill="#ffa8bd">♪</text>
        </svg>
      </div>
    </div>
  </div>
</template>

<style>
/* 桌宠窗口整体透明:不能有任何背景色,否则会看到一个方块 */
html, body, #app {
  margin: 0;
  padding: 0;
  background: transparent !important;
  overflow: hidden;
  width: 100%;
  height: 100%;
}
</style>

<style scoped>
/* ── 根容器 ── */
.pet-root {
  position: relative;
  background: transparent;
  user-select: none;
  cursor: grab;
  overflow: visible;
  -webkit-user-select: none;
}
.pet-root:active { cursor: grabbing; }

/* ── 本体舞台 ── */
.pet-stage {
  position: absolute;
  left: 0;
  bottom: 0;
}
.pet-body {
  position: relative;
  width: 100%;
  height: 100%;
  transform-origin: 50% 92%;
  animation: breathe 3s ease-in-out infinite;
  will-change: transform;
}

/* ── 图片双层渐变切换系统 ── */
.pet-img-wrap {
  position: relative;
  width: 100%;
  height: 100%;
}
.pet-img {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: contain;
  opacity: 0;
  transition: opacity 0.28s ease-in-out, transform 0.2s ease-out;
  pointer-events: none;
  will-change: opacity, transform;
}
.pet-img.img-top { opacity: 1; }

/* ── 地面阴影(随呼吸同步缩放) ── */
.pet-shadow-el {
  position: absolute;
  bottom: 1px;
  left: 50%;
  transform: translateX(-50%);
  width: 62%;
  height: 10px;
  background: rgba(0, 0, 0, 0.16);
  border-radius: 50%;
  filter: blur(5px);
  pointer-events: none;
  animation: shadow-pulse 3s ease-in-out infinite;
}
@keyframes shadow-pulse {
  0%, 100% { transform: translateX(-50%) scaleX(1);    opacity: 0.16; }
  40%      { transform: translateX(-50%) scaleX(0.96); opacity: 0.12; }
}

/* ── 朝向翻转 ── */
.f-left .pet-img { transform: scaleX(-1); }
/* 打字时强制正面朝向:键盘 SVG 不随翻转 */
.f-left.m-typing .pet-img { transform: none; }

/* ── 睡觉 ZZZ ── */
.zzz-box {
  position: absolute;
  top: 6%;
  right: 8%;
  pointer-events: none;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
}
.zzz {
  font-weight: 900;
  font-family: "Microsoft YaHei", "PingFang SC", sans-serif;
  color: #7aaad4;
  text-shadow: 0 1px 4px rgba(0, 0, 0, 0.18);
  font-size: 16px;
  opacity: 0;
  animation: zfloat 2.4s ease-out infinite;
}
.zzz.z2 {
  animation-delay: 1s;
  font-size: 12px;
  color: #9abfe0;
}
@keyframes zfloat {
  0%   { opacity: 0;   transform: translate(0, 4px) scale(0.55); }
  25%  { opacity: 0.9; }
  80%  { opacity: 0.3; }
  100% { opacity: 0;   transform: translate(5px, -18px) scale(1.1); }
}

/* ── 呼吸待机 ── */
@keyframes breathe {
  0%, 100% { transform: scaleY(1)     scaleX(1); }
  40%      { transform: scaleY(1.03)  scaleX(0.988); }
  70%      { transform: scaleY(0.997) scaleX(1.002); }
}
.m-sleep .pet-body { animation: breathe 5.5s ease-in-out infinite; }

/* ── 打字键盘 + 爪子 + 飞舞音符特效 ── */
.pet-typing-fx {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  opacity: 0;
  pointer-events: none;
  filter: drop-shadow(0 2px 4px rgba(0, 0, 0, 0.2));
  transition: opacity 0.2s ease-out;
  z-index: 5;
}
.m-typing .pet-typing-fx { opacity: 1; }

.m-typing .fx-kb { animation: kb-in 0.25s cubic-bezier(0.34, 1.56, 0.64, 1); }
@keyframes kb-in {
  0%   { transform: translateY(12px); opacity: 0; }
  100% { transform: translateY(0); opacity: 1; }
}

.fx-paw {
  animation: tap 0.2s ease-in-out infinite alternate;
  transform-origin: 50% 100%;
  fill: var(--paw, #fdfcfa);
}
.fx-paw.paw-r { animation-delay: 0.1s; }
@keyframes tap {
  0%   { transform: translateY(-5px) rotate(-6deg); }
  100% { transform: translateY(0) rotate(2deg); }
}

.fx-spark { opacity: 0; animation: spark 0.4s ease-out infinite; }
.fx-spark.s2 { animation-delay: 0.2s; }
@keyframes spark {
  0%   { opacity: 0.9; transform: scale(0.4); }
  100% { opacity: 0; transform: scale(1.5); }
}

.fx-note { opacity: 0; animation: note-fly 1.5s ease-out infinite; }
.fx-note.n2 { animation-delay: 0.5s; }
.fx-note.n3 { animation-delay: 1.0s; }
@keyframes note-fly {
  0%   { opacity: 0; transform: translate(0, 4px) scale(0.6) rotate(0deg); }
  30%  { opacity: 1; }
  100% { opacity: 0; transform: translate(-6px, -20px) scale(1.15) rotate(-16deg); }
}

/* 各角色打字爪子颜色 */
.k-lopcat  { --paw: #ffffff; }
.k-psyduck { --paw: #ffe978; }
.k-hamster { --paw: #f4d0c1; }
.k-corgi   { --paw: #fff8ee; }

/* ── 打字状态 ── */
.m-typing .pet-body { animation: nod 0.45s ease-in-out infinite; }
@keyframes nod {
  0%, 100% { transform: translateY(0); }
  50%      { transform: translateY(-4px) scaleY(0.98); }
}

/* ── 点击互动动作 ── */
.a-jump .pet-body { animation: act-jump 0.62s cubic-bezier(0.3, 1.4, 0.5, 1); }
@keyframes act-jump {
  0%   { transform: translateY(0) scaleY(1); }
  18%  { transform: translateY(3px) scaleY(0.88) scaleX(1.08); }
  46%  { transform: translateY(-22px) scaleY(1.08) scaleX(0.94); }
  74%  { transform: translateY(0) scaleY(0.94) scaleX(1.04); }
  100% { transform: translateY(0) scaleY(1) scaleX(1); }
}

.a-squash .pet-body { animation: act-squash 0.52s cubic-bezier(0.25, 1.5, 0.4, 1); }
@keyframes act-squash {
  0%   { transform: scaleY(1) scaleX(1); }
  28%  { transform: scaleY(0.72) scaleX(1.2); }
  58%  { transform: scaleY(1.12) scaleX(0.92); }
  80%  { transform: scaleY(0.97) scaleX(1.02); }
  100% { transform: scaleY(1) scaleX(1); }
}

.a-shake .pet-body { animation: act-shake 0.52s ease-in-out; }
@keyframes act-shake {
  0%, 100% { transform: rotate(0deg); }
  15%      { transform: rotate(-9deg); }
  38%      { transform: rotate(8deg); }
  62%      { transform: rotate(-6deg); }
  85%      { transform: rotate(3deg); }
}

/* ── 角色特有待机晃动 ── */
.k-lopcat.is-near:not(.a-jump):not(.a-squash):not(.a-shake):not(.m-typing):not(.m-sleep) .pet-body {
  animation: lop-tilt 1.8s ease-in-out infinite;
}
@keyframes lop-tilt {
  0%, 100% { transform: rotate(0deg); }
  50%      { transform: rotate(-5deg); }
}

.k-psyduck:not(.a-jump):not(.a-squash):not(.a-shake):not(.m-typing):not(.m-sleep) .pet-body {
  animation: duck-wobble 3s ease-in-out infinite; transform-origin: 50% 92%;
}
@keyframes duck-wobble {
  0%, 100% { transform: rotate(0deg); }
  25%      { transform: rotate(-4deg); }
  75%      { transform: rotate(4deg); }
}

.k-corgi:not(.a-jump):not(.a-squash):not(.a-shake):not(.m-typing):not(.m-sleep) .pet-body {
  animation: corgi-tilt 5s ease-in-out infinite; transform-origin: 50% 92%;
}
@keyframes corgi-tilt {
  0%, 88%, 100% { transform: rotate(0deg); }
  94%           { transform: rotate(6deg); }
}

/* ── 对话气泡 ── */
.pet-bubble {
  position: absolute;
  left: 50%;
  bottom: v-bind('bodySize + 4 + "px"');
  transform: translateX(-50%);
  width: max-content;
  max-width: 100%;
  padding: 6px 10px;
  box-sizing: border-box;
  background: #ffffff;
  color: #26303d;
  font-size: 11px;
  line-height: 1.35;
  font-family: "Microsoft YaHei", "PingFang SC", sans-serif;
  border: 1.5px solid #33302e;
  border-radius: 9px;
  box-shadow: 0 3px 6px rgba(0, 0, 0, 0.15);
  word-break: normal;
  overflow-wrap: break-word;
  text-align: center;
  animation: pop 0.2s cubic-bezier(0.34, 1.56, 0.64, 1);
  pointer-events: none;
  z-index: 10;
}
.pet-bubble::before,
.pet-bubble::after {
  content: "";
  position: absolute;
  left: 50%;
  top: 100%;
  transform: translateX(-50%);
  border: 6px solid transparent;
}
.pet-bubble::before {
  border-top-color: #33302e;
  margin-top: 1.5px;
}
.pet-bubble::after {
  border-top-color: #ffffff;
  border-width: 5px;
}
@keyframes pop {
  0%   { opacity: 0; transform: translateX(-50%) translateY(6px) scale(0.85); }
  100% { opacity: 1; transform: translateX(-50%) translateY(0) scale(1); }
}

@media (prefers-reduced-motion: reduce) {
  .pet-body, .pet-shadow-el, .zzz {
    animation: none !important;
  }
}
</style>
