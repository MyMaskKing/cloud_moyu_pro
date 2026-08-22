# cloud_moyu · 云摸鱼

**极致轻量、伪装更强、纯免费开源**的桌面摸鱼工具。

对标 Electron 级同类产品（~100MB），本项目基于 **Tauri 2 + Vue 3 + Rust**，目标包体 **≤ 10MB**、内存 **≤ 120MB**。主壳无边框透明，每个标签页是**独立的 OS 子窗口**（不是 iframe），可承载任意外网站点，并提供伪装皮肤、老板键、工作进程监测、画中画、桌面宠物、用户脚本等能力。

## ✨ 功能特性

- 🪟 **多标签真窗口浏览**：每个标签是独立 OS 子窗口，物理贴在主窗承载区；支持 inline（贴窗）/ popout（独立窗）/ pip（画中画）/ 应用内全屏四种模式
- 🧩 **用户脚本（迷你油猴）**：内置轻量油猴引擎，支持 `@match` / `@include` / `@exclude` / `@require` / `@run-at`，提供 `GM_xmlhttpRequest`（Rust 后端代理，绕过 CORS）、`GM_addStyle`、`GM_setValue/getValue`、`GM_openInTab` 等 API；脚本增删改后**热加载**，已打开标签自动生效
- 🎭 **伪装皮肤**：VSCode（换壳，包住真实网页可继续浏览）/ Excel / PPT（全屏遮蔽）
- 🕵️ **工作监测自动隐藏**：后台轮询进程，命中钉钉 / 腾讯会议 / Zoom / 飞书 / Teams / 企业微信等时，秒切伪装或隐藏，只在状态翻转时触发
- 🚨 **全局老板键**：`Ctrl+Alt+Q`（可改），处理全在 Rust 快捷键回调，不依赖前端往返；可配置为"隐藏"或"切桌宠"，开窗失败自动退化为隐藏
- 🖱️ **鼠标移出自动隐藏** + 透明度调节 + 窗口置顶
- 📺 **视频增强**：拦截网页全屏转"应用内全屏"、视频播放自动横屏 / 进画中画
- 🐾 **桌面宠物**：奶白垂耳猫 / 黄色可达鸭 / 雾灰蓝仓鼠 / 焦糖柯基，独立透明置顶小窗，可拖动缩放、视线跟随、点击互动、打字陪敲（仅检测"有键按下"，不记录内容）

## 🧱 技术栈

| 项 | 选择 |
|---|---|
| 桌面框架 | Tauri 2.x |
| 后端 | Rust（单文件 `src-tauri/src/lib.rs`，所有 `#[tauri::command]` 集中于此） |
| 前端 | Vue 3.5 + Vite 6 + TypeScript |
| 状态 | Pinia 4 |
| HTTP（用户脚本） | ureq（rustls，无 OpenSSL 依赖） |
| 打包目标 | Windows / macOS（Linux 后置） |

## 🚀 开发

```bash
pnpm install          # 首次（必须用 pnpm）
pnpm tauri dev        # 本地开发：Vite(1420) + 编译 Rust + 启动应用
pnpm dev              # 仅起前端（脱离 Tauri 时大部分 invoke 会失败）
pnpm build            # 前端类型检查 + 构建（vue-tsc --noEmit && vite build）
pnpm tauri build      # 完整打包（前端 + Rust release + 安装包）
```

- Vite 固定端口 `1420`、`strictPort: true`。
- 改 Rust 代码后 `tauri dev` 自动重编译；改 `capabilities/` / `permissions/` 的 TOML/JSON **必须重启 `tauri dev`** 才生效。
- 无测试框架、无 ESLint/Prettier；`vue-tsc --noEmit` 是唯一的静态校验。

## 🧩 用户脚本（迷你油猴）

设置页可粘贴 `.user.js` 源码导入。解析 `==UserScript==` 元数据块，支持：

