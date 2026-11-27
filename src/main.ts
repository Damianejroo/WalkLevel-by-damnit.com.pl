import { mount } from 'svelte';
import './styles/tokens.css';
import './styles/base.css';
import App from './App.svelte';
import { logUiErrors } from './lib/api';
import { loadLanguagePack } from './lib/language-sync';

logUiErrors();

const root = document.documentElement;
// Motyw przed pierwszym malowaniem (potem ustawia go App z ustawień osoby).
root.dataset.theme = matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
// Tło okna nieprzezroczyste, dopóki okno nie ma włączonej Miki (tauri.conf.json windowEffects).
root.classList.add('no-mica');
// WebView2: bez menu kontekstowego przeglądarki (kontrakt 7.1).
if ('__TAURI_INTERNALS__' in window) addEventListener('contextmenu', (e) => e.preventDefault());

// Podgląd w przeglądarce: ?szer=420 - aplikacja w kolumnie o szerokości okna Tauri.
const szer = Number(new URLSearchParams(location.search).get('szer'));
if (szer >= 320) {
  root.style.setProperty('--podglad-szer', `${szer}px`);
  root.classList.add('podglad-szer');
}

// Teksty w języku z rdzenia przed pierwszym malowaniem (bez nich zostaje pakiet wbudowany).
await loadLanguagePack();

export default mount(App, { target: document.getElementById('app')! });
