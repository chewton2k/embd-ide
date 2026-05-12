import { mount } from 'svelte'
import { addCollection } from '@iconify/svelte'
import vscodeIcons from '@iconify-json/vscode-icons/icons.json'
import simpleIcons from '@iconify-json/simple-icons/icons.json'
import './app.css'
import App from './App.svelte'
import SettingsWindow from './lib/settings/SettingsWindow.svelte'

// Load icon sets locally so they work offline (no API fetch needed)
addCollection(vscodeIcons)
addCollection(simpleIcons)

const target = document.getElementById('app')!
const isSettings = window.location.hash.startsWith('#settings')

const app = mount(isSettings ? SettingsWindow : App, { target })

export default app
