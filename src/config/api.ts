// ─────────────────────────────────────────────────────────────
// 远端接口 / 静态资源 集中配置
// 后续改后端地址、静态资源地址、超时,只改本文件即可。
// ─────────────────────────────────────────────────────────────

/** 所有远端请求的统一超时时间(毫秒) */
export const REQUEST_TIMEOUT_MS = 60_000;

/**
 * 本地"强制更新"版本号。
 * 与远端配置 remoteConfig.forceUpdate.version 比较:不一致即视为需要强制更新。
 * 发布新版本需要强制更新时,把这里改成远端下发的 version(整数)。
 */
export const LOCAL_FORCE_UPDATE_VERSION = 1;

/**
 * 远端接口清单。
 * 每个 endpoint 写明 method / url / 返回参数,真实接入时按需替换 url。
 * 目前只有"远端启动配置"是真实地址;鉴权类接口尚未接入后端,url 留空占位。
 */
export const ENDPOINTS = {
  /**
   * 启动时拉取的远端配置(JSON)。
   * method: GET
   * 返回:
   *   {
   *     "popDashang": boolean,      // 是否在启动时弹出打赏弹窗
   *     "lastUpdate": {             // version 与本地不一致时,每次启动都弹更新窗
   *       "url": string,            //   下载/更新页地址(系统浏览器打开)
   *       "message": string,        //   更新说明,多段用逗号或换行分隔
   *       "version": string|number  //   远端最新版本号
   *     },
   *     "forcedUpdate": boolean     // true=更新窗不可关闭(只能立即更新);
   *                                 // false=可关闭,但下次启动仍会再弹
   *   }
   */
  remoteConfig: {
    method: "GET" as const,
    url: "https://raw.githubusercontent.com/MyMaskKing/public_static_resource/main/moyu.json",
  },

  // ── 鉴权类(尚未接入后端,留空占位;接入时填 url,请求/返回契约见 src/services/auth.ts) ──
  /** method: POST  body:{account,code}  返回:AuthStatus */
  authLogin: { method: "POST" as const, url: "" },
  /** method: GET  返回:AuthStatus */
  authStatus: { method: "GET" as const, url: "" },
  /** method: POST  返回:void */
  authLogout: { method: "POST" as const, url: "" },
  /** method: POST  body:{code}  返回:AuthStatus */
  authRedeem: { method: "POST" as const, url: "" },
} as const;

/** 打赏二维码图片(在启动弹窗与"设置-打赏支持"里展示) */
export const DASHANG_IMAGES = [
  { name: "支付宝", url: "https://raw.githubusercontent.com/MyMaskKing/public_static_resource/main/image/blog/qrcode-alipay.jpg" },
  { name: "微信", url: "https://raw.githubusercontent.com/MyMaskKing/public_static_resource/main/image/blog/qrcode-weichat.jpg" },
] as const;

/** 远端启动配置 */
export interface ForceUpdateInfo {
  /** 下载 / 更新页地址(用系统默认浏览器打开) */
  url: string;
  /** 更新说明,多段用逗号或换行分隔 */
  message: string;
  /** 远端最新版本号(字符串或数字,比较时按字符串规范化) */
  version: string | number;
}

export interface RemoteConfig {
  popDashang?: boolean;
  /** 强制更新总开关;为 true 时才检查 lastUpdate */
  forcedUpdate?: boolean;
  lastUpdate?: ForceUpdateInfo;
}

/**
 * 统一的 JSON 请求:自带 60s 超时,非 2xx 抛错。
 * 供 services / 启动逻辑调用,不要在组件里直接 fetch。
 */
export async function requestJson<T>(
  endpoint: { method: "GET" | "POST"; url: string },
  init?: { body?: unknown },
): Promise<T> {
  if (!endpoint.url) throw new Error("endpoint url 未配置");

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
  try {
    const res = await fetch(endpoint.url, {
      method: endpoint.method,
      signal: controller.signal,
      headers: endpoint.method === "POST" ? { "Content-Type": "application/json" } : undefined,
      body: init?.body !== undefined ? JSON.stringify(init.body) : undefined,
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return (await res.json()) as T;
  } finally {
    clearTimeout(timer);
  }
}

/** 拉取远端启动配置;失败返回 null(静默,不打扰启动) */
export async function fetchRemoteConfig(signal?: AbortSignal): Promise<RemoteConfig | null> {
  try {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
    const onAbort = () => controller.abort();
    signal?.addEventListener("abort", onAbort);
    try {
      const res = await fetch(ENDPOINTS.remoteConfig.url, { signal: controller.signal });
      if (!res.ok) return null;
      return (await res.json()) as RemoteConfig;
    } finally {
      clearTimeout(timer);
      signal?.removeEventListener("abort", onAbort);
    }
  } catch {
    return null;
  }
}
