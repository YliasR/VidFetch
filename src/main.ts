import { mount } from 'svelte';
import App from './App.svelte';
import './styles/base.css';

const app = mount(App, {
  target: document.getElementById('app')!,
});

export default app;
