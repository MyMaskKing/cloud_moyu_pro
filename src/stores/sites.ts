import { defineStore } from "pinia";
import { computed, ref, watch } from "vue";

/** 站点预设(快捷栏) - 支持默认站 + 用户收藏,均可删除 */
export interface Site {
  name: string;
  icon: string;
  url: string;
  builtin?: boolean; // 默认站,可删除;删除后重装可回来
}

const STORAGE_KEY = "moyu-sites-v3";
const LEGACY_V2_KEY = "moyu-sites-v2";
const LEGACY_V1_KEY = "moyu-sites-v1";

// 默认顺序:热门视频/社交优先,其次阅读,再次海外站
const DEFAULT_SITES: Site[] = [
  { name: "抖音", icon: "🎵", url: "https://www.douyin.com", builtin: true },
  { name: "B 站", icon: "🎬", url: "https://www.bilibili.com", builtin: true },
  { name: "小红书", icon: "📕", url: "https://www.xiaohongshu.com", builtin: true },
  { name: "知乎", icon: "💡", url: "https://www.zhihu.com", builtin: true },
  { name: "微博", icon: "📢", url: "https://weibo.com", builtin: true },
  { name: "微信读书", icon: "📖", url: "https://weread.qq.com", builtin: true },
  { name: "番茄小说", icon: "🍅", url: "https://fanqienovel.com", builtin: true },
  { name: "晋江", icon: "📚", url: "https://m.jjwxc.net", builtin: true },
  { name: "油管", icon: "▶️", url: "https://www.youtube.com", builtin: true },
  { name: "X", icon: "🐦", url: "https://x.com", builtin: true },
  { name: "Reddit", icon: "👽", url: "https://www.reddit.com", builtin: true },
  { name: "Twitch", icon: "🎮", url: "https://www.twitch.tv", builtin: true },
  { name: "Netflix", icon: "🎞️", url: "https://www.netflix.com", builtin: true },
];

function loadPersisted(): Site[] {
  try {
    // v3:已存在直接用(用户拖拽排序后的顺序以此为准)
    const rawV3 = localStorage.getItem(STORAGE_KEY);
    if (rawV3) {
      const parsed = JSON.parse(rawV3) as Site[];
      if (Array.isArray(parsed)) return parsed;
    }
    // 从 v2/v1 迁移:以新的默认顺序为准,把用户自建的非内置站追加在后
    const legacyKey = localStorage.getItem(LEGACY_V2_KEY) ? LEGACY_V2_KEY
      : localStorage.getItem(LEGACY_V1_KEY) ? LEGACY_V1_KEY : null;
    let customs: Site[] = [];
    if (legacyKey) {
      const old = JSON.parse(localStorage.getItem(legacyKey)!) as Site[];
      if (Array.isArray(old)) customs = old.filter((s) => !s.builtin);
      localStorage.removeItem(legacyKey);
    }
    const merged = [...DEFAULT_SITES, ...customs.filter((c) => !DEFAULT_SITES.some((d) => d.url === c.url))];
    localStorage.setItem(STORAGE_KEY, JSON.stringify(merged));
    return merged;
  } catch {
    return [...DEFAULT_SITES];
  }
}

export const useSitesStore = defineStore("sites", () => {
  const sites = ref<Site[]>(loadPersisted());

  watch(sites, (v) => localStorage.setItem(STORAGE_KEY, JSON.stringify(v)), { deep: true });

  const hasUrl = (url: string) => sites.value.some((s) => s.url === url);

  function add(site: Site) {
    if (hasUrl(site.url)) return;
    sites.value.push({ ...site, builtin: false });
  }

  function remove(url: string) {
    const i = sites.value.findIndex((s) => s.url === url);
    if (i >= 0) sites.value.splice(i, 1);
  }

  /** 重命名快捷站点(按 url 定位) */
  function rename(url: string, newName: string) {
    const s = sites.value.find((x) => x.url === url);
    if (s && newName.trim()) s.name = newName.trim();
  }

  /** 拖拽排序:把 fromUrl 对应的项移动到 toUrl 的位置 */
  function reorder(fromUrl: string, toUrl: string) {
    if (fromUrl === toUrl) return;
    const from = sites.value.findIndex((s) => s.url === fromUrl);
    const to = sites.value.findIndex((s) => s.url === toUrl);
    if (from < 0 || to < 0) return;
    const [item] = sites.value.splice(from, 1);
    sites.value.splice(to, 0, item);
  }

  function resetDefaults() {
    sites.value = [...DEFAULT_SITES];
  }

  /** 从 URL 猜 icon(域名首字符 emoji fallback) */
  function guessIcon(url: string): string {
    try {
      const host = new URL(url).hostname.toLowerCase();
      if (host.includes("bilibili")) return "🎬";
      if (host.includes("douyin") || host.includes("tiktok")) return "🎵";
      if (host.includes("xiaohongshu")) return "📕";
      if (host.includes("weibo")) return "📢";
      if (host.includes("zhihu")) return "💡";
      if (host.includes("weread") || host.includes("book")) return "📖";
      if (host.includes("novel") || host.includes("wxwx") || host.includes("jjwxc")) return "📚";
      if (host.includes("youtube")) return "▶️";
      if (host.includes("github")) return "🐙";
    } catch { /* ignore */ }
    return "🌐";
  }

  const canAddCurrent = computed(() => (url: string) => !!url && !hasUrl(url));

  return { sites, add, remove, rename, reorder, resetDefaults, guessIcon, hasUrl, canAddCurrent };
});
