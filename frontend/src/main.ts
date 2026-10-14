import './assets/style/main.css'

import { createApp } from 'vue'
import { createPinia } from 'pinia'

import App from './App.vue'
import router from './router'
import i18n from './i18n'

const OPEN_SOURCE_BANNER = String.raw`
  ______ _ _      _    _ _           
 |  ____(_) |    | |  | (_)          
 | |__   _| | ___| |__| |_ _   _____ 
 |  __| | | |/ _ \  __  | \ \ / / _ \
 | |    | | |  __/ |  | | |\ V /  __/
 |_|    |_|_|\___|_|  |_|_| \_/ \___|

 FileHive - Fast & Lightweight File Sharing powered by Rust
`

console.info(OPEN_SOURCE_BANNER)

const app = createApp(App)

app.use(createPinia())
app.use(router)
app.use(i18n)

app.mount('#app')
