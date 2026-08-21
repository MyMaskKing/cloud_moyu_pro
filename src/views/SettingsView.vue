<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useAuthStore } from "../stores/auth";
import { AUTH_ENABLED } from "../services/auth";
import { useDisguiseStore } from "../stores/disguise";
import type { DisguiseSkin } from "../stores/disguise";
import { usePetStore, PET_MIN, PET_MAX, DEFAULT_PET_LINES } from "../stores/pet";
import type { PetKind } from "../stores/pet";
import { useUserScriptsStore } from "../stores/userscripts";
import { DASHANG_IMAGES } from "../config/api";
import VipLock from "../components/VipLock.vue";
import ZoomableImage from "../components/ZoomableImage.vue";

const auth = useAuthStore();
const disguise = useDisguiseStore();
const pet = usePetStore();
const userScripts = useUserScriptsStore();

// ────── 用户脚本(迷你油猴) ──────
const scriptDraft = ref("");
const scriptError = ref("");
function addScript() {
  scriptError.value = "";
  const src = scriptDraft.value.trim();
  if (!src) return;
  const res = userScripts.addFromSource(src);
  if (!res.ok) {
    scriptError.value = res.error;
    return;
  }
  scriptDraft.value = "";
}
function removeScript(id: string) {
  userScripts.remove(id);
}
function moveScript(id: string, dir: -1 | 1) {
  userScripts.move(id, dir);
}
function toggleScript(id: string, on: boolean) {
  userScripts.setEnabled(id, on);
}

const dashangImages = DASHANG_IMAGES;
const DASHANG_HIDE_KEY = "moyu-dashang-hidden";
const dashangHidden = ref(localStorage.getItem(DASHANG_HIDE_KEY) === "1");
function resetDashangHidden() {
  localStorage.removeItem(DASHANG_HIDE_KEY);
  dashangHidden.value = false;
}

// ────── M4 皮肤选择 ──────
// 行为按皮肤自动决定:vscode = 换壳式(真实网页可继续浏览);excel/ppt = 全屏遮蔽
const SKINS: { key: DisguiseSkin; name: string; desc: string }[] = [
  { key: "vscode", name: "VS Code", desc: "编辑器外壳 · 内容可继续浏览" },
  { key: "excel",  name: "Excel",   desc: "紧急遮蔽 · 全屏假表格" },
  { key: "ppt",    name: "PowerPoint", desc: "紧急遮蔽 · 全屏假演示" },
];
function pickSkin(s: DisguiseSkin) {
  disguise.setSkin(s);
}

import lopcatImg from "../images/奶白垂耳猫.png";
import psyduckImg from "../images/黄色可达鸭.png";
import hamsterImg from "../images/雾灰蓝仓鼠.png";
import corgiImg from "../images/焦糖柯基.png";

// ────── M8 桌面宠物 ──────
const PETS: { key: PetKind; name: string; desc: string; emoji: string; img: string }[] = [
  { key: "lopcat",   name: "奶白垂耳猫", desc: "温柔大眼,长耳轻晃,鼠标靠近会歪头", emoji: "🐱", img: lopcatImg },
  { key: "psyduck",  name: "黄色可达鸭", desc: "放空晃头,压力波纹,小短脚扑腾",     emoji: "🦆", img: psyduckImg },
  { key: "hamster",  name: "雾灰蓝仓鼠", desc: "活泼蹦跳,快速眨眼,鼓腮帮捂脸",     emoji: "🐹", img: hamsterImg },
  { key: "corgi",    name: "焦糖柯基",   desc: "慵懒半垂眼,慢半拍歪头打瞌睡",     emoji: "🐶", img: corgiImg },
];
/** 打字反应依赖 Windows API,其它平台需系统辅助功能权限,暂不支持 */
const typingSupported = navigator.userAgent.includes("Windows");

function onSizeInput(e: Event) {
  pet.setSize(Number((e.target as HTMLInputElement).value));
}

const newLine = ref("");
function addLine() {
  const t = newLine.value.trim();
  if (!t) return;
  if (pet.lines.includes(t)) {
    newLine.value = "";
    return;
  }
  pet.setLines([...pet.lines, t]);
  newLine.value = "";
}
function removeLine(t: string) {
  pet.setLines(pet.lines.filter((x) => x !== t));
}
function resetLines() {
  pet.setLines([...DEFAULT_PET_LINES]);
}
function onBossAction(a: "hide" | "pet") {
  emit("update:bossAction", a);
}

// ────── M5 关键字编辑 ──────
const newKeyword = ref("");
function addKeyword() {
  const k = newKeyword.value.trim();
  if (!k) return;
  if (props.workMonitorKeywords.some((x) => x.toLowerCase() === k.toLowerCase())) {
    newKeyword.value = "";
    return;
  }
  emit("update:workMonitorKeywords", [...props.workMonitorKeywords, k]);
  newKeyword.value = "";
}
function removeKeyword(k: string) {
  emit("update:workMonitorKeywords", props.workMonitorKeywords.filter((x) => x !== k));
}

