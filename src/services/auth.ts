// 鉴权服务层 —— 当前为 mock 实现,真实后端接入时只需替换本文件,保持导出接口不变。
// M4(伪装皮肤) / M5(工作进程监测) 为 VIP 功能,统一通过本服务判断权益。

import { ENDPOINTS } from "../config/api";

export type VipFeature = "disguise" | "workMonitor" | "pet";

/**
 * 是否接入了鉴权后端。
 * 在 src/config/api.ts 中填入 authStatus 等接口地址后即为 true,走登录/校验流程;
 * 未配置(默认)时直接以本地 VIP 身份使用,所有功能免登录开放。
 */
export const AUTH_ENABLED = !!ENDPOINTS.authStatus.url;

export interface VipEntitlements {
  /** M4:伪装皮肤 */
  disguise: boolean;
  /** M5:工作监测自动隐藏 */
  workMonitor: boolean;
  /** M8:桌面宠物 */
  pet: boolean;
}

export interface AuthUser {
  id: string;
  nickname: string;
  avatar?: string;
}

export interface AuthStatus {
  loggedIn: boolean;
  user?: AuthUser;
  isVip: boolean;
  /** VIP 到期时间(epoch ms);非 VIP 时为 undefined */
  vipExpireAt?: number;
  entitlements: VipEntitlements;
}

export type EntitlementResult =
  | { ok: true }
  | { ok: false; reason: "not-logged-in" | "not-vip" | "expired"; feature: VipFeature };

export interface AuthService {
  /** 拉取当前登录态(启动时调用一次) */
  getStatus(): Promise<AuthStatus>;
  /** 登录(mock:任意非空账号即可;真实环境替换为 OAuth/手机号验证码) */
  login(payload: { account: string; code: string }): Promise<AuthStatus>;
  logout(): Promise<void>;
  /** 兑换/激活码开通(mock:MUOYU-VIP 开通 365 天) */
  redeem(code: string): Promise<AuthStatus>;
  /** 订阅登录态变化,返回取消订阅函数 */
  onStatusChange(cb: (status: AuthStatus) => void): () => void;
}

const STORAGE_KEY = "moyu-auth-mock-v1";
const MOCK_DELAY = 400;
const VIP_REDEEM_CODE = "MUOYU-VIP";
const VIP_DAYS = 365;

const DAY_MS = 24 * 60 * 60 * 1000;

function entitlementsFor(isVip: boolean): VipEntitlements {
  // 当前 VIP 即拥有全部付费功能;后续若按功能单独售卖,在这里细化。
  return { disguise: isVip, workMonitor: isVip, pet: isVip };
}

function isVipActive(status: AuthStatus): boolean {
  // isVip=true 且无到期时间 = 永久 VIP(本地免登录模式);否则校验到期时间
  return !!status.isVip && (!status.vipExpireAt || status.vipExpireAt > Date.now());
}

interface StoredState {
  user?: AuthUser;
  vipExpireAt?: number;
}

function readStore(): StoredState {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}") as StoredState;
  } catch {
    return {};
  }
}

function writeStore(state: StoredState) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
}

function buildStatus(stored: StoredState): AuthStatus {
  const vip = stored.vipExpireAt ? stored.vipExpireAt > Date.now() : false;
  return {
    loggedIn: !!stored.user,
    user: stored.user,
    isVip: vip,
    vipExpireAt: vip ? stored.vipExpireAt : undefined,
    entitlements: entitlementsFor(vip),
  };
}

const listeners = new Set<(s: AuthStatus) => void>();
function emit(s: AuthStatus) {
  listeners.forEach((cb) => cb(s));
}

function delay<T>(value: T): Promise<T> {
  return new Promise((resolve) => setTimeout(() => resolve(value), MOCK_DELAY));
}

// ────── mock 实现 ──────

const mockService: AuthService = {
  async getStatus() {
    return buildStatus(readStore());
  },

  async login({ account, code }) {
    const a = account.trim();
    if (!a) throw new Error("账号不能为空");
    // mock 验证码固定 123456;真实环境替换为短信/邮箱验证码校验
    if (code.trim() !== "123456") throw new Error("验证码错误,测试验证码为 123456");
    const stored = readStore();
    stored.user = { id: "u_" + encodeURIComponent(a), nickname: a };
    writeStore(stored);
    const status = buildStatus(stored);
    emit(status);
    return delay(status);
  },

  async logout() {
    localStorage.removeItem(STORAGE_KEY);
    const status = buildStatus({});
    emit(status);
    await delay(undefined);
  },

  async redeem(code) {
    const stored = readStore();
    if (!stored.user) throw new Error("请先登录");
    if (code.trim() !== VIP_REDEEM_CODE) throw new Error("兑换码无效");
    const base = stored.vipExpireAt && stored.vipExpireAt > Date.now()
      ? stored.vipExpireAt
      : Date.now();
    stored.vipExpireAt = base + VIP_DAYS * DAY_MS;
    writeStore(stored);
    const status = buildStatus(stored);
    emit(status);
    return delay(status);
  },

  onStatusChange(cb) {
    listeners.add(cb);
    return () => listeners.delete(cb);
  },
};

// ────── 本地免登录模式(未配置鉴权接口时) ──────

const localStatus: AuthStatus = {
  loggedIn: true,
  user: { id: "local", nickname: "本地用户" },
  isVip: true,
  vipExpireAt: undefined, // 永久 VIP
  entitlements: { disguise: true, workMonitor: true, pet: true },
};

const localVipService: AuthService = {
  async getStatus() {
    return localStatus;
  },
  async login() {
    return localStatus;
  },
  async logout() {
    // 本地模式无需登录态,退出无意义
  },
  async redeem() {
    return localStatus;
  },
  onStatusChange(cb) {
    listeners.add(cb);
    return () => listeners.delete(cb);
  },
};

// 单例:未配置鉴权接口时直接开放全部 VIP;配置后走 mock/真实登录流程。
export const authService: AuthService = AUTH_ENABLED ? mockService : localVipService;

/** 供 store / 组件统一调用的权益判断 */
export function checkEntitlement(status: AuthStatus, feature: VipFeature): EntitlementResult {
  if (!status.loggedIn) return { ok: false, reason: "not-logged-in", feature };
  if (!isVipActive(status)) return { ok: false, reason: "not-vip", feature };
  if (!status.entitlements[feature]) return { ok: false, reason: "not-vip", feature };
  return { ok: true };
}
