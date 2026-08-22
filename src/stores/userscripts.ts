import { defineStore } from "pinia";
import { ref } from "vue";

export type RunAt = "document-start" | "document-end" | "document-idle";

export interface UserScript {
  id: string;
  name: string;
  code: string;
  matches: string[];
  includes: string[];
  excludes: string[];
  requires: string[];
  runAt: RunAt;
  noFrames: boolean;
  enabled: boolean;
  createdAt: number;
}

const STORAGE_KEY = "moyu-userscripts-v1";

interface RawScript {
  id: string;
  name: string;
  code: string;
  matches: string[];
  includes: string[];
  excludes: string[];
  requires?: string[];
  runAt: string;
  noFrames?: boolean;
  enabled: boolean;
  createdAt: number;
}

function loadPersisted(): UserScript[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as RawScript[];
    return parsed.map((s) => ({
      id: s.id,
      name: s.name,
      code: s.code,
      matches: s.matches ?? [],
      includes: s.includes ?? [],
      excludes: s.excludes ?? [],
      requires: s.requires ?? [],
      runAt:
        s.runAt === "document-start" || s.runAt === "document-idle"
          ? s.runAt
          : "document-end",
      noFrames: s.noFrames === true,
      enabled: s.enabled !== false,
      createdAt: s.createdAt ?? 0,
    }));
  } catch {
    return [];
  }
}

function makeId() {
  return "us_" + Math.floor(performance.now() * 1000).toString(36);
}

/** 从 ==UserScript== 块里取某 key 的所有值(保留出现顺序) */
function metaValues(lines: string[], key: string): string[] {
  const out: string[] = [];
  for (const line of lines) {
    const m = line.match(new RegExp("^//\\s*@" + key + "\\s+(.+?)\\s*$"));
    if (m) out.push(m[1]);
  }
  return out;
}

/**
 * 解析 .user.js 源码。成功返回脚本对象(不含 id/createdAt/enabled),
 * 失败返回错误信息。
 * 必填:==UserScript== ... ==/UserScript== 块、块后有代码体、至少一个 @name。
 */
export function parseUserScript(
  source: string,
): { ok: true; script: Omit<UserScript, "id" | "createdAt" | "enabled"> } | { ok: false; error: string } {
  const src = source.replace(/\r\n/g, "\n");
  const start = src.indexOf("==UserScript==");
  const end = src.indexOf("==/UserScript==");
  if (start < 0 || end < 0 || end < start) {
    return { ok: false, error: "缺少 ==UserScript== 元数据块" };
  }
  const header = src.slice(start + "==UserScript==".length, end);
  const lines = header.split("\n").map((l) => l.trim()).filter(Boolean);

  const names = metaValues(lines, "name");
  const code = src.slice(end + "==/UserScript==".length).trim();
  if (!code) return { ok: false, error: "元数据块后没有脚本代码" };
  const name = (names[0] || "").trim() || "未命名脚本";

  const matches = metaValues(lines, "match");
  const includes = metaValues(lines, "include");
  const excludes = [
    ...metaValues(lines, "exclude"),
    ...metaValues(lines, "exclude_match"),
  ];
  const requires = metaValues(lines, "require")
    .map((u) => u.trim())
    .filter((u) => /^https?:\/\//i.test(u));
  const runRaw = (metaValues(lines, "run-at")[0] || "").trim().toLowerCase();
  const runAt: RunAt =
    runRaw === "document-start" || runRaw === "document-idle"
      ? runRaw
      : "document-end";
  // @noframes:不在子 iframe 中执行(无值,出现即生效)
  const noFrames = lines.some((l) => /^\/\/\s*@noframes\b/.test(l));

  if (!matches.length && !includes.length) {
    return { ok: false, error: "至少需要一条 @match 或 @include 规则" };
  }

  return {
    ok: true,
    script: { name, code, matches, includes, excludes, requires, runAt, noFrames },
  };
}

export const useUserScriptsStore = defineStore("userScripts", () => {
  const scripts = ref<UserScript[]>(loadPersisted());

  function persist() {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(scripts.value));
  }

  function addFromSource(
    source: string,
  ): { ok: true; script: UserScript } | { ok: false; error: string } {
    const parsed = parseUserScript(source);
    if (!parsed.ok) return parsed;
    const script: UserScript = {
      ...parsed.script,
      id: makeId(),
      createdAt: Date.now(),
      enabled: true,
    };
    scripts.value.push(script);
    persist();
    return { ok: true, script };
  }

  function setEnabled(id: string, enabled: boolean) {
    const s = scripts.value.find((x) => x.id === id);
    if (s) {
      s.enabled = enabled;
      persist();
    }
  }

  function remove(id: string) {
    const idx = scripts.value.findIndex((x) => x.id === id);
    if (idx >= 0) {
      scripts.value.splice(idx, 1);
      persist();
    }
  }

  /** 上移(dir=-1)/下移(dir=1),调整注入顺序 */
  function move(id: string, dir: -1 | 1) {
    const idx = scripts.value.findIndex((x) => x.id === id);
    const j = idx + dir;
    if (idx < 0 || j < 0 || j >= scripts.value.length) return;
    const [item] = scripts.value.splice(idx, 1);
    scripts.value.splice(j, 0, item);
    persist();
  }

  return { scripts, addFromSource, setEnabled, remove, move };
});
