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
 *   - 左键按住拖动 → startDragging,放到哪停在哪(不自动归位);松手有小回弹
 *   - 单击 → 加权随机抽取一段连贯动作或单步动作,并随机说一句
 *   - 双击 → 退出桌宠、恢复摸鱼窗口
 *   - 右键 → 独立菜单窗口:调整大小 / 置顶开关 / 退出
 *   - 滚轮 → 连续缩放(底边中点为锚点,由 Rust 侧完成)
 *   - 鼠标移动 → 仅视线跟随,绝不移动窗口;靠近时所有角色都会欢迎晃动,离开会张望
 *   - 打字反应 → 主进程 pet-typing 驱动(仅 Windows,只知道"有没有在打字")
 *
 * 动画分层(优先级从高到低,后者在前者空闲时才接管):
 *   1. action     瞬时连贯动作 (jump/squash/shake/dance/stretch/spin/walk/land)
 *   2. idleFx     待机微动作   (blink/ear-twitch/look-around/weight-shift/yawn/sneeze/tail-wag)
 *   3. mood       持续态       (idle 呼吸 / sleep 慢呼吸 / typing 点头打字)
 *   4. near       角色特有待机晃动(垂耳猫歪头等)
 */

type PetKind = "lopcat" | "psyduck" | "hamster" | "corgi";
/** 持续态 */
type Mood = "idle" | "sleep" | "typing";
/** 瞬时与连贯动作 */
type PetAction = "" | "jump" | "squash" | "shake" | "dance" | "stretch" | "spin" | "walk" | "land";
/** 待机微动作(空闲时随机抽取,持续 0.2–1.6s 后自动清空) */
type IdleFx = "" | "blink" | "ear-twitch" | "look-around" | "weight-shift" | "yawn" | "sneeze" | "tail-wag";
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
const idleFx = ref<IdleFx>("");
const facing = ref<Facing>("right");
const bubble = ref("");
const isDragging = ref(false);
/** 按下但尚未判定为拖动:轻微拉伸,营造"被抓起"的物理感 */
const isGrabbing = ref(false);
/** 鼠标靠近时为 true:垂耳猫会歪头,其它角色用来欢迎晃动与眨眼放大反应 */
const near = ref(false);

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

interface MotionStep {
  action?: PetAction;
  exprIdx?: number;
  facing?: Facing;
  sayText?: string;
  durationMs: number;
}

/** 连贯组合动作序列库 —— 每步直接切换到下一个动作,不在中间清空 action,
 *  避免呼吸动画重启导致的"一帧弹回"生硬感。 */
const ACTION_COMBOS: Record<string, MotionStep[]> = {
  // 连贯套路 1: 萌宠三段欢快跳舞 (蓄力下蹲 -> 高空跳跃 -> 落地摇晃 -> 歪头卖萌)
  joy_dance: [
    { action: "squash", exprIdx: 2, durationMs: 320 },
    { action: "jump", exprIdx: 0, facing: "right", durationMs: 860 },
    { action: "shake", exprIdx: 3, facing: "left", durationMs: 520 },
    { action: "stretch", exprIdx: 1, sayText: "哼哼~ 看我给你表演个元气跳舞！", durationMs: 900 },
  ],
  // 连贯套路 2: 好奇巡逻 (左右观察 -> 原地小跳调头 -> 凑近镜头 -> 调皮摇摆)
  curious_patrol: [
    { action: "stretch", facing: "left", exprIdx: 1, durationMs: 480 },
    { action: "jump", facing: "right", exprIdx: 0, durationMs: 860 },
    { action: "squash", exprIdx: 2, durationMs: 420 },
    { action: "shake", exprIdx: 3, sayText: "主人主人,发现你在偷偷摸鱼哦！", durationMs: 720 },
  ],
  // 连贯套路 3: 慵懒伸展 (哈欠拉伸 -> 活力弹跳 -> 伸展摇晃)
  lazy_stretch: [
    { action: "stretch", exprIdx: 4, durationMs: 700 },
    { action: "jump", exprIdx: 0, durationMs: 860 },
    { action: "shake", exprIdx: 3, sayText: "伸个懒腰,摸鱼效率直接翻倍~", durationMs: 720 },
  ],
  // 连贯套路 4: 转圈秀 (蓄力 -> 旋风转一圈 -> 落地摇晃回正)
  spin_show: [
    { action: "squash", exprIdx: 2, durationMs: 280 },
    { action: "spin", exprIdx: 0, sayText: "转一圈给你看~", durationMs: 900 },
    { action: "shake", exprIdx: 3, durationMs: 420 },
  ],
  // 连贯套路 5: 踱步步态 (连贯迈步 -> 抖动 -> 调头)
  wander_walk: [
    { action: "walk", facing: "left", exprIdx: 1, durationMs: 480 },
    { action: "shake", facing: "left", exprIdx: 3, durationMs: 420 },
    { action: "walk", facing: "right", exprIdx: 0, durationMs: 480 },
    { action: "stretch", exprIdx: -1, durationMs: 320 },
  ],
};

