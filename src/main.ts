import { createApp } from "vue";
import { createPinia } from "pinia";

// 根据 URL query.view 分派到不同顶层组件
// - ?view=ctxmenu → 独立右键菜单
// - ?view=pet     → 桌面宠物(配置走 query,不挂 pinia)
// - 默认       → 主应用 App.vue
const params = new URLSearchParams(window.location.search);
const view = params.get("view");

if (view === "ctxmenu") {
  import("./views/ContextMenuStandalone.vue").then(({ default: Menu }) => {
    createApp(Menu).mount("#app");
  });
} else if (view === "pet") {
  import("./views/PetStandalone.vue").then(({ default: Pet }) => {
    createApp(Pet).mount("#app");
  });
} else {
  Promise.all([import("./App.vue")]).then(([{ default: App }]) => {
    createApp(App).use(createPinia()).mount("#app");
  });
}
