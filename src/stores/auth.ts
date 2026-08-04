import { defineStore } from "pinia";
import { computed, ref } from "vue";
import {
  authService,
  checkEntitlement,
  type AuthStatus,
  type AuthUser,
  type VipFeature,
} from "../services/auth";

/**
 * 鉴权状态:M4(伪装)、M5(工作监测)等 VIP 功能的统一入口。
 * 真实后端接入时只改 services/auth.ts,本 store 不动。
 */
export const useAuthStore = defineStore("auth", () => {
  const status = ref<AuthStatus>({
    loggedIn: false,
    isVip: false,
    entitlements: { disguise: false, workMonitor: false },
  });
  const ready = ref(false);
  const loading = ref(false);
  const error = ref<string | null>(null);

  const user = computed<AuthUser | undefined>(() => status.value.user);
  const isLoggedIn = computed(() => status.value.loggedIn);
  const isVip = computed(() => status.value.isVip);
  const vipExpireAt = computed(() => status.value.vipExpireAt);

  function canUse(feature: VipFeature): boolean {
    return checkEntitlement(status.value, feature).ok;
  }

  function denyReason(feature: VipFeature) {
    const r = checkEntitlement(status.value, feature);
    return r.ok ? null : r.reason;
  }

  async function init() {
    try {
      status.value = await authService.getStatus();
    } finally {
      ready.value = true;
    }
    authService.onStatusChange((s) => {
      status.value = s;
    });
  }

  async function login(account: string, code: string) {
    loading.value = true;
    error.value = null;
    try {
      status.value = await authService.login({ account, code });
    } catch (e) {
      error.value = (e as Error).message;
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function logout() {
    loading.value = true;
    try {
      await authService.logout();
    } finally {
      loading.value = false;
    }
  }

  async function redeem(code: string) {
    loading.value = true;
    error.value = null;
    try {
      status.value = await authService.redeem(code);
    } catch (e) {
      error.value = (e as Error).message;
      throw e;
    } finally {
      loading.value = false;
    }
  }

  return {
    status, ready, loading, error,
    user, isLoggedIn, isVip, vipExpireAt,
    canUse, denyReason,
    init, login, logout, redeem,
  };
});
