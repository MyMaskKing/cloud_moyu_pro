# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目定位

`cloud-moyu`（云摸鱼）：Tauri 2 + Vue 3 桌面摸鱼工具。对标 Electron 级同类产品，目标包体 ≤10MB / 内存 ≤120MB。主壳无边框透明，每个标签页是**独立的 OS 子窗口**（不是 iframe），承载任意外网站点；另有伪装皮肤、老板键、工作进程监测、画中画、桌面宠物等能力。

- 前端：Vue 3.5 + Vite 6 + Pinia 4 + TypeScript（`src/`）
- 后端：Rust，单文件 `src-tauri/src/lib.rs`（约 1500 行，所有 `#[tauri::command]` 集中于此）
- 目标平台：Windows / macOS（Linux 后置）
- 无测试框架、无 ESLint/Prettier 配置；`vue-tsc --noEmit` 是唯一的静态校验

## 常用命令

```bash
pnpm install                    # 首次安装（必须用 pnpm，仓库有 pnpm-lock.yaml）
pnpm tauri dev                  # 本地开发：起 Vite(1420) + 编译 Rust + 启动应用
pnpm dev                        # 仅起前端 Vite（脱离 Tauri 时大部分 invoke 会失败）
pnpm build                      # 仅前端类型检查 + 构建（vue-tsc --noEmit && vite build）
pnpm tauri build                # 完整打包（前端 + Rust release + 安装包）
```

- Vite 固定端口 `1420`、`strictPort: true`，HMR 备用端口 1421；`watch.ignored` 排除了 `src-tauri/**`。
- 改 Rust 代码后 `tauri dev` 会自动重编译；改 capability/permission TOML 必须重启 `tauri dev`。
- 发布走 GitHub Actions（`.github/workflows/main.yml`）手动触发，三目标：`x86_64-pc-windows-msvc`、`aarch64-apple-darwin`、`x86_64-apple-darwin`（在 ARM runner 上交叉编译 Intel）。

## 多窗口架构（核心心智模型）

应用同时存在多类 Tauri WebviewWindow，全部由 Rust 侧创建和协调：

| label | 用途 | 入口 |
|---|---|---|
| `main` | 主壳（TabBar、地址栏、设置、伪装层） | `App.vue`（挂 Pinia） |
| `web-tab-{id}` | 每个标签页一个独立 OS 窗口，物理贴在主窗的 holder 区域里 | 外链 URL，通过 `WebviewUrl::External` |
| `ctx-menu` | 右键菜单（独立置顶透明窗，失焦自关） | `index.html?view=ctxmenu` → `ContextMenuStandalone.vue` |
| `pet` | 桌面宠物（透明置顶小窗） | `index.html?view=pet` → `PetStandalone.vue` |

`src/main.ts` 读取 `?view=` query **分派入口**：只有默认入口挂 Pinia；ctxmenu/pet 是独立的 Vue app，不共享 store，配置通过 query string 传入。

### 标签页是真窗口，不是 iframe

`stores/tabs.ts` 只管标签的元数据（id/url/title/mode/pinned）和 localStorage 持久化；实际的"网页面板"由 Rust 命令 `open_web_tab` / `resize_web_tab` / `close_web_tab` / `set_web_tab_visible_only` 等操控子窗口位置与显隐。切换标签不是切 DOM，而是 `show()` 一个子窗、`hide()` 其他子窗（`set_web_tab_visible_only`）。冷启动后 tabs store 已恢复但 webview 尚未创建，用 `web_tab_exists` 探测。

### Tab 四种模式（`TabMode`）

- `inline`：贴在主窗 holder 里，`decorations=false`、`skip_taskbar=true`、owner=main
- `popout`：独立可缩放窗，`decorations=true`、`skip_taskbar=false`，注入的 JS 会在页面右上角画一个可拖动的 📌 置顶按钮
- `pip`：置顶小窗（`enter_pip` / `exit_pip`）
- `fullscreen`：应用内全屏（不是 OS 全屏）

模式切换涉及一组 Rust 命令：`set_web_tab_chrome`（装饰/任务栏/置顶/标题）、`set_web_tab_owner`（Win32 `SetWindowLongPtrW(GWLP_HWNDPARENT)` 控制主窗最小化是否连带）、`set_web_tab_resizable`。`set_web_tab_owner` 和透明度（`set_web_tab_opacity`，`SetLayeredWindowAttributes`）是 Windows-only，`#[cfg(not(windows))]` 分支是 no-op，跨平台改动时不要在 macOS 上等它们生效。

### 注入脚本（`open_web_tab` 里的 `init_script`）

