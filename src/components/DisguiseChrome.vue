<script setup lang="ts">
import { onMounted, onBeforeUnmount, computed } from "vue";
import { useTabsStore } from "../stores/tabs";
import type { Tab } from "../stores/tabs";

/**
 * M4 VSCode 换壳外壳(chrome 模式):
 * 在主壳里画出 VSCode 的标题栏 / 活动栏 / 侧边栏 / 标签栏 / 状态栏,
 * 把真实摸鱼网页承载区(holder)围在中间的"编辑器区"。
 * 真实网页是独立 web-tab 子窗,靠 App.vue 的 syncCurrentTab 精确对齐到 slot 区域,
 * 所以这里只负责 chrome,不碰内容。
 *
 * 退出三重保险(与 DisguiseOverlay 一致):
 *   1. 状态栏右侧看起来像通知图标的热区
 *   2. Esc(仅主窗有焦点时;焦点在网页子窗时靠老板键)
 *   3. 老板键(全局,App.vue 已处理)
 */
const emit = defineEmits<{
  (e: "exit"): void;
  (e: "switch", id: string): void;
  (e: "close", id: string): void;
  (e: "open-site", url: string): void;
  (e: "minimize"): void;
  (e: "toggle-maximize"): void;
  (e: "close-win"): void;
}>();

const tabs = useTabsStore();

/** 按域名把摸鱼站点伪装成源码文件名;兜底 index.ts */
function disguiseName(url: string): string {
  let host = "";
  try { host = new URL(url).hostname.toLowerCase(); } catch { return "index.ts"; }
  if (host.includes("weread")) return "reader.ts";
  if (host.includes("bilibili")) return "video.tsx";
  if (host.includes("zhihu")) return "feed.ts";
  if (host.includes("fanqienovel") || host.includes("jjwxc") || host.includes("wxwx")) return "novel.ts";
  if (host.includes("xiaohongshu")) return "notes.tsx";
  if (host.includes("youtube") || host.includes("twitch") || host.includes("netflix")) return "player.tsx";
  if (host.includes("x.com")) return "social.ts";
  if (host.includes("reddit")) return "forum.ts";
  if (host.includes("github")) return "repo.ts";
  return "index.ts";
}

/** 标签按扩展名给一个小徽标,模仿 VSCode 文件图标但只用文字避免穿帮 */
function extBadge(name: string): string {
  if (name.endsWith(".tsx")) return "TSX";
  if (name.endsWith(".vue")) return "VUE";
  if (name.endsWith(".ts")) return "TS";
  if (name.endsWith(".json")) return "{}";
  if (name.endsWith(".md")) return "MD";
  return "·";
}

const activeName = computed(() => (tabs.active ? disguiseName(tabs.active.url) : "index.ts"));
const titleText = computed(() => `${activeName.value} — cloud-order-service — Visual Studio Code`);

/** 侧边栏文件树:部分 .ts 项映射到真实站点(点击打开),其余为静态装饰 */
interface TreeItem { indent: number; name: string; url?: string; }
const TREE: TreeItem[] = [
  { indent: 0, name: "▾ src" },
  { indent: 1, name: "▾ stores" },
  { indent: 2, name: "reader.ts", url: "https://weread.qq.com" },
  { indent: 2, name: "video.tsx", url: "https://www.bilibili.com" },
  { indent: 2, name: "feed.ts", url: "https://www.zhihu.com" },
  { indent: 1, name: "▾ api" },
  { indent: 2, name: "client.ts" },
  { indent: 1, name: "▾ components" },
  { indent: 2, name: "Player.vue" },
  { indent: 1, name: "main.ts" },
  { indent: 0, name: "package.json" },
  { indent: 0, name: "tsconfig.json" },
  { indent: 0, name: "README.md" },
];

function isActiveItem(it: TreeItem): boolean {
  return !!tabs.active && disguiseName(tabs.active.url) === it.name;
}
function onTreeClick(it: TreeItem) {
  if (it.url) emit("open-site", it.url);
}

