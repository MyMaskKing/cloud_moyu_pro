import { defineStore } from "pinia";
import { ref } from "vue";

export type PetKind = "cat" | "capy" | "shiba" | "duck";

/** 桌宠本体边长范围(逻辑像素),滚轮/滑块都在这个区间内取值 */
export const PET_MIN = 80;
export const PET_MAX = 260;
export const PET_DEFAULT = 140;
/**
 * 窗口顶部为气泡预留的高度。
 * 气泡必须画在本体上方的留白里 —— 桌宠窗口只有本体那么大时,
 * 气泡会被窗口边界裁掉,而不是"浮"在桌面上。
 */
export const PET_BUBBLE_H = 56;

export function clampPetSize(n: number): number {
  if (!Number.isFinite(n)) return PET_DEFAULT;
  return Math.round(Math.min(PET_MAX, Math.max(PET_MIN, n)));
}

/** 默认气泡台词库,用户可在设置里增删 */
export const DEFAULT_PET_LINES: string[] = [
  "老板好像走了…",
  "要不要摸会儿鱼",
  "你已经坐了 40 分钟了",
  "打字这么快,是真在干活?",
  "喝口水吧",
  "颈椎还好吗",
  "偷偷划水中",
  "我帮你看着门",
];

const STORAGE_KEY = "moyu-pet-v1";

interface PetPersisted {
  kind: PetKind;
  /** 本体边长(逻辑像素) */
  size: number;
  /** 视线跟随鼠标 */
  followMouse: boolean;
  /** 偶尔原地转身 */
  wander: boolean;
  /** 打字监控反应(仅 Windows) */
  typingReact: boolean;
  /** 桌宠窗口置顶 */
  alwaysOnTop: boolean;
  /** 气泡台词库 */
  lines: string[];
}

const defaults: PetPersisted = {
  kind: "cat",
  size: PET_DEFAULT,
  followMouse: true,
  wander: true,
  typingReact: true,
  alwaysOnTop: true,
  lines: [...DEFAULT_PET_LINES],
};

/** 旧版本 size 是 "s"|"m"|"l" 三档,kind 里还有已下线的 slime/fish */
const LEGACY_SIZE: Record<string, number> = { s: 96, m: 140, l: 190 };
const LEGACY_KIND: Record<string, PetKind> = { slime: "capy", fish: "shiba" };
const KINDS: PetKind[] = ["cat", "capy", "shiba", "duck"];

function loadPersisted(): PetPersisted {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const p = { ...defaults, ...JSON.parse(raw) } as PetPersisted & { size: unknown; kind: string };
      // 迁移:三档枚举 → 像素值
      p.size =
        typeof p.size === "string"
          ? LEGACY_SIZE[p.size] ?? PET_DEFAULT
          : clampPetSize(Number(p.size));
      // 迁移:已下线的宠物换成同气质的新角色
      if (!KINDS.includes(p.kind as PetKind)) {
        p.kind = LEGACY_KIND[p.kind] ?? defaults.kind;
      }
      return p as PetPersisted;
    }
  } catch {
    /* ignore */
  }
  return { ...defaults };
}

/**
 * M8 桌面宠物配置。
 * 桌宠本体是独立窗口(?view=pet),配置通过 URL query 传给它 —— 那个窗口不挂 pinia,
 * 所以本 store 只负责主窗侧的持久化与设置界面读写。
 * (桌宠窗口会 import 本文件里的纯常量与 clampPetSize,但不会实例化 store)
 */
export const usePetStore = defineStore("pet", () => {
  const p = loadPersisted();
  const kind = ref<PetKind>(p.kind);
  const size = ref<number>(p.size);
  const followMouse = ref(p.followMouse);
  const wander = ref(p.wander);
  const typingReact = ref(p.typingReact);
  const alwaysOnTop = ref(p.alwaysOnTop);
  const lines = ref<string[]>(p.lines);
  /** 桌宠窗口当前是否开着 */
  const active = ref(false);

  function persist() {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        kind: kind.value,
        size: size.value,
        followMouse: followMouse.value,
        wander: wander.value,
        typingReact: typingReact.value,
        alwaysOnTop: alwaysOnTop.value,
        lines: lines.value,
      }),
    );
  }

  function setKind(k: PetKind) { kind.value = k; persist(); }
  function setSize(n: number) { size.value = clampPetSize(n); persist(); }
  function setFollowMouse(v: boolean) { followMouse.value = v; persist(); }
  function setWander(v: boolean) { wander.value = v; persist(); }
  function setTypingReact(v: boolean) { typingReact.value = v; persist(); }
  function setAlwaysOnTop(v: boolean) { alwaysOnTop.value = v; persist(); }
  function setLines(v: string[]) { lines.value = v; persist(); }
  function setActive(v: boolean) { active.value = v; }

  return {
    kind, size, followMouse, wander, typingReact, alwaysOnTop, lines, active,
    setKind, setSize, setFollowMouse, setWander, setTypingReact, setAlwaysOnTop,
    setLines, setActive,
  };
});