每个 `web-tab-*` 子窗创建时都注入一大段 IIFE，承担四类职责，改任一行为都要改这里而不是外链页面：

1. 拦截 `window.open` 和 `target="_blank"` 点击 → 同 tab 内跳转，避免弹出系统窗
2. Hook `Element.prototype.requestFullscreen` 及各前缀 → 派发 `signal_video_fullscreen`，由主窗做"应用内全屏"（给目标元素套 `position:fixed;z-index:2147483647`）；ESC/F 退出
3. 监听 `<video>` play/playing/timeupdate，在"视频播放页 + 横屏 + 时长>60s + 宽≥480"时派发 `signal_video_play`，主窗按用户设置决定是否自动横屏/PiP；SPA 切页通过 URL 变化重置触发锁
4. popout 模式下自绘标题栏与 📌 按钮：注入右上角可拖动的置顶按钮（位置存 localStorage，点击派发 `signal_pin_toggle`）；并因 popout 关闭了系统装饰，注入一套自绘窗口控件——拖动走 `popout_start_drag`，最小化/收回走 `popout_window_action`，最大化/还原走 `popout_toggle_maximize`（均只操作"调用方自身那个子窗"，按注入的 tab id 定位）

`request_video_fullscreen`（主窗 → 子窗）优先模拟站点自己的全屏按钮（YouTube/B 站等 selector 列表），找不到才回退到对播放器容器 `requestFullscreen`。改选择器时这两处都要顾到。

### Tauri v2 ACL：自定义命令必须显式白名单

`src-tauri/permissions/app-commands.toml` 定义了 `allow-app-commands`，**列出所有自定义 command 名**。一旦项目里存在任何 `permissions/*.toml`，Tauri v2 就连 local origin 的自定义命令也强制 ACL 校验——新增 `#[tauri::command]` 后必须：

1. 在 `lib.rs` 的 `tauri::generate_handler![...]` 注册
2. 在 `app-commands.toml` 的 `commands.allow` 里追加
3. 如果要让**外链网页**（remote URL）调用，还需在 `capabilities/web-tabs.json` 的 `permissions` 里追加。当前对 remote 放开的命令拆成两组 permission，各自定义在 `src-tauri/permissions/` 下：
   - `video-signals.toml`：`signal_video_play` / `signal_video_fullscreen` / `signal_pin_toggle`
   - `popout-controls.toml`：`popout_start_drag` / `popout_toggle_maximize` / `popout_window_action`（popout 自绘标题栏用，按注入的 tab id 只操作"调用方自身那个子窗"，无跨窗/任意句柄能力）

`capabilities/` 下四个文件按窗口 label 授权：`default.json`（main）、`web-tabs.json`（`web-tab-*`，含 remote url 通配）、`ctx-menu.json`、`pet.json`。新增窗口 label 时要同步加 capability。

## 关键设计约束（改动时不要破坏）

- **老板键是救命键，不能依赖前端往返**。`Ctrl+Alt+Q`（默认，可改）的处理全在 Rust 快捷键回调里；桌宠配置由前端预先通过 `set_pet_config` 缓存到 `static PET_CONFIG: Mutex<_>`，按键时 Rust 直接 `do_enter_pet_mode` 开窗，不走 emit → 前端 → invoke。开窗失败**必须退化为 `toggle_hide`**（宁可少个宠物，不能让用户按键后屏幕无反应）。
- **改窗口几何/建窗/关窗的命令必须是 `async`**。同步命令跑在主线程，而 `set_size`/`set_position`/`WebviewWindowBuilder::build()` 要主线程事件循环继续转才能完成，同步等就是自锁（表现为整个应用无响应）。文件里所有这类命令都已是 async，新增同类命令照做，原因注释见 `enter_pet_mode`/`set_pet_size`。
- **关闭子窗用 `destroy()` 而非 `close()`**：`open_web_tab` 给子窗挂了 `CloseRequested` → `api.prevent_close()` 来把系统 ✕ 转成 `web-tab-close-requested` 事件，主动关窗必须绕开拦截。
- **CSP 是 `null`（关闭）**，因为子窗加载任意外链。不要为了"看起来更安全"而收紧 CSP，会把外网站点打坏。
- **静音要在 `hide()` 之前 eval**：部分平台 hide 后 webview 不响应 eval（见 `toggle_hide` / `apply_web_tab_mute`）。恢复时按记录的原始 muted 回写，不强行取消用户自己设的静音。
- **桌宠打字监控只判"有键按下"，不记录内容**：`GetAsyncKeyState` 扫 0x08–0xDE 区段，仅翻转 `pet-typing` 布尔态。刻意不做键位/次数统计，避免这个功能变成键盘记录器。`set_pet_typing(false)` 要能让探测线程完全不扫键盘。
- **桌宠缩放锚点是底边中点**（`set_pet_size`），不是左上角——宠物"站"在桌面上，按左上角缩放会蹿位。

