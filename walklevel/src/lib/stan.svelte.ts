// Stan interfejsu (docs/kontrakt.md rozdz. 1 i 3): wycinki rdzenia w jednym obiekcie $state,
// telemetria osobno ($state.raw, 10-20 Hz), nawigacja jednego okna (stos ekranów).
// Komponenty tylko czytają: `stan.control.enabled`, `zywo.telemetria?.speedKmh`, `ui.trasa`.
import type * as K from './typy';
import * as api from './api';

/** Wszystkie wycinki + dane stałe. Pola są wypełnione, gdy `ui.gotowy` (po app_snapshot). */
export type StanRdzenia = K.SliceMap & { statics: K.StaticData };
export const stan: StanRdzenia = $state({} as StanRdzenia);

/** Trasa ekranu: trasy kontraktu + wzornik komponentów (tylko atrapa). */
export type Trasa = K.Route | 'wzornik';

/** Dodatkowe dane wejścia na ekran (np. która sesja, tryb formularza konta). */
export interface Parametry {
  sessionId?: string;
  profileId?: string;
  personId?: string;
  accountMode?: K.AccountFormMode;
  wizardMode?: K.WizardMode;
  /** Ustawienia: kategoria do rozwinięcia przy wejściu (np. „Zarządzaj profilami gier”). */
  kategoria?: 'osoby' | 'sterowanie';
}

/** Props każdego ekranu w src/ekrany/ (ekran montowany raz, potem tylko chowany). */
export interface PropsEkranu {
  /** Parametry ostatniego wejścia na ekran. */
  parametry: Parametry;
  /** Ekran jest teraz widoczny (np. odśwież dane przy wejściu: $effect na `aktywny`). */
  aktywny: boolean;
}

export interface WpisStosu {
  trasa: Trasa;
  parametry: Parametry;
}

class Zywo {
  /** Ostatnia telemetria (null przed pierwszą klatką). Podmieniana w całości, bez głębokiego proxy. */
  telemetria = $state.raw<K.Telemetry | null>(null);
}
export const zywo = new Zywo();

class Ui {
  gotowy = $state(false);
  stos = $state<WpisStosu[]>([{ trasa: 'main', parametry: {} }]);
  /** 1 - w głąb (wejście z prawej), -1 - wstecz. */
  kierunek = $state<1 | -1>(1);
  toast = $state<(K.Toast & { id: number }) | null>(null);
  /** Dziennik zdarzeń: ostatnie 500 linii. */
  dziennik = $state.raw<string[]>([]);
  /** Motyw systemu (dla ustawienia „jak Windows”). */
  ciemnySystem = $state(true);

  get wpis(): WpisStosu {
    return this.stos[this.stos.length - 1];
  }
  get trasa(): Trasa {
    return this.wpis.trasa;
  }
}
export const ui = new Ui();

// ============================================================================
// Nawigacja
// ============================================================================

/** Przejście na ekran. Ekran już na stosie - powrót do niego (stos nie rośnie w kółko). */
export function idz(trasa: Trasa, parametry: Parametry = {}) {
  if (trasa === 'main') {
    ui.kierunek = -1;
    ui.stos = [{ trasa: 'main', parametry: {} }];
    return;
  }
  const i = ui.stos.findIndex((w) => w.trasa === trasa);
  if (i >= 0) {
    ui.kierunek = i < ui.stos.length - 1 ? -1 : 1;
    ui.stos = [...ui.stos.slice(0, i), { trasa, parametry }];
  } else {
    ui.kierunek = 1;
    ui.stos = [...ui.stos, { trasa, parametry }];
  }
}

export function wroc() {
  if (ui.stos.length < 2) return;
  ui.kierunek = -1;
  ui.stos = ui.stos.slice(0, -1);
}

// ============================================================================
// Komunikaty (toast)
// ============================================================================

let numerToastu = 0;
let zegarToastu: ReturnType<typeof setTimeout> | undefined;

/** Toast na dole okna: 4 s, klik chowa, nowy zastępuje stary (wyglad.md 8.12). */
export function komunikat(text: string, kind: K.Toast['kind'] = 'info') {
  const id = ++numerToastu;
  ui.toast = { text, kind, id };
  clearTimeout(zegarToastu);
  zegarToastu = setTimeout(() => schowajKomunikat(id), 4000);
}

export function schowajKomunikat(id = ui.toast?.id) {
  if (ui.toast && ui.toast.id === id) ui.toast = null;
}

