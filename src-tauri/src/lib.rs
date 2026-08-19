// M1-M3: 单标签 WebView + 老板键 + 自动隐藏
// M2:    多标签 + 画中画 PiP + 视频横屏一体化
// M2 修复: capabilities 覆盖到 web-tab-* + opener scope 放开 http/https
// M2 交互: popout 用系统标题栏可拖可关, close 事件切回 inline
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::Duration;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const DEFAULT_BOSS_KEY: &str = "Ctrl+Alt+KeyQ";

/// 外链子窗(web-tab-*)使用的现代 Chrome UA。
/// macOS WKWebView 默认 UA 不含高版本 Chrome 标识,会被 B站等站点判为"浏览器版本过旧"拦截;
/// 显式声明一个现代 Chrome UA 通过版本门禁,与 Windows 上 WebView2 的真实 Chrome UA 表现更一致。
const WEB_TAB_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
     AppleWebKit/537.36 (KHTML, like Gecko) \
     Chrome/122.0.0.0 Safari/537.36";

/// 全局隐藏态(老板键切换用)
static HIDDEN: AtomicBool = AtomicBool::new(false);
/// 当前已注册的老板键
static CURRENT_BOSS_KEY: Mutex<Option<Shortcut>> = Mutex::new(None);
/// 当前已注册的"取消透明"快捷键(可选,用户没设就是 None)
static CURRENT_TRANSPARENCY_KEY: Mutex<Option<Shortcut>> = Mutex::new(None);
/// 记录所有已开启的 web tab label,用于老板键批量隐藏/恢复
static WEB_TABS: Mutex<Vec<String>> = Mutex::new(Vec::new());

// ────── 用户脚本(迷你油猴) ──────
//
// 前端把用户在设置里粘贴的 .user.js 解析为元数据后下发到这里缓存;
// open_web_tab 创建子窗时把启用的脚本拼进 initialization_script,
// 在 document-start 注入一个轻量 runner,提供基础 GM_* API(GM_addStyle /
// GM_setValue / GM_getValue 等,存储走子窗 localStorage),按 @match/@include/
// @exclude 决定是否执行。不提供 GM_xmlhttpRequest / @require,不做网络请求。
// 脚本与页面同源,权限等同页面自身脚本(ACL 不额外提权)。

#[derive(Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct UserScript {
    id: String,
    name: String,
    code: String,
    matches: Vec<String>,
    includes: Vec<String>,
    excludes: Vec<String>,
    run_at: String,
    enabled: bool,
}

/// 前端下发的启用脚本全集(仅 main 窗口调用,见 app-commands.toml)
static USER_SCRIPTS: Mutex<Vec<UserScript>> = Mutex::new(Vec::new());

#[tauri::command]
fn set_user_scripts(scripts: Vec<UserScript>) -> Result<(), String> {
    match USER_SCRIPTS.lock() {
        Ok(mut g) => {
            *g = scripts;
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

/// 生成注入到每个 web-tab 子窗的用户脚本 runner 代码。
/// 无启用脚本时返回空串,不注入任何东西。
fn build_userscript_boot() -> String {
    let scripts = match USER_SCRIPTS.lock() {
        Ok(g) => g.iter().filter(|s| s.enabled).cloned().collect::<Vec<_>>(),
        Err(_) => return String::new(),
    };
    if scripts.is_empty() {
        return String::new();
    }
    let json = match serde_json::to_string(&scripts) {
        Ok(j) => j,
        Err(_) => return String::new(),
    };
    // 注意:JSON 已是合法 JS 字面量,直接内联。runner 自身用普通函数作用域隔离,
    // 用户代码再经 new Function 包裹,顶层变量不污染页面全局。
    format!(
        r#"
        (function(){{
          var SCRIPTS = {json};
          if (!SCRIPTS.length) return;
          function escRe(s){{ return String(s).replace(/[.*+?^${{}}()|[\]\\]/g,'\\$&'); }}
          // @match 语法:scheme://host/path,* 为通配;<all_urls> 匹配任意 http(s)
          function patToRe(pat){{
            if (pat === '<all_urls>') return /^https?:\/\/.+/;
            var m = /^(\*|https?):\/\/([^/]+)(\/.*)?$/.exec(pat);
            if (!m) return /(?!)/; // 非法规则永不匹配
            var scheme = m[1]==='*' ? 'https?' : m[1];
            var host = m[2], path = m[3] || '/';
            var hostRe;
            if (host === '*') hostRe = '[^/]+';
            else if (host.charAt(0)==='*' && host.charAt(1)==='.')
              hostRe = '(?:[^/]+\\.)?' + escRe(host.slice(2));
            else hostRe = escRe(host);
            var pathRe = escRe(path).replace(/\\\*/g,'.*');
            return new RegExp('^' + scheme + '://' + hostRe + pathRe + '$');
          }}
          // @include 旧语法:含 * 当 glob 整串匹配,不含则按子串匹配
          function incToRe(inc){{
            if (inc.indexOf('*') >= 0)
              return new RegExp('^' + escRe(inc).replace(/\\\*/g,'.*') + '$');
            return {{ test: function(v){{ return v.indexOf(inc) >= 0; }} }};
          }}
          function matchLoc(s){{
            var loc = location.href;
            if (s.excludes.some(function(p){{ return patToRe(p).test(loc); }})) return false;
            if (s.matches.length || s.includes.length)
              return s.matches.some(function(p){{ return patToRe(p).test(loc); }})
                  || s.includes.some(function(p){{ return incToRe(p).test(loc); }});
            return false; // 无 include/match 规则 = 不注入(安全默认)
          }}
          function makeGm(s){{
            var ns = '__moyu_gm_' + s.id + '_';
            return {{
              GM_info: {{ scriptHandler:'cloud-moyu', version:'1.0',
                          script: {{ name:s.name, namespace:'moyu' }} }},
              unsafeWindow: window,
              GM_addStyle: function(css){{
                var el = document.createElement('style');
                el.textContent = css;
                (document.head || document.documentElement).appendChild(el);
                return el;
              }},
              GM_setValue: function(k,v){{
                try {{ localStorage.setItem(ns+k, JSON.stringify(v)); }} catch(e) {{}}
              }},
              GM_getValue: function(k,d){{
                try {{ var r = localStorage.getItem(ns+k);
                       return r==null ? d : JSON.parse(r); }} catch(e) {{ return d; }}
              }},
              GM_deleteValue: function(k){{ try {{ localStorage.removeItem(ns+k); }} catch(e) {{}} }},
              GM_listValues: function(){{
                return Object.keys(localStorage).filter(function(x){{
                  return x.indexOf(ns)===0;
                }}).map(function(x){{ return x.slice(ns.length); }});
              }}
            }};
          }}
          function run(s){{
            var g = makeGm(s);
            try {{
              (new Function('GM_info','GM_addStyle','GM_setValue','GM_getValue',
                'GM_deleteValue','GM_listValues','unsafeWindow', s.code))
                (g.GM_info, g.GM_addStyle, g.GM_setValue, g.GM_getValue,
                 g.GM_deleteValue, g.GM_listValues, g.unsafeWindow);
            }} catch(e) {{ console.warn('[userscript] ' + s.name + ':', e); }}
          }}
          var startNow = [], later = [];
          SCRIPTS.forEach(function(s){{
            if (!matchLoc(s)) return;
            (s.runAt === 'document-start' ? startNow : later).push(s);
          }});
          startNow.forEach(run);
          if (later.length) {{
            function fire(){{ later.forEach(run); }}
            if (document.readyState === 'loading')
              document.addEventListener('DOMContentLoaded', fire, {{ once:true }});
            else fire();
          }}
        }})();
"#,
        json = json
    )
}

// ────── M5: 工作进程监测 ──────
//
// 轮询系统进程列表,匹配"工作类"应用(钉钉/腾讯会议/Zoom 等),
// 命中时 emit `work-app-detected`,消失时 emit `work-app-gone`,
// 由前端按用户配置决定切伪装或隐藏。
//
// 设计要点:
// - 轮询线程在 setup 中常驻;是否启用、匹配关键字、轮询间隔由前端下发
// - 用 serde 透传 JSON,避免在 Rust 侧定死进程名单(前端可配置)
// - 命中/消失都只在状态翻转时 emit,不重复打扰

/// M5 监测配置(关键字列表)。空 = 关闭监测。
static WORK_MONITOR_ENABLED: AtomicBool = AtomicBool::new(false);
static WORK_MONITOR_KEYWORDS: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// 上次轮询是否已命中,用于状态翻转去重
static WORK_MONITOR_TRIGGERED: AtomicBool = AtomicBool::new(false);

/// 前端下发工作监测配置。keywords 为进程名/可执行路径的子串(大小写不敏感)。
#[tauri::command]
fn set_work_monitor_config(enabled: bool, keywords: Vec<String>) {
    WORK_MONITOR_ENABLED.store(enabled, Ordering::Relaxed);
    if let Ok(mut g) = WORK_MONITOR_KEYWORDS.lock() {
        *g = keywords
            .into_iter()
            .map(|k| k.trim().to_lowercase())
            .filter(|k| !k.is_empty())
            .collect();
    }
    // 切换配置时复位"已命中"状态,让下一轮按新配置重新判定
    WORK_MONITOR_TRIGGERED.store(false, Ordering::Relaxed);
}

/// 轮询线程主体:每 2 秒刷新一次进程列表,做大小写不敏感的子串匹配。
fn spawn_work_monitor(app: AppHandle) {
    std::thread::spawn(move || {
        use sysinfo::{ProcessesToUpdate, ProcessRefreshKind, System, UpdateKind};

        let mut sys = System::new();
        loop {
            std::thread::sleep(Duration::from_secs(2));

            if !WORK_MONITOR_ENABLED.load(Ordering::Relaxed) {
                continue;
            }
            let keywords = match WORK_MONITOR_KEYWORDS.lock() {
                Ok(g) if !g.is_empty() => g.clone(),
                _ => continue,
            };

            sys.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                // 只取我们需要的:进程名 + 可执行路径,不浪费 CPU 算使用率
                ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
            );

            let hit = sys.processes().values().any(|p| {
                let name = p.name().to_string_lossy().to_lowercase();
                let exe = p
                    .exe()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                keywords.iter().any(|kw| name.contains(kw) || exe.contains(kw))
            });

            let was_hit = WORK_MONITOR_TRIGGERED.load(Ordering::Relaxed);
            if hit && !was_hit {
                WORK_MONITOR_TRIGGERED.store(true, Ordering::Relaxed);
                let _ = app.emit("work-app-detected", ());
            } else if !hit && was_hit {
                WORK_MONITOR_TRIGGERED.store(false, Ordering::Relaxed);
                let _ = app.emit("work-app-gone", ());
            }
        }
    });
}

/// 批量隐藏/显示所有 web-tab(M5 切伪装时用:伪装层遮住主窗,但独立子窗会穿帮,必须一并隐藏)
#[tauri::command]
fn set_all_tabs_hidden(app: AppHandle, hidden: bool) {
    let labels = WEB_TABS.lock().map(|g| g.clone()).unwrap_or_default();
    for l in labels {
        if let Some(win) = app.get_webview_window(&l) {
            if hidden {
                let _ = win.hide();
            } else {
                let _ = win.show();
            }
        }
    }
}

// ────── M4 阅读式伪装:往 web-tab 注入暗色等宽样式 ──────
//
// 与"遮蔽式"(DisguiseOverlay 全屏盖住)并存的第二种模式:
// 只注入一个带固定 id 的 <style>,把网页染成 VSCode 暗色等宽观感,
// 不改动页面 DOM 结构 —— 原有滚动/点击/翻页全部保留,移除 style 即完全还原。
//
// CSS 策略故意保守:只改配色/字体/行高,不动布局。
// 图片/视频压到极低不透明度(而非 display:none,否则布局塌陷),这是伪装成立的关键。

const READING_DISGUISE_CSS: &str = concat!(
    "html,body{background:#1e1e1e !important;}",
    "*{background-color:transparent !important;border-color:#3c3c3c !important;",
    "color:#d4d4d4 !important;font-family:Consolas,\"Cascadia Code\",\"Courier New\",monospace !important;",
    "text-shadow:none !important;box-shadow:none !important;}",
    "a,a *{color:#569cd6 !important;}",
    "h1,h2,h3,h4,h5,h6{color:#4ec9b0 !important;font-weight:600 !important;}",
    "img,video,canvas,svg,iframe{opacity:.06 !important;filter:grayscale(1) !important;}",
    "::selection{background:#264f78 !important;}",
    "::-webkit-scrollbar{width:10px !important;}",
    "::-webkit-scrollbar-thumb{background:#424242 !important;}",
    "::-webkit-scrollbar-track{background:#1e1e1e !important;}",
);

const READING_DISGUISE_STYLE_ID: &str = "__moyu_disguise_style";

/// 阅读式伪装:往所有 web-tab 注入 / 移除暗色等宽样式。
/// 对已注入的窗口幂等(新开 tab 时可安全重复调用)。
#[tauri::command]
fn set_reading_disguise(app: AppHandle, enabled: bool) -> Result<(), String> {
    let js = if enabled {
        format!(
            r#"(function(){{
                try {{
                    if (document.getElementById('{id}')) return;
                    var s = document.createElement('style');
                    s.id = '{id}';
                    s.textContent = {css};
                    (document.head || document.documentElement).appendChild(s);
                }} catch(e) {{}}
            }})();"#,
            id = READING_DISGUISE_STYLE_ID,
            // serde_json 负责转义引号,避免手工拼 JS 字符串出错
            css = serde_json::to_string(READING_DISGUISE_CSS).map_err(|e| e.to_string())?,
        )
    } else {
        format!(
            r#"(function(){{
                try {{
                    var s = document.getElementById('{id}');
                    if (s) s.remove();
                }} catch(e) {{}}
            }})();"#,
            id = READING_DISGUISE_STYLE_ID,
        )
    };

    let labels = WEB_TABS.lock().map(|g| g.clone()).unwrap_or_default();
    for l in labels {
        if let Some(win) = app.get_webview_window(&l) {
            let _ = win.eval(&js);
        }
    }
    Ok(())
}

