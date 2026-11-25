import { createApp } from 'vue'
import App from './App.vue'
import { vRefract, vLight } from './glass/refract'
import './styles/base.css'
import './styles/glass.css'

const app = createApp(App)
app.directive('refract', vRefract)
app.directive('light', vLight)
app.mount('#app')

// 禁用浏览器默认右键菜单，让界面更像原生应用（输入框除外）
document.addEventListener('contextmenu', (e) => {
  const t = e.target as HTMLElement | null
  if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA')) return
  e.preventDefault()
})