/** 点击触发的动作池(加权随机抽取,不再循环固定套路,每次都不一样) */
interface ClickActionDef {
  comboKey?: string;
  action?: PetAction;
  exprIdx?: number;
  sayText?: string;
  durationMs: number;
  weight: number;
}
const CLICK_ACTIONS: ClickActionDef[] = [
  { comboKey: "joy_dance", weight: 12, durationMs: 0 },
  { comboKey: "curious_patrol", weight: 10, durationMs: 0 },
  { comboKey: "lazy_stretch", weight: 9, durationMs: 0 },
  { comboKey: "spin_show", weight: 9, durationMs: 0 },
  { comboKey: "wander_walk", weight: 5, durationMs: 0 },
  { action: "jump", exprIdx: 0, durationMs: 860, sayText: "嘿!", weight: 16 },
  { action: "spin", exprIdx: 0, durationMs: 900, sayText: "哒哒哒~", weight: 8 },
  { action: "shake", exprIdx: 3, durationMs: 520, weight: 6 },
  { action: "stretch", exprIdx: 4, durationMs: 700, sayText: "嗯——", weight: 6 },
];

// ── 待机微动作池(空闲时加权随机抽取,持续时长后自动清空) ──
interface IdleFxDef {
  name: IdleFx;
  weight: number;
  durationMs: number;
  /** 触发该微动作时锁定的表情帧(可选) */
  exprIdx?: number;
}
const IDLE_FX_POOL: IdleFxDef[] = [
  { name: "blink",        weight: 30, durationMs: 180 },
  { name: "ear-twitch",   weight: 18, durationMs: 320 },
  { name: "look-around",  weight: 14, durationMs: 1600 },
  { name: "weight-shift",  weight: 13, durationMs: 1400 },
  { name: "tail-wag",     weight: 10, durationMs: 900 },
  { name: "yawn",         weight: 8,  durationMs: 1200, exprIdx: 4 },
  { name: "sneeze",       weight: 7,  durationMs: 620,  exprIdx: 3 },
];

function weightedPick<T extends { weight: number }>(arr: T[]): T {
  const total = arr.reduce((s, x) => s + x.weight, 0);
  let r = Math.random() * total;
  for (const x of arr) {
    r -= x.weight;
    if (r <= 0) return x;
  }
  return arr[arr.length - 1];
}