const props = defineProps<{
  autoHide: boolean;
  autoHideDelay: number;
  videoAutoLandscape: boolean;
  bossKey: string;
  transparencyKey: string;
  bypassOpacity: boolean;
  workMonitor: boolean;
  workMonitorAction: "disguise" | "hide";
  workMonitorKeywords: string[];
  workAppPresent: boolean;
  bossAction: "hide" | "pet";
  popoutTheme: "dark" | "light" | "system";
}>();

const emit = defineEmits<{
  (e: "update:autoHide", v: boolean): void;
  (e: "update:autoHideDelay", v: number): void;
  (e: "update:videoAutoLandscape", v: boolean): void;
  (e: "update:bossKey", v: string): void;
  (e: "update:transparencyKey", v: string): void;
  (e: "update:workMonitor", v: boolean): void;
  (e: "update:workMonitorAction", v: "disguise" | "hide"): void;
  (e: "update:workMonitorKeywords", v: string[]): void;
  (e: "update:bossAction", v: "hide" | "pet"): void;
  (e: "update:popoutTheme", v: "dark" | "light" | "system"): void;
  (e: "previewDisguise"): void;
  (e: "previewPet"): void;
  (e: "close"): void;
}>();

// ────── 账号 / 会员(mock) ──────
const account = ref("");
const verifyCode = ref("");
const redeemCode = ref("");
const authBusy = computed(() => auth.loading);
const expireLabel = computed(() => {
  if (!auth.vipExpireAt) return "";
  return new Date(auth.vipExpireAt).toLocaleDateString();
});

async function onLogin() {
  if (!account.value.trim()) return;
  try {
    await auth.login(account.value.trim(), verifyCode.value.trim());
    verifyCode.value = "";
  } catch {
    // 错误信息已在 auth.error,这里无需额外处理
  }
}
async function onLogout() {
  await auth.logout();
  account.value = "";
  redeemCode.value = "";
}
async function onRedeem() {
  if (!redeemCode.value.trim()) return;
  try {
    await auth.redeem(redeemCode.value.trim());
    redeemCode.value = "";
  } catch {
    // 同上
  }
}
function onUnlockVip() {
  // 未登录时先登录;已登录则聚焦兑换码输入
  if (!auth.isLoggedIn) {
    document.querySelector<HTMLInputElement>(".account-input")?.focus();
  } else {
    document.querySelector<HTMLInputElement>(".redeem-input")?.focus();
  }
}

// 快捷键录制:两个快捷键(boss / transparency)复用同一段状态,用 target 标记当前在录哪个
type RecordTarget = "boss" | "transp";
const recording = ref<RecordTarget | null>(null);
const recordedKey = ref("");

/** 把 KeyboardEvent 转成 Tauri 兼容的快捷键字符串
 *  参考 https://tauri.app/plugin/global-shortcut/ 支持的键名 */
function keyToTauriShortcut(e: KeyboardEvent): string | null {
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Super");

  const key = e.key;
  // 忽略纯修饰键
  if (["Control", "Alt", "Shift", "Meta"].includes(key)) return null;

  let mainKey = "";
  if (/^[a-zA-Z]$/.test(key)) mainKey = "Key" + key.toUpperCase();
  else if (/^[0-9]$/.test(key)) mainKey = "Digit" + key;
  else if (key.startsWith("F") && /^F\d{1,2}$/.test(key)) mainKey = key; // F1-F24
  else if (key === " ") mainKey = "Space";
  else if (key === "Escape") mainKey = "Escape";
  else if (key === "Enter") mainKey = "Enter";
  else if (key === "Tab") mainKey = "Tab";
  else if (key === "ArrowUp") mainKey = "ArrowUp";
  else if (key === "ArrowDown") mainKey = "ArrowDown";
  else if (key === "ArrowLeft") mainKey = "ArrowLeft";
  else if (key === "ArrowRight") mainKey = "ArrowRight";
  else return null;

  parts.push(mainKey);
  return parts.length >= 2 ? parts.join("+") : null; // 至少要有一个修饰键
}

function onRecordKeydown(e: KeyboardEvent) {
  if (!recording.value) return;
  e.preventDefault();
  e.stopPropagation();

  if (e.key === "Escape") {
    recording.value = null;
    return;
  }
  const s = keyToTauriShortcut(e);
  if (s) recordedKey.value = s;
}

async function applyShortcut() {
  if (!recordedKey.value || !recording.value) return;
  const target = recording.value;
  try {
    if (target === "boss") {
      await invoke("update_boss_shortcut", { shortcut: recordedKey.value });
      emit("update:bossKey", recordedKey.value);
    } else {
      await invoke("update_transparency_shortcut", { shortcut: recordedKey.value });
      emit("update:transparencyKey", recordedKey.value);
    }
    recording.value = null;
    recordedKey.value = "";
  } catch (e) {
    alert("快捷键注册失败:" + e + "\n可能被其它应用或本应用其它快捷键占用,请换一个组合");
  }
}

