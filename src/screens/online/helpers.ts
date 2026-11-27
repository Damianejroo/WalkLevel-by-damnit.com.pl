// Wspólne drobiazgi ekranów konta online (KontoOnline, ProfilOsoby, Logowanie): poziom -> ranga
// i granice XP z definicji, daty po polsku, sprawdzanie pól konta jak w rdzeniu (check_*),
// wywołanie komendy z błędem do pokazania przy polu albo pod formularzem (zamiast toastu).
import type * as K from '../../lib/contract';
import { jestBledemKomendy } from '../../lib/api';
import { t } from '../../lib/texts';
import { data as formatDaty, liczba } from '../../lib/format';

/** Ranga dla poziomu: ostatnia z `level <= poziom` (rangi w definicjach rosnąco). */
export function rangaPoziomu(defs: K.LevelDefinitions, poziom: number): K.Rank | null {
  let wynik: K.Rank | null = null;
  for (const r of defs.ranks) if (r.level <= poziom) wynik = r;
  return wynik ?? defs.ranks[0] ?? null;
}

/** XP progu poziomu i następnego (na najwyższym oba równe). */
export function granicePoziomu(defs: K.LevelDefinitions, poziom: number): { od: number; do: number } {
  const od = defs.levelsXp[Math.max(0, poziom - 1)] ?? 0;
  return { od, do: defs.levelsXp[poziom] ?? od };
}

/** „26.11.2026” z czasu UTC w sekundach (strefa komputera). */
export function data(sekundy: number): string {
  return formatDaty(new Date(sekundy * 1000));
}

/** Wersja regulaminu „2026-11-01” -> „01.11.2026” (inne napisy bez zmian). */
export function wersjaRegulaminu(wersja: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(wersja);
  return m ? formatDaty(new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]))) : wersja;
}

/** „dziś”, „wczoraj”, „3 dni temu” - dni kalendarzowe. */
export function kiedy(sekundy: number): string {
  const dzien = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const dni = Math.round((dzien(new Date()) - dzien(new Date(sekundy * 1000))) / 86_400_000);
  if (dni <= 0) return t('profil_online.zdobyte_dzis');
  if (dni === 1) return t('profil_online.zdobyte_wczoraj');
  return t('poziomy.osiagniecie.dni_temu', { dni });
}

/** Dystans z jedną cyfrą po przecinku („12,3”). */
export const km1 = (x: number) => liczba(x, 1);

// ---------------------------------------------------------------------------
// Pola konta (bieznia/konto check_* - rdzeń sprawdza to samo, tu tylko szybciej przy polu)
// ---------------------------------------------------------------------------

export function bladPseudonimu(nazwa: string, z: K.AccountRules): string {
  if (nazwa !== nazwa.trim()) return t('konto.blad.pseudonim_spacje');
  if (nazwa.length < z.nameMin || nazwa.length > z.nameMax) return t('konto.blad.pseudonim_dlugosc', { min: z.nameMin, max: z.nameMax });
  if (!new RegExp(z.nameRe).test(nazwa)) return t('konto.blad.pseudonim_znaki');
  if (z.reservedNames.includes(nazwa.toLowerCase())) return t('konto.blad.pseudonim_zarezerwowany');
  return '';
}

export const bladEmaila = (email: string, z: K.AccountRules) =>
  new RegExp(z.emailRe).test(email.trim()) ? '' : t('konto.blad.email');

export const bladHasla = (haslo: string, z: K.AccountRules) =>
  haslo.length >= z.passwordMin ? '' : t('konto.blad.haslo_krotkie', { liczba: z.passwordMin });

// ---------------------------------------------------------------------------
// Komendy z błędem pokazywanym na ekranie
// ---------------------------------------------------------------------------

export interface Wynik<T> {
  ok: boolean;
  wynik?: T;
  /** Komunikat ogólny (pod formularzem); pusty, gdy błąd dotyczy tylko pól albo go nie ma. */
  blad: string;
  /** Błędy pól z rdzenia (`invalid` z `fields`). */
  pola: Record<string, string>;
}

/**
 * Wykonuje komendę i zwraca błąd do pokazania w miejscu (pole, linia pod przyciskiem) zamiast
 * toastu. `loggedOut` i `cancelled` bez komunikatu (rdzeń już powiedział swoje).
 */
export async function sprobuj<T>(obietnica: Promise<T>): Promise<Wynik<T>> {
  try {
    return { ok: true, wynik: await obietnica, blad: '', pola: {} };
  } catch (e) {
    if (!jestBledemKomendy(e)) {
      console.error(e);
      return { ok: false, blad: t('wspolne.blad_nieznany'), pola: {} };
    }
    const pola = e.fields ?? {};
    const cichy = e.code === 'loggedOut' || e.code === 'cancelled';
    return { ok: false, blad: cichy || Object.keys(pola).length ? '' : e.message, pola };
  }
}