- `@match` / `@include` / `@exclude` / `@exclude_match`：URL 匹配
- `@require https://...`：外链依赖（如 jQuery），由 Rust 预拉取并缓存，在用户代码前注入
- `@run-at document-start | document-end | document-idle`：注入时机（idle 为 DOM 就绪后让出一轮）
- `@noframes`：声明后不在子 iframe 中执行
- GM API：`GM_info` / `unsafeWindow` / `GM_addStyle` / `GM_setValue` / `GM_getValue` / `GM_deleteValue` / `GM_listValues` / `GM_xmlhttpRequest` / `GM_openInTab`

### 实现要点

- 脚本在子窗**建窗时**通过 WebView2 的 `initialization_script`（`AddScriptToExecuteOnDocumentCreated`）注入，这是唯一能保证 `@run-at document-start` 在页面前执行的方式；该字符串建窗后固化、不可更改。
- 因此脚本增删改 / 启停后，已打开的 inline / fullscreen 标签会**销毁子窗并按同一 URL / 几何重建**以加载新脚本（等价于一次彻底刷新，会丢失该标签的滚动 / SPA / 表单状态）。popout / pip 浮窗下次打开时自然生效。
- `GM_xmlhttpRequest` 由 Rust 命令 `us_xhr` 用 `ureq` 在阻塞线程里发起，绕过浏览器 CORS / 混合内容限制；能请求任意 http(s) URL，等同于安装 Tampermonkey 后脚本获得的跨域权限，脚本来源与启停由用户在设置页控制。
- 冷启动首次同步会 `await` 脚本下发（含 `@require` 预拉取）完成后再建子窗，避免竞态导致插件不运行。

## 🏗️ 多窗口架构

应用同时存在多类 Tauri WebviewWindow，全部由 Rust 侧创建和协调：

| label | 用途 | 入口 |
|---|---|---|
| `main` | 主壳（TabBar、地址栏、设置、伪装层） | `App.vue`（挂 Pinia） |
| `web-tab-{id}` | 每个标签页一个独立 OS 窗口，承载外链 URL | `WebviewUrl::External` |
| `ctx-menu` | 右键菜单（独立置顶透明窗，失焦自关） | `index.html?view=ctxmenu` |
| `pet` | 桌面宠物（透明置顶小窗） | `index.html?view=pet` |

`src/main.ts` 按 `?view=` query 分派入口；ctxmenu / pet 是独立 Vue app，不共享 store。

> 完整的架构约束、注入脚本职责、Tauri v2 ACL 规则见 [`CLAUDE.md`](./CLAUDE.md)。

## 🔐 安全与隐私

- CSP 为 `null`（关闭），因为子窗加载任意外链；不要为"看起来更安全"而收紧，会打坏外网站点。
- 桌宠打字监控只判"有键按下"（扫键码区段翻转布尔态），**不记录键位 / 次数 / 内容**，刻意避免变成键盘记录器。
- 用户脚本 `GM_xmlhttpRequest` 的跨域能力属于用户主动安装脚本的预期授权，脚本可在设置页随时停用 / 删除。

## 📦 发布

走 GitHub Actions（`.github/workflows/main.yml`）手动触发，三目标：`x86_64-pc-windows-msvc`、`aarch64-apple-darwin`、`x86_64-apple-darwin`（ARM runner 交叉编译 Intel）。

## 🗺️ 里程碑

- [x] **M0** 脚手架 · 无边框透明窗口
- [x] **M1** 单标签 MVP
- [x] **M2** 多标签 + 画中画 + 内置站点预设
- [x] **M3** 老板键 · 透明度 · 鼠标移出自动隐藏 · 置顶
- [x] **M4** VSCode / Excel / PPT 伪装皮肤
- [x] **M5** 工作进程监测自动隐藏
- [x] **M6** 画中画 + 视频自动横屏
- [x] **M8** 桌面宠物（4 角色 · 拖动 / 缩放 / 视线跟随 / 点击互动 · 打字陪敲）
- [x] **M9** 用户脚本（迷你油猴引擎 · @require · GM_xmlhttpRequest · 热加载）
- [ ] **M7** 打包 · 官网 · 双平台发行

## 📄 许可证

核心功能永久免费、开源可审计。本工具仅供学习与合理使用，请遵守所在公司的规章制度。