function onTabClick(t: Tab) { emit("switch", t.id); }
function onTabClose(e: MouseEvent, t: Tab) {
  e.stopPropagation();
  emit("close", t.id);
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    e.stopPropagation();
    emit("exit");
  }
}
onMounted(() => window.addEventListener("keydown", onKeydown, true));
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown, true));
</script>

<template>
  <div class="vs-chrome" @contextmenu.stop.prevent>
    <!-- 标题栏 -->
    <div class="vs-title" data-tauri-drag-region>
      <span class="vs-menu" data-tauri-drag-region>
        文件(F)　编辑(E)　选择(S)　查看(V)　转到(G)　运行(R)　终端(T)　帮助(H)
      </span>
      <span class="vs-tname" data-tauri-drag-region>{{ titleText }}</span>
      <div class="vs-win">
        <button class="vs-wbtn" @click="emit('minimize')" title="最小化">—</button>
        <button class="vs-wbtn" @click="emit('toggle-maximize')" title="最大化">▢</button>
        <button class="vs-wbtn close" @click="emit('close-win')" title="关闭">✕</button>
      </div>
    </div>

    <div class="vs-body">
      <!-- 活动栏 -->
      <div class="vs-activity">
        <span class="vs-ai act">🗎</span>
        <span class="vs-ai">🔍</span>
        <span class="vs-ai">⑂</span>
        <span class="vs-ai">▷</span>
        <span class="vs-ai">⬢</span>
      </div>

      <!-- 侧边栏 -->
      <div class="vs-side">
        <div class="vs-side-h">资源管理器</div>
        <div class="vs-tree">
          <div class="vs-tr root">▾ CLOUD-ORDER-SERVICE</div>
          <div
            v-for="(it, i) in TREE"
            :key="i"
            class="vs-tr"
            :class="[`i${it.indent}`, { sel: isActiveItem(it), clickable: !!it.url }]"
            @click="onTreeClick(it)"
          >{{ it.name }}</div>
        </div>
      </div>

      <!-- 主区:标签栏 + 真实网页(slot) + 状态栏 -->
      <div class="vs-main">
        <div class="vs-tabs">
          <div
            v-for="t in tabs.tabs"
            :key="t.id"
            class="vs-tab"
            :class="{ act: tabs.activeId === t.id }"
            :title="t.url"
            @click="onTabClick(t)"
          >
            <span class="vs-tab-ic">{{ extBadge(disguiseName(t.url)) }}</span>
            <span class="vs-tab-nm">{{ disguiseName(t.url) }}</span>
            <span class="vs-tab-x" @click="onTabClose($event, t)" title="关闭">×</span>
          </div>
        </div>

        <!-- 真实摸鱼网页承载区(App.vue 把 holder 放这里) -->
        <div class="vs-editor">
          <slot />
        </div>

        <div class="vs-status">
          <span>⑂ main*</span>
          <span>⊗ 0　⚠ 0</span>
          <span class="vs-sp"></span>
          <span>Ln 1, Col 1</span>
          <span>UTF-8</span>
          <span>TypeScript</span>
          <span class="exit-hot" @click="emit('exit')" title="退出伪装">退出伪装</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.vs-chrome {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  background: #1e1e1e;
  color: #cccccc;
  font-family: "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 13px;
  overflow: hidden;
  border-radius: 12px;
}

