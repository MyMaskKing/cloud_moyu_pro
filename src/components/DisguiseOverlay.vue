<script setup lang="ts">
import { onMounted, onBeforeUnmount, computed } from "vue";
import { useDisguiseStore } from "../stores/disguise";

/**
 * M4 伪装皮肤覆盖层:
 * 全屏盖住摸鱼界面,渲染仿 VSCode / Excel / PPT 的静态工作界面。
 * 退出方式(三重保险,避免用户被"锁"在伪装里):
 *   1. 按 Esc
 *   2. 点右下角状态栏的隐蔽区域(看起来像普通状态栏文字)
 *   3. 老板键(由 App.vue 处理)
 */
const disguise = useDisguiseStore();
const emit = defineEmits<{ (e: "exit"): void }>();

const skin = computed(() => disguise.skin);

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    e.stopPropagation();
    emit("exit");
  }
}

onMounted(() => window.addEventListener("keydown", onKeydown, true));
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown, true));

// ── 各皮肤的静态假数据 ──
const codeLines = [
  { n: 1,  t: [["kw", "import"], ["p", " { defineStore } "], ["kw", "from"], ["s", " \"pinia\""], ["p", ";"]] },
  { n: 2,  t: [["kw", "import"], ["p", " { ref, computed } "], ["kw", "from"], ["s", " \"vue\""], ["p", ";"]] },
  { n: 3,  t: [] },
  { n: 4,  t: [["c", "// 订单服务:聚合下游库存与支付状态"]] },
  { n: 5,  t: [["kw", "export const"], ["fn", " useOrderStore"], ["p", " = defineStore("], ["s", "\"order\""], ["p", ", () => {"]] },
  { n: 6,  t: [["p", "  "], ["kw", "const"], ["v", " items"], ["p", " = ref<OrderItem[]>([]);"]] },
  { n: 7,  t: [["p", "  "], ["kw", "const"], ["v", " loading"], ["p", " = ref("], ["kw", "false"], ["p", ");"]] },
  { n: 8,  t: [] },
  { n: 9,  t: [["p", "  "], ["kw", "const"], ["v", " total"], ["p", " = computed(() => "]] },
  { n: 10, t: [["p", "    items.value.reduce((s, i) => s + i.price * i.qty, "], ["nu", "0"], ["p", "));"]] },
  { n: 11, t: [] },
  { n: 12, t: [["p", "  "], ["kw", "async function"], ["fn", " fetchOrders"], ["p", "(page = "], ["nu", "1"], ["p", ") {"]] },
  { n: 13, t: [["p", "    loading.value = "], ["kw", "true"], ["p", ";"]] },
  { n: 14, t: [["p", "    "], ["kw", "try"], ["p", " {"]] },
  { n: 15, t: [["p", "      "], ["kw", "const"], ["v", " res"], ["p", " = "], ["kw", "await"], ["p", " api.get("], ["s", "\"/orders\""], ["p", ", { page });"]] },
  { n: 16, t: [["p", "      items.value = res.data.list;"]] },
  { n: 17, t: [["p", "    } "], ["kw", "finally"], ["p", " {"]] },
  { n: 18, t: [["p", "      loading.value = "], ["kw", "false"], ["p", ";"]] },
  { n: 19, t: [["p", "    }"]] },
  { n: 20, t: [["p", "  }"]] },
  { n: 21, t: [] },
  { n: 22, t: [["p", "  "], ["kw", "return"], ["p", " { items, loading, total, fetchOrders };"]] },
  { n: 23, t: [["p", "});"]] },
];