fn tab_label(id: &str) -> String {
    format!("web-tab-{id}")
}
fn remember_tab(label: String) {
    if let Ok(mut g) = WEB_TABS.lock() {
        if !g.iter().any(|l| l == &label) {
            g.push(label);
        }
    }
}

fn forget_tab(label: &str) {
    if let Ok(mut g) = WEB_TABS.lock() {
        g.retain(|l| l != label);
    }
}

// ────── M2: 多标签 WebView 协调 ──────

#[tauri::command]
async fn open_web_tab(
    app: AppHandle,
    id: String,
    url: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let label = tab_label(&id);

    // 已存在 = 只切位置/尺寸并显示(可能是复用同一 tab 的 inline 刷新)
    if let Some(existing) = app.get_webview_window(&label) {
        existing.set_position(LogicalPosition::new(x, y)).map_err(|e| e.to_string())?;
        existing.set_size(LogicalSize::new(width, height)).map_err(|e| e.to_string())?;
        existing.show().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let main = app.get_webview_window("main").ok_or("main window missing")?;
    let parsed = WebviewUrl::External(url.parse().map_err(|e: url::ParseError| e.to_string())?);

    // 注入脚本:
    //   1. 重写 window.open / _blank → 在当前 tab 内跳转
    //   2. 拦截 HTML5 fullscreen → 转为主窗口的"应用内全屏"(不让子窗占满 OS)
    //   3. 首个 <video> play → 触发"自动横屏"信号(设置开启才响应)
    let init_script_tpl = r#"
        (function() {
          const TAB_ID = "__MUOYU_TAB_ID__";
          const bridge = window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke;
          const invoke = function(cmd, args) {
            try { if (bridge) bridge(cmd, args || {}); } catch (e) {}
          };

          // ── window.open / _blank 拦截 ──
          const nativeOpen = window.open;
          window.open = function(url, target, features) {
            if (url) {
              try {
                window.location.href = new URL(url, window.location.href).href;
              } catch (e) {
                return nativeOpen.call(window, url, target, features);
              }
              return window;
            }
            return nativeOpen.call(window, url, target, features);
          };
          document.addEventListener('click', function(e) {
            const a = e.target && e.target.closest && e.target.closest('a[href]');
            if (a && a.target === '_blank' && a.href) {
              e.preventDefault();
              e.stopPropagation();
              window.location.href = a.href;
            }
          }, true);

          // ── HTML5 fullscreen 拦截:改由主窗口做"应用内全屏" ──
          //   同时把目标元素套 position:fixed 全屏样式,让视频真填满 webview
          let fakeFsEl = null;
          let fakeFsPrev = null; // 保存原始 style,退出时还原
          const FS_STYLE = 'position:fixed !important;left:0 !important;top:0 !important;width:100vw !important;height:100vh !important;z-index:2147483647 !important;background:#000 !important;margin:0 !important;padding:0 !important;';
          function enterFake(el) {
            fakeFsEl = el;
            try {
              fakeFsPrev = el.getAttribute('style') || '';
              el.setAttribute('style', fakeFsPrev + ';' + FS_STYLE);
              // 页面 body 禁止滚动
              document.documentElement.style.overflow = 'hidden';
              document.body.style.overflow = 'hidden';
            } catch (e) {}
            try {
              Object.defineProperty(document, 'fullscreenElement', {
                configurable: true, get: function() { return fakeFsEl; }
              });
              Object.defineProperty(document, 'webkitFullscreenElement', {
                configurable: true, get: function() { return fakeFsEl; }
              });
            } catch (e) {}
            try { document.dispatchEvent(new Event('fullscreenchange')); } catch (e) {}
            try { document.dispatchEvent(new Event('webkitfullscreenchange')); } catch (e) {}
            try { window.__muoyuSetFs && window.__muoyuSetFs(true); } catch (e) {}
            invoke('signal_video_fullscreen', { id: TAB_ID, entering: true });
          }
          // 退出:先还原样式 + 派 fullscreenchange 让站点收到"退出"消息,
          //   一次性归零站点内部状态(按钮 icon / class),
          //   再 signal 给主窗回嵌入模式
          function exitFake() {
            if (!fakeFsEl) {
              // 未进 fakeFs 也让主窗保险回一次嵌入(避免 signal 漏发导致卡在 fullscreen mode)
              try { invoke('signal_video_fullscreen', { id: TAB_ID, entering: false }); } catch (e) {}
              return;
            }
            try {
              if (fakeFsPrev !== null) fakeFsEl.setAttribute('style', fakeFsPrev);
              else fakeFsEl.removeAttribute('style');
              document.documentElement.style.overflow = '';
              document.body.style.overflow = '';
            } catch (e) {}
            fakeFsEl = null;
            fakeFsPrev = null;
            try {
              Object.defineProperty(document, 'fullscreenElement', {
                configurable: true, get: function() { return null; }
              });
              Object.defineProperty(document, 'webkitFullscreenElement', {
                configurable: true, get: function() { return null; }
              });
            } catch (e) {}
            try { document.dispatchEvent(new Event('fullscreenchange')); } catch (e) {}
            try { document.dispatchEvent(new Event('webkitfullscreenchange')); } catch (e) {}
            try { window.__muoyuSetFs && window.__muoyuSetFs(false); } catch (e) {}
            invoke('signal_video_fullscreen', { id: TAB_ID, entering: false });
          }
          const proto = Element.prototype;
          proto.requestFullscreen = function() { enterFake(this); return Promise.resolve(); };
          if (proto.webkitRequestFullscreen)     proto.webkitRequestFullscreen     = function() { enterFake(this); };
          if (proto.webkitRequestFullScreen)     proto.webkitRequestFullScreen     = function() { enterFake(this); };
          if (proto.msRequestFullscreen)         proto.msRequestFullscreen         = function() { enterFake(this); };
          // exit 同时 hook Document.prototype 和 document 实例:
          //   YouTube/B站有时走 Document.prototype.exitFullscreen.call(document),必须原型也换
          const dproto = Document.prototype;
          if (dproto.exitFullscreen)       dproto.exitFullscreen       = function() { exitFake(); return Promise.resolve(); };
          if (dproto.webkitExitFullscreen) dproto.webkitExitFullscreen = function() { exitFake(); };
          if (dproto.msExitFullscreen)     dproto.msExitFullscreen     = function() { exitFake(); };
          document.exitFullscreen        = function() { exitFake(); return Promise.resolve(); };
          document.webkitExitFullscreen  = function() { exitFake(); };
          document.msExitFullscreen      = function() { exitFake(); };
          // 页面读 document.fullscreen / webkitIsFullScreen 判断当前状态时也要认账
          try {
              Object.defineProperty(document, 'fullscreen', {
                  configurable: true, get: function() { return !!fakeFsEl; }
              });
              Object.defineProperty(document, 'webkitIsFullScreen', {
                  configurable: true, get: function() { return !!fakeFsEl; }
              });
          } catch (e) {}
          // 键盘 ESC / F 退出:hook keydown
          document.addEventListener('keydown', function(e) {
            if (e.key === 'Escape' && fakeFsEl) { e.stopPropagation(); exitFake(); }
          }, true);

          // ── 视频 play → 自动横屏信号(严格过滤:短视频/封面缩略图不触发) ──
          //   条件:
          //   1. 当前 URL 是"视频播放页"(而不是首页/推荐/搜索页,避免一进站就全屏)
          //   2. videoWidth ≥ 480,duration > 60s,宽高比 > 1.4(横屏)
          //   SPA 站(B站/油管)切页 URL 变时,重置触发锁允许下一个视频页再触发
          const VIDEO_PAGE_RE = /(^|\.)bilibili\.com\/(video\/|bangumi\/play\/)|(^|\.)youtube\.com\/(watch|shorts\/|embed\/)|(^|\.)douyin\.com\/(video\/|user\/[^/]+\/video)|(^|\.)iqiyi\.com\/(v_|w_|play\/)|(^|\.)v\.qq\.com\/x\/(cover|page)\/|(^|\.)youku\.com\/v(_show)?\//i;
          function isVideoPage() {
            try {
              const h = location.host || '';
              const p = location.pathname || '';
              return VIDEO_PAGE_RE.test(h + p);
            } catch (e) { return false; }
          }
          let videoSignaled = false;
          let lastUrl = location.href;
          function scanPlayingVideos() {
            if (videoSignaled) return;
            if (!isVideoPage()) return;
            var vids = document.querySelectorAll('video');
            for (var i = 0; i < vids.length; i++) {
              var v = vids[i];
              // 已经在播放的视频:SPA 复用场景不会再派 play 事件,主动检查
              if (!v.paused && v.readyState >= 1) checkVideoForAutoFs(v);
            }
          }
          setInterval(function() {
            if (location.href !== lastUrl) {
              lastUrl = location.href;
              videoSignaled = false;
              // URL 变到视频页时,给站点点时间挂载播放器,再主动扫一次
              setTimeout(scanPlayingVideos, 800);
              setTimeout(scanPlayingVideos, 2000);
              setTimeout(scanPlayingVideos, 4000);
            }
          }, 500);
          function checkVideoForAutoFs(v) {
            if (videoSignaled) return;
            if (!v || v.tagName !== 'VIDEO') return;
            if (!isVideoPage()) return;
            const w = v.videoWidth || 0;
            const h = v.videoHeight || 0;
            const d = v.duration || 0;
            if (w < 480 || h < 1 || d < 60) return;
            if (w / h < 1.4) return;
            videoSignaled = true;
            invoke('signal_video_play', { id: TAB_ID });
          }
          document.addEventListener('play', function(e) {
            if (!e.target || e.target.tagName !== 'VIDEO') return;
            // 若 metadata 还没加载,等 loadedmetadata 再判定
            if (e.target.readyState < 1) {
              e.target.addEventListener('loadedmetadata', function once() {
                e.target.removeEventListener('loadedmetadata', once);
                checkVideoForAutoFs(e.target);
              });
            } else {
              checkVideoForAutoFs(e.target);
            }
          }, true);
          // playing / timeupdate 兜底:SPA 复用 video 元素不派 play,但 timeupdate 一定会派
          document.addEventListener('playing', function(e) {
            if (e.target && e.target.tagName === 'VIDEO') checkVideoForAutoFs(e.target);
          }, true);
          document.addEventListener('timeupdate', function(e) {
            if (videoSignaled) return;
            if (e.target && e.target.tagName === 'VIDEO' && !e.target.paused) {
              checkVideoForAutoFs(e.target);
            }
          }, true);

          // ── popout 自绘极简标题栏 ──
          //   popout 改为 decorations=false(无系统标题栏),由这里画一条 34px 标题栏:
          //   平时隐藏,鼠标碰到窗口最顶部 5px 热区时滑出,离开约 0.9s 后收回,不挡网页。
          //   左侧网页标题(可拖动窗口/双击最大化),右侧 📌置顶 / ➖最小化 / ✕收回摸鱼窗。
          //   主题(深/浅/跟随系统)由主窗通过 __muoyuOnMode 下发。
          var tbBar = null, tbPin = null, tbHideTimer = null, tbInFs = false, tbActive = false;
          var TB_THEME = 'system';
          function tbPalette(name) {
            if (name === 'dark') return { bg:'rgba(22,27,34,.92)', fg:'#c9d1d9', hover:'rgba(255,255,255,.10)', closeBg:'#e81123', closeFg:'#fff' };
            if (name === 'light') return { bg:'rgba(245,246,248,.94)', fg:'#1f2328', hover:'rgba(0,0,0,.07)', closeBg:'#e81123', closeFg:'#fff' };
            return window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches
              ? tbPalette('dark') : tbPalette('light');
          }
          function tbApplyTheme() {
            if (!tbBar) return;
            var p = tbPalette(TB_THEME);
            tbBar.style.background = p.bg;
            tbBar.style.color = p.fg;
            var btns = tbBar.querySelectorAll('[data-tb-btn]');
            for (var i = 0; i < btns.length; i++) {
              var b = btns[i];
              b.style.color = p.fg;
              b.setAttribute('data-tb-hover', b.getAttribute('data-tb-close') !== null ? p.closeBg : p.hover);
              if (b.getAttribute('data-tb-close') !== null) b.setAttribute('data-tb-close-fg', p.closeFg);
            }
          }
          function tbShow() {
            if (!tbBar) return;
            if (tbHideTimer) { clearTimeout(tbHideTimer); tbHideTimer = null; }
            tbBar.style.transform = 'translateY(0)';
            tbBar.style.pointerEvents = 'auto';
          }
          function tbScheduleHide() {
            if (tbHideTimer) clearTimeout(tbHideTimer);
            tbHideTimer = setTimeout(function() {
              if (!tbBar) return;
              tbBar.style.transform = 'translateY(-100%)';
              tbBar.style.pointerEvents = 'none';
              tbHideTimer = null;
            }, 900);
          }
          function tbCancelHide() { if (tbHideTimer) { clearTimeout(tbHideTimer); tbHideTimer = null; } }
          function tbUpdateTitle() {
            if (!tbBar) return;
            var t = (document.title || location.hostname || '').trim();
            var el = tbBar.querySelector('[data-tb-title]');
            if (el) el.textContent = t;
          }
          function ensureTitlebar() {
            if (tbBar) return tbBar;
            var host = document.documentElement || document.body;
            // 标题栏本体
            tbBar = document.createElement('div');
            tbBar.setAttribute('data-muoyu-titlebar', '');
            tbBar.style.cssText = [
              'position:fixed','top:0','left:0','right:0','height:34px',
              'display:flex','align-items:center',
              'padding:0 6px 0 12px','box-sizing:border-box',
              'font:12px/1 -apple-system,Segoe UI,Microsoft YaHei,sans-serif',
              'z-index:2147483647','pointer-events:none',
              'transform:translateY(-100%)','transition:transform .18s ease',
              'box-shadow:0 1px 6px rgba(0,0,0,.18)','will-change:transform'
            ].join(';') + ';';
            var title = document.createElement('div');
            title.setAttribute('data-tb-title', '');
            title.style.cssText = 'flex:1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;opacity:.85;padding-right:8px;';
            tbBar.appendChild(title);
            function mkBtn(sym, key, titleText, isClose) {
              var b = document.createElement('div');
              b.setAttribute('data-tb-btn', '');
              if (isClose) b.setAttribute('data-tb-close', '');
              b.textContent = sym;
              b.title = titleText;
              b.style.cssText = 'width:30px;height:24px;display:flex;align-items:center;justify-content:center;border-radius:4px;cursor:pointer;font-size:13px;user-select:none;transition:background .12s;';
              b.addEventListener('mouseenter', function() {
                if (b.__pinned) return; // 置顶态保持蓝底,hover 不覆盖
                b.style.background = b.getAttribute('data-tb-hover') || 'rgba(127,127,127,.2)';
                if (isClose) b.style.color = b.getAttribute('data-tb-close-fg') || '#fff';
              });
              b.addEventListener('mouseleave', function() {
                if (b.__pinned) return; // 置顶态保持蓝底
                b.style.background = 'transparent';
                if (isClose) b.style.color = tbPalette(TB_THEME).fg;
              });
              b.addEventListener('click', function(e) {
                e.stopPropagation();
                if (key === 'pin') {
                  var next = !b.__pinned;
                  invoke('signal_pin_toggle', { id: TAB_ID, pinned: next });
                  applyPinVisual(next);
                } else if (key === 'min') {
                  invoke('popout_window_action', { id: TAB_ID, action: 'minimize' });
                } else if (key === 'close') {
                  invoke('popout_window_action', { id: TAB_ID, action: 'close' });
                }
              });
              return b;
            }
            tbPin = mkBtn('📌', 'pin', '窗口置顶', false);
            tbBar.appendChild(tbPin);
            tbBar.appendChild(mkBtn('➖', 'min', '最小化', false));
            tbBar.appendChild(mkBtn('✕', 'close', '收回摸鱼窗口', true));
            // 拖动窗口 / 双击最大化:点在按钮上不触发
            tbBar.addEventListener('mousedown', function(e) {
              if (e.button !== 0) return;
              if (e.target && e.target.hasAttribute && e.target.hasAttribute('data-tb-btn')) return;
              e.preventDefault();
              if (e.detail >= 2) invoke('popout_toggle_maximize', { id: TAB_ID });
              else invoke('popout_start_drag', { id: TAB_ID });
            });
            tbBar.addEventListener('mouseenter', tbCancelHide);
            tbBar.addEventListener('mouseleave', tbScheduleHide);
            host.appendChild(tbBar);
            // 顶部热区:用 document 捕获式 mousemove 判"光标距顶 ≤5px",
            //   不放置任何 DOM 元素,避免拦截窗口上边缘的原生 resize 和外站点击。
            //   网页全屏(fake fullscreen)期间也允许滑出标题栏(类似 YouTube 全屏顶栏)。
            document.addEventListener('mousemove', function(e) {
              if (!tbActive) return;
              if (e.clientY <= 5) tbShow();
            }, true);
            // 标题变化(SPA 切页)
            tbUpdateTitle();
            setInterval(tbUpdateTitle, 1500);
            try {
              var mo = new MutationObserver(tbUpdateTitle);
              var tEl = document.querySelector('title');
              if (tEl) mo.observe(tEl, { childList: true, subtree: true });
            } catch (e) {}
            // 跟随系统主题变化
            if (window.matchMedia) {
              try { window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function() { if (TB_THEME === 'system') tbApplyTheme(); }); } catch (e) {}
            }
            return tbBar;
          }
          function applyPinVisual(pinned) {
            if (!tbPin) return;
            tbPin.__pinned = pinned;
            tbPin.style.background = pinned ? 'rgba(31,111,235,.85)' : 'transparent';
            tbPin.style.color = pinned ? '#fff' : tbPalette(TB_THEME).fg;
            tbPin.title = pinned ? '取消置顶' : '窗口置顶';
          }
          // 主窗调:window.__muoyuOnMode(mode, pinned, theme)
          //   theme: 'dark' | 'light' | 'system'
          window.__muoyuOnMode = function(mode, pinned, theme) {
            ensureTitlebar();
            var show = mode === 'popout';
            tbActive = show;
            if (show) {
              // 从"应用内全屏"直接切到独立窗口时,网页内 fake fullscreen 仍在
              //   (视频被 position:fixed 撑满、tbInFs=true),会盖住标题栏并让标题栏无法滑出。
              //   进 popout 时退出它,还原视频样式并复位 tbInFs。
              //   主窗监听收到 signal(fullscreen,false)时因 mode 已是 popout 会直接 return,无副作用。
              try { if (typeof exitFake === 'function') exitFake(); } catch (e) {}
            }
            tbBar.style.display = show ? 'flex' : 'none';
            if (!show) { tbCancelHide(); tbBar.style.transform = 'translateY(-100%)'; tbBar.style.pointerEvents = 'none'; }
            if (theme === 'dark' || theme === 'light' || theme === 'system') TB_THEME = theme;
            tbApplyTheme();
            applyPinVisual(!!pinned);
            if (show) tbUpdateTitle();
          };
          // 网页内全屏(fake fullscreen)期间强制隐藏标题栏,避免挡视频
          window.__muoyuSetFs = function(inFs) {
            tbInFs = !!inFs;
            if (tbInFs) { tbCancelHide(); if (tbBar) { tbBar.style.transform = 'translateY(-100%)'; tbBar.style.pointerEvents = 'none'; } }
          };
        })();
    "#;
    let init_script = init_script_tpl.replace("__MUOYU_TAB_ID__", &id);
    // 用户脚本 runner(无启用脚本时为空串),追加在应用自身注入之后
    let init_script = init_script + &build_userscript_boot();

    let win = WebviewWindowBuilder::new(&app, &label, parsed)
        .parent(&main).map_err(|e| e.to_string())?
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .inner_size(width, height)
        .position(x, y)
        .initialization_script(init_script)
        .user_agent(WEB_TAB_USER_AGENT)
        .build()
        .map_err(|e| e.to_string())?;

    // popout 模式下用户按系统 ✕ 时,不销毁 tab 而是通知前端"回到摸鱼窗口"
    let id_for_event = id.clone();
    let app_for_event = app.clone();
    let label_for_event = label.clone();
    win.on_window_event(move |ev| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = ev {
            // 主窗在:拦截系统 ✕,交给主窗前端决定"切回 inline"还是"真关"。
            // 主窗已关闭(孤儿独立窗):放行原生关闭,否则 prevent 后事件无人接收,窗口永远关不掉。
            if app_for_event.get_webview_window("main").is_some() {
                api.prevent_close();
                let _ = app_for_event.emit(
                    "web-tab-close-requested",
                    serde_json::json!({ "id": id_for_event.clone(), "label": label_for_event.clone() }),
                );
            } else {
                forget_tab(&label_for_event);
            }
        }
    });

    let _ = win.show();
    remember_tab(label);
    Ok(())
}

