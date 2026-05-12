import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import SettingsWindow from './lib/settings/SettingsWindow.svelte'

const target = document.getElementById('app')!
const isSettings = window.location.hash.startsWith('#settings')

const app = mount(isSettings ? SettingsWindow : App, { target })

export default app