const excelCols = ["A", "B", "C", "D", "E", "F", "G"];
const excelRows = [
  ["部门", "季度", "预算(万)", "实际(万)", "差异", "完成率", "备注"],
  ["市场部", "Q1", "120.00", "118.40", "-1.60", "98.7%", "达标"],
  ["市场部", "Q2", "135.00", "141.20", "6.20", "104.6%", "超支"],
  ["研发部", "Q1", "320.00", "305.80", "-14.20", "95.6%", "达标"],
  ["研发部", "Q2", "350.00", "348.90", "-1.10", "99.7%", "达标"],
  ["销售部", "Q1", "210.00", "225.50", "15.50", "107.4%", "超支"],
  ["销售部", "Q2", "240.00", "231.70", "-8.30", "96.5%", "达标"],
  ["运营部", "Q1", "95.00", "92.30", "-2.70", "97.2%", "达标"],
  ["运营部", "Q2", "105.00", "108.60", "3.60", "103.4%", "关注"],
  ["合计", "H1", "1575.00", "1572.40", "-2.60", "99.8%", ""],
];

const pptSlides = [
  { n: 1, title: "2026 上半年经营回顾" },
  { n: 2, title: "核心指标完成情况" },
  { n: 3, title: "分部门预算执行" },
  { n: 4, title: "风险与改进项" },
  { n: 5, title: "下半年重点规划" },
];
</script>

