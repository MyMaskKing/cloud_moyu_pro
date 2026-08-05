<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, computed } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { listen, emit as emitGlobal, type UnlistenFn } from "@tauri-apps/api/event";
import { PET_BUBBLE_H, clampPetSize } from "../stores/pet";

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

type PetKind = "cat" | "capy" | "shiba" | "duck";
/** 持续态 */
type Mood = "idle" | "sleep" | "typing";
/** 瞬时动作:点击时按序轮换 */
type PetAction = "" | "jump" | "squash" | "shake";
type Facing = "left" | "right";

const win = getCurrentWindow();
const params = new URLSearchParams(window.location.search);

const kind = (params.get("kind") || "cat") as PetKind;
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
/** 视线偏移,-1..1 */
const eyeX = ref(0);
const eyeY = ref(0);

/** 点击动作轮换顺序 —— 每次点击取下一个,而不是随机,让人感觉"逗它有反应" */
const CLICK_ACTIONS: Exclude<PetAction, "">[] = ["jump", "squash", "shake"];
const ACTION_MS: Record<Exclude<PetAction, "">, number> = { jump: 620, squash: 520, shake: 520 };
let clickIdx = 0;

let bubbleTimer: number | undefined;
let actionTimer: number | undefined;
let wanderTimer: number | undefined;
let idleTimer: number | undefined;
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

/** 眼球位移像素(SVG 100 视口内,最多偏 3.2) */
const eyeShift = computed(
  () => `translate(${(eyeX.value * 3.2).toFixed(2)} ${(eyeY.value * 3.2).toFixed(2)})`,
);

function say(text: string, ms = 2600) {
  if (!text) return;
  bubble.value = text;
  if (bubbleTimer) window.clearTimeout(bubbleTimer);
  bubbleTimer = window.setTimeout(() => { bubble.value = ""; }, ms);
}

