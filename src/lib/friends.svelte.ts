// Lista znajomych na żywo - jedno źródło dla panelu znajomych i Konto online > Znajomi (0.6.2,
// docs/sync.md): odświeżenie GET znajomi co 10 s, gdy okno jest widoczne, co 60 s, gdy
// zminimalizowane; najwyżej jedno zapytanie naraz. Dane trafiają do stan.online.friends
// (rdzeń podmienia je w miejscu, listy z kluczem nie przebudowują się). Między odświeżeniami
// dystans sesji (a z nim XP) dociągamy z tempa: `dystansTeraz`.
import { untrack } from 'svelte';
import * as api from './api';
import { komunikat, stan, ui } from './state.svelte';
import { t } from './texts';
import type { Friend } from './contract';

export const ODSWIEZ_WIDOCZNE_MS = 10_000;
export const ODSWIEZ_UKRYTE_MS = 60_000;
/** Dłużej niż tyle po odświeżeniu nie dociągamy dystansu (brak sieci - liczby nie uciekają). */
const DOCIAGANIE_MAX_S = 120;

class Znajomi {
  /** Ile miejsc chce teraz listy (panel, zakładka Znajomi). */
  potrzeba = $state(0);
  /** Chwila ostatniej odpowiedzi (ms, performance.now) - od niej dociągamy dystans. */
  odebrano = $state(0);
  /** Zegar 1 s do dociągania XP (tylko gdy ktoś chodzi i lista jest widoczna). */
  teraz = $state(0);
  /** Ostatni błąd odświeżenia (pusty - w porządku); karta błędu tylko, gdy nie ma listy. */
  blad = $state('');
  /** performance.now() kolejnej próby w tle (0 - nie ponawiamy). */
  nastepna = $state(0);
}
export const znajomi = new Znajomi();

let trwa = false;
let ostatnio = -Infinity;
/** Kto chodził przy poprzednim odświeżeniu (null - jeszcze nie było odświeżenia). */
let chodzili: Set<string> | null = null;
let osoba = '';

/** Zgłasza, że lista jest potrzebna; zwraca funkcję, która to odwołuje (do `$effect`). */
export function potrzebujeZnajomych(): () => void {
  // bez śledzenia: wołane w `$effect`, który inaczej zależałby od licznika, który sam zmienia
  untrack(() => znajomi.potrzeba++);
  return () => {
    untrack(() => znajomi.potrzeba--);
  };
}

/** Odświeżenie teraz (ciche: błąd zostawia ostatnią listę; komunikat pokazuje Konto online). */
export async function odswiezZnajomych(force = false): Promise<void> {
  if (trwa || !stan.account?.loggedIn) return;
  trwa = true;
  ostatnio = performance.now();
  try {
    await api.onlineLoad('friends', force);
    znajomi.odebrano = performance.now();
    znajomi.teraz = znajomi.odebrano;
    znajomi.blad = '';
    powiadomONowych(stan.online.friends);
  } catch (e) {
    console.warn('Znajomi: odświeżenie nie powiodło się', e);
    if (api.jestBledemKomendy(e) && e.code !== 'busy' && e.code !== 'loggedOut') znajomi.blad = e.message;
  } finally {
    znajomi.nastepna = ostatnio + (document.hidden ? ODSWIEZ_UKRYTE_MS : ODSWIEZ_WIDOCZNE_MS);
    trwa = false;
  }
}

/** „Powiadom, gdy znajomy zacznie chodzić”: komunikat w oknie WalkLevel (nie nad grą). */
function powiadomONowych(lista: Friend[] | null) {
  if (!lista) return;
  if (osoba !== stan.online.personId) {
    osoba = stan.online.personId;
    chodzili = null; // inna osoba - bez powiadomień o całej jej liście
  }
  const teraz = new Set(lista.filter((f) => f.live).map((f) => f.id));
  if (chodzili && stan.settings.friendsNotify) {
    const nowi = lista.filter((f) => f.live && !chodzili?.has(f.id));
    if (nowi.length) komunikat(nowi.map((f) => t('panel.zaczyna', { pseudonim: f.name })).join(' '));
  }
  chodzili = teraz;
}

/** Dystans bieżącej sesji teraz: z serwera plus dociągnięty z tempa (null bez zgody tej osoby). */
export function dystansTeraz(f: Friend): number | null {
  const live = f.live;
  if (!live || live.distanceKm === null) return null;
  const s = Math.min(DOCIAGANIE_MAX_S, Math.max(0, (znajomi.teraz - znajomi.odebrano) / 1000));
  return live.distanceKm + (live.kmh * s) / 3600;
}

/** XP na żywo: `xp + floor(dystans_km * 100)` (docs/sync.md); null - poziom ukryty. */
export function xpTeraz(f: Friend, xpZaKm: number): { xp: number | null; zSesji: number | null } {
  if (f.xp === null) return { xp: null, zSesji: null };
  const km = dystansTeraz(f);
  const zSesji = km === null ? null : Math.floor(km * xpZaKm);
  return { xp: f.xp + (zSesji ?? 0), zSesji };
}

/**
 * Pętla odświeżania (raz, z App): działa, gdy ktoś potrzebuje listy i osoba ma konto.
 * Zegar co 1 s sprawdza, czy minął odstęp (10 s / 60 s przy ukrytym oknie) i tyka XP.
 */
export function uruchomZnajomych(): () => void {
  return $effect.root(() => {
    $effect(() => {
      if (!ui.gotowy || znajomi.potrzeba <= 0 || !stan.account.loggedIn) return;
      void stan.online.personId;
      const zegar = setInterval(() => {
        const odstep = document.hidden ? ODSWIEZ_UKRYTE_MS : ODSWIEZ_WIDOCZNE_MS;
        if (performance.now() - ostatnio >= odstep) void odswiezZnajomych();
        // zegar co sekundę: XP chodzących i odliczanie „Kolejna próba za N s” przy błędzie
        if (!document.hidden && (znajomi.blad || stan.online.friends?.some((f) => f.live))) znajomi.teraz = performance.now();
      }, 1000);
      // pierwsze pobranie od razu (panel otwarty po starcie, zmiana osoby)
      if (performance.now() - ostatnio >= 2000) void odswiezZnajomych();
      return () => clearInterval(zegar);
    });
  });
}
