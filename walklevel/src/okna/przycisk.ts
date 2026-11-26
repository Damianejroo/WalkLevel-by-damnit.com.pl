// Okno „przycisk” - pływający przycisk włączania sterowania nad grą (docs/kontrakt.md 7.3,
// docs/wyglad.md 9.7). Okno 56 × 56 tworzy rdzeń: przezroczyste, bez ramki, bez fokusu
// (klik nie zabiera gry). Klik - bubble_click, prawy klik - bubble_open, przeciąganie - okno
// przesuwa system (startDragging) między bubble_drag_start i bubble_drag_end.
// Podgląd w przeglądarce (atrapa rdzenia): /przycisk.html?demo=1 (&stan=klawisze - włączone).
import { mount } from 'svelte';
import '../styles/tokeny.css';
import '../styles/baza.css';
import Przycisk from './Przycisk.svelte';
import { wTauri } from '../lib/api';
import { t } from '../lib/teksty';

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

export default mount(Przycisk, { target: document.getElementById('app')!, props: { przeciagnijOkno } });
