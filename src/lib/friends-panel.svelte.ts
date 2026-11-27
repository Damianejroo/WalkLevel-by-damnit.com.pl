// Układ panelu znajomych w oknie (docs/design.md 8.16) i przejście przy zwijaniu, rozwijaniu,
// włączaniu i wyłączaniu panelu. Okno zmienia szerokość (window.rs), a WebView2 dogania nowy
// rozmiar o kilka klatek - bez przejścia główna kolumna drgała, a w miejscu panelu prześwitywał
// pulpit. Dlatego na czas przejścia szerokość głównej kolumny jest zamrożona w pikselach, a
// panel bierze resztę okna:
// - zwijanie / wyłączanie: najpierw w środku pasek (albo puste tło), po klatce okno się zwęża;
// - rozwijanie / włączanie: najpierw okno się poszerza (nowe miejsce w kolorze panelu), po
//   zdarzeniu resize (albo po czasie, gdy okno się nie zmienia, np. zmaksymalizowane) pełny panel.
// W aplikacji (0.6.3): miejsce panelu ma zawsze 288 px i zawsze jest w nim pełny panel; zwinięty
// to pasek nałożony na jego brzeg od strony kolumny, a WebView wysunięty o 224 px poza okno od
// strony panelu (window.rs `hidden`). Zwijanie i rozwijanie zmienia tylko okno - WebView nie
// zmienia rozmiaru ani miejsca, szerokość strony i miejsce głównej kolumny zostają (WebView2 przy
// zmianie rozmiaru pokazywał klatkę samego tła albo pustego panelu). Kolejność: zwijanie -
// najpierw pasek nałożony, po klatce okno; rozwijanie - najpierw okno, pasek znika po zdarzeniu
// `wl:panel-okno`. W klatce „pomiędzy” widać pełny panel z paskiem na brzegu, nigdy pustki.
// Podgląd w przeglądarce (bez Tauri) - układ z przejściem z zamrożoną kolumną jak wyżej.
import * as api from './api';
import { stan, ui, wykonaj } from './state.svelte';
import type { SettingKey, Settings } from './contract';

export type TrybPanelu = 'pelny' | 'waski' | 'wylaczony';
export const PANEL_PELNY = 288;
export const PANEL_WASKI = 64;
const KOLUMNA_MIN = 420;
/** Okno nie zmieniło rozmiaru (zmaksymalizowane, przyciągnięte) - koniec przejścia po tym czasie. */
const PRZEJSCIE_MAX_MS = 500;

export const szerokoscPanelu = (tryb: TrybPanelu) => (tryb === 'pelny' ? PANEL_PELNY : tryb === 'waski' ? PANEL_WASKI : 0);

/** Panel w aplikacji (nie w podglądzie): pełny panel pod spodem, pasek nakładany. */
const wAplikacji = (s: Settings) => api.wTauri && s.friendsPanel && stan.account.enabled;

class UkladPanelu {
  szerOkna = $state(typeof window === 'undefined' ? 1200 : window.innerWidth);
  /** W trakcie przejścia: szerokość głównej kolumny (px) i to, co panel pokazuje. */
  kolumna = $state<number | null>(null);
  pokaz = $state<TrybPanelu>('wylaczony');
  /** Szerokość okna, której czekamy (koniec przejścia). */
  cel = 0;