## 前端模块地图

- `src/App.vue`（~1500 行）：主壳控制器，持有所有设置 state（localStorage key `moyu-settings-v1`），监听 Rust 事件（`work-app-detected/gone`、`web-tab-close-requested`、`web-tab-video-*`、`pet-*`、`boss-key-toggled`、`transparency-toggle-requested`），协调 tabs/disguise/pet stores 与 Rust 命令。设置的默认值和 `DEFAULT_WORK_KEYWORDS`（钉钉/腾讯会议/Zoom/飞书/Teams/企业微信）在此。
- `src/stores/`：`tabs`、`sites`（快捷栏预设，`moyu-sites-v2`，含 v1→v2 迁移）、`disguise`（`moyu-disguise-v1`）、`pet`（`moyu-pet-v1`）、`auth`。所有持久化都用带版本号的 localStorage key，改 schema 时写迁移而不是改名丢数据。
- `src/components/DisguiseChrome.vue`：VSCode 换壳式伪装（包住真实网页继续浏览）。
- `src/components/DisguiseOverlay.vue`：Excel/PPT 遮蔽式伪装（全屏静态假界面）。
- `src/views/SettingsView.vue`（~1040 行，App.vue 内挂的设置面板，**非独立窗口**）：集中 M4 皮肤、M8 桌宠等 VIP 设置项，直接读写 `auth`/`disguise`/`pet` store。改设置项 UI 时改这里而不是 App.vue。
- `src/composables/useMouseAutoHide.ts`：鼠标离开主窗+所有子窗 N 秒后整体变淡。**不能靠 DOM mouseleave/enter**——子窗是独立 OS 窗口，鼠标进子窗对主窗 DOM 是 mouseleave。实现走 `get_cursor_position` + 每个 `web-tab-bounds` 的全局矩形命中。
- `src/services/auth.ts`：鉴权 mock 层。M4 伪装 / M5 工作监测 / M8 桌宠是 VIP 功能，统一 `authStore.canUse(feature)` 门控。真实后端接入时**只替换本文件**，保持导出接口不变，store 与组件不动。

## 伪装皮肤两套机制

1. **换壳/遮蔽式**（当前主路径）：`disguise` store 的 `skin: vscode | excel | ppt`。vscode → `DisguiseChrome` 套住真实网页可继续浏览；excel/ppt → `DisguiseOverlay` 全屏盖住。激活时 App.vue 调 `set_all_tabs_hidden` 把子窗一并隐藏（伪装层只遮主窗，独立子窗会穿帮）。
2. **阅读式注入**（旧路径，仍保留）：`set_reading_disguise(true)` 往所有子窗 eval 一段固定 id 的 `<style>`（`__moyu_disguise_style`），把网页染成暗色等宽、图片/视频压到 `opacity:.06`，不动 DOM 布局。移除 style 即完全还原。

两者独立，不要假设一个开关能驱动两边。

## M5 工作进程监测

Rust 侧 `spawn_work_monitor` 每 2 秒 `sysinfo::refresh_processes_specifics`（只刷进程名 + exe 路径，不算使用率），对进程名/路径做大小写不敏感子串匹配。**只在状态翻转时 emit**（命中→`work-app-detected`，消失→`work-app-gone`），不重复打扰。关键字、启用、命中后动作（disguise/hide）全部前端下发，Rust 不写死名单。

## M8 桌宠

- 宠物是紧贴宠物尺寸（含顶部 `PET_BUBBLE_H=68` 气泡留白）的透明置顶窗，不是全屏透明窗（透明区会挡桌面点击）。刻意不为气泡加宽窗口。
- 探测线程 220ms 一轮，鼠标位移 >6px 才 emit `pet-cursor`；键盘用约 2.6s（12 ticks）空闲滞回去抖。
- 四个角色（lopcat/psyduck/hamster/corgi）各 6 帧 PNG 在 `src/images/`，`PetStandalone.vue`（~890 行）负责动画、视线跟随、点击互动、打字陪敲。
- 老板键可配成"隐藏"或"切桌宠"（`set_boss_action_pet`）；桌宠开着时再按老板键 = `do_exit_pet_mode` 恢复主窗，且必须把所有子窗 `show()` 回来（Rust 当初是直接 hide 的，前端只对活跃 tab 调 visible，不 show 全部会出现"标签还在但内容空白"）。