// ── 图片双层渐变切换系统 ──
function getTargetSrc(): string {
  const data = PET_IMAGES[kind] || PET_IMAGES.lopcat;
  if (mood.value === "sleep") return data.exprs[4] || data.main;
  if (mood.value === "typing") return data.exprs[1] || data.main;
  if (isDragging.value) return data.exprs[2] || data.main;
  if (isGrabbing.value) return data.exprs[2] || data.main;
  if (action.value === "jump") return data.exprs[0] || data.main;
  if (action.value === "squash") return data.exprs[2] || data.main;
  if (action.value === "shake") return data.exprs[3] || data.main;
  if (action.value === "dance") return data.exprs[0] || data.main;
  if (action.value === "stretch") return data.exprs[1] || data.main;
  if (action.value === "walk") return data.exprs[0] || data.main;
  if (action.value === "spin") return data.exprs[0] || data.main;
  if (action.value === "land") return data.exprs[2] || data.main;
  if (activeExprIdx.value >= 0 && data.exprs[activeExprIdx.value]) return data.exprs[activeExprIdx.value];
  if (near.value && data.exprs[0]) return data.exprs[0];
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

watch([mood, action, idleFx, isDragging, isGrabbing, activeExprIdx, near], () => {
  switchTo(getTargetSrc());
});

// 鼠标靠近/离开的瞬时反应:所有角色都给个欢迎/张望微动作
watch(near, (v, old) => {
  if (v === old) return;
  if (mood.value !== "idle" || action.value || isDragging.value) return;
  if (v) {
    // 鼠标刚靠近 → 短暂摇尾欢迎
    playIdleFxByName("tail-wag", 900);
  } else if (old) {
    // 鼠标刚离开 → 张望一下
    playIdleFxByName("look-around", 1600);
  }
});

let bubbleTimer: number | undefined;
let actionTimer: number | undefined;
let comboTimer: number | undefined;
let wanderTimer: number | undefined;
let idleTimer: number | undefined;
let idleFxTimer: number | undefined;
/** 待机微动作调度器的自延时器(与 idleFxTimer 是两个独立计时:
 *  idleFxTimer 控制某个微动作播放多久后清空;idleSchedulerTimer 控制下一次抽取微动作的间隔) */
let idleSchedulerTimer: number | undefined;
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

/** 播放连贯组合动作序列 —— 每步用"先清空再 rAF 设置"保证 CSS 动画从头播;
 *  呼吸动画在两帧之间的 0% 关键帧也是 identity,不会产生肉眼可见的弹回 */
function playComboSequence(steps: MotionStep[]) {
  if (comboTimer) window.clearTimeout(comboTimer);
  if (actionTimer) window.clearTimeout(actionTimer);
  if (imgSwitchTimer) window.clearTimeout(imgSwitchTimer);
  // 切换前先清掉微动作,避免和连贯动作的 transform 打架
  idleFx.value = "";
  if (idleFxTimer) { window.clearTimeout(idleFxTimer); idleFxTimer = undefined; }

  let stepIdx = 0;

  function runNextStep() {
    if (stepIdx >= steps.length) {
      // 序列结束:动作清空时配合 .pet-body 的 transform 过渡平滑回到 idle
      action.value = "";
      imgSwitchTimer = window.setTimeout(() => {
        activeExprIdx.value = -1;
      }, 260);
      return;
    }

    const step = steps[stepIdx];
    action.value = "";
    window.requestAnimationFrame(() => {
      if (step.action !== undefined) action.value = step.action;
      if (step.exprIdx !== undefined) activeExprIdx.value = step.exprIdx;
      if (step.facing !== undefined) facing.value = step.facing;
      if (step.sayText) say(step.sayText);

      stepIdx++;
      comboTimer = window.setTimeout(runNextStep, step.durationMs);
    });
  }

  runNextStep();
}

/** 播放一段单步动作;兼容旧接口 */
function playAction(a: Exclude<PetAction, "">) {
  if (comboTimer) window.clearTimeout(comboTimer);
  if (actionTimer) window.clearTimeout(actionTimer);
  if (imgSwitchTimer) window.clearTimeout(imgSwitchTimer);
  idleFx.value = "";
  if (idleFxTimer) { window.clearTimeout(idleFxTimer); idleFxTimer = undefined; }
  action.value = "";
  window.requestAnimationFrame(() => {
    action.value = a;
    actionTimer = window.setTimeout(() => {
      action.value = "";
      imgSwitchTimer = window.setTimeout(() => {
        activeExprIdx.value = -1;
      }, 280);
    }, ACTION_DURATION_MS[a] ?? 600);
  });
}

/** 各动作的播放时长(与 CSS keyframe 时长对齐) */
const ACTION_DURATION_MS: Record<Exclude<PetAction, "">, number> = {
  jump: 850,
  squash: 520,
  shake: 520,
  dance: 720,
  stretch: 700,
  spin: 900,
  walk: 500,
  land: 360,
};

/** 空闲久了打瞌睡;任何互动都会重置 */
function resetIdle() {
  if (idleTimer) window.clearTimeout(idleTimer);
  if (mood.value === "sleep") mood.value = "idle";
  idleTimer = window.setTimeout(() => {
    if (mood.value === "idle") mood.value = "sleep";
  }, 45000);
  scheduleIdleFx();
}

/** 待机微动作调度:加权随机抽一个微动作,2.5–7s 后再来一次。
 *  任何互动(resetIdle)都会重置此调度,保证交互中不会插一个微动作进来。 */
function scheduleIdleFx() {
  if (idleSchedulerTimer) window.clearTimeout(idleSchedulerTimer);
  const delay = 2500 + Math.random() * 4500;
  idleSchedulerTimer = window.setTimeout(() => {
    idleSchedulerTimer = undefined;
    // 只在真正空闲时才触发,且不和当前进行中的动作冲突
    if (mood.value === "idle" && !action.value && !isDragging.value && !isGrabbing.value && !idleFx.value) {
      const def = weightedPick(IDLE_FX_POOL);
      playIdleFxByName(def.name, def.durationMs, def.exprIdx);
    }
    scheduleIdleFx();
  }, delay);
}

/** 按名字播放一个待机微动作;持续 durationMs 后自动清空 */
function playIdleFxByName(name: IdleFx, durationMs: number, exprIdx?: number) {
  // 微动作和连贯 action / 拖拽态互斥:有动作或被指尖捏住时不清空原状态
  if (action.value || isDragging.value || isGrabbing.value) return;
  if (idleFxTimer) { window.clearTimeout(idleFxTimer); idleFxTimer = undefined; }
  if (exprIdx !== undefined) activeExprIdx.value = exprIdx;
  idleFx.value = name;
  idleFxTimer = window.setTimeout(() => {
    idleFx.value = "";
    idleFxTimer = undefined;
    if (exprIdx !== undefined) {
      if (imgSwitchTimer) window.clearTimeout(imgSwitchTimer);
      imgSwitchTimer = window.setTimeout(() => { activeExprIdx.value = -1; }, 200);
    }
  }, durationMs);
}

function onPointerDown(e: PointerEvent) {
  if (e.button !== 0) return;
  downAt = { x: e.screenX, y: e.screenY };
  isDragging.value = false;
  // 按下即轻微拉伸,营造"被指尖捏住"的物理感
  isGrabbing.value = true;
  // 清掉进行中的微动作:is-grab 的 CSS 优先级低于 idle-*,不主动清会被覆盖
  idleFx.value = "";
  if (idleFxTimer) { window.clearTimeout(idleFxTimer); idleFxTimer = undefined; }
  resetIdle();
}

function onPointerMove(e: PointerEvent) {
  if (!downAt || isDragging.value) return;
  const moved =
    Math.abs(e.screenX - downAt.x) > DRAG_THRESHOLD ||
    Math.abs(e.screenY - downAt.y) > DRAG_THRESHOLD;
  if (moved) {
    isDragging.value = true;
    isGrabbing.value = false;
    // 交给系统拖动:此后本 webview 不再收到 move/up,收尾在 pointercancel/blur
    win.startDragging().catch(() => {});
  }
}

/** 拖动结束:位置保持用户松手处,不做任何自动归位;松手播一段小回弹而非完整跳 */
function endDrag() {
  if (isDragging.value) {
    isDragging.value = false;
    isGrabbing.value = false;
    downAt = null;
    // 用 land(短促挤压回弹)替代原来的 jump,更贴合"刚被放下"的物理感
    if (comboTimer) window.clearTimeout(comboTimer);
    if (actionTimer) window.clearTimeout(actionTimer);
    action.value = "";
    window.requestAnimationFrame(() => {
      action.value = "land";
      actionTimer = window.setTimeout(() => {
        action.value = "";
      }, ACTION_DURATION_MS.land);
    });
    return;
  }
  isGrabbing.value = false;
  downAt = null;
}

function onClick() {
  if (isDragging.value) return;
  resetIdle();
  // 加权随机抽一个动作
  const def = weightedPick(CLICK_ACTIONS);
  if (def.comboKey) {
    const steps = ACTION_COMBOS[def.comboKey];
    if (steps) playComboSequence(steps);
  } else if (def.action) {
    const a = def.action;
    if (def.exprIdx !== undefined) activeExprIdx.value = def.exprIdx;
    if (def.sayText) say(def.sayText);
    playAction(a);
  }
  if (lines.length && Math.random() > 0.4) {
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

/** 自主漫步:播放连贯踱步套路 */
function tickWander() {
  if (!wander || isDragging.value || mood.value === "sleep") return;
  if (Math.random() > 0.3) return;
  playComboSequence(ACTION_COMBOS.wander_walk);
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
  if (imgSwitchTimer) window.clearTimeout(imgSwitchTimer);
  if (idleFxTimer) window.clearTimeout(idleFxTimer);
  if (idleSchedulerTimer) window.clearTimeout(idleSchedulerTimer);
  if (comboTimer) window.clearTimeout(comboTimer);
  unlistens.forEach((u) => u());
});
</script>

<template>
  <div
    class="pet-root"
    :style="rootStyle"
    :class="[`m-${mood}`, `f-${facing}`, `k-${kind}`,
      action && `a-${action}`, idleFx && `idle-${idleFx}`,
      { 'is-drag': isDragging, 'is-grab': isGrabbing, 'is-near': near }]"
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

        <!-- 打喷嚏时的粉尘小颗粒(由 idle-sneeze 触发) -->
        <div v-if="idleFx === 'sneeze'" class="sneeze-puff">
          <span class="puff p1"></span>
          <span class="puff p2"></span>
          <span class="puff p3"></span>
          <span class="puff p4"></span>
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
  /* 动作结束时(action class 移除)transform 通过此过渡平滑回 idle,
     避免动作结束瞬间"啪"一下弹回原位的生硬感 */
  transition: transform 0.18s cubic-bezier(0.34, 1.56, 0.64, 1);
}
/* 任何 action / idleFx / 拖拽态生效时禁用过渡:动画自身已定义起止,
   transition 会和动画抢着改 transform,反而抖动 */
