// Teksty interfejsu. Jedyne źródło: docs/teksty.md -> src/lib/teksty-slownik.ts (`npm run teksty`).
// Użycie: t('glowny.status.polaczono.tytul', { model: 'KingSmith X21' })
//         t('glowny.osoby.dzis', { kroki: 1234 })  ->  „dziś 1 234 kroki”
import { SLOWNIK } from './teksty-slownik';
import { liczba } from './format';

export type Klucz = keyof typeof SLOWNIK;
export type Zmienne = Record<string, string | number>;

/** Czy napis jest kluczem słownika (dla kluczy składanych w locie). */
export function jestKlucz(k: string): k is Klucz {
  return Object.hasOwn(SLOWNIK, k);
}

/** Odmiana po liczbie: 1 -> f1; końcówka 2-4 (bez 12-14) -> f2; reszta -> f5. Ułamki -> f2 („2,5 kroku”). */
export function odmiana(n: number, f1: string, f2: string, f5: string): string {
  if (!Number.isInteger(n)) return f2;
  const a = Math.abs(n);
  if (a === 1) return f1;
  const d = a % 10;
  const s = a % 100;
  return d >= 2 && d <= 4 && (s < 12 || s > 14) ? f2 : f5;
}

/** Wartość liczbowa zmiennej (liczba albo już sformatowany napis „12 345”, „2,5”). */
function wartosc(v: string | number | undefined): number | null {
  if (typeof v === 'number') return v;
  if (typeof v !== 'string') return null;
  const n = Number(v.replace(/[\s  ]/g, '').replace(',', '.'));
  return Number.isFinite(n) ? n : null;
}

/**
 * Tekst ze słownika z podstawionymi zmiennymi.
 * - `{nazwa}` - wartość; liczby całkowite formatowane z odstępem tysięcy, ułamki z jednym
 *   miejscem po przecinku (inną dokładność podaj jako gotowy napis: `liczba(km, 2)`),
 * - `{nazwa:f1/f2/f5}` - odmiana według wartości zmiennej `nazwa`.
 * Brak zmiennej zostawia `{nazwa}` w tekście (widać to od razu na ekranie).
 */
export function t(klucz: Klucz, zmienne: Zmienne = {}): string {
  const wzor: string | undefined = SLOWNIK[klucz];
  if (wzor === undefined) {
    console.error(`Brak tekstu „${klucz}” w docs/teksty.md`);
    return klucz;
  }
  return wzor
    .replace(/\{([a-z_0-9]+):([^/{}]*)\/([^/{}]*)\/([^/{}]*)\}/g, (cale, nazwa: string, f1: string, f2: string, f5: string) => {
      const n = wartosc(zmienne[nazwa]);
      return n === null ? cale : odmiana(n, f1, f2, f5);
    })
    .replace(/\{([a-z_0-9]+)\}/g, (cale, nazwa: string) => {
      const v = zmienne[nazwa];
      if (v === undefined) return cale;
      if (typeof v === 'number') return liczba(v, Number.isInteger(v) ? 0 : 1);
      return v;
    });
}

/** Tekst dzielony na części „ / ” (np. tytuł / opis kafelka, warianty „jak Windows / jasny / ciemny”). */
export function tl(klucz: Klucz, zmienne: Zmienne = {}): string[] {
  return t(klucz, zmienne).split(' / ');
}

/** Czas ruchu: „12 min”, „1 h 05 min”. */
export function czas(sekundy: number): string {
  const minuty = Math.floor(Math.max(0, sekundy) / 60);
  const godziny = Math.floor(minuty / 60);
  return godziny > 0
    ? t('historia.czas.godziny', { godziny, minuty: String(minuty % 60).padStart(2, '0') })
    : t('historia.czas.minuty', { minuty });
}

/** „Konto online · Damianejro” -> tytuł „Konto online” i podpis „Damianejro” (wyglad.md 9.1). */
export function tytulIPodpis(tekst: string): { tytul: string; podpis: string } {
  const i = tekst.indexOf(' · ');
  return i < 0 ? { tytul: tekst, podpis: '' } : { tytul: tekst.slice(0, i), podpis: tekst.slice(i + 3) };
}