/* 标题栏 */
.vs-title {
  position: relative;
  height: 30px;
  background: #3c3c3c;
  display: flex;
  align-items: center;
  padding: 0 8px;
  font-size: 12px;
  flex-shrink: 0;
  user-select: none;
}
.vs-menu { color: #cccccc; white-space: nowrap; }
.vs-tname {
  position: absolute;
  left: 50%;
  transform: translateX(-50%);
  color: #9d9d9d;
  white-space: nowrap;
  pointer-events: none;
}
.vs-win { margin-left: auto; display: flex; gap: 2px; z-index: 1; }
.vs-wbtn {
  width: 34px;
  height: 24px;
  border: none;
  background: transparent;
  color: #cccccc;
  font-size: 13px;
  cursor: pointer;
  border-radius: 4px;
}
.vs-wbtn:hover { background: rgba(255, 255, 255, 0.1); }
.vs-wbtn.close:hover { background: #e81123; color: #fff; }

/* body 三栏 */
.vs-body { flex: 1; display: flex; min-height: 0; }

.vs-activity {
  width: 48px;
  background: #333333;
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 8px 0;
  gap: 12px;
  flex-shrink: 0;
}
.vs-ai { color: #858585; font-size: 20px; cursor: default; }
.vs-ai.act { color: #fff; }

.vs-side {
  width: 210px;
  background: #252526;
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  overflow: hidden;
}
.vs-side-h {
  padding: 8px 14px 6px;
  font-size: 11px;
  text-transform: uppercase;
  color: #bbbbbb;
  letter-spacing: 0.5px;
}
.vs-tree { padding: 0 4px 8px; overflow-y: auto; }
.vs-tr {
  padding: 2px 8px;
  white-space: nowrap;
  color: #cccccc;
  cursor: default;
  border-radius: 3px;
}
.vs-tr.root { color: #cccccc; font-weight: 600; }
.vs-tr.i1 { padding-left: 18px; }
.vs-tr.i2 { padding-left: 30px; }
.vs-tr.clickable { cursor: pointer; }
.vs-tr.sel { background: #37373d; color: #fff; }

.vs-main { flex: 1; display: flex; flex-direction: column; min-width: 0; }

/* 标签栏 */
.vs-tabs {
  height: 35px;
  background: #252526;
  display: flex;
  align-items: stretch;
  overflow-x: auto;
  overflow-y: hidden;
  flex-shrink: 0;
}
.vs-tabs::-webkit-scrollbar { height: 3px; }
.vs-tab {
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 0 10px;
  background: #2d2d2d;
  color: #969696;
  border-right: 1px solid #1e1e1e;
  cursor: pointer;
  white-space: nowrap;
  min-width: 100px;
  max-width: 180px;
}
.vs-tab.act { background: #1e1e1e; color: #ffffff; }
.vs-tab-ic {
  font-size: 9px;
  font-weight: 700;
  color: #569cd6;
  letter-spacing: -0.5px;
}
.vs-tab-nm { font-size: 13px; overflow: hidden; text-overflow: ellipsis; }
.vs-tab-x {
  margin-left: auto;
  opacity: 0.6;
  font-size: 15px;
  line-height: 1;
  padding: 0 2px;
}
.vs-tab-x:hover { opacity: 1; }

/* 编辑器区:真实网页占位,flex 撑满 */
.vs-editor {
  flex: 1;
  position: relative;
  background: #1e1e1e;
  min-height: 0;
  overflow: hidden;
}
/* slot 里是主壳的 .web-holder(带圆角/深色背景/边框),在编辑区内重置为铺满、无装饰 */
.vs-editor :deep(.web-holder) {
  width: 100%;
  height: 100%;
  border-radius: 0;
  border: none;
  background: #1e1e1e;
}

/* 状态栏 */
.vs-status {
  height: 22px;
  background: #007acc;
  color: #ffffff;
  display: flex;
  align-items: center;
  padding: 0 10px;
  gap: 14px;
  font-size: 11px;
  flex-shrink: 0;
}
.vs-sp { flex: 1; }
.exit-hot {
  cursor: pointer;
  margin-left: 4px;
  padding: 1px 8px;
  border-radius: 3px;
  font-weight: 600;
  background: rgba(255, 255, 255, 0.16);
}
.exit-hot:hover { background: rgba(255, 255, 255, 0.28); }
</style>