<template>
  <div class="disguise-root" @contextmenu.stop.prevent>
    <!-- ══════════ VSCode ══════════ -->
    <div v-if="skin === 'vscode'" class="vs">
      <div class="vs-title">
        <span class="vs-menu">文件(F)　编辑(E)　选择(S)　查看(V)　转到(G)　运行(R)　终端(T)　帮助(H)</span>
        <span class="vs-tname">order.ts — cloud-order-service — Visual Studio Code</span>
      </div>
      <div class="vs-body">
        <div class="vs-activity">
          <span class="vs-ai act">🗎</span><span class="vs-ai">🔍</span>
          <span class="vs-ai">⑂</span><span class="vs-ai">▷</span><span class="vs-ai">⬢</span>
        </div>
        <div class="vs-side">
          <div class="vs-side-h">资源管理器</div>
          <div class="vs-tree">
            <div class="vs-tr">▾ CLOUD-ORDER-SERVICE</div>
            <div class="vs-tr i1">▾ src</div>
            <div class="vs-tr i2">▾ stores</div>
            <div class="vs-tr i3 sel">TS order.ts</div>
            <div class="vs-tr i3">TS user.ts</div>
            <div class="vs-tr i2">▾ api</div>
            <div class="vs-tr i3">TS client.ts</div>
            <div class="vs-tr i2">▸ components</div>
            <div class="vs-tr i2">TS main.ts</div>
            <div class="vs-tr i1">{} package.json</div>
            <div class="vs-tr i1">{} tsconfig.json</div>
            <div class="vs-tr i1">MD README.md</div>
          </div>
        </div>
        <div class="vs-main">
          <div class="vs-tabs">
            <span class="vs-tab act">TS order.ts</span>
            <span class="vs-tab">TS client.ts</span>
            <span class="vs-tab">{} package.json</span>
          </div>
          <div class="vs-code">
            <div v-for="l in codeLines" :key="l.n" class="vs-line">
              <span class="vs-ln">{{ l.n }}</span>
              <span class="vs-lt">
                <span v-for="(seg, i) in l.t" :key="i" :class="'t-' + seg[0]">{{ seg[1] }}</span>
              </span>
            </div>
          </div>
          <div class="vs-term">
            <div class="vs-term-h">
              <span class="act">终端</span><span>问题</span><span>输出</span><span>调试控制台</span>
            </div>
            <div class="vs-term-b">
              <div><span class="t-p">$ pnpm test</span></div>
              <div class="t-ok">✓ order.spec.ts (12 tests) 842ms</div>
              <div class="t-ok">✓ client.spec.ts (7 tests) 311ms</div>
              <div class="t-p">Test Files  2 passed (2)</div>
              <div class="t-p">     Tests  19 passed (19)</div>
              <div><span class="t-p">$ </span><span class="vs-caret">▌</span></div>
            </div>
          </div>
        </div>
      </div>
      <div class="vs-status">
        <span>⑂ main*</span><span>⊗ 0  ⚠ 0</span>
        <span class="vs-sp"></span>
        <span>行 15,列 42</span><span>空格: 2</span><span>UTF-8</span><span>TypeScript</span>
        <span class="exit-hot" @click="emit('exit')" title="退出伪装">退出伪装</span>
      </div>
    </div>

    <!-- ══════════ Excel ══════════ -->
    <div v-else-if="skin === 'excel'" class="xl">
      <div class="xl-title">
        <span class="xl-badge">X</span>
        <span class="xl-tname">2026上半年预算执行分析.xlsx - Excel</span>
      </div>
      <div class="xl-ribbon">
        <span class="act">开始</span><span>插入</span><span>页面布局</span><span>公式</span>
        <span>数据</span><span>审阅</span><span>视图</span><span>帮助</span>
      </div>
      <div class="xl-toolbar">
        <span class="xl-tb">宋体</span><span class="xl-tb">11</span>
        <span class="xl-tb b">B</span><span class="xl-tb i">I</span><span class="xl-tb u">U</span>
        <span class="xl-tb">￥</span><span class="xl-tb">%</span><span class="xl-tb">.00</span>
        <span class="xl-tb">条件格式</span><span class="xl-tb">求和 Σ</span>
      </div>
      <div class="xl-formula">
        <span class="xl-cell-ref">E11</span>
        <span class="xl-fx">fx</span>
        <span class="xl-fval">=SUM(E2:E10)</span>
      </div>
      <div class="xl-grid">
        <div class="xl-row xl-head">
          <div class="xl-c xl-rh"></div>
          <div v-for="c in excelCols" :key="c" class="xl-c xl-ch">{{ c }}</div>
        </div>
        <div v-for="(row, ri) in excelRows" :key="ri" class="xl-row">
          <div class="xl-c xl-rh">{{ ri + 1 }}</div>
          <div
            v-for="(cell, ci) in row"
            :key="ci"
            class="xl-c"
            :class="{
              'xl-b': ri === 0 || ri === excelRows.length - 1,
              'xl-num': ri > 0 && ci >= 2 && ci <= 5,
              'xl-neg': ri > 0 && ci === 4 && cell.startsWith('-'),
              'xl-sel': ri === 10 && ci === 4,
            }"
          >{{ cell }}</div>
        </div>
        <div v-for="i in 6" :key="'e' + i" class="xl-row">
          <div class="xl-c xl-rh">{{ excelRows.length + i }}</div>
          <div v-for="c in excelCols" :key="c" class="xl-c"></div>
        </div>
      </div>
      <div class="xl-sheets">
        <span class="act">预算执行</span><span>分部门明细</span><span>Q1原始数据</span><span>Q2原始数据</span>
        <span class="xl-add">＋</span>
        <span class="xl-sp"></span>
        <span>就绪</span>
        <span class="exit-hot" @click="emit('exit')" title="退出伪装">退出伪装</span>
      </div>
    </div>

    <!-- ══════════ PPT ══════════ -->
    <div v-else class="pp">
      <div class="pp-title">
        <span class="pp-badge">P</span>
        <span class="pp-tname">2026上半年经营回顾.pptx - PowerPoint</span>
      </div>
      <div class="pp-ribbon">
        <span class="act">开始</span><span>插入</span><span>设计</span><span>切换</span>
        <span>动画</span><span>幻灯片放映</span><span>审阅</span><span>视图</span>
      </div>
      <div class="pp-body">
        <div class="pp-thumbs">
          <div
            v-for="s in pptSlides"
            :key="s.n"
            class="pp-th"
            :class="{ act: s.n === 3 }"
          >
            <span class="pp-thn">{{ s.n }}</span>
            <div class="pp-thbox"><span>{{ s.title }}</span></div>
          </div>
        </div>
        <div class="pp-stage">
          <div class="pp-slide">
            <div class="pp-s-title">分部门预算执行</div>
            <div class="pp-s-sub">2026 H1 · 财务中心</div>
            <div class="pp-chart">
              <div class="pp-bar" style="height:58%"><span>研发 95.6%</span></div>
              <div class="pp-bar" style="height:72%"><span>市场 98.7%</span></div>
              <div class="pp-bar" style="height:88%"><span>销售 107.4%</span></div>
              <div class="pp-bar" style="height:64%"><span>运营 97.2%</span></div>
              <div class="pp-bar" style="height:80%"><span>合计 99.8%</span></div>
            </div>
            <div class="pp-s-note">· 整体预算执行率 99.8%,控制在目标区间内</div>
          </div>
        </div>
      </div>
      <div class="pp-status">
        <span>幻灯片 3 / 5</span><span>中文(简体)</span>
        <span class="pp-sp"></span>
        <span>备注</span><span>批注</span>
        <span class="exit-hot" @click="emit('exit')" title="">▭</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 覆盖层铺满整个窗口,盖住摸鱼壳;不透明,避免透出真实内容 */