  /** Okno ma WebView wysunięty poza okno (zdarzenie z rdzenia; null - przed pierwszym). */
  oknoWysuniete = $state<boolean | null>(null);
  /** Zwijanie: pasek nałożony, zanim okno się zwęzi. */
  przedZwinieciem = $state(false);
  /** Panel w aplikacji: stałe miejsce 288 px, pod spodem pełny panel. */
  staly = $derived(ui.gotowy && ui.trasa !== 'wizard' && wAplikacji(stan.settings));
  /** Pasek nałożony na brzeg pełnego panelu od strony kolumny (zwinięty albo zwijany). */
  nakladka = $derived(
    this.staly && (this.przedZwinieciem || (this.oknoWysuniete ?? stan.settings.friendsPanelCollapsed)),
  );
  /** Część strony wysunięta poza okno od strony panelu (px): 224, gdy okno jest zwinięte. */
  ukryte = $derived(this.staly && (this.oknoWysuniete ?? stan.settings.friendsPanelCollapsed) ? PANEL_PELNY - PANEL_WASKI : 0);
  /** Tryb z ustawień i szerokości okna: gdy za wąsko - pasek, a przy 420 px bez panelu. */
  tryb = $derived.by((): TrybPanelu => {
    if (!ui.gotowy || !stan.settings.friendsPanel || !stan.account.enabled || ui.trasa === 'wizard') return 'wylaczony';
    const chce = stan.settings.friendsPanelCollapsed ? 'waski' : 'pelny';
    const widac = this.szerOkna - this.ukryte;
    if (chce === 'pelny' && widac >= KOLUMNA_MIN + PANEL_PELNY) return 'pelny';
    return widac >= KOLUMNA_MIN + PANEL_WASKI ? 'waski' : 'wylaczony';
  });
  /** Co rysować teraz: w przejściu `pokaz`, inaczej `tryb`. */
  widoczny = $derived(this.kolumna === null ? this.tryb : this.pokaz);
  strona = $derived<'left' | 'right'>(ui.gotowy && stan.settings.friendsPanelSide === 'left' ? 'left' : 'right');
  /** Szerokość miejsca panelu w stronie (w aplikacji zawsze 288 - z ukrytą częścią). */
  slot = $derived(this.staly ? PANEL_PELNY : szerokoscPanelu(this.widoczny));
}
export const panel = new UkladPanelu();

let zegar: ReturnType<typeof setTimeout> | undefined;

function koniec() {
  clearTimeout(zegar);
  panel.kolumna = null;
}

/** Nasłuch zdarzenia rdzenia po zmianie okna pod panel (App, raz). */
export function sluchajOknaPanelu(): Promise<() => void> {
  return api.sluchaj('wl:panel-okno', (wysuniete) => {
    panel.oknoWysuniete = wysuniete;
    panel.przedZwinieciem = false;
  });
}

/** Zdarzenie resize okna (App): po dojściu do celu - pełny układ w następnej klatce. */
export function oknoZmienione(szer: number) {
  panel.szerOkna = szer;
  if (panel.kolumna !== null && Math.abs(szer - panel.cel) <= 1) requestAnimationFrame(koniec);
}

const klatka = () => new Promise<void>((ok) => requestAnimationFrame(() => requestAnimationFrame(() => ok())));

/**
 * Zmiana ustawienia panelu (zwiń/rozwiń, wł./wył.) z przejściem bez drgań: główna kolumna stoi,
 * zmienia się tylko miejsce panelu. Strona panelu i inne klucze - zwykłe `settings_set`.
 */
export async function zmienPanel<S extends SettingKey>(klucz: S, wartosc: Settings[S]): Promise<void> {
  if (panel.staly && klucz === 'friendsPanelCollapsed') {
    // zwijanie - najpierw pasek na pełnym panelu, po klatce okno (rozwijanie: okno, potem pasek
    // znika po `wl:panel-okno`); strona nie zmienia szerokości
    if (wartosc) {
      panel.przedZwinieciem = true;
      await klatka();
    }
    const ok = await wykonaj(api.settingsSet(klucz, wartosc).then(() => true));
    if (!ok) panel.przedZwinieciem = false;
    return;
  }
  const przed = panel.tryb;
  const s = { ...stan.settings, [klucz]: wartosc } as Settings;
  const po: TrybPanelu = !s.friendsPanel || !stan.account.enabled ? 'wylaczony' : s.friendsPanelCollapsed ? 'waski' : 'pelny';
  // w aplikacji miejsce panelu ma 288 (włączanie/wyłączanie - z przejściem jak w podglądzie)
  const stara = panel.slot;
  const nowa = wAplikacji(s) && po !== 'wylaczony' ? PANEL_PELNY : szerokoscPanelu(po);
  if (stara === nowa || (klucz !== 'friendsPanel' && klucz !== 'friendsPanelCollapsed')) {
    await wykonaj(api.settingsSet(klucz, wartosc));
    return;
  }
  clearTimeout(zegar);
  panel.kolumna = panel.szerOkna - stara;
  panel.cel = panel.kolumna + nowa;
  if (nowa < stara) {
    // zwijanie: najpierw pasek w panelu (CSS), po klatce okno się zwęża
    panel.pokaz = po;
    await klatka();
  } else {
    // rozwijanie: panel zostaje, jaki był, aż okno się poszerzy
    panel.pokaz = przed;
  }
  zegar = setTimeout(koniec, PRZEJSCIE_MAX_MS);
  const wynik = await wykonaj(api.settingsSet(klucz, wartosc).then(() => true));
  if (!wynik) koniec();
}