.a-jump .pet-body, .a-squash .pet-body, .a-shake .pet-body, .a-dance .pet-body,
.a-stretch .pet-body, .a-spin .pet-body, .a-walk .pet-body, .a-land .pet-body,
.idle-blink .pet-body, .idle-ear-twitch .pet-body, .idle-look-around .pet-body,
.idle-weight-shift .pet-body, .idle-yawn .pet-body, .idle-sneeze .pet-body,
.idle-tail-wag .pet-body, .is-grab .pet-body, .is-drag .pet-body {
  transition: none;
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
  transition: opacity 0.28s ease-in-out, transform 0.2s ease-out, filter 0.18s ease-out;
  pointer-events: none;
  will-change: opacity, transform, filter;
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
  transition: width 0.18s ease-out, opacity 0.18s ease-out;
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

/* ── 呼吸待机(主基准,所有动作动画都通过覆盖 animation 来叠加) ── */
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

/* ════════════════════════════════════════════════════════════════════
   拖拽物理感:按下时轻微拉伸,模拟"被指尖捏住"
   ════════════════════════════════════════════════════════════════════ */
.is-grab .pet-body {
  animation: grab-stretch 0.35s cubic-bezier(0.34, 1.56, 0.64, 1) forwards;
}
@keyframes grab-stretch {
  0%   { transform: scaleY(1) scaleX(1); }
  60%  { transform: scaleY(1.05) scaleX(0.95); }
  100% { transform: scaleY(1.04) scaleX(0.96); }
}
.is-drag .pet-body {
  animation: drag-wiggle 0.7s ease-in-out infinite;
}
@keyframes drag-wiggle {
  0%, 100% { transform: scaleY(1.06) scaleX(0.94) rotate(-1.5deg); }
  50%      { transform: scaleY(1.06) scaleX(0.94) rotate(1.5deg); }
}

/* ════════════════════════════════════════════════════════════════════
   点击连贯动作:每段都加了 预期(蓄力) → 主力 → 落地挤压 → 回弹恢复
   ════════════════════════════════════════════════════════════════════ */

/* 跳跃:预期下蹲 → 起跳 → 顶点 → 下落 → 落地挤压 → 回弹 → 收 */
.a-jump .pet-body { animation: act-jump 0.85s cubic-bezier(0.3, 1.3, 0.5, 1); }
@keyframes act-jump {
  0%   { transform: translateY(0) scaleY(1) scaleX(1); }
  12%  { transform: translateY(4px) scaleY(0.82) scaleX(1.14); }
  25%  { transform: translateY(-8px) scaleY(1.06) scaleX(0.96); }
  50%  { transform: translateY(-26px) scaleY(1.04) scaleX(0.97); }
  70%  { transform: translateY(-2px) scaleY(0.86) scaleX(1.12); }
  85%  { transform: translateY(0) scaleY(1.08) scaleX(0.95); }
  100% { transform: translateY(0) scaleY(1) scaleX(1); }
}
/* 跳跃时阴影也跟着缩小再放大,增强空中悬浮感 */
.a-jump .pet-shadow-el {
  animation: shadow-jump 0.85s cubic-bezier(0.3, 1.3, 0.5, 1);
}
@keyframes shadow-jump {
  0%, 100% { transform: translateX(-50%) scaleX(1);   opacity: 0.16; }
  50%      { transform: translateX(-50%) scaleX(0.55); opacity: 0.07; }
}

/* 压扁:蓄力挤压 → 回弹震荡 → 收 */
.a-squash .pet-body { animation: act-squash 0.52s cubic-bezier(0.25, 1.5, 0.4, 1); }
@keyframes act-squash {
  0%   { transform: scaleY(1) scaleX(1); }
  28%  { transform: scaleY(0.72) scaleX(1.2); }
  58%  { transform: scaleY(1.12) scaleX(0.92); }
  80%  { transform: scaleY(0.97) scaleX(1.02); }
  100% { transform: scaleY(1) scaleX(1); }
}

/* 摇晃:左右摆动 + 收尾余震 */
.a-shake .pet-body { animation: act-shake 0.52s ease-in-out; }
@keyframes act-shake {
  0%, 100% { transform: rotate(0deg); }
  15%      { transform: rotate(-9deg); }
  38%      { transform: rotate(8deg); }
  62%      { transform: rotate(-6deg); }
  85%      { transform: rotate(3deg); }
}

/* 跳舞:三段上下 + 旋转混合 */
.a-dance .pet-body { animation: act-dance 0.72s cubic-bezier(0.34, 1.56, 0.64, 1); }
@keyframes act-dance {
  0%   { transform: translateY(0) rotate(0deg) scale(1); }
  25%  { transform: translateY(-12px) rotate(-8deg) scaleY(1.05); }
  50%  { transform: translateY(-4px) rotate(6deg) scaleY(0.96); }
  75%  { transform: translateY(-10px) rotate(-4deg) scaleY(1.03); }
  100% { transform: translateY(0) rotate(0deg) scale(1); }
}

/* 伸展:慢拉伸 → 顶点停顿 → 慢回弹 */
.a-stretch .pet-body { animation: act-stretch 0.7s cubic-bezier(0.25, 1.4, 0.5, 1); }
@keyframes act-stretch {
  0%   { transform: scaleY(1) scaleX(1); }
  35%  { transform: scaleY(1.18) scaleX(0.88) translateY(-6px); }
  60%  { transform: scaleY(1.14) scaleX(0.9) translateY(-4px); }
  100% { transform: scaleY(1) scaleX(1); }
}

/* 旋风:起跳同时原地旋转一圈,落地回正 */
.a-spin .pet-body { animation: act-spin 0.9s cubic-bezier(0.4, 0.0, 0.2, 1); }
@keyframes act-spin {
  0%   { transform: rotate(0deg) translateY(0) scale(1); }
  20%  { transform: rotate(-90deg) translateY(-6px) scale(0.97); }
  50%  { transform: rotate(-180deg) translateY(-12px) scale(0.94); }
  80%  { transform: rotate(-340deg) translateY(-4px) scale(0.97); }
  100% { transform: rotate(-360deg) translateY(0) scale(1); }
}

/* 走步:两段交替小跳,模拟左右脚交替踏地 */
.a-walk .pet-body { animation: act-walk 0.5s ease-in-out; }
@keyframes act-walk {
  0%, 100% { transform: translateY(0) rotate(0deg) scaleY(1); }
  25%      { transform: translateY(-6px) rotate(-4deg) scaleY(1.02); }
  50%      { transform: translateY(0) rotate(0deg) scaleY(0.98); }
  75%      { transform: translateY(-6px) rotate(3deg) scaleY(1.02); }
}

/* 松手回弹:短暂挤压 → 弹起 → 收,极短 */
.a-land .pet-body { animation: act-land 0.36s cubic-bezier(0.34, 1.56, 0.64, 1); }
@keyframes act-land {
  0%   { transform: scaleY(0.9) scaleX(1.1); }
  55%  { transform: scaleY(1.05) scaleX(0.96); }
  100% { transform: scaleY(1) scaleX(1); }
}

/* ════════════════════════════════════════════════════════════════════
   待机微动作:加权随机抽取,持续 0.18–1.6s 后自动清空,与 action 互斥
   ════════════════════════════════════════════════════════════════════ */

/* 眨眼:图片整体短暂压扁 + 轻微暗化,模拟眼皮闭合 */
.idle-blink .pet-body { animation: idle-blink-body 0.18s ease-out; }
.idle-blink .pet-img { animation: idle-blink-img 0.18s ease-out; }
@keyframes idle-blink-body {
  0%, 100% { transform: scaleY(1) scaleX(1); }
  50%      { transform: scaleY(0.97) scaleX(1.01); }
}
@keyframes idle-blink-img {
  0%, 100% { filter: brightness(1); }
  50%      { filter: brightness(0.88); }
}

/* 抖耳:快速小幅旋转两次 */
.idle-ear-twitch .pet-body { animation: idle-ear-twitch 0.32s ease-out; }
@keyframes idle-ear-twitch {
  0%, 100% { transform: rotate(0deg); }
  30%      { transform: rotate(-5deg); }
  55%      { transform: rotate(2deg); }
  75%      { transform: rotate(-1deg); }
}

/* 张望:缓慢左右转头一圈,持续较长 */
.idle-look-around .pet-body { animation: idle-look-around 1.6s ease-in-out; }
@keyframes idle-look-around {
  0%, 100% { transform: rotate(0deg); }
  20%      { transform: rotate(-6deg); }
  45%      { transform: rotate(0deg); }
  70%      { transform: rotate(6deg); }
  90%      { transform: rotate(0deg); }
}

/* 重心转移:缓慢向一侧倾斜 + 微移,像在换脚 */
.idle-weight-shift .pet-body { animation: idle-weight-shift 1.4s ease-in-out; }
@keyframes idle-weight-shift {
  0%, 100% { transform: translateX(0) rotate(0deg); }
  35%      { transform: translateX(3px) rotate(2deg); }
  65%      { transform: translateX(3px) rotate(2deg); }
}

/* 摇尾/摇身:快速左右小幅摆动多次(对所有角色通用,柯基摇身=摇尾) */
.idle-tail-wag .pet-body { animation: idle-tail-wag 0.9s ease-in-out; }
@keyframes idle-tail-wag {
  0%, 100% { transform: rotate(0deg); }
  15%      { transform: rotate(-5deg); }
  35%      { transform: rotate(5deg); }
  55%      { transform: rotate(-4deg); }
  75%      { transform: rotate(3deg); }
  90%      { transform: rotate(-1deg); }
}

/* 打哈欠:缓慢大幅纵向拉伸 + 顶点停顿,像在张大嘴 */
.idle-yawn .pet-body { animation: idle-yawn 1.2s ease-in-out; }
@keyframes idle-yawn {
  0%, 100% { transform: scaleY(1) scaleX(1) translateY(0); }
  35%      { transform: scaleY(1.12) scaleX(0.9) translateY(-4px); }
  60%      { transform: scaleY(1.1) scaleX(0.91) translateY(-3px); }
}

/* 打喷嚏:蓄力下蹲 → 抬头前冲 → 收;伴随粉尘颗粒 */
.idle-sneeze .pet-body { animation: idle-sneeze 0.62s ease-out; }
@keyframes idle-sneeze {
  0%   { transform: translateY(0) scaleY(1); }
  20%  { transform: translateY(3px) scaleY(0.92) scaleX(1.08); }
  45%  { transform: translateY(-4px) scaleY(1.06) scaleX(0.94) rotate(-3deg); }
  60%  { transform: translateY(-2px) scaleY(1.04) scaleX(0.96) rotate(2deg); }
  80%  { transform: translateY(1px) scaleY(0.98) scaleX(1.02); }
  100% { transform: translateY(0) scaleY(1) scaleX(1); }
}
.sneeze-puff {
  position: absolute;
  top: 18%;
  left: 50%;
  width: 0;
  height: 0;
  pointer-events: none;
  z-index: 6;
}
.sneeze-puff .puff {
  position: absolute;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: radial-gradient(circle, rgba(255, 255, 255, 0.85), rgba(220, 220, 220, 0.0));
  opacity: 0;
  animation: puff-fly 0.62s ease-out;
}
.sneeze-puff .p1 { animation-delay: 0.18s; }
.sneeze-puff .p2 { animation-delay: 0.22s; }
.sneeze-puff .p3 { animation-delay: 0.26s; }
.sneeze-puff .p4 { animation-delay: 0.30s; }
@keyframes puff-fly {
  0%   { opacity: 0; transform: translate(0, 0) scale(0.3); }
  30%  { opacity: 0.9; }
  100% { opacity: 0; transform: translate(var(--dx, 14px), -14px) scale(1.6); }
}
.sneeze-puff .p1 { --dx: -14px; }
.sneeze-puff .p2 { --dx: -5px; }
.sneeze-puff .p3 { --dx: 8px; }
.sneeze-puff .p4 { --dx: 16px; }

/* ── 角色特有待机晃动(只在空闲且无微动作时接管) ── */
.k-lopcat.is-near:not(.a-jump):not(.a-squash):not(.a-shake):not(.a-spin):not(.a-walk):not(.a-dance):not(.a-stretch):not(.a-land):not(.m-typing):not(.m-sleep):not([class*="idle-"]):not(.is-grab):not(.is-drag) .pet-body {
  animation: lop-tilt 1.8s ease-in-out infinite;
}
@keyframes lop-tilt {
  0%, 100% { transform: rotate(0deg); }
  50%      { transform: rotate(-5deg); }
}

.k-psyduck:not(.a-jump):not(.a-squash):not(.a-shake):not(.a-spin):not(.a-walk):not(.a-dance):not(.a-stretch):not(.a-land):not(.m-typing):not(.m-sleep):not([class*="idle-"]):not(.is-grab):not(.is-drag) .pet-body {
  animation: duck-wobble 3s ease-in-out infinite; transform-origin: 50% 92%;
}
@keyframes duck-wobble {
  0%, 100% { transform: rotate(0deg); }
  25%      { transform: rotate(-4deg); }
  75%      { transform: rotate(4deg); }
}

.k-corgi:not(.a-jump):not(.a-squash):not(.a-shake):not(.a-spin):not(.a-walk):not(.a-dance):not(.a-stretch):not(.a-land):not(.m-typing):not(.m-sleep):not([class*="idle-"]):not(.is-grab):not(.is-drag) .pet-body {
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
