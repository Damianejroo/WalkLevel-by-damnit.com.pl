// Okno „popup” - powiadomienia o osiągnięciach i awansach nad grą (docs/contract.md 7.2,
// docs/design.md 8.13). Okno tworzy rdzeń: przezroczyste, bez ramki, bez fokusu, przepuszcza
// kliknięcia. Kolejka i czasy są w rdzeniu - tu tylko wygląd i animacje wejścia/wyjścia.
// Podgląd w przeglądarce (atrapa rdzenia): /popup.html?demo=1 (&stan=dlugie - najdłuższe nazwy).
import { mount } from 'svelte';
import '../styles/tokens.css';
import '../styles/base.css';
import Popup from './popup-window.svelte';
import { logUiErrors, wTauri } from '../lib/api';
import { loadLanguagePack, watchLanguage } from '../lib/language-sync';

logUiErrors();

const root = document.documentElement;
// Powiadomienie jest zawsze ciemne (czytelne nad każdą grą), niezależnie od motywu aplikacji.
root.dataset.theme = 'dark';
root.classList.add('okno-przezroczyste');
if (wTauri) addEventListener('contextmenu', (e) => e.preventDefault());
// Podgląd: stos w prawym dolnym rogu strony na neutralnym tle (jak nad grą).
else if (new URLSearchParams(location.search).has('demo')) root.classList.add('podglad-okna');

await loadLanguagePack();
await watchLanguage();

export default mount(Popup, { target: document.getElementById('app')! });