.disguise-root {
  position: fixed;
  inset: 0;
  z-index: 9999;
  overflow: hidden;
  user-select: none;
  cursor: default;
  font-size: 12px;
}
/* 明确可见的退出伪装按钮(混在状态栏文字里但有底色,hover 加深) */
.exit-hot {
  cursor: pointer;
  opacity: 0.9;
  margin-left: 6px;
  padding: 1px 8px;
  border-radius: 3px;
  font-weight: 600;
  background: rgba(255, 255, 255, 0.16);
}
.exit-hot:hover { opacity: 1; background: rgba(255, 255, 255, 0.3); }

/* ══════════ VSCode ══════════ */
.vs {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: #1e1e1e;
  color: #cccccc;
  font-family: "Segoe UI", "Microsoft YaHei", sans-serif;
}
.vs-title {
  height: 30px; flex-shrink: 0;
  display: flex; align-items: center;
  background: #3c3c3c;
  padding: 0 10px;
  font-size: 11px;
  position: relative;
}
.vs-menu { color: #cccccc; }
.vs-tname {
  position: absolute; left: 0; right: 0;
  text-align: center; color: #9d9d9d; pointer-events: none;
}
.vs-body { flex: 1; display: flex; min-height: 0; }
.vs-activity {
  width: 48px; flex-shrink: 0;
  background: #333333;
  display: flex; flex-direction: column; align-items: center;
  padding-top: 8px; gap: 16px;
}
.vs-ai { font-size: 18px; color: #858585; }
.vs-ai.act { color: #ffffff; }
.vs-side {
  width: 210px; flex-shrink: 0;
  background: #252526;
  display: flex; flex-direction: column;
}
.vs-side-h {
  height: 32px; display: flex; align-items: center;
  padding: 0 16px; font-size: 11px; color: #bbbbbb;
  text-transform: uppercase; letter-spacing: 0.5px;
}
.vs-tree { flex: 1; overflow: hidden; }
.vs-tr {
  height: 22px; line-height: 22px;
  padding-left: 12px; font-size: 12px; color: #cccccc;
  white-space: nowrap;
}
.vs-tr.i1 { padding-left: 24px; }
.vs-tr.i2 { padding-left: 36px; }
.vs-tr.i3 { padding-left: 48px; }
.vs-tr.sel { background: #37373d; }
.vs-main { flex: 1; display: flex; flex-direction: column; min-width: 0; }
.vs-tabs {
  height: 35px; flex-shrink: 0;
  display: flex; background: #252526;
}
.vs-tab {
  display: flex; align-items: center;
  padding: 0 14px; font-size: 12px;
  color: #969696; background: #2d2d2d;
  border-right: 1px solid #1e1e1e;
  white-space: nowrap;
}
.vs-tab.act { background: #1e1e1e; color: #ffffff; }
.vs-code {
  flex: 1; overflow: hidden;
  padding: 8px 0;
  font-family: Consolas, "Cascadia Code", monospace;
  font-size: 13px; line-height: 19px;
}
.vs-line { display: flex; white-space: pre; }
.vs-ln {
  width: 48px; flex-shrink: 0;
  text-align: right; padding-right: 18px;
  color: #858585;
}
.vs-lt { flex: 1; }
.t-kw { color: #569cd6; }
.t-fn { color: #dcdcaa; }
.t-s  { color: #ce9178; }
.t-c  { color: #6a9955; }
.t-v  { color: #9cdcfe; }
.t-nu { color: #b5cea8; }
.t-p  { color: #d4d4d4; }
.t-ok { color: #4ec9b0; }
.vs-term {
  height: 132px; flex-shrink: 0;
  border-top: 1px solid #3c3c3c;
  display: flex; flex-direction: column;
}
.vs-term-h {
  height: 28px; flex-shrink: 0;
  display: flex; align-items: center; gap: 16px;
  padding: 0 14px; font-size: 11px; color: #969696;
  text-transform: uppercase;
}
.vs-term-h .act { color: #e7e7e7; border-bottom: 1px solid #e7e7e7; }
.vs-term-b {
  flex: 1; padding: 4px 14px;
  font-family: Consolas, monospace; font-size: 12px; line-height: 18px;
  overflow: hidden;
}
.vs-caret { color: #d4d4d4; animation: blink 1.1s step-end infinite; }
@keyframes blink { 50% { opacity: 0; } }
.vs-status {
  height: 22px; flex-shrink: 0;
  display: flex; align-items: center; gap: 14px;
  padding: 0 10px;
  background: #007acc; color: #ffffff; font-size: 11px;
}
.vs-sp { flex: 1; }

/* ══════════ Excel ══════════ */
.xl {
  height: 100%; display: flex; flex-direction: column;
  background: #ffffff; color: #212121;
  font-family: "Microsoft YaHei", "Segoe UI", sans-serif;
}
.xl-title {
  height: 30px; flex-shrink: 0;
  display: flex; align-items: center; gap: 8px;
  padding: 0 12px; background: #217346; color: #ffffff; font-size: 12px;
}
.xl-badge {
  width: 18px; height: 18px; border-radius: 3px;
  background: #ffffff; color: #217346;
  display: flex; align-items: center; justify-content: center;
  font-weight: 700; font-size: 12px;
}
.xl-ribbon {
  height: 28px; flex-shrink: 0;
  display: flex; align-items: center; gap: 20px;
  padding: 0 14px; background: #f3f2f1;
  border-bottom: 1px solid #e1dfdd; font-size: 12px; color: #444;
}
.xl-ribbon .act {
  color: #217346; font-weight: 600;
  border-bottom: 2px solid #217346; height: 100%;
  display: flex; align-items: center;
}
.xl-toolbar {
  height: 34px; flex-shrink: 0;
  display: flex; align-items: center; gap: 8px;
  padding: 0 14px; background: #f9f8f7;
  border-bottom: 1px solid #e1dfdd;
}
.xl-tb {
  padding: 2px 7px; font-size: 11px; color: #333;
  border: 1px solid #e1dfdd; border-radius: 3px; background: #fff;
}
.xl-tb.b { font-weight: 700; }
.xl-tb.i { font-style: italic; }
.xl-tb.u { text-decoration: underline; }
.xl-formula {
  height: 26px; flex-shrink: 0;
  display: flex; align-items: center; gap: 8px;
  padding: 0 10px; border-bottom: 1px solid #d0cfce; background: #fff;
}
.xl-cell-ref {
  width: 60px; font-size: 11px; padding: 2px 6px;
  border: 1px solid #d0cfce; text-align: center;
}
.xl-fx { font-style: italic; color: #666; font-size: 11px; }
.xl-fval { font-size: 12px; font-family: Consolas, monospace; color: #212121; }
.xl-grid { flex: 1; overflow: hidden; background: #fff; }
.xl-row { display: flex; height: 21px; }
.xl-c {
  border-right: 1px solid #d4d4d4;
  border-bottom: 1px solid #d4d4d4;
  padding: 0 6px;
  font-size: 11px; line-height: 20px;
  width: 104px; flex-shrink: 0;
  white-space: nowrap; overflow: hidden;
}
.xl-rh {
  width: 34px; background: #f3f2f1; color: #666;
  text-align: center; padding: 0;
  border-right: 1px solid #b7b7b7;
}
.xl-ch {
  background: #f3f2f1; color: #666; text-align: center;
  border-bottom: 1px solid #b7b7b7;
}
.xl-b { font-weight: 700; background: #f7f7f7; }
.xl-num { text-align: right; font-family: Consolas, monospace; }
.xl-neg { color: #c00000; }
.xl-sel { outline: 2px solid #217346; outline-offset: -2px; }
.xl-sheets {
  height: 24px; flex-shrink: 0;
  display: flex; align-items: center; gap: 14px;
  padding: 0 12px; background: #f3f2f1;
  border-top: 1px solid #d0cfce; font-size: 11px; color: #444;
}
.xl-sheets .act {
  color: #217346; font-weight: 600;
  border-bottom: 2px solid #217346;
}
.xl-add { color: #666; }
.xl-sp { flex: 1; }
/* Excel 状态栏是浅底,退出按钮改深底白字才看得清 */
.xl-sheets .exit-hot {
  color: #fff;
  background: #217346;
}
.xl-sheets .exit-hot:hover { background: #1a5c38; }

/* ══════════ PPT ══════════ */
.pp {
  height: 100%; display: flex; flex-direction: column;
  background: #f3f2f1; color: #212121;
  font-family: "Microsoft YaHei", "Segoe UI", sans-serif;
}
.pp-title {
  height: 30px; flex-shrink: 0;
  display: flex; align-items: center; gap: 8px;
  padding: 0 12px; background: #b7472a; color: #fff; font-size: 12px;
}
.pp-badge {
  width: 18px; height: 18px; border-radius: 3px;
  background: #fff; color: #b7472a;
  display: flex; align-items: center; justify-content: center;
  font-weight: 700; font-size: 12px;
}
.pp-ribbon {
  height: 28px; flex-shrink: 0;
  display: flex; align-items: center; gap: 18px;
  padding: 0 14px; background: #f3f2f1;
  border-bottom: 1px solid #e1dfdd; font-size: 12px; color: #444;
}
.pp-ribbon .act {
  color: #b7472a; font-weight: 600;
  border-bottom: 2px solid #b7472a; height: 100%;
  display: flex; align-items: center;
}
.pp-body { flex: 1; display: flex; min-height: 0; }
.pp-thumbs {
  width: 150px; flex-shrink: 0;
  background: #eaeaea; border-right: 1px solid #d0cfce;
  padding: 8px 6px; display: flex; flex-direction: column; gap: 8px;
  overflow: hidden;
}
.pp-th { display: flex; gap: 5px; align-items: flex-start; }
.pp-thn { font-size: 10px; color: #888; width: 10px; padding-top: 2px; }
.pp-thbox {
  flex: 1; height: 52px; background: #fff;
  border: 1px solid #d0cfce; border-radius: 2px;
  padding: 4px 5px; font-size: 8px; color: #444;
  overflow: hidden; line-height: 1.3;
}
.pp-th.act .pp-thbox { border: 2px solid #b7472a; }
.pp-stage {
  flex: 1; display: flex; align-items: center; justify-content: center;
  padding: 16px; min-width: 0; background: #d9d9d9;
}
.pp-slide {
  width: 100%; max-width: 620px; aspect-ratio: 16 / 9;
  background: #fff; box-shadow: 0 2px 10px rgba(0, 0, 0, 0.2);
  padding: 22px 28px; display: flex; flex-direction: column;
}
.pp-s-title { font-size: 20px; font-weight: 700; color: #b7472a; }
.pp-s-sub { font-size: 11px; color: #888; margin-top: 3px; }
.pp-chart {
  flex: 1; display: flex; align-items: flex-end; gap: 14px;
  padding: 16px 4px 0;
}
.pp-bar {
  flex: 1; background: linear-gradient(180deg, #d9694a, #b7472a);
  border-radius: 2px 2px 0 0;
  position: relative; min-height: 12px;
}
.pp-bar span {
  position: absolute; bottom: -16px; left: 0; right: 0;
  text-align: center; font-size: 9px; color: #555; white-space: nowrap;
}
.pp-s-note { font-size: 10px; color: #666; margin-top: 22px; }
.pp-status {
  height: 24px; flex-shrink: 0;
  display: flex; align-items: center; gap: 14px;
  padding: 0 12px; background: #b7472a; color: #fff; font-size: 11px;
}
.pp-sp { flex: 1; }
</style>