#[tauri::command]
async fn resize_web_tab(
    app: AppHandle,
    id: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(&tab_label(&id)) {
        win.set_position(LogicalPosition::new(x, y)).map_err(|e| e.to_string())?;
        win.set_size(LogicalSize::new(width, height)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn close_web_tab(app: AppHandle, id: String) -> Result<(), String> {
    let label = tab_label(&id);
    if let Some(win) = app.get_webview_window(&label) {
        // 用 destroy() 强制销毁,绕过窗口自身的 CloseRequested 拦截
        // (open_web_tab 里挂了 prevent_close 用来处理"用户点系统 ✕",不能拦到后端主动关闭)
        win.destroy().map_err(|e| e.to_string())?;
    }
    forget_tab(&label);
    Ok(())
}

/// 销毁所有 web-tab 子窗口(destroy 绕过 CloseRequested 拦截)。
/// 主窗关闭时用它级联清理,否则 popout 等独立子窗会成为孤儿留在桌面上。
fn destroy_all_web_tabs(app: &AppHandle) {
    let labels = WEB_TABS.lock().map(|g| g.clone()).unwrap_or_default();
    for l in labels {
        if let Some(win) = app.get_webview_window(&l) {
            let _ = win.destroy();
        }
    }
    if let Ok(mut g) = WEB_TABS.lock() {
        g.clear();
    }
}

/// 兜底:关掉所有 web-tab 子窗口(用于"最后一个标签关闭后清空幽灵画面")
#[tauri::command]
async fn close_all_web_tabs(app: AppHandle) -> Result<(), String> {
    destroy_all_web_tabs(&app);
    Ok(())
}

#[tauri::command]
async fn set_web_tab_visible(app: AppHandle, id: String, visible: bool) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(&tab_label(&id)) {
        if visible { win.show().map_err(|e| e.to_string())?; }
        else { win.hide().map_err(|e| e.to_string())?; }
    }
    Ok(())
}

/// 前端查:某 tab 的子窗口当前是否存在(冷启动后 tabs store 已恢复但 webview 还没建时会返回 false)
#[tauri::command]
fn web_tab_exists(app: AppHandle, id: String) -> bool {
    app.get_webview_window(&tab_label(&id)).is_some()
}

/// 获取 web-tab 子窗口在屏幕上的物理坐标矩形,用于"鼠标是否在窗口上"判断
#[tauri::command]
fn web_tab_bounds(app: AppHandle, id: String) -> Option<(i32, i32, u32, u32)> {
    let win = app.get_webview_window(&tab_label(&id))?;
    let pos = win.outer_position().ok()?;
    let size = win.outer_size().ok()?;
    Some((pos.x, pos.y, size.width, size.height))
}

/// 全局光标屏幕坐标(物理像素)
#[tauri::command]
fn get_cursor_position(app: AppHandle) -> Result<(f64, f64), String> {
    let p = app.cursor_position().map_err(|e| e.to_string())?;
    Ok((p.x, p.y))
}

// ────── 导航:后退 / 前进 / 刷新 ──────

#[tauri::command]
async fn web_tab_go_back(app: AppHandle, id: String) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(&tab_label(&id)) {
        win.eval("window.history.back()").map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn web_tab_go_forward(app: AppHandle, id: String) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(&tab_label(&id)) {
        win.eval("window.history.forward()").map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn web_tab_reload(app: AppHandle, id: String) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(&tab_label(&id)) {
        win.eval("window.location.reload()").map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 运行时切换 tab 是否可 resize(popout 独立窗口用)
#[tauri::command]
async fn set_web_tab_resizable(app: AppHandle, id: String, resizable: bool) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(&tab_label(&id)) {
        win.set_resizable(resizable).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 切 popout 的窗口外观:是否显示系统标题栏、任务栏、置顶
/// popout 需要:decorations=true + skip_taskbar=false → 可拖可关可任务栏切换
/// inline 需要:decorations=false + skip_taskbar=true → 完全无边框贴在主窗口里
#[tauri::command]
async fn set_web_tab_chrome(
    app: AppHandle,
    id: String,
    decorations: bool,
    skip_taskbar: bool,
    always_on_top: bool,
    title: Option<String>,
) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(&tab_label(&id)) {
        win.set_decorations(decorations).map_err(|e| e.to_string())?;
        win.set_skip_taskbar(skip_taskbar).map_err(|e| e.to_string())?;
        win.set_always_on_top(always_on_top).map_err(|e| e.to_string())?;
        if let Some(t) = title {
            win.set_title(&t).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// popout / inline 切换时用:owner=false 断开与主窗的 owner 关系(主窗最小化不再拖走它);
/// owner=true 重新挂回主窗(inline 状态需要一起隐藏跟随主窗)
#[tauri::command]
fn set_web_tab_owner(app: AppHandle, id: String, owner: bool) -> Result<(), String> {
    let win = match app.get_webview_window(&tab_label(&id)) {
        Some(w) => w,
        None => return Ok(()),
    };

    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW,
            GWLP_HWNDPARENT, GWL_EXSTYLE, WS_EX_APPWINDOW,
        };

        let raw = win.hwnd().map_err(|e| e.to_string())?;
        let child_hwnd = HWND(raw.0 as *mut _);

        let new_owner: isize = if owner {
            let main = app.get_webview_window("main").ok_or("main window missing")?;
            let mh = main.hwnd().map_err(|e| e.to_string())?;
            mh.0 as isize
        } else {
            0
        };

        unsafe {
            // popout/pip(owner=false):无边框窗口默认不进任务栏,显式加 WS_EX_APPWINDOW
            //   让它有独立任务栏按钮;inline(owner=true)挂回主窗时移除该标志。
            let ex = GetWindowLongPtrW(child_hwnd, GWL_EXSTYLE);
            let ex2 = if owner {
                ex & !(WS_EX_APPWINDOW.0 as isize)
            } else {
                ex | WS_EX_APPWINDOW.0 as isize
            };
            SetWindowLongPtrW(child_hwnd, GWL_EXSTYLE, ex2);
            SetWindowLongPtrW(child_hwnd, GWLP_HWNDPARENT, new_owner);
        }
    }

    #[cfg(not(windows))]
    {
        let _ = (win, owner);
    }
    Ok(())
}

// ────── 右键菜单:独立 always_on_top 窗口,能盖住 web-tab ──────

const CTX_MENU_LABEL: &str = "ctx-menu";

/// 弹出独立右键菜单窗口
/// - `data`:URL-encoded JSON 的菜单项数组
/// - 尺寸由前端预算好传入
#[tauri::command]
async fn show_context_menu(
    app: AppHandle,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    data: String,
) -> Result<(), String> {
    // 已存在的先关掉,避免重复
    if let Some(old) = app.get_webview_window(CTX_MENU_LABEL) {
        let _ = old.close();
    }

    // 用 query string 把菜单数据传给新窗口的前端
    let url = format!("index.html?view=ctxmenu&data={data}");
    let parsed = WebviewUrl::App(url.into());

    let win = WebviewWindowBuilder::new(&app, CTX_MENU_LABEL, parsed)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .focused(true)
        .inner_size(width, height)
        .position(x, y)
        .shadow(false)
        .build()
        .map_err(|e| e.to_string())?;

    // 失焦自动关闭(点其它地方消失)
    let win_close = win.clone();
    win.on_window_event(move |ev| {
        if let tauri::WindowEvent::Focused(false) = ev {
            let _ = win_close.close();
        }
    });

    Ok(())
}

/// 前端主动关闭菜单(选中项后调用)
#[tauri::command]
async fn hide_context_menu(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(CTX_MENU_LABEL) {
        let _ = win.close();
    }
    Ok(())
}

/// 一次性隐藏/显示所有 tab(切换非活跃 tab 时,把上一个隐藏)
#[tauri::command]
async fn set_web_tab_visible_only(app: AppHandle, id: String) -> Result<(), String> {
    let target = tab_label(&id);
    let labels = WEB_TABS.lock().map(|g| g.clone()).unwrap_or_default();
    for l in labels {
        if let Some(win) = app.get_webview_window(&l) {
            if l == target { let _ = win.show(); }
            else { let _ = win.hide(); }
        }
    }
    Ok(())
}

/// 让指定 tab 的子窗口获取键鼠焦点(切换后必调,否则 B 站等站点点击无响应)
#[tauri::command]
async fn focus_web_tab(app: AppHandle, id: String) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(&tab_label(&id)) {
        win.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ────── 透明度 ──────

#[tauri::command]
async fn set_web_tab_opacity(app: AppHandle, id: String, opacity: f64) -> Result<(), String> {
    let win = match app.get_webview_window(&tab_label(&id)) {
        Some(w) => w,
        None => return Ok(()),
    };

    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{COLORREF, HWND};
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetLayeredWindowAttributes, SetWindowLongPtrW,
            GWL_EXSTYLE, LWA_ALPHA, WS_EX_LAYERED,
        };

        let raw = win.hwnd().map_err(|e| e.to_string())?;
        let hwnd = HWND(raw.0 as *mut _);
        let alpha = (opacity.clamp(0.0, 1.0) * 255.0) as u8;

        unsafe {
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_LAYERED.0 as isize);
            SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA)
                .map_err(|e| e.to_string())?;
        }
    }

    #[cfg(target_os = "macos")]
    {
        use objc2::{msg_send, runtime::AnyObject};
        let raw = win.ns_window().map_err(|e| e.to_string())? as *mut AnyObject;
        // 64 位 Apple 平台 CGFloat 即 f64,直接传 f64 ABI 兼容;
        // setAlphaValue: 与 Windows SetLayeredWindowAttributes(LWA_ALPHA) 语义对等(整窗 alpha 0~1)
        let alpha = opacity.clamp(0.0, 1.0);
        unsafe {
            let _: () = msg_send![raw, setAlphaValue: alpha];
        }
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (win, opacity);
    }

    Ok(())
}

/// 对所有 tab 批量设置透明度
#[tauri::command]
async fn set_all_web_tabs_opacity(app: AppHandle, opacity: f64) -> Result<(), String> {
    let labels = WEB_TABS.lock().map(|g| g.clone()).unwrap_or_default();
    for l in labels {
        if let Some(id) = l.strip_prefix("web-tab-") {
            let _ = set_web_tab_opacity(app.clone(), id.to_string(), opacity).await;
        }
    }
    Ok(())
}

// ────── PiP: 画中画 ──────

/// 进入画中画:切成置顶顶层小窗
#[tauri::command]
async fn enter_pip(
    app: AppHandle,
    id: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let label = tab_label(&id);
    let win = app.get_webview_window(&label).ok_or("tab not found")?;

    win.set_always_on_top(true).map_err(|e| e.to_string())?;
    // Tauri v2 目前不允许运行时切父窗口,直接把窗口置顶+定位即可满足 PiP 场景
    win.set_position(LogicalPosition::new(x, y)).map_err(|e| e.to_string())?;
    win.set_size(LogicalSize::new(width, height)).map_err(|e| e.to_string())?;
    win.show().map_err(|e| e.to_string())?;

    let _ = app.emit("tab-mode-changed", (id, "pip"));
    Ok(())
}

/// 退出画中画:取消置顶,交给前端重新调 resize_web_tab 摆回 holder 位置
#[tauri::command]
async fn exit_pip(app: AppHandle, id: String) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(&tab_label(&id)) {
        win.set_always_on_top(false).map_err(|e| e.to_string())?;
    }
    let _ = app.emit("tab-mode-changed", (id, "inline"));
    Ok(())
}

// ────── 视频信号:play(自动横屏) / requestFullscreen(应用内全屏) ──────

/// 子 webview 的注入脚本调用:视频首次 play
#[tauri::command]
fn signal_video_play(app: AppHandle, id: String) -> Result<(), String> {
    let _ = app.emit("web-tab-video-play", id);
    Ok(())
}

/// 子 webview 的注入脚本调用:HTML5 fullscreen 进入/退出
#[tauri::command]
fn signal_video_fullscreen(app: AppHandle, id: String, entering: bool) -> Result<(), String> {
    let _ = app.emit("web-tab-video-fullscreen", serde_json::json!({ "id": id, "entering": entering }));
    Ok(())
}

/// 主窗口触发:让指定 tab 里"当前正在播放的视频"进入全屏
/// 策略:优先模拟点击网站自己的全屏按钮 —— 让站点走它自己的全屏流程,
///       样式、控件、快捷键 100% 和用户手动点击一致;
///       找不到全屏按钮时才退回容器/video 元素 requestFullscreen。
/// requestFullscreen 会被 init_script hook,fake fullscreen 撑满 webview,
/// 并 signal 回主窗做应用内全屏。
#[tauri::command]
fn request_video_fullscreen(app: AppHandle, id: String) -> Result<(), String> {
    let label = tab_label(&id);
    let js = r#"
        (function(){
            try {
                // 1) 已经在 fake fullscreen 里,不重复
                if (document.fullscreenElement || document.webkitFullscreenElement) return;
                // 2) 优先找站点自己的全屏按钮
                var BTN_SELECTORS = [
                    '.ytp-fullscreen-button',                  // YouTube
                    '.bpx-player-ctrl-full',                   // B站新版
                    '.bilibili-player-video-btn-fullscreen',   // B站旧版
                    'button[aria-label*="全屏"]',
                    'button[aria-label*="Fullscreen" i]',
                    'button[title*="全屏"]',
                    'button[title*="Fullscreen" i]',
                    '[data-tooltip-content*="全屏"]',
                    '[data-purpose*="fullscreen"]'
                ];
                for (var i = 0; i < BTN_SELECTORS.length; i++) {
                    var btn = document.querySelector(BTN_SELECTORS[i]);
                    if (btn) {
                        btn.click();
                        return;
                    }
                }
                // 3) 兜底:找播放中的最大视频,优先对其播放器容器 requestFullscreen
                var vids = document.querySelectorAll('video');
                var bestVid = null;
                for (var i = 0; i < vids.length; i++) {
                    var v = vids[i];
                    if (v.paused) continue;
                    if ((v.videoWidth||0) < 480) continue;
                    if (!bestVid || (v.videoWidth * v.videoHeight) > (bestVid.videoWidth * bestVid.videoHeight)) {
                        bestVid = v;
                    }
                }
                if (!bestVid) return;
                var SELECTORS = [
                    '#movie_player', '.html5-video-player',
                    '.bpx-player-container', '.bilibili-player',
                    '.player-container', '#player', '[data-player]'
                ];
                var target = null;
                for (var j = 0; j < SELECTORS.length; j++) {
                    var el = bestVid.closest(SELECTORS[j]);
                    if (el) { target = el; break; }
                }
                target = target || bestVid;
                if (target.requestFullscreen) target.requestFullscreen();
                else if (target.webkitRequestFullscreen) target.webkitRequestFullscreen();
            } catch(e) {}
        })();
    "#;
    if let Some(win) = app.get_webview_window(&label) {
        win.eval(js).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ────── M3: 老板键 ──────

/// 让指定 web tab 静音 / 恢复。原理:eval 一段 JS,把页面里所有 <video> <audio> 的 muted 置位。
/// - 隐藏进入时:记录每个元素原始 muted,置为 true
/// - 恢复时:如果我们之前记录过它,则回写原始值(用户在 mute 期间自己开静音的场景也保留)
fn apply_web_tab_mute(app: &AppHandle, label: &str, mute: bool) {
    let js = if mute {
        r#"
        (function(){
            try {
                var els = document.querySelectorAll('video, audio');
                for (var i = 0; i < els.length; i++) {
                    var el = els[i];
                    if (el.__moyuOriginalMuted === undefined) {
                        el.__moyuOriginalMuted = !!el.muted;
                    }
                    el.muted = true;
                }
            } catch(e) {}
        })();
        "#
    } else {
        r#"
        (function(){
            try {
                var els = document.querySelectorAll('video, audio');
                for (var i = 0; i < els.length; i++) {
                    var el = els[i];
                    if (el.__moyuOriginalMuted !== undefined) {
                        el.muted = !!el.__moyuOriginalMuted;
                        delete el.__moyuOriginalMuted;
                    }
                }
            } catch(e) {}
        })();
        "#
    };
    if let Some(win) = app.get_webview_window(label) {
        let _ = win.eval(js);
    }
}

fn toggle_hide(app: &AppHandle) -> bool {
    let now_hidden = !HIDDEN.load(Ordering::Relaxed);
    HIDDEN.store(now_hidden, Ordering::Relaxed);

    if let Some(main) = app.get_webview_window("main") {
        if now_hidden { let _ = main.hide(); }
        else { let _ = main.show(); let _ = main.set_focus(); }
    }
    // 遍历所有 web tab:静音同步 + 显示切换
    let labels = WEB_TABS.lock().map(|g| g.clone()).unwrap_or_default();
    for l in labels {
        // 静音必须先 eval,再 hide;否则 hide 后 webview 可能不响应 eval(某些平台)
        apply_web_tab_mute(app, &l, now_hidden);
        if let Some(win) = app.get_webview_window(&l) {
            if now_hidden { let _ = win.hide(); }
            else { let _ = win.show(); }
        }
    }

    let _ = app.emit("boss-key-toggled", now_hidden);
    now_hidden
}

#[tauri::command]
fn is_hidden() -> bool {
    HIDDEN.load(Ordering::Relaxed)
}

#[tauri::command]
fn trigger_boss_key(app: AppHandle) -> bool {
    toggle_hide(&app)
}

#[tauri::command]
fn update_boss_shortcut(app: AppHandle, shortcut: String) -> Result<(), String> {
    let new_sc: Shortcut = shortcut.parse().map_err(|e| format!("invalid shortcut: {e}"))?;

    let gs = app.global_shortcut();
    if let Ok(mut guard) = CURRENT_BOSS_KEY.lock() {
        if let Some(old) = guard.take() {
            let _ = gs.unregister(old);
        }
        gs.register(new_sc.clone()).map_err(|e| e.to_string())?;
        *guard = Some(new_sc);
    }
    Ok(())
}

/// 设置或清除"取消透明"快捷键。shortcut 为空字符串 = 清除。
#[tauri::command]
fn update_transparency_shortcut(app: AppHandle, shortcut: String) -> Result<(), String> {
    let gs = app.global_shortcut();
    // 先卸载旧的
    if let Ok(mut guard) = CURRENT_TRANSPARENCY_KEY.lock() {
        if let Some(old) = guard.take() {
            let _ = gs.unregister(old);
        }
        if shortcut.trim().is_empty() {
            return Ok(());
        }
        let new_sc: Shortcut = shortcut.parse().map_err(|e| format!("invalid shortcut: {e}"))?;
        gs.register(new_sc.clone()).map_err(|e| e.to_string())?;
        *guard = Some(new_sc);
    }
    Ok(())
}

/// 主窗口(或任意子 tab 窗口)置顶开关
#[tauri::command]
fn set_main_window_pinned(app: AppHandle, pinned: bool) -> Result<(), String> {
    let main = app.get_webview_window("main").ok_or("main window missing")?;
    main.set_always_on_top(pinned).map_err(|e| e.to_string())?;
    Ok(())
}

/// popout 独立窗口置顶开关(pip 已默认置顶,不用此接口)
#[tauri::command]
fn set_web_tab_pinned(app: AppHandle, id: String, pinned: bool) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(&tab_label(&id)) {
        win.set_always_on_top(pinned).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// popout 内注入按钮 → 上报给主窗 → 主窗协调置顶状态与 tabs.setPinned
#[tauri::command]
fn signal_pin_toggle(app: AppHandle, id: String, pinned: bool) -> Result<(), String> {
    let _ = app.emit("web-tab-pin-toggle", serde_json::json!({ "id": id, "pinned": pinned }));
    Ok(())
}

/// 主窗通知子 webview:当前 tab 模式(popout/inline/pip/fullscreen)+ 置顶状态 + 标题栏主题,
/// 子 webview 里的注入脚本据此显示/隐藏自绘标题栏并套用深/浅色
#[tauri::command]
fn notify_web_tab_mode(app: AppHandle, id: String, mode: String, pinned: bool, theme: Option<String>) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(&tab_label(&id)) {
        let theme_js = match theme.as_deref() {
            Some("dark") | Some("light") | Some("system") => format!("{:?}", theme.as_deref().unwrap()),
            _ => "\"system\"".to_string(),
        };
        let js = format!(
            "try {{ window.__muoyuOnMode && window.__muoyuOnMode({:?}, {}, {}); }} catch(e) {{}}",
            mode, pinned, theme_js
        );
        let _ = win.eval(&js);
    }
    Ok(())
}

/// popout 自绘标题栏:按下空白处开始拖动窗口。
/// 必须 async:start_dragging 要窗口消息循环继续转,同步命令会自锁(同本文件其它几何命令)。
#[tauri::command]
async fn popout_start_drag(app: AppHandle, id: String) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(&tab_label(&id)) {
        win.start_dragging().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// popout 自绘标题栏:双击标题栏切换最大化/还原。async 同理。
#[tauri::command]
async fn popout_toggle_maximize(app: AppHandle, id: String) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(&tab_label(&id)) {
        if win.is_maximized().unwrap_or(false) {
            win.unmaximize().map_err(|e| e.to_string())?;
        } else {
            win.maximize().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// popout 自绘标题栏按钮:最小化 / 收回摸鱼窗(切回 inline,不销毁 tab)。
/// close 不直接关窗,而是复用 web-tab-close-requested 事件,交由主窗决定收回逻辑。
#[tauri::command]
async fn popout_window_action(app: AppHandle, id: String, action: String) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(&tab_label(&id)) {
        match action.as_str() {
            "minimize" => { win.minimize().map_err(|e| e.to_string())?; }
            "close" => {
                // 正常情况:把"收回摸鱼窗口"交给主窗前端处理(它会 restoreInline)。
                // 若主窗已关闭(孤儿独立窗),前端监听器不存在,emit 会落空导致 ✕ 无反应,
                // 此时直接销毁自身。
                if app.get_webview_window("main").is_some() {
                    let _ = app.emit(
                        "web-tab-close-requested",
                        serde_json::json!({ "id": id.clone(), "label": tab_label(&id) }),
                    );
                } else if let Some(win) = app.get_webview_window(&tab_label(&id)) {
                    let _ = win.destroy();
                    forget_tab(&tab_label(&id));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

// ────── M8: 桌面宠物 ──────
//
// 老板键可配置为两种动作:直接隐藏(默认) 或 切换成桌面宠物。
// 桌宠是一个紧贴宠物尺寸的小透明窗(不做全屏透明窗,避免遮挡桌面操作),
// 复用右键菜单窗口已验证的参数组合:无边框 + 透明 + 不进任务栏 + 置顶 + 无阴影。
//
// 探测线程职责(仅在桌宠开着时工作):
// - 鼠标位置 → emit `pet-cursor`,驱动视线跟随/走向鼠标
// - 键盘活跃度 → emit `pet-typing`(仅 Windows)
//   注意:只用 GetAsyncKeyState 判断"有没有键被按下",不记录任何按键内容,
//   刻意不做键位统计,避免这个功能变成键盘记录器。

const PET_LABEL: &str = "pet";

/// 桌宠窗口当前是否开着
static PET_MODE: AtomicBool = AtomicBool::new(false);
/// 老板键动作:false=隐藏(默认) / true=切桌宠
static BOSS_ACTION_PET: AtomicBool = AtomicBool::new(false);
/// 探测线程是否应工作(仅桌宠开启期间为 true)
static PET_PROBE_ON: AtomicBool = AtomicBool::new(false);
/// 打字监控开关:关闭时探测线程完全不扫键盘
static PET_TYPING_ON: AtomicBool = AtomicBool::new(true);
/// 桌宠配置缓存 (width, height, query),由前端在启动时与配置变更时下发。
///
/// 老板键按下时 Rust 直接用它开窗,不再走"emit → 前端 → invoke"往返:
/// 那条链有三个环节,任何一环断掉(事件丢失、前端异常、监听未就绪)
/// 都会让老板键彻底失效 —— 对一个"救命键"来说不可接受。
static PET_CONFIG: Mutex<Option<(f64, f64, String)>> = Mutex::new(None);

/// 前端下发桌宠配置(种类/尺寸/台词已拼成 query)
#[tauri::command]
fn set_pet_config(width: f64, height: f64, query: String) {
    if let Ok(mut g) = PET_CONFIG.lock() {
        *g = Some((width, height, query));
    }
}

/// 打字监控开关。关掉后探测线程不再扫键盘,也不再 emit `pet-typing`。
#[tauri::command]
fn set_pet_typing(on: bool) {
    PET_TYPING_ON.store(on, Ordering::Relaxed);
}

/// 桌宠窗口置顶开关(右键菜单/设置页用)。async 同 `enter_pet_mode`。
#[tauri::command]
async fn set_pet_always_on_top(app: AppHandle, on: bool) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(PET_LABEL) {
        win.set_always_on_top(on).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 改桌宠窗口尺寸(滚轮缩放 / 右键菜单选档)。
///
/// 缩放时保持窗口**底边中点**不动:桌宠是"站在"桌面某处的,
/// 若按左上角锚点缩放,它会在放大时往右下方蹿走,手感很怪。
///
/// 必须是 async:同步命令跑在主线程,而 set_size/set_position 要靠
/// 主线程事件循环继续转才能完成,在主线程里同步等就是自锁
/// (本文件所有改窗命令都是 async,原因相同)。
#[tauri::command]
async fn set_pet_size(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    let Some(win) = app.get_webview_window(PET_LABEL) else {
        return Ok(());
    };
    let scale = win.scale_factor().unwrap_or(1.0);
    // 先读旧几何,算出底边中点,改完尺寸再按新尺寸反推左上角
    let old_pos = win.outer_position().map_err(|e| e.to_string())?;
    let old_size = win.outer_size().map_err(|e| e.to_string())?;
    let anchor_x = old_pos.x as f64 / scale + old_size.width as f64 / scale / 2.0;
    let anchor_y = old_pos.y as f64 / scale + old_size.height as f64 / scale;

    win.set_size(LogicalSize::new(width, height))
        .map_err(|e| e.to_string())?;
    win.set_position(LogicalPosition::new(anchor_x - width / 2.0, anchor_y - height))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 打开桌宠窗口的实际逻辑。
///
/// 抽成普通函数是为了让老板键回调能直接调用(它跑在快捷键线程,不是主线程),
/// 而 #[tauri::command] 那层必须是 async —— 见 `enter_pet_mode` 的说明。
fn do_enter_pet_mode(app: &AppHandle, width: f64, height: f64, query: &str) -> Result<(), String> {
    // 已存在则只显示(幂等,避免重复建窗)。
    // 这里刻意不改尺寸:窗口内部布局由前端按自己的 bodySize 排,
    // 只改窗口不通知前端会错位;活跃期的尺寸同步走 pet-size-push 事件。
    if let Some(existing) = app.get_webview_window(PET_LABEL) {
        let _ = existing.show();
        let _ = existing.set_focus();
        PET_MODE.store(true, Ordering::Relaxed);
        PET_PROBE_ON.store(true, Ordering::Relaxed);
        return Ok(());
    }

    // 默认摆到主屏右下角内缩 40px;拿不到显示器信息时退回固定坐标
    let (x, y) = match app.primary_monitor() {
        Ok(Some(m)) => {
            let scale = m.scale_factor();
            let ms = m.size();
            let w = ms.width as f64 / scale;
            let h = ms.height as f64 / scale;
            (w - width - 40.0, h - height - 80.0)
        }
        _ => (600.0, 400.0),
    };

    let url = format!("index.html?view=pet&{query}");
    let parsed = WebviewUrl::App(url.clone().into());

    if let Err(e) = WebviewWindowBuilder::new(app, PET_LABEL, parsed)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .shadow(false)
        .inner_size(width, height)
        .position(x, y)
        .build()
    {
        eprintln!("[moyu] enter_pet_mode build failed: url={url} err={e}");
        return Err(e.to_string());
    }

    PET_MODE.store(true, Ordering::Relaxed);
    PET_PROBE_ON.store(true, Ordering::Relaxed);

    // 语义与老板键一致:主窗和所有 tab 一起收起(tab 先静音再隐藏)
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }
    let labels = WEB_TABS.lock().map(|g| g.clone()).unwrap_or_default();
    for l in labels {
        apply_web_tab_mute(app, &l, true);
        if let Some(win) = app.get_webview_window(&l) {
            let _ = win.hide();
        }
    }
    HIDDEN.store(true, Ordering::Relaxed);

    Ok(())
}

/// 打开桌宠窗口。`width`/`height` 为窗口逻辑尺寸(高度含气泡留白),
/// `query` 为前端拼好的配置 query string。
///
/// 必须是 async:同步命令跑在主线程,而 WebviewWindowBuilder::build()
/// 要靠主线程事件循环继续转才能完成建窗,在主线程里同步等就是自锁
/// —— 表现为整个应用无响应(本文件所有建窗命令都是 async,原因相同)。
#[tauri::command]
async fn enter_pet_mode(
    app: AppHandle,
    width: f64,
    height: f64,
    query: String,
) -> Result<(), String> {
    do_enter_pet_mode(&app, width, height, &query)
}

/// 关闭桌宠窗口并恢复摸鱼主窗的实际逻辑(同上,供老板键回调直接调用)。
fn do_exit_pet_mode(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(PET_LABEL) {
        let _ = win.close();
    }
    PET_MODE.store(false, Ordering::Relaxed);
    PET_PROBE_ON.store(false, Ordering::Relaxed);

    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
    }
    // 解除静音,并把子窗恢复为"可见"状态。
    // 必须在这里 show():enter_pet_mode 时是 Rust 直接 hide() 的,
    // 前端只会对活跃 tab 调 set_web_tab_visible(true),其余 tab 若停在 hidden
    // 就会表现为"标签还在但里面没内容"。show() 后再由前端 restoreTabsVisual()
    // 按各 tab 的 mode 收掉非活跃的 inline。
    let labels = WEB_TABS.lock().map(|g| g.clone()).unwrap_or_default();
    for l in labels {
        apply_web_tab_mute(app, &l, false);
        if let Some(win) = app.get_webview_window(&l) {
            let _ = win.show();
        }
    }
    HIDDEN.store(false, Ordering::Relaxed);

    let _ = app.emit("pet-exited", ());
}

/// 关闭桌宠窗口并恢复摸鱼主窗。tab 的显示交给前端(它才知道每个 tab 的 mode)。
/// async 的原因同 `enter_pet_mode`:close/show 都要主线程事件循环配合。
#[tauri::command]
async fn exit_pet_mode(app: AppHandle) -> Result<(), String> {
    do_exit_pet_mode(&app);
    Ok(())
}

/// 设置老板键触发时的动作(true=切桌宠 / false=隐藏)
#[tauri::command]
fn set_boss_action_pet(pet: bool) {
    BOSS_ACTION_PET.store(pet, Ordering::Relaxed);
}

#[tauri::command]
fn is_pet_mode() -> bool {
    PET_MODE.load(Ordering::Relaxed)
}

/// 主动给桌宠推一句台词(设置页"试一下"、以及后续提醒场景用)
#[tauri::command]
fn pet_say(app: AppHandle, text: String) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(PET_LABEL) {
        let _ = win.emit("pet-say", text);
    }
    Ok(())
}

/// 桌宠探测线程:220ms 一轮,只在桌宠开着时干活。
/// 鼠标位移超过阈值才 emit,键盘只在活跃态翻转时 emit —— 都是为了别把事件刷爆。
fn spawn_pet_probe(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_pos = (f64::MIN, f64::MIN);
        let mut last_typing = false;
        let mut idle_ticks: u32 = 0;

        loop {
            std::thread::sleep(Duration::from_millis(220));

            if !PET_PROBE_ON.load(Ordering::Relaxed) {
                last_pos = (f64::MIN, f64::MIN);
                last_typing = false;
                idle_ticks = 0;
                continue;
            }

            // ── 鼠标位置 ──
            if let Ok(p) = app.cursor_position() {
                let moved = (p.x - last_pos.0).abs() > 6.0 || (p.y - last_pos.1).abs() > 6.0;
                if moved {
                    last_pos = (p.x, p.y);
                    let _ = app.emit("pet-cursor", serde_json::json!({ "x": p.x, "y": p.y }));
                }
            }

            // ── 键盘活跃度(仅 Windows,且用户开启了打字监控) ──
            #[cfg(windows)]
            if PET_TYPING_ON.load(Ordering::Relaxed) {
                use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

                // 只扫常用输入键区(字母/数字/符号/空格/回退),不含鼠标键与修饰键单击
                let mut pressed = false;
                for vk in 0x08u32..=0xDEu32 {
                    // 跳过鼠标键(0x01-0x06)已在范围外;跳过 F 区无所谓,误判成本低
                    let st = unsafe { GetAsyncKeyState(vk as i32) };
                    if (st as u16 & 0x8000) != 0 {
                        pressed = true;
                        break;
                    }
                }

                if pressed {
                    idle_ticks = 0;
                    if !last_typing {
                        last_typing = true;
                        let _ = app.emit("pet-typing", true);
                    }
                } else if last_typing {
                    idle_ticks += 1;
                    // 约 2.6 秒无输入才判定停止打字(220ms × 12),避免打字间隙来回翻转
                    if idle_ticks >= 12 {
                        last_typing = false;
                        idle_ticks = 0;
                        let _ = app.emit("pet-typing", false);
                    }
                }
            }
        }
    });
}

// ────── 入口 ──────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let default_boss: Shortcut = DEFAULT_BOSS_KEY.parse().expect("bad default shortcut");
    let default_boss_for_handler = default_boss.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let is_boss = CURRENT_BOSS_KEY
                            .lock()
                            .ok()
                            .and_then(|g| g.clone())
                            .map(|s| &s == shortcut)
                            .unwrap_or_else(|| shortcut == &default_boss_for_handler);
                        if is_boss {
                            // 老板键动作分流。桌宠由 Rust 直接开窗:配置已由前端预先
                            // 下发到 PET_CONFIG,不依赖任何前端往返 —— 老板键是救命键,
                            // 不能因为前端没响应就整个失效。
                            let want_pet = BOSS_ACTION_PET.load(Ordering::Relaxed);
                            let in_pet = PET_MODE.load(Ordering::Relaxed);
                            if want_pet {
                                if in_pet {
                                    do_exit_pet_mode(app);
                                } else {
                                    let cfg = PET_CONFIG.lock().ok().and_then(|g| g.clone());
                                    let opened = match cfg {
                                        Some((width, height, query)) => {
                                            match do_enter_pet_mode(app, width, height, &query) {
                                                Ok(()) => true,
                                                Err(e) => {
                                                    eprintln!("[moyu] enter_pet_mode failed: {e}");
                                                    false
                                                }
                                            }
                                        }
                                        None => {
                                            eprintln!("[moyu] pet config missing, fallback to hide");
                                            false
                                        }
                                    };
                                    // 开窗失败必须退化为隐藏:宁可少个宠物,
                                    // 也不能让用户以为按了键、实际屏幕纹丝不动。
                                    if !opened {
                                        toggle_hide(app);
                                    }
                                }
                            } else {
                                toggle_hide(app);
                            }
                            return;
                        }
                        let is_transp = CURRENT_TRANSPARENCY_KEY
                            .lock()
                            .ok()
                            .and_then(|g| g.clone())
                            .map(|s| &s == shortcut)
                            .unwrap_or(false);
                        if is_transp {
                            let _ = app.emit("transparency-toggle-requested", ());
                        }
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            open_web_tab,
            resize_web_tab,
            close_web_tab,
            close_all_web_tabs,
            set_web_tab_visible,
            web_tab_exists,
            web_tab_bounds,
            get_cursor_position,
            set_web_tab_visible_only,
            focus_web_tab,
            set_web_tab_opacity,
            set_all_web_tabs_opacity,
            set_web_tab_resizable,
            set_web_tab_chrome,
            set_web_tab_owner,
            web_tab_go_back,
            web_tab_go_forward,
            web_tab_reload,
            enter_pip,
            exit_pip,
            signal_video_play,
            signal_video_fullscreen,
            request_video_fullscreen,
            show_context_menu,
            hide_context_menu,
            is_hidden,
            trigger_boss_key,
            update_boss_shortcut,
            update_transparency_shortcut,
            set_main_window_pinned,
            set_web_tab_pinned,
            signal_pin_toggle,
            notify_web_tab_mode,
            popout_start_drag,
            popout_toggle_maximize,
            popout_window_action,
            set_work_monitor_config,
            set_all_tabs_hidden,
            set_reading_disguise,
            enter_pet_mode,
            exit_pet_mode,
            set_boss_action_pet,
            set_pet_config,
            set_pet_size,
            set_pet_always_on_top,
            set_pet_typing,
            is_pet_mode,
            pet_say,
            set_user_scripts,
        ])
        .setup(move |app| {
            if let Some(win) = app.get_webview_window("main") {
                // 主窗关闭 → 级联销毁所有子窗。
                // popout 等通过 owner 挂靠主窗,但 Windows 不会随 owner 关闭自动关闭 owned 窗口,
                // 不主动 destroy 会留下孤儿独立窗(且此时它的自绘 ✕ 因主窗已死、事件无人接收而失灵)。
                let app_for_close = app.handle().clone();
                win.on_window_event(move |ev| {
                    if let tauri::WindowEvent::CloseRequested { .. } = ev {
                        destroy_all_web_tabs(&app_for_close);
                        if let Some(pet) = app_for_close.get_webview_window(PET_LABEL) {
                            let _ = pet.destroy();
                        }
                    }
                });
                let _ = win.show();
                let _ = win.set_focus();
            }
            // 老板键注册失败(被其它程序占用等)不能让整个应用启动崩溃:
            // 这里改为记录失败、照常启动,用户仍可在设置里换一个快捷键。
            if let Err(e) = app.global_shortcut().register(default_boss.clone()) {
                eprintln!("[moyu] register default boss shortcut failed: {e}");
            } else if let Ok(mut g) = CURRENT_BOSS_KEY.lock() {
                *g = Some(default_boss);
            }
            // M5: 启动工作进程监测轮询线程(默认关闭,前端下发配置后生效)
            spawn_work_monitor(app.handle().clone());
            // M8: 启动桌宠探测线程(默认关闭,开桌宠后生效)
            spawn_pet_probe(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