function startRecord(target: RecordTarget) {
  recording.value = target;
  recordedKey.value = "";
}

function cancelRecord() {
  recording.value = null;
  recordedKey.value = "";
}

/** 清除"取消透明"快捷键(设置为空) */
async function clearTransparencyKey() {
  try {
    await invoke("update_transparency_shortcut", { shortcut: "" });
    emit("update:transparencyKey", "");
  } catch (e) {
    alert("清除失败:" + e);
  }
}

onMounted(() => {
  window.addEventListener("keydown", onRecordKeydown, true);
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onRecordKeydown, true);
});

// 显示用：把 Tauri 格式转回好看的展示
function humanize(sc: string): string {
  return sc.split("+").map(p => {
    if (p.startsWith("Key")) return p.slice(3);
    if (p.startsWith("Digit")) return p.slice(5);
    return p;
  }).join(" + ");
}
const bossKeyDisplay = computed(() => humanize(props.bossKey));
const transpKeyDisplay = computed(() => props.transparencyKey ? humanize(props.transparencyKey) : "未设置");
</script>

<template>
  <section class="settings-view">
    <header class="settings-head" data-tauri-drag-region>
      <button class="back" @click="emit('close')" title="返回">‹ 返回</button>
      <h2 data-tauri-drag-region>设置</h2>
      <div class="spacer" data-tauri-drag-region></div>
    </header>

    <div class="settings-body">
      <!-- 账号与会员(mock) -->
      <div class="group">
        <div class="group-title">👤 账号与会员</div>
        <div class="group-desc">
          M4 伪装皮肤、M5 工作监测自动隐藏为 VIP 功能。
          <template v-if="AUTH_ENABLED">请输入您的用户和密码即可登录,登录后输入兑换码开通（）。
          </template>
          <template v-else>当前以本地 VIP 身份使用,全部功能已开放。</template>
        </div>

        <!-- 未登录:登录表单 -->
        <div v-if="!auth.isLoggedIn" class="group-content">
          <div class="row">
            <span class="label">账号</span>
            <input
              v-model="account"
              class="text-input account-input"
              placeholder="昵称 / 手机号 / 邮箱"
              @keydown.enter="onLogin"
            />
          </div>
          <div class="row">
            <span class="label">验证码</span>
            <input
              v-model="verifyCode"
              class="text-input"
              placeholder="mock: 123456"
              @keydown.enter="onLogin"
            />
          </div>
          <div v-if="auth.error" class="row">
            <span class="hint error">{{ auth.error }}</span>
          </div>
          <div class="row actions">
            <button class="btn primary" :disabled="authBusy || !account.trim()" @click="onLogin">
              {{ authBusy ? '…' : '登录' }}
            </button>
          </div>
        </div>

        <!-- 已登录:会员状态 -->
        <div v-else class="group-content">
          <div class="row">
            <span class="label">当前账号</span>
            <span class="mono">{{ auth.user?.nickname }}</span>
          </div>
          <div class="row">
            <span class="label">会员状态</span>
            <span v-if="auth.isVip" class="vip-badge">✓ VIP 至 {{ expireLabel }}</span>
            <span v-else class="hint">未开通</span>
          </div>
          <div v-if="!auth.isVip" class="row">
            <span class="label">兑换码</span>
            <input
              v-model="redeemCode"
              class="text-input redeem-input"
              placeholder="输入兑换码"
              @keydown.enter="onRedeem"
            />
            <button class="btn primary" :disabled="authBusy" @click="onRedeem">
              {{ authBusy ? '…' : '开通' }}
            </button>
          </div>
          <div v-if="auth.error" class="row">
            <span class="hint error">{{ auth.error }}</span>
          </div>
          <div v-if="AUTH_ENABLED" class="row actions">
            <button class="btn" :disabled="authBusy" @click="onLogout">退出登录</button>
          </div>
        </div>
      </div>

      <!-- 老板快捷键 -->
      <div class="group">
        <div class="group-title">🚨 老板快捷键</div>
        <div class="group-desc">按下瞬间隐藏整个应用,再按一次恢复。至少需要一个修饰键 (Ctrl/Alt/Shift)。</div>
        <div class="group-content">
          <div class="row">
            <span class="label">当前快捷键</span>
            <kbd v-if="recording !== 'boss'" class="kbd">{{ bossKeyDisplay }}</kbd>
            <kbd v-else class="kbd recording">
              {{ recordedKey ? humanize(recordedKey) : '按下组合键…' }}
            </kbd>
          </div>
          <div class="row actions">
            <button v-if="recording !== 'boss'" class="btn" :disabled="!!recording" @click="startRecord('boss')">✎ 修改</button>
            <template v-else>
              <button class="btn primary" :disabled="!recordedKey" @click="applyShortcut">应用</button>
              <button class="btn" @click="cancelRecord">取消</button>
              <span class="hint">按 Esc 取消</span>
            </template>
          </div>

          <!-- 触发动作:直接隐藏 / 变成桌面宠物 -->
          <div class="row" style="align-items:flex-start">
            <span class="label">触发动作</span>
            <div style="display:flex;flex-direction:column;gap:6px">
              <label class="radio">
                <input
                  type="radio"
                  :checked="bossAction === 'hide'"
                  @change="onBossAction('hide')"
                />
                <span>直接隐藏 —— 应用瞬间消失</span>
              </label>
              <label class="radio" :class="{ locked: !auth.canUse('pet') }">
                <input
                  type="radio"
                  :checked="bossAction === 'pet'"
                  :disabled="!auth.canUse('pet')"
                  @change="onBossAction('pet')"
                />
                <span>变成桌面宠物 <span class="vip-tag">VIP</span> —— 摸鱼窗口收起,桌面留一只小宠物</span>
              </label>
              <span v-if="!auth.canUse('pet')" class="hint">桌宠为 VIP 功能,未开通时按老板键仍走"直接隐藏"</span>
            </div>
          </div>
        </div>
      </div>

      <!-- 取消透明快捷键 -->
      <div class="group">
        <div class="group-title">👁 取消透明快捷键</div>
        <div class="group-desc">
          透明状态下按钮不好点?按下这个组合键立即把窗口和网页恢复到不透明,再按一次回到原透明度。
          期间"鼠标离开自动隐藏"也会临时关闭,方便操作。
        </div>
        <div class="group-content">
          <div class="row">
            <span class="label">当前快捷键</span>
            <kbd v-if="recording !== 'transp'" class="kbd" :class="{ 'kbd-empty': !transparencyKey }">{{ transpKeyDisplay }}</kbd>
            <kbd v-else class="kbd recording">
              {{ recordedKey ? humanize(recordedKey) : '按下组合键…' }}
            </kbd>
            <span v-if="transparencyKey && !recording" class="hint" style="margin-left:8px">
              {{ bypassOpacity ? '当前:已取消透明' : '当前:按设置的透明度显示' }}
            </span>
          </div>
          <div class="row actions">
            <template v-if="recording !== 'transp'">
              <button class="btn" :disabled="!!recording" @click="startRecord('transp')">
                {{ transparencyKey ? '✎ 修改' : '＋ 设置' }}
              </button>
              <button v-if="transparencyKey" class="btn" :disabled="!!recording" @click="clearTransparencyKey">清除</button>
            </template>
            <template v-else>
              <button class="btn primary" :disabled="!recordedKey" @click="applyShortcut">应用</button>
              <button class="btn" @click="cancelRecord">取消</button>
              <span class="hint">按 Esc 取消</span>
            </template>
          </div>
        </div>
      </div>

      <!-- 独立窗口标题栏主题 -->
      <div class="group">
        <div class="group-title">🪟 独立窗口标题栏</div>
        <div class="group-desc">
          标签页"独立浮窗(popout)"模式下,顶部自绘标题栏的配色。鼠标移到窗口最顶端会滑出,可拖动、置顶、最小化或收回摸鱼窗。
        </div>
        <div class="group-content">
          <div class="row" style="align-items:flex-start">
            <span class="label">标题栏配色</span>
            <div style="display:flex;flex-direction:column;gap:6px">
              <label class="radio">
                <input
                  type="radio"
                  :checked="popoutTheme === 'dark'"
                  @change="emit('update:popoutTheme', 'dark')"
                />
                <span>深色 —— 深色半透明底,浅色字</span>
              </label>
              <label class="radio">
                <input
                  type="radio"
                  :checked="popoutTheme === 'light'"
                  @change="emit('update:popoutTheme', 'light')"
                />
                <span>浅色 —— 浅色半透明底,深色字</span>
              </label>
              <label class="radio">
                <input
                  type="radio"
                  :checked="popoutTheme === 'system'"
                  @change="emit('update:popoutTheme', 'system')"
                />
                <span>跟随系统 —— 自动匹配系统深浅色</span>
              </label>
              <span class="hint">改动会立即应用到已打开的独立浮窗。</span>
            </div>
          </div>
        </div>
      </div>

      <!-- 视频自动横屏 -->
      <div class="group">
        <div class="group-title">🎬 视频播放自动横屏</div>
        <div class="group-desc">检测到当前页面播放视频时,自动切换到 16:9 宽屏比例并进入沉浸模式。当前为占位实现,与画中画一同在后续版本开放。</div>
        <div class="group-content">
          <div class="row">
            <span class="label">启用</span>
            <label class="switch">
              <input
                type="checkbox"
                :checked="videoAutoLandscape"
                @change="emit('update:videoAutoLandscape', ($event.target as HTMLInputElement).checked)"
              />
              <span class="slider"></span>
            </label>
          </div>
          <div v-if="videoAutoLandscape" class="row hint-row">
            <span class="hint">✓ 已开启,功能上线后自动生效,无需再改</span>
          </div>
        </div>
      </div>

      <!-- 鼠标离开自动隐藏 -->
      <div class="group">
        <div class="group-title">🖱️ 鼠标离开自动隐藏</div>
        <div class="group-desc">鼠标移出窗口指定时间后,主窗口淡出,网页临时隐藏;鼠标移回立即恢复。</div>
        <div class="group-content">
          <div class="row">
            <span class="label">启用</span>
            <label class="switch">
              <input
                type="checkbox"
                :checked="autoHide"
                @change="emit('update:autoHide', ($event.target as HTMLInputElement).checked)"
              />
              <span class="slider"></span>
            </label>
          </div>
          <div v-if="autoHide" class="row">
            <span class="label">延迟</span>
            <input
              type="range"
              min="300"
              max="5000"
              step="100"
              :value="autoHideDelay"
              @input="emit('update:autoHideDelay', Number(($event.target as HTMLInputElement).value))"
              style="flex:1;max-width:240px;accent-color:#58a6ff"
            />
            <span class="mono">{{ autoHideDelay }} ms</span>
          </div>
        </div>
      </div>

      <!-- M4 伪装皮肤 (VIP) -->
      <div class="group">
        <div class="group-title">🎭 伪装皮肤 <span class="vip-tag">VIP</span></div>
        <div class="group-desc">
          一键把窗口伪装成工作界面。进入后按 <code>Esc</code> 或点状态栏右侧图标退出,
          老板键恢复时也会自动退出伪装。标题栏 🎭 按钮可随时切换。
        </div>
        <div class="group-content">
          <VipLock feature="disguise" @unlock="onUnlockVip">
            <!-- 选择伪装皮肤:vscode 换壳可继续浏览,excel/ppt 为全屏紧急遮蔽 -->
            <div class="skin-list">
              <div
                v-for="s in SKINS"
                :key="s.key"
                class="skin-card"
                :class="{ act: disguise.skin === s.key }"
                @click="pickSkin(s.key)"
              >
                <div class="skin-name">{{ s.name }}</div>
                <div class="skin-desc">{{ s.desc }}</div>
              </div>
            </div>

            <div class="row hint-row">
              <span class="hint">
                <b>VS Code</b> 皮肤把真实网页包进编辑器外壳,标签、文件树一应俱全,
                内容可正常阅读、滚动、切标签;网页会被染成暗色等宽观感,图片/视频压暗以保证伪装度。
                <b>Excel / PowerPoint</b> 为全屏静态假界面,会中断摸鱼,适合老板已站到身后的紧急场景。
              </span>
            </div>

            <div class="row actions">
              <button class="btn primary" @click="emit('previewDisguise')">▶ 立即伪装</button>
              <span class="hint">
                当前皮肤:{{ SKINS.find(x => x.key === disguise.skin)?.name ?? '' }}
              </span>
            </div>
          </VipLock>
        </div>
      </div>

      <!-- M8 桌面宠物 (VIP) -->
      <div class="group">
        <div class="group-title">🐾 桌面宠物 <span class="vip-tag">VIP</span></div>
        <div class="group-desc">
          把老板快捷键的动作设为"变成桌面宠物",按下后摸鱼窗口收起,桌面只留一只小宠物。
          <b>单击</b>轮流触发跳跃 / 压扁 / 抖动并冒一句话;<b>双击</b>恢复摸鱼界面;
          <b>拖动</b>放到哪就停在哪;<b>滚轮</b>缩放;<b>右键</b>调大小与置顶。
        </div>
        <div class="group-content">
          <VipLock feature="pet" @unlock="onUnlockVip">
            <div class="skin-list">
              <div
                v-for="p in PETS"
                :key="p.key"
                class="skin-card"
                :class="{ act: pet.kind === p.key }"
                @click="pet.setKind(p.key)"
              >
                <img :src="p.img" :alt="p.name" class="skin-img-preview" />
                <div class="skin-info">
                  <div class="skin-name">{{ p.emoji }} {{ p.name }}</div>
                  <div class="skin-desc">{{ p.desc }}</div>
                </div>
              </div>
            </div>

            <div class="row">
              <span class="label">大小</span>
              <input
                class="size-range"
                type="range"
                :min="PET_MIN"
                :max="PET_MAX"
                step="2"
                :value="pet.size"
                @input="onSizeInput"
              />
              <span class="hint">{{ pet.size }} px · 桌面上滚轮也能调</span>
            </div>

            <div class="row">
              <span class="label">窗口置顶</span>
              <label class="switch">
                <input
                  type="checkbox"
                  :checked="pet.alwaysOnTop"
                  @change="pet.setAlwaysOnTop(($event.target as HTMLInputElement).checked)"
                />
                <span class="slider"></span>
              </label>
              <span class="hint">关掉后宠物会被其它窗口挡住</span>
            </div>

            <div class="row">
              <span class="label">视线跟随</span>
              <label class="switch">
                <input
                  type="checkbox"
                  :checked="pet.followMouse"
                  @change="pet.setFollowMouse(($event.target as HTMLInputElement).checked)"
                />
                <span class="slider"></span>
              </label>
              <span class="hint">眼睛跟着鼠标转(宠物不会自己移动,位置始终由你拖拽决定)</span>
            </div>

            <div class="row">
              <span class="label">偶尔转身</span>
              <label class="switch">
                <input
                  type="checkbox"
                  :checked="pet.wander"
                  @change="pet.setWander(($event.target as HTMLInputElement).checked)"
                />
                <span class="slider"></span>
              </label>
              <span class="hint">待着无聊时原地换个朝向</span>
            </div>

            <div class="row">
              <span class="label">监控键盘</span>
              <label class="switch" :class="{ locked: !typingSupported }">
                <input
                  type="checkbox"
                  :checked="pet.typingReact && typingSupported"
                  :disabled="!typingSupported"
                  @change="pet.setTypingReact(($event.target as HTMLInputElement).checked)"
                />
                <span class="slider"></span>
              </label>
              <span class="hint">
                {{ typingSupported
                  ? '你一打字宠物就掏出键盘跟着敲(只感知有没有输入,不记录按了什么键)'
                  : 'macOS 需系统辅助功能权限,暂不支持' }}
              </span>
            </div>

            <div class="row" style="align-items:flex-start">
              <span class="label">台词库</span>
              <div style="flex:1;min-width:0">
                <div class="kw-box">
                  <span v-for="l in pet.lines" :key="l" class="kw">
                    {{ l }}
                    <span class="kw-x" @click="removeLine(l)">×</span>
                  </span>
                </div>
                <div class="row" style="margin-top:6px">
                  <input
                    v-model="newLine"
                    class="text-input"
                    placeholder="新台词,回车添加"
                    @keydown.enter.prevent="addLine"
                  />
                  <button class="btn" @click="addLine">＋ 添加</button>
                  <button class="btn" @click="resetLines">恢复默认</button>
                </div>
              </div>
            </div>

            <div class="row actions">
              <button class="btn primary" @click="emit('previewPet')">▶ 试一下</button>
              <span class="hint">双击宠物或右键"退出桌宠"可收回;老板键也能切回</span>
            </div>
          </VipLock>
        </div>
      </div>

      <!-- M5 工作监测自动隐藏 (VIP) -->
      <div class="group">
        <div class="group-title">🕵️ 工作监测自动隐藏 <span class="vip-tag">VIP</span></div>
        <div class="group-desc">
          后台每 2 秒扫描一次进程列表,命中下列关键字(进程名或路径,不区分大小写)时
          自动切伪装或隐藏。为避免"老板还在旁边就弹回摸鱼界面",进程退出后<b>不会</b>自动恢复,
          需手动退出。
        </div>
        <div class="group-content">
          <VipLock feature="workMonitor" @unlock="onUnlockVip">
            <div class="row">
              <span class="label">启用监测</span>
              <label class="switch">
                <input
                  type="checkbox"
                  :checked="workMonitor"
                  @change="emit('update:workMonitor', ($event.target as HTMLInputElement).checked)"
                />
                <span class="slider"></span>
              </label>
              <span v-if="workMonitor" class="hint">
                {{ workAppPresent ? '⚠ 当前检测到工作应用运行中' : '✓ 监测中,未发现工作应用' }}
              </span>
            </div>

            <template v-if="workMonitor">
              <div class="row">
                <span class="label">命中动作</span>
                <label class="radio">
                  <input
                    type="radio"
                    :checked="workMonitorAction === 'disguise'"
                    @change="emit('update:workMonitorAction', 'disguise')"
                  />
                  <span>切伪装皮肤</span>
                </label>
                <label class="radio">
                  <input
                    type="radio"
                    :checked="workMonitorAction === 'hide'"
                    @change="emit('update:workMonitorAction', 'hide')"
                  />
                  <span>直接隐藏(等同老板键)</span>
                </label>
              </div>

              <div class="row" style="align-items:flex-start">
                <span class="label" style="padding-top:5px">监测关键字</span>
                <div class="kw-box">
                  <span v-for="k in workMonitorKeywords" :key="k" class="kw">
                    {{ k }}
                    <i class="kw-x" @click="removeKeyword(k)" title="移除">×</i>
                  </span>
                </div>
              </div>
              <div class="row">
                <span class="label"></span>
                <input
                  v-model="newKeyword"
                  class="text-input"
                  style="max-width:180px"
                  placeholder="新增关键字,如 zoom"
                  @keydown.enter="addKeyword"
                />
                <button class="btn" :disabled="!newKeyword.trim()" @click="addKeyword">＋ 添加</button>
              </div>
            </template>
          </VipLock>
        </div>
      </div>

      <!-- 用户脚本(迷你油猴) -->
      <div class="group">
        <div class="group-title">🛠 用户脚本（去广告）</div>
        <div class="group-desc">
          粘贴油猴 / Tampermonkey 的 <code>.user.js</code> 源码后添加。支持
          <code>@name</code>、<code>@match</code>、<code>@include</code>、
          <code>@exclude</code>、<code>@run-at</code>，以及
          <code>GM_addStyle / GM_setValue / GM_getValue</code> 等基础接口。
          脚本在标签页创建时注入，<b>新增或修改后需关闭并重新打开标签页生效</b>。
        </div>
        <div class="group-content">
          <div class="row" style="align-items:flex-start">
            <span class="label" style="padding-top:5px">已装脚本</span>
            <div v-if="userScripts.scripts.length" class="us-list">
              <div v-for="(s, i) in userScripts.scripts" :key="s.id" class="us-item">
                <label class="us-check">
                  <input type="checkbox" :checked="s.enabled" @change="toggleScript(s.id, ($event.target as HTMLInputElement).checked)" />
                  <span class="us-name">{{ s.name }}</span>
                </label>
                <span class="us-meta">{{ s.matches[0] || s.includes[0] }}{{ (s.matches.length + s.includes.length) > 1 ? ' 等' : '' }}</span>
                <button class="btn us-order" :disabled="i === 0" @click="moveScript(s.id, -1)" title="上移（先注入）">▲</button>
                <button class="btn us-order" :disabled="i === userScripts.scripts.length - 1" @click="moveScript(s.id, 1)" title="下移（后注入）">▼</button>
                <button class="btn us-del" @click="removeScript(s.id)">删除</button>
              </div>
            </div>
            <span v-else class="hint">还没有脚本。</span>
          </div>
          <div class="row" style="align-items:flex-start">
            <span class="label" style="padding-top:5px">添加脚本</span>
            <div style="display:flex;flex-direction:column;gap:6px;flex:1">
              <textarea
                v-model="scriptDraft"
                class="us-input"
                rows="6"
                spellcheck="false"
                placeholder="// ==UserScript==&#10;// @name        示例去广告&#10;// @match       https://example.com/*&#10;// ==/UserScript==&#10;GM_addStyle('.ad { display:none !important; }');"
              ></textarea>
              <div style="display:flex;align-items:center;gap:8px">
                <button class="btn primary" :disabled="!scriptDraft.trim()" @click="addScript">＋ 添加</button>
                <span v-if="scriptError" class="hint error">{{ scriptError }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 打赏支持 -->
      <div class="group">
        <div class="group-title">☕ 支持云摸鱼</div>
        <div class="group-desc">
          如果这个小工具让你摸鱼愉快,欢迎扫码请作者喝杯咖啡。
          启动时是否弹出此窗口由远端配置控制。
        </div>
        <div class="group-content">
          <div class="ds-qrs">
            <div v-for="img in dashangImages" :key="img.url" class="ds-qr">
              <ZoomableImage :src="img.url" :alt="img.name" img-class="ds-qr-img" />
              <span>{{ img.name }}</span>
            </div>
          </div>
          <div v-if="dashangHidden" class="row" style="margin-top:12px">
            <span class="hint">你之前选择了"不再弹出",</span>
            <button class="btn" @click="resetDashangHidden">重新启用启动弹窗</button>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.settings-view {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  background: rgba(22, 27, 34, calc(var(--shell-alpha) * 0.92));
  z-index: 10;
}

.settings-head {
  height: 44px;
  display: flex;
  align-items: center;
  padding: 0 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  flex-shrink: 0;
}
.settings-head h2 {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
  color: #e6edf3;
  margin-left: 12px;
}
.spacer { flex: 1; height: 100%; }
.back {
  height: 28px;
  padding: 0 10px;
  border: none;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.05);
  color: #c9d1d9;
  cursor: pointer;
  font-size: 12px;
}
.back:hover { background: rgba(255, 255, 255, 0.12); }