/**
 * Wykonuje komendę i pokazuje jej błąd jako toast (CmdError.message po polsku). Pomija
 * `loggedOut` (rdzeń sam wysłał komunikat) i `cancelled` (zamknięte okno wyboru pliku).
 * Zwraca wynik albo undefined przy błędzie. Błędy pól (`invalid` z `fields`) obsługuj w ekranie.
 */
export async function wykonaj<T>(obietnica: Promise<T>): Promise<T | undefined> {
  try {
    return await obietnica;
  } catch (e) {
    if (api.jestBledemKomendy(e)) {
      if (e.code !== 'loggedOut' && e.code !== 'cancelled') komunikat(e.message, 'error');
    } else {
      console.error(e);
    }
    return undefined;
  }
}

// ============================================================================
// Wycinki: pełny stan na starcie, potem przyrosty z rev (kontrakt 3.2)
// ============================================================================

const jestObiektem = (v: unknown): v is Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v);

/**
 * Wpisuje nową wartość w miejsce starej, zmieniając tylko pola, które się różnią - dzięki temu
 * Svelte aktualizuje wyłącznie miejsca zależne od zmienionych pól, a listy z kluczem zachowują
 * swoje elementy.
 */
function scal(cel: Record<string, unknown> | unknown[], klucz: string | number, nowa: unknown) {
  const c = cel as Record<string | number, unknown>;
  const stara = c[klucz];
  if (jestObiektem(stara) && jestObiektem(nowa)) {
    for (const k of Object.keys(stara)) if (!(k in nowa)) delete stara[k];
    for (const [k, v] of Object.entries(nowa)) scal(stara, k, v);
  } else if (Array.isArray(stara) && Array.isArray(nowa) && stara.length === nowa.length) {
    nowa.forEach((v, i) => scal(stara, i, v));
  } else if (!Object.is(stara, nowa)) {
    c[klucz] = nowa;
  }
}

const ostatniRev = new Map<K.SliceName, number>();
let revStartu = 0;

function zastosuj(ev: K.SliceEvent) {
  if (ev.rev <= (ostatniRev.get(ev.slice) ?? revStartu)) return;
  ostatniRev.set(ev.slice, ev.rev);
  scal(stan as unknown as Record<string, unknown>, ev.slice, ev.value);
}

function obsluzNawigacje(n: K.NavRequest) {
  // Rdzeń tylko prosi - nie przerywamy konfiguratora niczym poza pierwszym uruchomieniem.
  if (ui.trasa === 'wizard' && n.reason !== 'firstRun') return;
  idz(n.to, n.accountMode ? { accountMode: n.accountMode } : n.to === 'wizard' ? { wizardMode: 'first' } : {});
}

const TRASY: readonly Trasa[] = [
  'main', 'devices', 'settings', 'log', 'report', 'profile', 'profileNew', 'person', 'wizard', 'history',
  'session', 'export', 'import', 'models', 'account', 'online', 'onlinePerson', 'achievements', 'trail', 'wzornik',
];
export const jestTrasa = (v: unknown): v is Trasa => TRASY.includes(v as Trasa);

/** Start okna głównego: nasłuch, potem snapshot, potem zaległe zdarzenia, na końcu ui_ready. */
export async function uruchom() {
  const zalegle: K.SliceEvent[] = [];
  let gotowe = false;
  await api.sluchaj('wl:stan', (ev) => (gotowe ? zastosuj(ev) : zalegle.push(ev)));
  await api.sluchaj('wl:telemetria', (tel) => (zywo.telemetria = tel));
  await api.sluchaj('wl:komunikat', (tst) => komunikat(tst.text, tst.kind));
  await api.sluchaj('wl:log', (linia) => (ui.dziennik = [...ui.dziennik.slice(-499), linia]));
  await api.sluchaj('wl:nawigacja', obsluzNawigacje);

  const { rev, logTail, ...wycinki } = await api.appSnapshot();
  Object.assign(stan, wycinki);
  revStartu = rev;
  ui.dziennik = logTail.slice(-500);
  for (const ev of zalegle) zastosuj(ev);
  gotowe = true;

  // Ekran startowy z adresu (?ekran=history) - podgląd w przeglądarce i test zrzutów.
  const ekran = new URLSearchParams(location.search).get('ekran');
  if (jestTrasa(ekran) && ekran !== 'main') ui.stos = [{ trasa: 'main', parametry: {} }, { trasa: ekran, parametry: {} }];

  ui.gotowy = true;
  await api.uiReady('main');
}
