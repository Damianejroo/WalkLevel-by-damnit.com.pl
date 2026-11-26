// Układ panelu znajomych w oknie (docs/wyglad.md 8.16) i przejście przy zwijaniu, rozwijaniu,
// włączaniu i wyłączaniu panelu. Okno zmienia szerokość (okna.rs), a WebView2 dogania nowy
// rozmiar o kilka klatek - bez przejścia główna kolumna drgała, a w miejscu panelu prześwitywał
// pulpit. Dlatego na czas przejścia szerokość głównej kolumny jest zamrożona w pikselach, a
// panel bierze resztę okna:
// - zwijanie / wyłączanie: najpierw w środku pasek (albo puste tło), po klatce okno się zwęża;
// - rozwijanie / włączanie: najpierw okno się poszerza (nowe miejsce w kolorze panelu), po
//   zdarzeniu resize (albo po czasie, gdy okno się nie zmienia, np. zmaksymalizowane) pełny panel.
// Panel z lewej zwinięty: WebView jest wysunięty w lewo o 224 px poza okno (okna.rs
// `hidden_left`), więc strona ma to miejsce z lewej (`ukryte`) i układa w nim pasek przy prawej
// krawędzi. Zwijanie i rozwijanie z lewej zmienia wtedy tylko treść panelu - szerokość strony i
// miejsce głównej kolumny w niej zostają (bez klatki z przesuniętą kolumną).
import * as api from './api';
import { stan, ui, wykonaj } from './stan.svelte';
import type { SettingKey, Settings } from './typy';

export type TrybPanelu = 'pelny' | 'waski' | 'wylaczony';
export const PANEL_PELNY = 288;
export const PANEL_WASKI = 64;
const KOLUMNA_MIN = 420;
/** Okno nie zmieniło rozmiaru (zmaksymalizowane, przyciągnięte) - koniec przejścia po tym czasie. */
const PRZEJSCIE_MAX_MS = 500;

export const szerokoscPanelu = (tryb: TrybPanelu) => (tryb === 'pelny' ? PANEL_PELNY : tryb === 'waski' ? PANEL_WASKI : 0);

/** Szerokość wysuniętej poza okno części strony z lewej (tylko w aplikacji, nie w podglądzie). */
const ukrytePrzy = (s: Settings) =>
  api.wTauri && s.friendsPanel && stan.account.enabled && s.friendsPanelSide === 'left' && s.friendsPanelCollapsed
    ? PANEL_PELNY - PANEL_WASKI
    : 0;

class UkladPanelu {
  szerOkna = $state(typeof window === 'undefined' ? 1200 : window.innerWidth);
  /** W trakcie przejścia: szerokość głównej kolumny (px) i to, co panel pokazuje. */
  kolumna = $state<number | null>(null);
  pokaz = $state<TrybPanelu>('wylaczony');
  /** Szerokość okna, której czekamy (koniec przejścia). */
  cel = 0;

  /** Część strony z lewej wysunięta poza okno (px): 224 przy zwiniętym panelu z lewej. */
  ukryte = $derived(ui.gotowy ? ukrytePrzy(stan.settings) : 0);
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
  /** Szerokość miejsca panelu w stronie: panel i ukryta część z lewej. */
  slot = $derived(this.ukryte + szerokoscPanelu(this.widoczny));
}
export const panel = new UkladPanelu();

let zegar: ReturnType<typeof setTimeout> | undefined;

function koniec() {
  clearTimeout(zegar);
  panel.kolumna = null;
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
  const przed = panel.tryb;
  const s = { ...stan.settings, [klucz]: wartosc } as Settings;
  const po: TrybPanelu = !s.friendsPanel || !stan.account.enabled ? 'wylaczony' : s.friendsPanelCollapsed ? 'waski' : 'pelny';
  // z ukrytą częścią: przy panelu z lewej zwijanie i rozwijanie nie zmienia miejsca panelu
  const stara = panel.ukryte + szerokoscPanelu(przed);
  const nowa = ukrytePrzy(s) + szerokoscPanelu(po);
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
