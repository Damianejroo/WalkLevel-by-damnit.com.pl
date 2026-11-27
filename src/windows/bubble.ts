// Okno „przycisk” - pływający przycisk włączania sterowania nad grą (docs/contract.md 7.3,
// docs/design.md 9.7). Okno 56 × 56 tworzy rdzeń: przezroczyste, bez ramki, bez fokusu
// (klik nie zabiera gry). Klik - bubble_click, prawy klik - bubble_open, przeciąganie - okno
// przesuwa system (startDragging) między bubble_drag_start i bubble_drag_end.
// Podgląd w przeglądarce (atrapa rdzenia): /bubble.html?demo=1 (&stan=klawisze - włączone).
import { mount } from 'svelte';
import '../styles/tokens.css';
import '../styles/base.css';
import Przycisk from './bubble-window.svelte';
import { logUiErrors, wTauri } from '../lib/api';
import { loadLanguagePack, watchLanguage } from '../lib/language-sync';
import { t } from '../lib/texts';

logUiErrors();

const root = document.documentElement;
// Motyw przed pierwszym malowaniem; potem z wycinka „bubble” (motyw osoby).
root.dataset.theme = matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
root.classList.add('okno-przezroczyste');
document.title = t('dymek.tytul_okna');
if (wTauri) addEventListener('contextmenu', (e) => e.preventDefault());
// Podgląd: przycisk na środku strony na neutralnym tle (jak nad grą).
else if (new URLSearchParams(location.search).has('demo')) root.classList.add('podglad-okna');

/** Przesuwanie okna przez system (poza Tauri podgląd przesuwa przycisk po stronie sam). */
async function przeciagnijOkno(): Promise<void> {
  if (!wTauri) return;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  await getCurrentWindow().startDragging();
}

await loadLanguagePack();
await watchLanguage();

export default mount(Przycisk, { target: document.getElementById('app')!, props: { przeciagnijOkno } });
