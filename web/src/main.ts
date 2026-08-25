import { mount } from 'svelte';
import App from './App.svelte';
import { TauriTransport } from './lib/tauri-transport';
import './styles.css';

const target = document.getElementById('app');
if (!target) throw new Error('App mount target is missing');

mount(App, {
  target,
  props: { transport: new TauriTransport() },
});