.settings-body {
  flex: 1;
  overflow-y: auto;
  padding: 20px 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.group {
  padding: 16px 18px;
  background: rgba(13, 17, 23, 0.4);
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 10px;
}
.group-title {
  font-size: 14px;
  font-weight: 600;
  color: #e6edf3;
  margin-bottom: 4px;
}
.group-desc {
  font-size: 12px;
  color: #8b949e;
  margin-bottom: 12px;
  line-height: 1.6;
}
.group-content {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.row {
  display: flex;
  align-items: center;
  gap: 12px;
}
.row.actions { gap: 8px; }
.row.hint-row { margin-top: -4px; }
.label {
  font-size: 12px;
  color: #c9d1d9;
  min-width: 80px;
}
.hint {
  font-size: 11px;
  color: #6e7681;
}
.mono {
  font-family: ui-monospace, "Cascadia Code", Consolas, monospace;
  font-size: 11px;
  color: #58a6ff;
  min-width: 60px;
}
.kbd {
  font-family: ui-monospace, "Cascadia Code", Consolas, monospace;
  font-size: 12px;
  padding: 5px 10px;
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 4px;
  background: rgba(255, 255, 255, 0.05);
  color: #58a6ff;
  min-width: 120px;
  text-align: center;
}
.kbd.recording {
  border-color: #f85149;
  color: #f85149;
  animation: pulse 1.2s infinite;
}
.kbd.kbd-empty { color: #6e7681; }
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.55; }
}

.btn {
  height: 28px;
  padding: 0 12px;
  border: none;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.06);
  color: #c9d1d9;
  cursor: pointer;
  font-size: 12px;
  transition: background 0.15s;
}
.btn:hover:not(:disabled) { background: rgba(255, 255, 255, 0.12); }
.btn:disabled { opacity: 0.4; cursor: not-allowed; }
.btn.primary { background: #1f6feb; color: #fff; }
.btn.primary:hover:not(:disabled) { background: #388bfd; }

.switch {
  position: relative;
  display: inline-block;
  width: 36px;
  height: 20px;
}
.switch input { opacity: 0; width: 0; height: 0; }
.slider {
  position: absolute;
  inset: 0;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 20px;
  transition: 0.2s;
  cursor: pointer;
}
.slider::before {
  content: "";
  position: absolute;
  left: 3px;
  top: 3px;
  width: 14px;
  height: 14px;
  background: #fff;
  border-radius: 50%;
  transition: 0.2s;
}
.switch input:checked + .slider { background: #1f6feb; }
.switch input:checked + .slider::before { transform: translateX(16px); }

.text-input {
  flex: 1;
  height: 30px;
  max-width: 260px;
  padding: 0 10px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.04);
  color: #e6edf3;
  font-size: 12px;
  outline: none;
}
.text-input:focus { border-color: #58a6ff; }
.size-range {
  width: 160px;
  height: 4px;
  border-radius: 2px;
  background: rgba(255, 255, 255, 0.12);
  accent-color: #58a6ff;
  cursor: pointer;
}
.hint.error { color: #f85149; }
.vip-badge {
  font-size: 12px;
  color: #d4a017;
  font-weight: 600;
}
.vip-tag {
  display: inline-block;
  margin-left: 6px;
  padding: 1px 6px;
  font-size: 10px;
  font-weight: 700;
  color: #1c1c1c;
  background: linear-gradient(135deg, #d4a017, #f0c040);
  border-radius: 4px;
  vertical-align: middle;
}
code {
  padding: 1px 5px;
  background: rgba(255, 255, 255, 0.08);
  border-radius: 4px;
  font-family: ui-monospace, Consolas, monospace;
  font-size: 11px;
  color: #58a6ff;
}

/* M4 皮肤卡片 */
.skin-list { display: flex; gap: 8px; flex-wrap: wrap; }
.skin-card {
  flex: 1;
  min-width: 150px;
  padding: 10px 12px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.03);
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 10px;
  transition: border-color 0.15s, background 0.15s;
}
.skin-img-preview {
  width: 42px;
  height: 42px;
  object-fit: contain;
  flex-shrink: 0;
  filter: drop-shadow(0 2px 4px rgba(0, 0, 0, 0.3));
}
.skin-info {
  flex: 1;
  min-width: 0;
}
.skin-card:hover { background: rgba(255, 255, 255, 0.07); }
.skin-card.act {
  border-color: #1f6feb;
  background: rgba(31, 111, 235, 0.15);
}
.skin-name { font-size: 12px; color: #e6edf3; font-weight: 600; }
.skin-desc { font-size: 11px; color: #6e7681; margin-top: 3px; line-height: 1.4; }

/* M5 关键字标签 */
.kw-box {
  flex: 1;
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  max-height: 92px;
  overflow-y: auto;
}
.kw {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px 6px 3px 8px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.04);
  font-size: 11px;
  color: #c9d1d9;
}
.kw-x {
  font-style: normal;
  cursor: pointer;
  color: #6e7681;
  font-size: 12px;
  line-height: 1;
}
.kw-x:hover { color: #f85149; }

.radio {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 12px;
  color: #c9d1d9;
  cursor: pointer;
}
.radio input { accent-color: #58a6ff; }
/* 无权益 / 平台不支持:置灰但仍可见,让用户知道有这个能力 */
.radio.locked,
.switch.locked {
  opacity: 0.5;
  cursor: not-allowed;
}

/* 用户脚本 */
.group-desc code {
  background: rgba(110, 118, 129, 0.28);
  padding: 0 4px;
  border-radius: 3px;
  font-size: 11px;
}
.us-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
}
.us-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  background: rgba(110, 118, 129, 0.14);
  border-radius: 6px;
}
.us-check {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}
.us-check input { accent-color: #58a6ff; }
.us-name { font-size: 12px; color: #c9d1d9; }
.us-meta {
  flex: 1;
  font-size: 11px;
  color: #8b949e;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.us-del { padding: 2px 8px; font-size: 11px; }
.us-order { padding: 2px 6px; font-size: 10px; line-height: 1; }
.us-order:disabled { opacity: 0.3; cursor: default; }
.us-input {
  width: 100%;
  resize: vertical;
  background: rgba(13, 17, 23, 0.6);
  color: #c9d1d9;
  border: 1px solid rgba(110, 118, 129, 0.4);
  border-radius: 6px;
  padding: 8px;
  font-size: 11px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  line-height: 1.5;
}
.us-input:focus { outline: none; border-color: #58a6ff; }
</style>