/** 播放一段瞬时动作;结束后自动清空,不影响 idle/typing/sleep 持续态 */
function playAction(a: Exclude<PetAction, "">) {
  if (actionTimer) window.clearTimeout(actionTimer);
  // 同一个动作连点时先清空一帧,否则 CSS 动画不会重新触发
  action.value = "";
  window.requestAnimationFrame(() => {
    action.value = a;
    actionTimer = window.setTimeout(() => { action.value = ""; }, ACTION_MS[a]);
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
 * 由 pet-cursor 驱动:只做视线跟随,绝不移动窗口。
 * 位置完全由用户拖拽决定 —— 宠物自己乱跑会挡住桌面操作,也让人找不到它。
 */
async function applyEyeTrack(cx: number, cy: number) {
  if (!followMouse) return;
  try {
    const scale = window.devicePixelRatio || 1;
    const pos = await win.innerPosition();
    // 本体在窗口下半部分,视线中心要按本体算而不是按整窗算
    const centerX = pos.x / scale + bodySize.value / 2;
    const centerY = pos.y / scale + PET_BUBBLE_H + bodySize.value / 2;
    const dx = cx / scale - centerX;
    const dy = cy / scale - centerY;
    // 归一化到 -1..1(200px 外视为看到底)
    eyeX.value = Math.max(-1, Math.min(1, dx / 200));
    eyeY.value = Math.max(-1, Math.min(1, dy / 200));
    if (dx > 12) facing.value = "right";
    else if (dx < -12) facing.value = "left";
  } catch {
    /* ignore */
  }
}

onMounted(async () => {
  resetIdle();
  wanderTimer = window.setInterval(tickWander, 6000);

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
  unlistens.forEach((u) => u());
});
</script>

<template>
  <div
    class="pet-root"
    :style="rootStyle"
    :class="[`m-${mood}`, `f-${facing}`, action && `a-${action}`, { 'is-drag': isDragging }]"
    @pointerdown="onPointerDown"
    @click="onClick"
    @dblclick="onDblClick"
    @contextmenu="onContextMenu"
  >
    <!-- 气泡:画在顶部留白区,不与本体重叠 -->
    <div v-if="bubble" class="pet-bubble">{{ bubble }}</div>

    <div class="pet-stage" :style="stageStyle">
      <div class="pet-body">
        <!-- ══════ 白猫(BongoCat 风:粗描边 + 扁平填色) ══════ -->
        <svg v-if="kind === 'cat'" viewBox="0 0 100 100" class="pet-svg">
          <ellipse class="pet-shadow" cx="50" cy="96" rx="27" ry="3.4" />
          <g stroke="#33302e" stroke-width="2.4" stroke-linejoin="round" stroke-linecap="round">
            <!-- 尾巴 -->
            <path class="cat-tail" d="M74 84 q20 -2 15 -20" fill="none" />
            <!-- 身体 -->
            <path d="M28 92 q-3 -26 22 -26 q25 0 22 26 Z" fill="#fdfcfa" />
            <!-- 前爪 -->
            <ellipse cx="40" cy="90" rx="7" ry="4.6" fill="#fdfcfa" />
            <ellipse cx="60" cy="90" rx="7" ry="4.6" fill="#fdfcfa" />
            <!-- 耳朵 -->
            <path d="M26 38 q-2 -19 9 -20 q3 8 9 13 Z" fill="#fdfcfa" />
            <path d="M74 38 q2 -19 -9 -20 q-3 8 -9 13 Z" fill="#fdfcfa" />
            <path d="M30 34 q-1 -11 5 -12 q1 5 4 8 Z" fill="#ffb9c4" stroke="none" />
            <path d="M70 34 q1 -11 -5 -12 q-1 5 -4 8 Z" fill="#ffb9c4" stroke="none" />
            <!-- 头 -->
            <circle cx="50" cy="44" r="27" fill="#fdfcfa" />
          </g>
          <g :transform="eyeShift">
            <g class="eyes-open">
              <ellipse cx="39" cy="43" rx="4.4" ry="6" fill="#33302e" />
              <ellipse cx="61" cy="43" rx="4.4" ry="6" fill="#33302e" />
              <circle cx="40.6" cy="40.6" r="1.7" fill="#fff" />
              <circle cx="62.6" cy="40.6" r="1.7" fill="#fff" />
            </g>
            <g class="eyes-closed">
              <path d="M34 43 q5 4.5 10 0" fill="none" stroke="#33302e" stroke-width="2.4" stroke-linecap="round" />
              <path d="M56 43 q5 4.5 10 0" fill="none" stroke="#33302e" stroke-width="2.4" stroke-linecap="round" />
            </g>
            <g class="eyes-happy">
              <path d="M34 45 q5 -6 10 0" fill="none" stroke="#33302e" stroke-width="2.4" stroke-linecap="round" />
              <path d="M56 45 q5 -6 10 0" fill="none" stroke="#33302e" stroke-width="2.4" stroke-linecap="round" />
            </g>
          </g>
          <!-- 鼻 + ω 嘴 -->
          <path d="M47.6 52 h4.8 l-2.4 2.4 Z" fill="#ff9aa8" />
          <path d="M50 55 q-3 3.4 -5.6 0.4 M50 55 q3 3.4 5.6 0.4" fill="none" stroke="#33302e" stroke-width="1.7" stroke-linecap="round" />
          <!-- 胡须 -->
          <g stroke="#c9c3bd" stroke-width="1.4" stroke-linecap="round">
            <path d="M27 51 L13 48" /><path d="M27 55 L13 57" />
            <path d="M73 51 L87 48" /><path d="M73 55 L87 57" />
          </g>
          <text class="zzz" x="80" y="20" font-size="13" fill="#8fa8c0">z</text>
        </svg>

        <!-- ══════ 水豚(方头 + 一脸淡定 + 头顶橘子) ══════ -->
        <svg v-else-if="kind === 'capy'" viewBox="0 0 100 100" class="pet-svg">
          <ellipse class="pet-shadow" cx="50" cy="96" rx="28" ry="3.4" />
          <g stroke="#5a4032" stroke-width="2.4" stroke-linejoin="round" stroke-linecap="round">
            <!-- 身体(躺平感:宽而扁) -->
            <path d="M24 92 q-2 -22 26 -22 q28 0 26 22 Z" fill="#a87c58" />
            <ellipse cx="39" cy="90" rx="7" ry="4.4" fill="#966d4c" />
            <ellipse cx="61" cy="90" rx="7" ry="4.4" fill="#966d4c" />
            <!-- 耳朵(小圆) -->
            <ellipse cx="27" cy="34" rx="6.4" ry="5.4" fill="#966d4c" />
            <ellipse cx="73" cy="34" rx="6.4" ry="5.4" fill="#966d4c" />
            <!-- 头(圆角方,水豚的招牌轮廓) -->
            <rect x="23" y="24" width="54" height="44" rx="17" fill="#b98a63" />
            <!-- 口鼻区 -->
            <ellipse cx="50" cy="58" rx="15" ry="9" fill="#a67c56" />
          </g>
          <!-- 头顶橘子(温泉水豚梗) -->
          <g class="capy-orange">
            <circle cx="50" cy="18" r="7.4" fill="#ff9d33" stroke="#d97a17" stroke-width="1.8" />
            <path d="M50 11.5 q4 -3.4 6 -1" fill="none" stroke="#4c8c3a" stroke-width="2" stroke-linecap="round" />
          </g>
          <g :transform="eyeShift">
            <g class="eyes-open">
              <ellipse cx="38" cy="44" rx="3.4" ry="3.9" fill="#3a2a1e" />
              <ellipse cx="62" cy="44" rx="3.4" ry="3.9" fill="#3a2a1e" />
              <circle cx="39.2" cy="42.6" r="1.3" fill="#fff" />
              <circle cx="63.2" cy="42.6" r="1.3" fill="#fff" />
            </g>
            <g class="eyes-closed">
              <path d="M34 44 q4 3.6 8 0" fill="none" stroke="#3a2a1e" stroke-width="2.2" stroke-linecap="round" />
              <path d="M58 44 q4 3.6 8 0" fill="none" stroke="#3a2a1e" stroke-width="2.2" stroke-linecap="round" />
            </g>
            <g class="eyes-happy">
              <path d="M34 46 q4 -5 8 0" fill="none" stroke="#3a2a1e" stroke-width="2.2" stroke-linecap="round" />
              <path d="M58 46 q4 -5 8 0" fill="none" stroke="#3a2a1e" stroke-width="2.2" stroke-linecap="round" />
            </g>
          </g>
          <!-- 鼻孔 + 平嘴(水豚的"无所谓"表情全靠这条直线) -->
          <ellipse cx="45.6" cy="54" rx="1.5" ry="1.1" fill="#5a4032" />
          <ellipse cx="54.4" cy="54" rx="1.5" ry="1.1" fill="#5a4032" />
          <path d="M43 61 h14" fill="none" stroke="#5a4032" stroke-width="1.9" stroke-linecap="round" />
          <text class="zzz" x="82" y="20" font-size="13" fill="#8fa8c0">z</text>
        </svg>

        <!-- ══════ 柴犬(白脸 mask + 尖耳 + doge 笑) ══════ -->
        <svg v-else-if="kind === 'shiba'" viewBox="0 0 100 100" class="pet-svg">
          <ellipse class="pet-shadow" cx="50" cy="96" rx="26" ry="3.4" />
          <g stroke="#4a3524" stroke-width="2.4" stroke-linejoin="round" stroke-linecap="round">
            <!-- 尾巴(柴犬卷尾) -->
            <path class="shiba-tail" d="M74 84 q18 -6 10 -18 q-6 -6 -11 2" fill="#e8a44c" />
            <!-- 身体 -->
            <path d="M28 92 q-2 -25 22 -25 q24 0 22 25 Z" fill="#e8a44c" />
            <path d="M38 92 q2 -18 12 -18 q10 0 12 18 Z" fill="#fdf3e3" stroke="none" />
            <ellipse cx="40" cy="90" rx="6.8" ry="4.4" fill="#fdf3e3" />
            <ellipse cx="60" cy="90" rx="6.8" ry="4.4" fill="#fdf3e3" />
            <!-- 尖耳 -->
            <path d="M27 36 q-3 -20 8 -20 q3 9 9 14 Z" fill="#e8a44c" />
            <path d="M73 36 q3 -20 -8 -20 q-3 9 -9 14 Z" fill="#e8a44c" />
            <path d="M31 33 q-1 -12 5 -12 q1 6 4 9 Z" fill="#c98c3c" stroke="none" />
            <path d="M69 33 q1 -12 -5 -12 q-1 6 -4 9 Z" fill="#c98c3c" stroke="none" />
            <!-- 头 -->
            <circle cx="50" cy="44" r="27" fill="#e8a44c" />
            <!-- 白脸 mask:柴犬的辨识度全在这块 -->
            <path d="M50 22 q-16 4 -16 22 q0 18 16 24 q16 -6 16 -24 q0 -18 -16 -22 Z" fill="#fdf3e3" />
          </g>
          <g :transform="eyeShift">
            <g class="eyes-open">
              <ellipse cx="38.5" cy="42" rx="4" ry="5" fill="#3d2b1c" />
              <ellipse cx="61.5" cy="42" rx="4" ry="5" fill="#3d2b1c" />
              <circle cx="39.9" cy="40" r="1.5" fill="#fff" />
              <circle cx="62.9" cy="40" r="1.5" fill="#fff" />
            </g>
            <g class="eyes-closed">
              <path d="M34 42 q4.5 4 9 0" fill="none" stroke="#3d2b1c" stroke-width="2.3" stroke-linecap="round" />
              <path d="M57 42 q4.5 4 9 0" fill="none" stroke="#3d2b1c" stroke-width="2.3" stroke-linecap="round" />
            </g>
            <g class="eyes-happy">
              <path d="M34 44 q4.5 -5.5 9 0" fill="none" stroke="#3d2b1c" stroke-width="2.3" stroke-linecap="round" />
              <path d="M57 44 q4.5 -5.5 9 0" fill="none" stroke="#3d2b1c" stroke-width="2.3" stroke-linecap="round" />
            </g>
          </g>
          <!-- 黑鼻 + doge 的 W 形笑嘴 -->
          <path d="M46.4 52 q3.6 -2.6 7.2 0 q-1.6 3.4 -3.6 3.4 q-2 0 -3.6 -3.4 Z" fill="#3d2b1c" />
          <path d="M50 56.4 q-4 4.4 -7 1 M50 56.4 q4 4.4 7 1" fill="none" stroke="#3d2b1c" stroke-width="1.8" stroke-linecap="round" />
          <!-- 吐出的小舌头 -->
          <path class="shiba-tongue" d="M46.6 59.6 q3.4 5 6.8 0 Z" fill="#ff8095" stroke="#e0637c" stroke-width="1.2" />
          <text class="zzz" x="80" y="20" font-size="13" fill="#8fa8c0">z</text>
        </svg>

        <!-- ══════ 小黄鸭 ══════ -->
        <svg v-else viewBox="0 0 100 100" class="pet-svg">
          <ellipse class="pet-shadow" cx="50" cy="96" rx="24" ry="3.4" />
          <g class="duck-sway" stroke="#8a6410" stroke-width="2.4" stroke-linejoin="round" stroke-linecap="round">
            <!-- 身体 -->
            <path d="M26 90 q-2 -24 24 -24 q26 0 24 24 Z" fill="#ffd94a" />
            <!-- 翅膀 -->
            <path class="duck-wing" d="M30 72 q-8 6 -1 12 q8 2 10 -6 Z" fill="#f5c92e" />
            <!-- 头 -->
            <circle cx="50" cy="42" r="24" fill="#ffe066" />
            <!-- 喙 -->
            <path d="M40 50 q10 -3 20 0 q-3 8 -10 8 q-7 0 -10 -8 Z" fill="#ff9f2e" />
            <!-- 脚 -->
            <path d="M42 90 q-6 4 -8 6 h16 Z" fill="#ff9f2e" />
            <path d="M58 90 q6 4 8 6 h-16 Z" fill="#ff9f2e" />
          </g>
          <path d="M41 53.6 q9 2 18 0" fill="none" stroke="#d9821a" stroke-width="1.4" />
          <g :transform="eyeShift">
            <g class="eyes-open">
              <ellipse cx="40" cy="39" rx="4.2" ry="5.4" fill="#3a3020" />
              <ellipse cx="60" cy="39" rx="4.2" ry="5.4" fill="#3a3020" />
              <circle cx="41.5" cy="37" r="1.6" fill="#fff" />
              <circle cx="61.5" cy="37" r="1.6" fill="#fff" />
            </g>
            <g class="eyes-closed">
              <path d="M35.5 39 q4.5 4 9 0" fill="none" stroke="#3a3020" stroke-width="2.3" stroke-linecap="round" />
              <path d="M55.5 39 q4.5 4 9 0" fill="none" stroke="#3a3020" stroke-width="2.3" stroke-linecap="round" />
            </g>
            <g class="eyes-happy">
              <path d="M35.5 41 q4.5 -5.5 9 0" fill="none" stroke="#3a3020" stroke-width="2.3" stroke-linecap="round" />
              <path d="M55.5 41 q4.5 -5.5 9 0" fill="none" stroke="#3a3020" stroke-width="2.3" stroke-linecap="round" />
            </g>
          </g>
          <text class="zzz" x="82" y="20" font-size="13" fill="#8fa8c0">z</text>
        </svg>

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
          <!-- 两只爪子交替拍(BongoCat 的灵魂就是这个错开半拍) -->
          <ellipse class="fx-paw paw-l" cx="35" cy="84" rx="7.4" ry="5.2" fill="#fdfcfa" stroke="#33302e" stroke-width="2" />
          <ellipse class="fx-paw paw-r" cx="65" cy="84" rx="7.4" ry="5.2" fill="#fdfcfa" stroke="#33302e" stroke-width="2" />
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
.pet-root {
  position: relative;
  background: transparent;
  user-select: none;
  cursor: grab;
  overflow: hidden;
  -webkit-user-select: none;
}
.pet-root:active { cursor: grabbing; }

/* 本体舞台:固定在窗口底部,顶部那段留给气泡 */
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
  animation: breathe 2.6s ease-in-out infinite;
}
.pet-svg {
  width: 100%;
  height: 100%;
  /* 让宠物在任意桌面底色上都能看清轮廓 */
  filter: drop-shadow(0 3px 4px rgba(0, 0, 0, 0.3));
}
/* 地面投影:让宠物看起来"站"在桌面上而不是飘着 */
.pet-shadow { fill: rgba(0, 0, 0, 0.15); }
/* 朝向:翻转必须挂在不带动画的元素上 —— .pet-body 上的 breathe/nod 等
   都在动画 transform,会把 scaleX(-1) 覆盖掉 */
.f-left .pet-svg { transform: scaleX(-1); }

@keyframes breathe {
  0%, 100% { transform: scaleY(1) scaleX(1); }
  50%      { transform: scaleY(1.035) scaleX(0.985); }
}

/* ── 眼睛三态:睁 / 闭(睡) / 笑(互动中) ── */
.eyes-closed,
.eyes-happy { display: none; }
.m-sleep .eyes-open { display: none; }
.m-sleep .eyes-closed { display: block; }
.a-jump .eyes-open,
.a-squash .eyes-open,
.a-shake .eyes-open { display: none; }
.a-jump .eyes-happy,
.a-squash .eyes-happy,
.a-shake .eyes-happy { display: block; }
/* 拖起来时瞪大眼 */
.is-drag .eyes-open { transform: scale(1.25); transform-origin: 50% 44%; }

.zzz { opacity: 0; }
.m-sleep .zzz {
  opacity: 1;
  animation: zfloat 2.2s ease-out infinite;
}
@keyframes zfloat {
  0%   { opacity: 0; transform: translate(0, 0) scale(0.7); }
  30%  { opacity: 1; }
  100% { opacity: 0; transform: translate(6px, -14px) scale(1.15); }
}
.m-sleep .pet-body { animation: breathe 4.4s ease-in-out infinite; }

/* ── 打字中:掏出键盘一起敲 ── */
.pet-typing-fx {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  opacity: 0;
  pointer-events: none;
  filter: drop-shadow(0 2px 3px rgba(0, 0, 0, 0.28));
  transition: opacity 0.16s ease-out;
}
.m-typing .pet-typing-fx { opacity: 1; }
/* 打字时强制朝正前方:宠物翻转而键盘不翻转会左右错位 */
.f-left.m-typing .pet-svg { transform: none; }
.m-typing .pet-body { animation: nod 0.4s ease-in-out infinite; }
@keyframes nod {
  0%, 100% { transform: translateY(-4px); }
  50%      { transform: translateY(-7px) scaleY(0.985); }
}
.m-typing .fx-kb { animation: kb-in 0.24s ease-out; }
@keyframes kb-in {
  0%   { transform: translateY(14px); opacity: 0; }
  100% { transform: translateY(0); opacity: 1; }
}
/* 爪子交替拍打 —— 错开半个周期才像在打字 */
.fx-paw { animation: tap 0.2s ease-in-out infinite; transform-origin: 50% 100%; }
.fx-paw.paw-r { animation-delay: 0.1s; }
@keyframes tap {
  0%, 100% { transform: translateY(-5px) rotate(-4deg); }
  50%      { transform: translateY(0) rotate(0deg); }
}
.fx-spark { opacity: 0; animation: spark 0.4s ease-out infinite; }
.fx-spark.s2 { animation-delay: 0.2s; }
@keyframes spark {
  0%   { opacity: 0.9; transform: scale(0.3); }
  100% { opacity: 0; transform: scale(1.5); }
}
.fx-note { opacity: 0; animation: note-fly 1.5s ease-out infinite; }
.fx-note.n2 { animation-delay: 0.5s; }
.fx-note.n3 { animation-delay: 1s; }
@keyframes note-fly {
  0%   { opacity: 0; transform: translate(0, 4px) scale(0.6) rotate(0deg); }
  30%  { opacity: 1; }
  100% { opacity: 0; transform: translate(-4px, -18px) scale(1.1) rotate(-14deg); }
}

/* ── 点击互动:三种动作轮流播放。写在持续态之后,优先级更高 ── */
.a-jump .pet-body { animation: act-jump 0.62s cubic-bezier(0.3, 1.4, 0.5, 1); }
@keyframes act-jump {
  0%   { transform: translateY(0) scaleY(1); }
  18%  { transform: translateY(2px) scaleY(0.9) scaleX(1.08); }
  46%  { transform: translateY(-22px) scaleY(1.1) scaleX(0.93); }
  74%  { transform: translateY(0) scaleY(0.92) scaleX(1.06); }
  100% { transform: translateY(0) scaleY(1) scaleX(1); }
}
.a-squash .pet-body { animation: act-squash 0.52s cubic-bezier(0.25, 1.5, 0.4, 1); }
@keyframes act-squash {
  0%   { transform: scaleY(1) scaleX(1); }
  28%  { transform: scaleY(0.7) scaleX(1.24); }
  58%  { transform: scaleY(1.14) scaleX(0.9); }
  80%  { transform: scaleY(0.96) scaleX(1.03); }
  100% { transform: scaleY(1) scaleX(1); }
}
.a-shake .pet-body { animation: act-shake 0.52s ease-in-out; }
@keyframes act-shake {
  0%, 100% { transform: rotate(0deg); }
  15%      { transform: rotate(-11deg); }
  38%      { transform: rotate(9deg); }
  62%      { transform: rotate(-7deg); }
  85%      { transform: rotate(4deg); }
}

/* ── 各宠物特征动画 ── */
.cat-tail {
  transform-origin: 76px 82px;
  animation: tail 3s ease-in-out infinite;
}
@keyframes tail {
  0%, 100% { transform: rotate(-8deg); }
  50%      { transform: rotate(14deg); }
}
.m-typing .cat-tail { animation-duration: 1.1s; }

.capy-orange {
  transform-origin: 50px 25px;
  animation: orange-bob 3.4s ease-in-out infinite;
}
@keyframes orange-bob {
  0%, 100% { transform: rotate(-5deg); }
  50%      { transform: rotate(5deg); }
}

.shiba-tail {
  transform-origin: 76px 82px;
  animation: tail 1.8s ease-in-out infinite;
}
.shiba-tongue {
  transform-origin: 50px 59px;
  animation: pant 1.4s ease-in-out infinite;
}
@keyframes pant {
  0%, 100% { transform: scaleY(0.85); }
  50%      { transform: scaleY(1.2); }
}

.duck-sway { animation: sway 2.4s ease-in-out infinite; transform-origin: 50% 94px; }
@keyframes sway {
  0%, 100% { transform: rotate(-4deg); }
  50%      { transform: rotate(4deg); }
}
.duck-wing { transform-origin: 34px 74px; animation: flap 2.4s ease-in-out infinite; }
@keyframes flap {
  0%, 100% { transform: rotate(0deg); }
  50%      { transform: rotate(-18deg); }
}

/* ── 气泡:不透明白底,固定在顶部留白区内,永不遮挡本体 ── */
.pet-bubble {
  position: absolute;
  left: 50%;
  top: 2px;
  transform: translateX(-50%);
  max-width: 96%;
  max-height: 44px;
  overflow: hidden;
  padding: 5px 9px;
  box-sizing: border-box;
  background: #ffffff;
  color: #26303d;
  font-size: 12px;
  line-height: 1.4;
  font-family: "Microsoft YaHei", "PingFang SC", sans-serif;
  border: 1.5px solid #33302e;
  border-radius: 10px;
  box-shadow: 0 2px 0 rgba(51, 48, 46, 0.9);
  white-space: normal;
  word-break: break-all;
  animation: pop 0.2s ease-out;
  pointer-events: none;
}
/* 小尾巴指向宠物:两层三角描出描边效果 */
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
/* 翻转时气泡不能跟着镜像(文字会反),所以气泡挂在 root 上而非 body 上 */
@keyframes pop {
  0%   { opacity: 0; transform: translateX(-50%) translateY(-4px) scale(0.9); }
  100% { opacity: 1; transform: translateX(-50%) translateY(0) scale(1); }
}

@media (prefers-reduced-motion: reduce) {
  .pet-body, .fx-paw, .fx-spark, .fx-note, .cat-tail, .shiba-tail,
  .shiba-tongue, .capy-orange, .duck-sway, .duck-wing {
    animation: none !important;
  }
}
</style>
